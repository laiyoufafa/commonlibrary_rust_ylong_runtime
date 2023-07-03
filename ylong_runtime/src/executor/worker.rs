// Copyright (c) 2023 Huawei Device Co., Ltd.
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

///worker struct info and method
use crate::executor::async_pool::MultiThreadScheduler;
use crate::executor::parker::Parker;
use crate::executor::queue::LocalQueue;
use crate::task::yield_now::wake_yielded_tasks;
use crate::task::Task;
#[cfg(feature = "net")]
use crate::net::Handle;
use std::cell::{Cell, RefCell};
use std::ptr;
use std::sync::Arc;
use std::task::Waker;

thread_local! {
    pub(crate) static CURRENT_WORKER : Cell<* const ()> = Cell::new(ptr::null());
}

pub(crate) enum WorkerContext {
    Multi(MultiWorkerContext),
    #[cfg(feature = "net")]
    Curr(CurrentWorkerContext)
}

#[cfg(feature = "net")]
macro_rules! get_multi_worker_context {
    ($e:expr) => {
        match $e {
            crate::executor::worker::WorkerContext::Multi(ctx) => ctx,
            crate::executor::worker::WorkerContext::Curr(_) => unreachable!(),
        }
    };
}

#[cfg(not(feature = "net"))]
macro_rules! get_multi_worker_context {
    ($e:expr) => {
        {
            let crate::executor::worker::WorkerContext::Multi(ctx) = $e;
            ctx
        }
    };
}

pub(crate) use get_multi_worker_context;

pub(crate) struct MultiWorkerContext {
    pub(crate) worker: Arc<Worker>,
    #[cfg(feature = "net")]
    pub(crate) handle: Arc<Handle>
}

#[cfg(feature = "net")]
pub(crate) struct CurrentWorkerContext {
    pub(crate) handle: Arc<Handle>
}

/// Gets the worker context of the current thread
#[inline]
pub(crate) fn get_current_ctx() -> Option<&'static WorkerContext> {
    CURRENT_WORKER.with(|ctx| {
        let val = ctx.get();
        if val.is_null() {
            None
        } else {
            Some(unsafe { &*(val as *const _ as *const WorkerContext) })
        }
    })
}

impl MultiWorkerContext {
    fn run(&mut self) {
        let worker_ref = &self.worker;
        worker_ref.run();
    }

    fn release(&mut self) {
        self.worker.release();
    }
}

/// Runs the worker thread
pub(crate) fn run_worker(
    worker: Arc<Worker>,
    #[cfg(feature = "net")]
    handle: Arc<Handle>,
) {
    let cur_context = WorkerContext::Multi( MultiWorkerContext {
        worker,
        #[cfg(feature = "net")]
        handle,
    });

    struct Reset(*const ());

    impl Drop for Reset {
        fn drop(&mut self) {
            CURRENT_WORKER.with(|ctx| ctx.set(self.0))
        }
    }
    // store the worker to tls
    let _guard = CURRENT_WORKER.with(|cur| {
        let prev = cur.get();
        cur.set(&cur_context as *const _ as *const ());
        Reset(prev)
    });

    let mut cur_context = get_multi_worker_context!(cur_context);
    cur_context.run();
    cur_context.release();
}

pub(crate) struct Worker {
    pub(crate) index: u8,
    pub(crate) scheduler: Arc<MultiThreadScheduler>,
    pub(crate) inner: RefCell<Box<Inner>>,
    pub(crate) lifo: RefCell<Option<Task>>,
    pub(crate) yielded: RefCell<Vec<Waker>>,
}

unsafe impl Send for Worker {}
unsafe impl Sync for Worker {}

impl Worker {
    fn run(&self) {
        let mut inner = self.inner.borrow_mut();
        let inner = inner.as_mut();

        while !inner.is_cancel() {
            inner.increment_count();
            inner.periodic_check(self);

            // get a task from the queues and execute it
            if let Some(task) = self.get_task(inner) {
                self.run_task(task, inner);
                continue;
            }
            wake_yielded_tasks();
            // if there is no task, park the worker
            self.park_timeout(inner);

            self.check_cancel(inner);
        }
        self.pre_shutdown(inner);
    }

