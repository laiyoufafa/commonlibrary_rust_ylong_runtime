/*
 * Copyright (c) 2023 Huawei Device Co., Ltd.
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::collections::VecDeque;
use std::option::Option::Some;
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::thread;
use std::time::Duration;

use crate::builder::{CallbackHook, CommonBuilder};
use crate::error::{ErrorKind, ScheduleError};
use crate::executor::PlaceholderScheduler;
use crate::task::TaskBuilder;
use crate::task::VirtualTableType;
use crate::{task, JoinHandle};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

pub(crate) const BLOCKING_THREAD_QUIT_WAIT_TIME: Duration = Duration::from_secs(3);

#[derive(Clone)]
pub(crate) struct BlockPoolSpawner {
    inner: Arc<Inner>,
}

impl Drop for BlockPoolSpawner {
    fn drop(&mut self) {
        self.shutdown(BLOCKING_THREAD_QUIT_WAIT_TIME);
    }
}

impl BlockPoolSpawner {
    pub fn new(builder: &CommonBuilder) -> BlockPoolSpawner {
        let keep_alive_time = builder
            .keep_alive_time
            .unwrap_or(BLOCKING_THREAD_KEEP_ALIVE_TIME);
        let max_thread_num = builder
            .max_blocking_pool_size
            .unwrap_or(BLOCKING_MAX_THEAD_NUM);
        BlockPoolSpawner {
            inner: Arc::new(Inner {
                shared: Mutex::new(Shared {
                    queue: VecDeque::new(),
                    total_thread_num: 0,
                    idle_thread_num: 0,
                    notify_num: 0,
                    current_permanent_thread_num: 0,
                    shutdown: false,
                    worker_id: 0,
                    worker_threads: VecDeque::new(),
                }),
                condvar: Condvar::new(),
                shutdown_shared: Mutex::new(false),
                shutdown_condvar: Condvar::new(),
                stack_size: builder.stack_size,
                after_start: builder.after_start.clone(),
                before_stop: builder.before_stop.clone(),
                max_thread_num,
                keep_alive_time,
                max_permanent_thread_num: builder.blocking_permanent_thread_num,
            }),
        }
    }

    pub fn shutdown(&mut self, timeout: Duration) -> bool {
        let mut shared = self.inner.shared.lock().unwrap();

        if shared.shutdown {
            return false;
        }
        self.inner.condvar.notify_all();
        let workers = std::mem::take(&mut shared.worker_threads);
        drop(shared);

        let shutdown_shared = self.inner.shutdown_shared.lock().unwrap();

        if *self
            .inner
            .shutdown_condvar
            .wait_timeout(shutdown_shared, timeout)
            .unwrap()
            .0
        {
            for handle in workers {
                let _ = handle.1.join();
            }
            return true;
        }
        false
    }
}

const BLOCKING_THREAD_KEEP_ALIVE_TIME: Duration = Duration::from_secs(5);
pub const BLOCKING_MAX_THEAD_NUM: u8 = 50;

/// Inner struct for [`BlockPoolSpawner`].
struct Inner {
    /// Shared information of the threads in the blocking pool
    shared: Mutex<Shared>,

    /// Used for thread synchronization
    condvar: Condvar,

    /// Stores the notification for shutting down
    shutdown_shared: Mutex<bool>,

    /// Used for thread shutdown synchronization
    shutdown_condvar: Condvar,

    /// Stack size of each thread in the blocking pool
    stack_size: Option<usize>,

    /// A callback func to be called after thread starts
    after_start: Option<CallbackHook>,

    /// A callback func to be called before thread stops
    before_stop: Option<CallbackHook>,

    /// Maximum thread number for the blocking pool
    max_thread_num: u8,

    /// Maximum keep-alive time for idle threads
    keep_alive_time: Duration,

    /// Max number of permanent threads
    max_permanent_thread_num: u8,
}

/// Shared info among the blocking pool
struct Shared {
    /// Task queue
    queue: VecDeque<Task>,

    /// Number of current created threads
    total_thread_num: u8,

    /// Number of current idle threads
    idle_thread_num: u8,

    /// Number of calls to `notify_one`, prevents spurious wakeup of condvar.
    notify_num: u8,

    /// number of permanent threads in the pool
    current_permanent_thread_num: u8,

    /// Shutdown flag of the pool
    shutdown: bool,

    /// Corresponds with the JoinHandles of the worker threads
    worker_id: usize,

    /// Stores the JoinHandles of the worker threads
    worker_threads: VecDeque<(usize, thread::JoinHandle<()>)>,
}

type Task = task::Task;

// ===== impl BlockPoolSpawner =====
impl BlockPoolSpawner {
    pub fn create_permanent_threads(&self) -> Result<(), ScheduleError> {
        for _ in 0..self.inner.max_permanent_thread_num {
            let mut shared = self.inner.shared.lock().unwrap();
            shared.total_thread_num += 1;
            let worker_id = shared.worker_id;
            let mut builder = thread::Builder::new().name(format!("block-r-{worker_id}"));
            if let Some(stack_size) = self.inner.stack_size {
                builder = builder.stack_size(stack_size);
            }
            let inner = self.inner.clone();
            let join_handle = builder.spawn(move || inner.run(worker_id));
            match join_handle {
                Ok(join_handle) => {
                    shared.worker_threads.push_back((worker_id, join_handle));
                    shared.worker_id += 1;
                }
                Err(err) => {
                    return Err(ScheduleError::new(ErrorKind::BlockSpawnErr, err));
                }
            }
        }
        Ok(())
    }

    pub(crate) fn spawn_blocking<T, R>(&self, builder: &TaskBuilder, task: T) -> JoinHandle<R>
    where
        T: FnOnce() -> R,
        T: Send + 'static,
        R: Send + 'static,
    {
        let task = BlockingTask(Some(task));
        let scheduler: Weak<PlaceholderScheduler> = Weak::new();
        let (task, handle) = Task::create_task(builder, scheduler, task, VirtualTableType::Ylong);
        let _ = self.spawn(task);
        handle
    }

    fn spawn(&self, task: Task) -> Result<(), ScheduleError> {
        let mut shared = self.inner.shared.lock().unwrap();

        // if the shutdown flag is on, cancel the task
        if shared.shutdown {
            return Err(ErrorKind::TaskShutdown.into());
        }

        shared.queue.push_back(task);

        if shared.idle_thread_num == 0 {
            if shared.total_thread_num == self.inner.max_thread_num {
                // thread number has reached maximum, do nothing
            } else {
                // there is no idle thread and the maximum thread number has not been reached,
                // therefore create a new thread
                shared.total_thread_num += 1;
                // sets all required attributes for the thread
                let worker_id = shared.worker_id;
                let mut builder = thread::Builder::new().name(format!("block-{worker_id}"));
                if let Some(stack_size) = self.inner.stack_size {
                    builder = builder.stack_size(stack_size);
                }

                let inner = self.inner.clone();
                let join_handle = builder.spawn(move || inner.run(worker_id));
                match join_handle {
                    Ok(join_handle) => {
                        shared.worker_threads.push_back((worker_id, join_handle));
                        shared.worker_id += 1;
                    }
                    Err(e) => {
                        panic!("os can't spawn worker thread: {}", e);
                    }
                }
            }
        } else {
            shared.idle_thread_num -= 1;
            shared.notify_num += 1;
            self.inner.condvar.notify_one();
        }
        Ok(())
    }
}

impl Inner {
    fn run(&self, worker_id: usize) {
        if let Some(f) = &self.after_start {
            f()
        }

        let mut shared = self.shared.lock().unwrap();

        'main: loop {
            // get a task from the global queue
            while let Some(task) = shared.queue.pop_front() {
                drop(shared);
                task.run();
                shared = self.shared.lock().unwrap();
            }

            shared.idle_thread_num += 1;
            while !shared.shutdown {
                // permanent waits, the thread keep alive until shutdown.
                if shared.current_permanent_thread_num < self.max_permanent_thread_num {
                    shared.current_permanent_thread_num += 1;
                    shared = self.condvar.wait(shared).unwrap();
                    shared.current_permanent_thread_num -= 1;
                    // Combining a loop to prevent spurious wakeup of condvar, if there is a
                    // spurious wakeup, the `notify_num` will be 0 and the loop will continue.
                    if shared.notify_num != 0 {
                        shared.notify_num -= 1;
                        break;
                    }
                } else {
                    // if the thread is not permanent, set the keep-alive time for releasing
                    // the thread
                    let time_out_lock_res = self
                        .condvar
                        .wait_timeout(shared, self.keep_alive_time)
                        .unwrap();
                    shared = time_out_lock_res.0;
                    let timeout_result = time_out_lock_res.1;

                    // Combining a loop to prevent spurious wakeup of condvar, if there is a
                    // spurious wakeup, the `notify_num` will be 0 and the loop will continue.
                    if shared.notify_num != 0 {
                        shared.notify_num -= 1;
                        break;
                    }
                    // expires, release the thread
                    if !shared.shutdown && timeout_result.timed_out() {
                        for (thread_id, thread) in shared.worker_threads.iter().enumerate() {
                            if thread.0 == worker_id {
                                shared.worker_threads.remove(thread_id);
                                break;
                            }
                        }
                        break 'main;
                    }
                }
            }

            if shared.shutdown {
                // empty the tasks in the global queue
                while let Some(_task) = shared.queue.pop_front() {
                    drop(shared);
                    shared = self.shared.lock().unwrap();
                }
                break;
            }
        }

        // thread exit
        shared.total_thread_num = shared
            .total_thread_num
            .checked_sub(1)
            .expect("total thread num underflowed");
        shared.idle_thread_num = shared
            .idle_thread_num
            .checked_sub(1)
            .expect("idle thread num underflowed");

        let shutdown = shared.shutdown;
        drop(shared);

        if shutdown {
            *self.shutdown_shared.lock().unwrap() = true;
            self.shutdown_condvar.notify_one();
        }

        if let Some(f) = &self.before_stop {
            f()
        }
    }
}

struct BlockingTask<T>(Option<T>);

impl<T> Unpin for BlockingTask<T> {}

impl<T, R> Future for BlockingTask<T>
where
    T: FnOnce() -> R,
{
    type Output = R;

    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let func = self.0.take().expect("no run two times");
        Poll::Ready(func())
    }
}

#[cfg(all(test))]
mod test {
    use crate::builder::RuntimeBuilder;
    use crate::executor::blocking_pool::BlockPoolSpawner;
    use crate::executor::PlaceholderScheduler;
    use crate::task::Task;
    use crate::task::VirtualTableType;
    use std::sync::Weak;
    use std::time::Duration;

    /*
     * @title  BlockPoolSpawner::new() UT test
     * @design The function has no invalid values in the input, no exception branch, direct check function, return value
     * @precon Use BlockPoolSpawner::new(), Get its creation object
     * @brief  Describe test case execution
     *         1、Checking the parameters after initialization is completed
     * @expect The function entry has no invalid value, no exception branch, and the property value should be related to the entry after the initialization is completed
     * @auto   true
     */
    #[test]
    fn ut_blocking_pool_new() {
        let thread_pool_builder =
            RuntimeBuilder::new_multi_thread().keep_alive_time(Duration::from_secs(1));
        let blocking_pool = BlockPoolSpawner::new(&thread_pool_builder.common);
        assert_eq!(
            blocking_pool.inner.stack_size,
            thread_pool_builder.common.stack_size
        );
        assert_eq!(
            blocking_pool.inner.max_thread_num,
            thread_pool_builder.common.max_blocking_pool_size.unwrap()
        );
        assert_eq!(
            blocking_pool.inner.keep_alive_time,
            thread_pool_builder.common.keep_alive_time.unwrap()
        );
        assert_eq!(
            blocking_pool.inner.max_permanent_thread_num,
            thread_pool_builder.common.blocking_permanent_thread_num
        );
    }

    /*
     * @title  BlockPoolSpawner::shutdown() UT test
     * @design The function entry has no invalid value, there is an exception branch, direct check function, return value
     * @precon Use BlockPoolSpawner::new(), Get its creation object
     * @brief  Describe test case execution
     *         1、When shared.shutdown is false, the thread is safely exited without a timeout
     *         2、When shared.shutdown is false, the thread is not safely exited in case of timeout
     *         3、When shared.shutdown is true, BlockPoolSpawner::shutdown returns directly, representing that the blocking thread pool has safely exited
     * @expect The function entry has no invalid value, no exception branch, and the property value should be related to the entry after the initialization is completed
     * @auto   true
     */
    #[test]
    fn ut_blocking_pool_shutdown() {
        let thread_pool_builder = RuntimeBuilder::new_multi_thread();
        let mut blocking_pool = BlockPoolSpawner::new(&thread_pool_builder.common);
        blocking_pool.inner.shared.lock().unwrap().shutdown = true;
        assert!(!blocking_pool.shutdown(Duration::from_secs(3)));

        let thread_pool_builder = RuntimeBuilder::new_multi_thread();
        let mut blocking_pool = BlockPoolSpawner::new(&thread_pool_builder.common);
        let spawner_inner_clone = blocking_pool.inner.clone();
        let _thread = std::thread::spawn(move || {
            *spawner_inner_clone.shutdown_shared.lock().unwrap() = true;
            spawner_inner_clone.shutdown_condvar.notify_one();
        });
        assert!(blocking_pool.shutdown(Duration::from_secs(3)));

        let thread_pool_builder = RuntimeBuilder::new_multi_thread();
        let mut blocking_pool = BlockPoolSpawner::new(&thread_pool_builder.common);
        let spawner_inner_clone = blocking_pool.inner.clone();
        let _thread = std::thread::spawn(move || {
            spawner_inner_clone.shutdown_condvar.notify_one();
        });

        blocking_pool.inner.shared.lock().unwrap().shutdown = true;
        assert!(!blocking_pool.shutdown(Duration::from_secs(0)));
    }

    /*
     * @title  BlockPoolSpawner::create_permanent_threads() UT test
     * @design The function has no input parameters, there is an exception branch, direct check function, return value
     * @precon Use BlockPoolSpawner::new(), Get its creation object
     * @brief  Describe test case execution
     *         1、self.inner.is_permanent == true, self.inner.worker_name.clone() != None, self.inner.stack_size != None
     *         2、self.inner.is_permanent == true, self.inner.worker_name.clone() == None, self.inner.stack_size == None
     *         3、self.inner.is_permanent == false
     * @expect The function entry has no invalid value, no exception branch, and the property value should be related to the entry after the initialization is completed. Compare whether the naming under different branches corresponds to each other.
     * @auto   true
     */
    #[test]
    fn ut_blocking_pool_spawner_create_permanent_threads() {
        let thread_pool_builder =
            RuntimeBuilder::new_multi_thread().blocking_permanent_thread_num(4);
        let blocking_pool = BlockPoolSpawner::new(&thread_pool_builder.common);
        assert!(blocking_pool.create_permanent_threads().is_ok());
        assert_eq!(blocking_pool.inner.shared.lock().unwrap().worker_id, 4);

        let thread_pool_builder =
            RuntimeBuilder::new_multi_thread().blocking_permanent_thread_num(4);
        let common = RuntimeBuilder::new_multi_thread().blocking_permanent_thread_num(4);
        let blocking_pool = BlockPoolSpawner::new(&common.common);
        assert!(blocking_pool.create_permanent_threads().is_ok());
        assert_eq!(
            blocking_pool.inner.shared.lock().unwrap().worker_id,
            thread_pool_builder.common.blocking_permanent_thread_num as usize
        );
        assert_eq!(
            blocking_pool
                .inner
                .shared
                .lock()
                .unwrap()
                .worker_threads
                .pop_front()
                .unwrap()
                .1
                .thread()
                .name()
                .unwrap(),
            "block-r-0"
        );

        let thread_pool_builder = RuntimeBuilder::new_multi_thread()
            .blocking_permanent_thread_num(4)
            .worker_name(String::from("test"));
        let common = RuntimeBuilder::new_multi_thread()
            .blocking_permanent_thread_num(4)
            .worker_name(String::from("test"));
        let blocking_pool = BlockPoolSpawner::new(&common.common);
        assert!(blocking_pool.create_permanent_threads().is_ok());
        assert_eq!(
            blocking_pool.inner.shared.lock().unwrap().worker_id,
            thread_pool_builder.common.blocking_permanent_thread_num as usize
        );
        assert_eq!(
            blocking_pool
                .inner
                .shared
                .lock()
                .unwrap()
                .worker_threads
                .pop_front()
                .unwrap()
                .1
                .thread()
                .name()
                .unwrap(),
            "block-r-0"
        );
    }

    /*
     * @title  BlockPoolSpawner::spawn() UT test
     * @design The function has no input parameters, there is an exception branch, direct check function, return value
     * @precon Use BlockPoolSpawner::new(), Get its creation object
     * @brief  Describe test case execution
     *         1、shared.shutdown == true, return directly.
     *         2、shared.shutdown == false, shared.idle_thread_num != 0
     *         3、shared.shutdown == false, shared.idle_thread_num == 0, shared.total_thread_num == self.inner.max_pool_size
     *         4、shared.shutdown == false, shared.idle_thread_num == 0, shared.total_thread_num != self.inner.max_pool_size, self.inner.worker_name.clone() != None
     *         5、shared.shutdown == false, shared.idle_thread_num == 0, shared.total_thread_num != self.inner.max_pool_size, self.inner.worker_name.clone() == None
     * @expect The function entry has no invalid value, no exception branch, and the property value should be related to the entry after the initialization is completed. Compare whether the naming under different branches corresponds to each other.
     * @auto   true
     */
    #[test]
    fn ut_blocking_pool_spawner_spawn() {
        use crate::executor::blocking_pool::BlockingTask;
        use crate::task::TaskBuilder;
        use std::thread::sleep;

        let thread_pool_builder = RuntimeBuilder::new_multi_thread();
        let blocking_pool = BlockPoolSpawner::new(&thread_pool_builder.common);
        blocking_pool.inner.shared.lock().unwrap().shutdown = true;
        let task = BlockingTask(Some(move || {
            sleep(Duration::from_millis(10));
            String::from("task")
        }));
        let builder = TaskBuilder::new();
        let scheduler: Weak<PlaceholderScheduler> = Weak::new();
        let (task, _) = Task::create_task(&builder, scheduler, task, VirtualTableType::Ylong);
        assert!(blocking_pool.spawn(task).is_err());

        let thread_pool_builder = RuntimeBuilder::new_multi_thread();
        let blocking_pool = BlockPoolSpawner::new(&thread_pool_builder.common);
        blocking_pool.inner.shared.lock().unwrap().shutdown = false;
        blocking_pool.inner.shared.lock().unwrap().idle_thread_num = 1;
        let task = BlockingTask(Some(move || {
            sleep(Duration::from_millis(10));
            String::from("task")
        }));
        let scheduler: Weak<PlaceholderScheduler> = Weak::new();
        let (task, _) = Task::create_task(&builder, scheduler, task, VirtualTableType::Ylong);
        blocking_pool.spawn(task).expect("failed");
        assert_eq!(blocking_pool.inner.shared.lock().unwrap().notify_num, 1);

        let thread_pool_builder = RuntimeBuilder::new_multi_thread().max_blocking_pool_size(4);
        let blocking_pool = BlockPoolSpawner::new(&thread_pool_builder.common);
        blocking_pool.inner.shared.lock().unwrap().shutdown = false;
        blocking_pool.inner.shared.lock().unwrap().idle_thread_num = 0;
        blocking_pool.inner.shared.lock().unwrap().total_thread_num = 4;
        let task = BlockingTask(Some(move || {
            sleep(Duration::from_millis(10));
            String::from("task")
        }));
        let scheduler: Weak<PlaceholderScheduler> = Weak::new();
        let (task, _) = Task::create_task(&builder, scheduler, task, VirtualTableType::Ylong);
        blocking_pool.spawn(task).expect("failed");
        assert_eq!(blocking_pool.inner.shared.lock().unwrap().worker_id, 0);

        let thread_pool_builder = RuntimeBuilder::new_multi_thread().max_blocking_pool_size(4);
        let blocking_pool = BlockPoolSpawner::new(&thread_pool_builder.common);
        blocking_pool.inner.shared.lock().unwrap().shutdown = false;
        blocking_pool.inner.shared.lock().unwrap().idle_thread_num = 0;
        blocking_pool.inner.shared.lock().unwrap().total_thread_num = 3;

        let task = BlockingTask(Some(move || {
            sleep(Duration::from_millis(10));
            String::from("task")
        }));
        let scheduler: Weak<PlaceholderScheduler> = Weak::new();
        let (task, _) = Task::create_task(&builder, scheduler, task, VirtualTableType::Ylong);
        blocking_pool.spawn(task).expect("failed");
        assert_eq!(
            blocking_pool
                .inner
                .shared
                .lock()
                .unwrap()
                .worker_threads
                .pop_front()
                .unwrap()
                .1
                .thread()
                .name()
                .unwrap(),
            "block-0"
        );

        let thread_pool_builder = RuntimeBuilder::new_multi_thread()
            .max_blocking_pool_size(4)
            .worker_name(String::from("test"));
        let blocking_pool = BlockPoolSpawner::new(&thread_pool_builder.common);
        blocking_pool.inner.shared.lock().unwrap().shutdown = false;
        blocking_pool.inner.shared.lock().unwrap().idle_thread_num = 0;
        blocking_pool.inner.shared.lock().unwrap().total_thread_num = 3;
        let task = BlockingTask(Some(move || {
            sleep(Duration::from_millis(10));
            String::from("task")
        }));
        let scheduler: Weak<PlaceholderScheduler> = Weak::new();
        let (task, _) = Task::create_task(&builder, scheduler, task, VirtualTableType::Ylong);
        blocking_pool.spawn(task).expect("failed");
        assert_eq!(
            blocking_pool
                .inner
                .shared
                .lock()
                .unwrap()
                .worker_threads
                .pop_front()
                .unwrap()
                .1
                .thread()
                .name()
                .unwrap(),
            "block-0"
        );
    }
}