    fn pre_shutdown(&self, inner: &mut Inner) {
        // drop all tasks in the local queue
        loop {
            if let Some(task) = self.get_task(inner) {
                task.shutdown();
                continue;
            }
            if self.scheduler.has_no_work() {
                break;
            }
        }
        // thread 0 is responsible for dropping the tasks inside the global queue
        if self.index == 0 {
            let mut global = self.scheduler.get_global().lock().unwrap();
            loop {
                if let Some(task) = global.pop_front() {
                    task.shutdown();
                    continue;
                }
                if global.is_empty() {
                    break;
                }
            }
        }
    }

    fn get_task(&self, inner: &mut Inner) -> Option<Task> {
        // We're under worker environment, so it's safe to unwrap
        let ctx = get_multi_worker_context!(get_current_ctx().expect("worker get_current_ctx() fail"));

        // schedule lifo task first
        let mut lifo_slot = ctx.worker.lifo.borrow_mut();
        if let Some(task) = lifo_slot.take() {
            return Some(task);
        }

        self.scheduler.dequeue(self.index, inner)
    }

    fn run_task(&self, task: Task, inner: &mut Inner) {
        if inner.is_searching {
            inner.is_searching = false;
            if self.scheduler.record.dec_searching_num() {
                self.scheduler.wake_up_rand_one();
            }
        }

        task.run();
    }

    #[inline]
    fn check_cancel(&self, inner: &mut Inner) {
        inner.check_cancel(self)
    }

    fn park_timeout(&self, inner: &mut Inner) {
        let ctx = get_multi_worker_context!(get_current_ctx().unwrap());

        // still has works to do, go back to work
        if ctx.worker.lifo.borrow().is_some() || !inner.run_queue.is_empty() {
            return;
        }

        let is_searching = inner.is_searching;
        inner.is_searching = false;
        self.scheduler.turn_to_sleep(self.index, is_searching);

        while !inner.is_cancel {
            inner.parker.park();
            if self.scheduler.is_parked(self.index as usize) {
                self.check_cancel(inner);
                continue;
            }
            inner.is_searching = true;
            break;
        }
    }

    #[inline]
    fn release(&self) {
        // wait for tasks in queue to finish
        while !self.scheduler.has_no_work() {}
    }
}

pub(crate) struct Inner {
    /// A counter to define whether schedule global queue or local queue
    pub(crate) count: u32,
    /// Whether in searching state
    pub(crate) is_searching: bool,
    /// Whether the workers are canceled
    is_cancel: bool,
    /// local queue
    pub(crate) run_queue: LocalQueue,
    pub(crate) parker: Parker,
}

impl Inner {
    pub(crate) fn new(run_queues: LocalQueue, parker: Parker) -> Self {
        Inner {
            count: 0,
            is_searching: false,
            is_cancel: false,
            run_queue: run_queues,
            parker,
        }
    }
}

const GLOBAL_PERIODIC_INTERVAL: u8 = 61;

impl Inner {
    #[inline]
    fn increment_count(&mut self) {
        self.count = self.count.wrapping_add(1);
    }

    // checks if the worker is canceled
    #[inline]
    fn check_cancel(&mut self, worker: &Worker) {
        if !self.is_cancel {
            self.is_cancel = worker.scheduler.is_cancel();
        }
    }

    #[inline]
    fn periodic_check(&mut self, worker: &Worker) {
        if self.count & GLOBAL_PERIODIC_INTERVAL as u32 == 0 {
            self.check_cancel(worker);
            #[cfg(feature = "net")]
            if let Ok(mut driver) = self.parker.get_driver().try_lock() {
                let _ = driver
                    .drive(Some(std::time::Duration::from_millis(0)))
                    .expect("io driver failed");
            }
        }
    }

    #[inline]
    fn is_cancel(&self) -> bool {
        self.is_cancel
    }
}
