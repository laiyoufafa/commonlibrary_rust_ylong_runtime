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

use crate::builder::CallbackHook;
use crate::executor::sleeper::Sleepers;
use crate::executor::{worker, Schedule};
use crate::executor::worker::{get_current_ctx, run_worker, Worker, WorkerContext};
use crate::task::{Task, TaskBuilder, VirtualTableType};
use crate::util::num_cpus::get_cpu_num;
use std::cell::RefCell;
use std::collections::LinkedList;
use std::sync::atomic::Ordering::{Acquire, SeqCst};
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::time::Duration;
use std::{cmp, thread};

use crate::util::core_affinity::set_current_affinity;

use crate::builder::multi_thread_builder::MultiThreadBuilder;
use crate::executor::parker::Parker;
use crate::executor::queue::{GlobalQueue, LocalQueue, LOCAL_QUEUE_CAP};
use crate::util::fastrand::fast_random;
use crate::JoinHandle;
use std::future::Future;
#[cfg(feature = "net")]
use crate::net::{Driver, Handle};

const ASYNC_THREAD_QUIT_WAIT_TIME: Duration = Duration::from_secs(3);
pub(crate) const GLOBAL_POLL_INTERVAL: u8 = 61;

pub(crate) struct MultiThreadScheduler {
    /// Async pool shutdown state
    is_cancel: AtomicBool,
    /// Sleeping workers info
    sleepers: Sleepers,
    /// Number of total workers
    pub(crate) num_workers: usize,
    /// Join Handles for all threads in the executor
    handles: RwLock<Vec<Parker>>,
    /// records the number of stealing workers and the number of working workers.
    pub(crate) record: Record,
    /// The global queue of the executor
    global: GlobalQueue,
    /// A set of all the local queues in the executor
    locals: Vec<LocalQueue>,
    #[cfg(feature = "net")]
    pub(crate) io_handle: Arc<Handle>,
}

const ACTIVE_WORKER_SHIFT: usize = 16;
const SEARCHING_MASK: usize = (1 << ACTIVE_WORKER_SHIFT) - 1;
const ACTIVE_MASK: usize = !SEARCHING_MASK;

//        32 bits          16 bits       16 bits
// |-------------------| working num | searching num|
pub(crate) struct Record(AtomicUsize);

impl Record {
    pub(crate) fn new(num_unpark: usize) -> Self {
        Self(AtomicUsize::new(num_unpark << ACTIVE_WORKER_SHIFT))
    }

    //Return true if it is the last searching thread
    pub(crate) fn dec_searching_num(&self) -> bool {
        let ret = self.0.fetch_sub(1, SeqCst);
        (ret & SEARCHING_MASK) == 1
    }

    pub(crate) fn inc_searching_num(&self) {
        self.0.fetch_add(1, SeqCst);
    }

    pub(crate) fn inc_active_num(&self, to_searching: bool) {
        let mut inc = 1 << ACTIVE_WORKER_SHIFT;
        if to_searching {
            inc += 1;
        }
        self.0.fetch_add(inc, SeqCst);
    }

    pub(crate) fn dec_active_num(&self, is_searching: bool) -> bool {
        let mut dec = 1 << ACTIVE_WORKER_SHIFT;
        if is_searching {
            dec += 1;
        }

        let ret = self.0.fetch_sub(dec, SeqCst);
        let unpark_num = ((ret & ACTIVE_MASK) >> ACTIVE_WORKER_SHIFT) - 1;
        unpark_num == 0
    }

    pub(crate) fn load_state(&self) -> (usize, usize) {
        let union_num = self.0.load(SeqCst);

        let searching_num = union_num & SEARCHING_MASK;
        let unpark_num = (union_num & ACTIVE_MASK) >> ACTIVE_WORKER_SHIFT;

        (unpark_num, searching_num)
    }
}

impl Schedule for MultiThreadScheduler {
    #[inline]
    fn schedule(&self, task: Task, lifo: bool) {
        if self.enqueue(task, lifo) {
            self.wake_up_rand_one();
        }
    }
}

impl MultiThreadScheduler {
    pub(crate) fn new(
        thread_num: usize,
        #[cfg(feature = "net")]
        io_handle: Arc<Handle>,
    ) -> Self {
        let mut locals = Vec::new();
        for _ in 0..thread_num {
            locals.push(LocalQueue::new());
        }

        Self {
            is_cancel: AtomicBool::new(false),
            sleepers: Sleepers::new(thread_num),
            num_workers: thread_num,
            handles: RwLock::new(Vec::new()),
            record: Record::new(thread_num),
            global: GlobalQueue::new(),
            locals,
            #[cfg(feature = "net")]
            io_handle
        }
    }

    pub(crate) fn is_cancel(&self) -> bool {
        self.is_cancel.load(Acquire)
    }

    pub(crate) fn set_cancel(&self) {
        self.is_cancel.store(true, SeqCst);
    }

    pub(crate) fn cancel(&self) {
        self.set_cancel();
        self.wake_up_all();
    }

    fn wake_up_all(&self) {
        let join_handle = self.handles.read().unwrap();
        for item in join_handle.iter() {
            item.unpark(
                #[cfg(feature = "net")]
                self.io_handle.clone()
            );
        }
    }

    #[inline]
    pub(crate) fn wake_up_specific_one(&self, worker_id: usize) -> bool {
        if self.sleepers.pop_specific(worker_id) {
            self.record.inc_active_num(false);
            true
        } else {
            false
        }
    }

    #[inline]
    pub(crate) fn is_parked(&self, worker_id: usize) -> bool {
        self.sleepers.is_parked(worker_id)
    }

    pub(crate) fn wake_up_rand_one(&self) {
        let (num_unpark, num_searching) = self.record.load_state();

        if num_unpark >= self.num_workers || num_searching > 0 {
            return;
        }
        if let Some(index) = self.sleepers.pop() {
            self.record.inc_active_num(true);

            self.handles.read().unwrap().get(index).unwrap().unpark(#[cfg(feature = "net")]self.io_handle.clone());
        }
    }

    #[inline]
    pub(crate) fn wake_up_if_one_task_left(&self) {
        if !self.has_no_work() {
            self.wake_up_rand_one()
        }
    }

    pub(crate) fn turn_to_sleep(&self, index: u8, is_searching: bool) {
        // If it's the last thread going to sleep, check if there are any tasks left.
        // If yes, wakes up a thread.
        self.sleepers.push(index as usize);

        if self.record.dec_active_num(is_searching) {
            self.wake_up_if_one_task_left();
        }
    }

    pub(crate) fn create_local_queue(&self, index: u8) -> LocalQueue {
        let local_run_queue = self.locals.get(index as usize).unwrap();
        LocalQueue {
            inner: local_run_queue.inner.clone(),
        }
    }

    pub(crate) fn has_no_work(&self) -> bool {
        // check if local queues are empty
        for index in 0..self.num_workers {
            let item = self.locals.get(index).unwrap();
            if !item.is_empty() {
                return false;
            }
        }
        // then check is global queue empty
        self.global.is_empty()
    }

    // The returned value indicates whether or not to wake up another worker
    // We need to wake another worker under these circumstances:
    // 1. The task has been inserted into the global queue
    // 2. The lifo slot is taken, we push the old task into the local queue
    pub(crate) fn enqueue(&self, mut task: Task, lifo: bool) -> bool {
        let cur_worker = get_current_ctx();

        // WorkerContext::Curr will never enter here.
        if let Some(WorkerContext::Multi(cur_worker)) = cur_worker {
            if !std::ptr::eq(&self.global, &cur_worker.worker.scheduler.global) {
                self.global.push_back(task);
                return true;
            }

            if lifo {
                let mut lifo_slot = cur_worker.worker.lifo.borrow_mut();
                let prev_task = lifo_slot.take();
                if let Some(prev) = prev_task {
                    // there is some task in lifo slot, therefore we put the prev task
                    // into run queue, and put the current task into the lifo slot
                    *lifo_slot = Some(task);
                    task = prev;
                } else {
                    // there is no task in lifo slot, return immediately
                    *lifo_slot = Some(task);
                    return false;
                }
            }

            let local_run_queue = self.locals.get(cur_worker.worker.index as usize).unwrap();
            local_run_queue.push_back(task, &self.global);
            return true;
        }

        // If the local queue of the current worker is full, push the task into the global queue
        self.global.push_back(task);
        true
    }

    pub(crate) fn dequeue(&self, index: u8, worker_inner: &mut worker::Inner) -> Option<Task> {
        let local_run_queue = &worker_inner.run_queue;
        let count = worker_inner.count;

        let task = {
            // For every 61 times of execution, dequeue a task from the global queue first.
            // Otherwise, dequeue a task from the local queue. However, if the local queue
            // has no task, dequeue a task from the global queue instead.
            if count % GLOBAL_POLL_INTERVAL as u32 == 0 {
                let limit = local_run_queue.remaining() as usize;
                let task = self
                    .global
                    .pop_batch(self.num_workers, local_run_queue, limit);
                match task {
                    Some(task) => Some(task),
                    None => local_run_queue.pop_front(),
                }
            } else {
                let local_task = local_run_queue.pop_front();
                match local_task {
                    Some(task) => Some(task),
                    None => {
                        self.global
                            .pop_batch(self.num_workers, local_run_queue, LOCAL_QUEUE_CAP)
                    }
                }
            }
        };

        if task.is_some() {
            return task;
        }

        // There is no task in the local queue or the global queue, so we try to steal
        // tasks from another worker's local queue.
        // The number of stealing worker should be less than half of the total worker number.
        if !worker_inner.is_searching {
            let (_, num_searching) = self.record.load_state();
            if num_searching * 2 < self.num_workers {
                // increment searching worker number
                self.record.inc_searching_num();
                worker_inner.is_searching = true;
            } else {
                return None;
            }
        }

        let num = self.locals.len();
        let start = fast_random() >> 56;

        for i in 0..num {
            let i = (start + i) % num;
            // skip the current worker's local queue
            if i == index as usize {
                continue;
            }
            let target = self.locals.get(i).unwrap();
            if let Some(task) = target.steal_into(local_run_queue) {
                return Some(task);
            }
        }
        // if there is no task to steal, we check global queue for one last time
        self.global.pop_front()
    }

    pub(crate) fn get_global(&self) -> &Mutex<LinkedList<Task>> {
        self.global.get_global()
    }
}

#[derive(Clone)]
pub(crate) struct AsyncPoolSpawner {
    pub(crate) inner: Arc<Inner>,

    pub(crate) exe_mng_info: Arc<MultiThreadScheduler>,
}

impl Drop for AsyncPoolSpawner {
    fn drop(&mut self) {
        self.release()
    }
}

pub(crate) struct Inner {
    /// Number of total threads
    pub(crate) total: u8,
    /// Core-affinity setting of the threads
    is_affinity: bool,
    /// Handle for shutting down the pool
    shutdown_handle: Arc<(Mutex<u8>, Condvar)>,
    /// A callback func to be called after thread starts
    after_start: Option<CallbackHook>,
    /// A callback func to be called before thread stops
    before_stop: Option<CallbackHook>,
    /// Name of the worker threads
    worker_name: Option<String>,
    /// Stack size of each thread
    stack_size: Option<usize>,
}

fn get_cpu_core() -> u8 {
    cmp::max(1, get_cpu_num() as u8)
}

fn async_thread_proc(
    inner: Arc<Inner>, 
    worker: Arc<Worker>,
    #[cfg(feature = "net")]
    handle: Arc<Handle>
) {
    if let Some(f) = inner.after_start.clone() {
        f();
    }

    run_worker(worker, #[cfg(feature = "net")]handle);
    let (lock, cvar) = &*(inner.shutdown_handle.clone());
    let mut finished = lock.lock().unwrap();
    *finished += 1;

    // the last thread wakes up the main thread
    if *finished >= inner.total {
        cvar.notify_one();
    }

    if let Some(f) = inner.before_stop.clone() {
        f();
    }
}

impl AsyncPoolSpawner {
    pub(crate) fn new(builder: &MultiThreadBuilder) -> Self {
        #[cfg(feature = "net")]
        let (handle, driver) = Driver::initialize();

        let thread_num = builder.core_thread_size.unwrap_or_else(get_cpu_core);
        let spawner = AsyncPoolSpawner {
            inner: Arc::new(Inner {
                total: thread_num,
                is_affinity: builder.common.is_affinity,
                shutdown_handle: Arc::new((Mutex::new(0u8), Condvar::new())),
                after_start: builder.common.after_start.clone(),
                before_stop: builder.common.before_stop.clone(),
                worker_name: builder.common.worker_name.clone(),
                stack_size: builder.common.stack_size,
            }),
            exe_mng_info: Arc::new(
                MultiThreadScheduler::new(
                    thread_num as usize,
                    #[cfg(feature = "net")]
                    handle
                )
            ),
        };
        spawner.create_async_thread_pool(
            #[cfg(feature = "net")]
            driver
        );
        spawner
    }

    pub(crate) fn create_async_thread_pool(
        &self,
        #[cfg(feature = "net")]
        io_driver: Arc<Mutex<Driver>>
    ) {
        let mut workers = vec![];
        for index in 0..self.inner.total {
            let local_queue = self.exe_mng_info.create_local_queue(index);
            let local_run_queue = Box::new(
                worker::Inner::new(
                    local_queue, 
                    Parker::new(
                        #[cfg(feature = "net")]
                        io_driver.clone()
                    )
                )
            );
            workers.push(Arc::new(Worker {
                index,
                scheduler: self.exe_mng_info.clone(),
                inner: RefCell::new(local_run_queue),
                lifo: RefCell::new(None),
                yielded: RefCell::new(Vec::new()),
            }))
        }

        for (worker_id, worker) in workers.drain(..).enumerate() {
            #[cfg(feature = "net")]
            let work_arc_handle = self.exe_mng_info.io_handle.clone();
            // set up thread attributes
            let mut builder = thread::Builder::new();
            if let Some(worker_name) = self.inner.worker_name.clone() {
                builder = builder.name(format!("async-{worker_id}-{worker_name}"));
            } else {
                builder = builder.name(format!("async-{worker_id}"));
            }
            if let Some(stack_size) = self.inner.stack_size {
                builder = builder.stack_size(stack_size);
            }

            let inner = self.inner.clone();

            if self.inner.is_affinity {
                let parker = worker.inner.borrow().parker.clone();

                let result = builder.spawn(move || {
                    let cpu_core_num = get_cpu_core() as usize;
                    let cpu_id = worker_id % cpu_core_num;
                    set_current_affinity(cpu_id).expect("set_current_affinity() fail!");
                    async_thread_proc(
                        inner, 
                        worker,
                        #[cfg(feature = "net")]
                        work_arc_handle
                    );
                });

                match result {
                    Ok(_) => {
                        self.exe_mng_info.handles.write().unwrap().push(parker);
                    }
                    Err(e) => panic!("os cannot spawn worker threads: {}", e),
                }
            } else {
                let parker = worker.inner.borrow().parker.clone();
                let result = builder.spawn(move || {
                    async_thread_proc(
                        inner, 
                        worker,
                        #[cfg(feature = "net")]
                        work_arc_handle
                    );
                });
                match result {
                    Ok(_) => {
                        self.exe_mng_info.handles.write().unwrap().push(parker);
                    }
                    Err(e) => panic!("os cannot spawn worker threads: {}", e),
                }
            }
        }
    }

    pub(crate) fn spawn<T>(&self, builder: &TaskBuilder, task: T) -> JoinHandle<T::Output>
    where
        T: Future + Send + 'static,
        T::Output: Send + 'static,
    {
        let exe_scheduler = Arc::downgrade(&self.exe_mng_info);
        let (task, join_handle) =
            Task::create_task(builder, exe_scheduler, task, VirtualTableType::Ylong);

        self.exe_mng_info.schedule(task, false);
        join_handle
    }

    /// # Safety
    /// Users need to guarantee that the future will remember lifetime and thus compiler will capture
    /// lifetime issues, or the future will complete when its context remains valid. If not, currently
    /// runtime initialization will cause memory error.
    ///
    /// ## Memory issue example
    /// No matter using which type (current / multi thread) of runtime, the following code can compile.
    /// When the variable `slice` gets released when the function ends, any handles returned from
    /// this function rely on a dangled pointer.
    ///
    /// ```no run
    ///  fn err_example(runtime: &Runtime) -> JoinHandle<()> {
    ///     let builder = TaskBuilder::default();
    ///     let mut slice = [1, 2, 3, 4, 5];
    ///     let borrow = &mut slice;
    ///     match &runtime.async_spawner {
    ///         AsyncHandle::CurrentThread(pool) => {
    ///             pool.spawn_with_ref(
    ///                 &builder,
    ///                 async { borrow.iter_mut().for_each(|x| *x *= 2) }
    ///             )
    ///        }
    ///        AsyncHandle::MultiThread(pool) => {
    ///             pool.spawn_with_ref(
    ///                 &builder,
    ///                 async { borrow.iter_mut().for_each(|x| *x *= 2) }
    ///             )
    ///        }
    ///     }
    /// }
    ///
    /// let runtime = Runtime::new().unwrap();
    /// let handle = spawn_blocking(
    ///     move || block_on(err_example(&runtime)).unwrap()
    /// );
    /// ```
    pub(crate) unsafe fn spawn_with_ref<T>(
        &self,
        builder: &TaskBuilder,
        task: T,
    ) -> JoinHandle<T::Output>
    where
        T: Future + Send,
        T::Output: Send,
    {
        let exe_scheduler = Arc::downgrade(&self.exe_mng_info);
        let raw_task =
            Task::create_raw_task(builder, exe_scheduler, task, VirtualTableType::Ylong);
        let handle = JoinHandle::new(raw_task);
        let task = Task(raw_task);
        self.exe_mng_info.schedule(task, false);
        handle
    }

    /// Waits 3 seconds for threads to finish before releasing the async pool.
    /// If threads could not finish before releasing, there could be possible memory leak.
    fn release_wait(&self) -> Result<(), ()> {
        self.exe_mng_info.cancel();
        let pair = self.inner.shutdown_handle.clone();
        let total = self.inner.total;
        let (lock, cvar) = &*pair;
        let finished = lock.lock().unwrap();
        let res = cvar
            .wait_timeout_while(finished, ASYNC_THREAD_QUIT_WAIT_TIME, |&mut finished| {
                finished < total
            })
            .unwrap();
        // if time limit has been reached, the unfinished threads would not get released
        if res.1.timed_out() {
            Err(())
        } else {
            Ok(())
        }
    }

    pub(crate) fn release(&self) {
        if let Ok(()) = self.release_wait() {
            let mut join_handle = self.exe_mng_info.handles.write().unwrap();
            #[allow(clippy::mem_replace_with_default)]
            let mut worker_handles = std::mem::replace(join_handle.as_mut(), vec![]);
            drop(join_handle);
            for parker in worker_handles.drain(..) {
                parker.release();
            }
        }
    }
}

#[cfg(all(test))]
mod test {
    use crate::builder::RuntimeBuilder;
    use crate::executor::async_pool::{get_cpu_core, AsyncPoolSpawner, MultiThreadScheduler};

    use crate::executor::parker::Parker;
    use crate::task::{Task, TaskBuilder, VirtualTableType};
    #[cfg(feature = "net")]
    use crate::net::Driver;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::atomic::Ordering::{Acquire, Release};
    use std::sync::mpsc::channel;
    use std::sync::{Arc, Mutex};
    use std::task::{Context, Poll};
    use std::thread::spawn;

    pub struct TestFuture {
        value: usize,
        total: usize,
    }

    pub fn create_new() -> TestFuture {
        TestFuture {
            value: 0,
            total: 1000,
        }
    }

    impl Future for TestFuture {
        type Output = usize;
        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            if self.total > self.value {
                self.get_mut().value += 1;
                cx.waker().wake_by_ref();
                Poll::Pending
            } else {
                Poll::Ready(self.total)
            }
        }
    }

    async fn test_future() -> usize {
        create_new().await
    }

    /*
     * @title  ExecutorMngInfo::new()
     * @brief  描述测试用例执行
     *         1、Creates a ExecutorMsgInfo with thread number 1
     *         2、Creates a ExecutorMsgInfo with thread number 2
     * @expect 对比创建完成后的参数，应当与所期待值相同
     * @auto   true
     */
    #[test]
    fn ut_executor_mng_info_new_001() {
        #[cfg(feature = "net")]
        let (arc_handle, _) = Driver::initialize();
        let executor_mng_info = MultiThreadScheduler::new(
            1,
            #[cfg(feature = "net")]
            arc_handle.clone()
        );
        assert!(!executor_mng_info.is_cancel.load(Acquire));
        assert_eq!(executor_mng_info.handles.read().unwrap().capacity(), 0);

        let executor_mng_info = MultiThreadScheduler::new(
            64,
            #[cfg(feature = "net")]
            arc_handle
        );
        assert!(!executor_mng_info.is_cancel.load(Acquire));
        assert_eq!(executor_mng_info.handles.read().unwrap().capacity(), 0);
    }

    /*
     * @title  ExecutorMngInfo::create_local_queues() UT test
     * @design No invalid value in the input, no exception branch, direct check function, return value
     * @precon Use ExecutorMngInfo::new(), get its creation object
     * @brief  Describe test case execution
     *         1、index set to 0, check the return value
     *         2、index set to ExecutorMngInfo.inner.total, check the return value
     * @expect Compare the parameters after creation, they should be the same as the expected value
     * @auto   true
     */
    #[test]
    fn ut_executor_mng_info_create_local_queues() {
        #[cfg(feature = "net")]
        let (arc_handle, _) = Driver::initialize();
        let executor_mng_info = MultiThreadScheduler::new(
            1,
            #[cfg(feature = "net")]
            arc_handle.clone()
        );
        let local_run_queue_info = executor_mng_info.create_local_queue(0);
        assert!(local_run_queue_info.is_empty());

        let executor_mng_info = MultiThreadScheduler::new(
            64,
            #[cfg(feature = "net")]
            arc_handle
        );
        let local_run_queue_info = executor_mng_info.create_local_queue(63);
        assert!(local_run_queue_info.is_empty());
    }

    /*
     * @title  ExecutorMngInfo::enqueue() UT test
     * @design No invalid value in the input, no exception branch, direct check function, return value
     * @precon Use ExecutorMngInfo::new(), get its creation object
     * @brief  Describe test case execution
     *         1、index set to 0, check the return value
     *         2、index set to ExecutorMngInfo.inner.total, check the return value
     * @expect Compare the parameters after creation, they should be the same as the expected value
     * @auto   true
     */
    #[test]
    fn ut_executor_mng_info_enqueue() {
        #[cfg(feature = "net")]
        let (arc_handle, _) = Driver::initialize();
        let executor_mng_info = MultiThreadScheduler::new(
            1,
            #[cfg(feature = "net")]
            arc_handle.clone()
        );

        let builder = TaskBuilder::new();
        let exe_scheduler = Arc::downgrade(
            &Arc::new(
                MultiThreadScheduler::new(
                    1,
                    #[cfg(feature = "net")]
                    arc_handle
                )
            )
        );
        let (task, _) = Task::create_task(
            &builder,
            exe_scheduler,
            test_future(),
            VirtualTableType::Ylong,
        );

        executor_mng_info.enqueue(task, true);
        assert!(!executor_mng_info.has_no_work());
    }

    /*
     * @title  ExecutorMngInfo::is_cancel() UT test
     * @design No invalid value in the input, no exception branch, direct check function, return value
     * @precon Use ExecutorMngInfo::new(), get its creation object
     * @brief  Describe test case execution
     *         1、The is_cancel value is set to true to check the return value
     *         2、The is_cancel value is set to false to check the return value
     * @expect Compare the parameters after creation, they should be the same as the expected value
     * @auto   true
     */
    #[test]
    fn ut_executor_mng_info_is_cancel() {
        #[cfg(feature = "net")]
        let (arc_handle, _) = Driver::initialize();
        let executor_mng_info = MultiThreadScheduler::new(
            1,
            #[cfg(feature = "net")]
            arc_handle
        );
        executor_mng_info.is_cancel.store(false, Release);
        assert!(!executor_mng_info.is_cancel());
        executor_mng_info.is_cancel.store(true, Release);
        assert!(executor_mng_info.is_cancel());
    }

    /*
     * @title  ExecutorMngInfo::set_cancel() UT test
     * @design No invalid value in the input, no exception branch, direct check function, return value
     * @precon Use ExecutorMngInfo::new(), get its creation object
     * @brief  Describe test case execution
     *         1、Check if the is_cancel parameter becomes true after set_cancel
     * @expect Compare the parameters after creation, they should be the same as the expected value
     * @auto   true
     */
    #[test]
    fn ut_executor_mng_info_set_cancel() {
        #[cfg(feature = "net")]
        let (arc_handle, _) = Driver::initialize();
        let executor_mng_info = MultiThreadScheduler::new(
            1,
            #[cfg(feature = "net")]
            arc_handle
        );
        assert!(!executor_mng_info.is_cancel.load(Acquire));
        executor_mng_info.set_cancel();
        assert!(executor_mng_info.is_cancel.load(Acquire));
    }

    /*
     * @title  ExecutorMngInfo::cancel() UT test
     * @design No invalid values in the input, the existence of exception branches, direct check function, return value
     * @precon Use ExecutorMngInfo::new(), get its creation object
     * @brief  Describe test case execution
     *         1、Check if the is_cancel parameter becomes true after set_cancel
     * @expect Compare the parameters after creation, they should be the same as the expected value
     * @auto   true
     */
    #[test]
    fn ut_executor_mng_info_cancel() {
        #[cfg(feature = "net")]
        let (arc_handle, arc_driver) = Driver::initialize();
        let executor_mng_info = MultiThreadScheduler::new(
            1,
            #[cfg(feature = "net")]
            arc_handle
        );

        let flag = Arc::new(Mutex::new(0));
        let (tx, rx) = channel();

        let (flag_clone, tx) = (flag.clone(), tx);

        let mut parker = Parker::new(
            #[cfg(feature = "net")]
            arc_driver
        );
        let parker_cpy = parker.clone();
        let _ = spawn(move || {
            parker.park();
            *flag_clone.lock().unwrap() = 1;
            tx.send(()).unwrap()
        });
        executor_mng_info.handles.write().unwrap().push(parker_cpy);

        executor_mng_info.cancel();
        rx.recv().unwrap();
        assert_eq!(*flag.lock().unwrap(), 1);
    }

    /*
     * @title  ExecutorMngInfo::wake_up_all() UT test
     * @design No invalid value in the input, no exception branch, direct check function, return value
     * @precon Use ExecutorMngInfo::new(), get its creation object
     * @brief  Describe test case execution
     *         1、Constructs an environment to check if all threads are woken up and executed via thread hooks
     * @expect Here, if the function is not correct, the thread will stay parked, if the function is correct, it will execute normally
     * @auto   true
     */
    #[test]
    fn ut_executor_mng_info_wake_up_all() {      
        #[cfg(feature = "net")]
        let (arc_handle, arc_driver) = Driver::initialize();
        let executor_mng_info = MultiThreadScheduler::new(
            1,
            #[cfg(feature = "net")]
            arc_handle
        );

        let flag = Arc::new(Mutex::new(0));
        let (tx, rx) = channel();

        let (flag_clone, tx) = (flag.clone(), tx);

        let mut parker = Parker::new(
            #[cfg(feature = "net")]
            arc_driver
        );
        let parker_cpy = parker.clone();

        let _ = spawn(move || {
            parker.park();
            *flag_clone.lock().unwrap() = 1;
            tx.send(()).unwrap()
        });

        executor_mng_info.handles.write().unwrap().push(parker_cpy);

        executor_mng_info.wake_up_all();
        rx.recv().unwrap();
        assert_eq!(*flag.lock().unwrap(), 1);
    }

    /*
     * @title  ExecutorMngInfo::wake_up_rand_one() UT test
     * @design No invalid values in the input, the existence of exception branches, direct check function, return value
     * @precon Use ExecutorMngInfo::new(), get its creation object
     * @brief  Describe test case execution
     *         1、Constructs an environment to check if a thread is woken up and executed by a thread hook
     * @expect Here, if the function is not correct, the thread will stay parked, if the function is correct, it will execute normally
     * @auto   true
     */
    #[test]
    fn ut_executor_mng_info_wake_up_rand_one() {
        #[cfg(feature = "net")]
        let (arc_handle, arc_driver) = Driver::initialize();
        let executor_mng_info = MultiThreadScheduler::new(
            1,
            #[cfg(feature = "net")]
            arc_handle
        );
        executor_mng_info.turn_to_sleep(0, false);

        let flag = Arc::new(Mutex::new(0));
        let (tx, rx) = channel();

        let (flag_clone, tx) = (flag.clone(), tx);

        let mut parker = Parker::new(
            #[cfg(feature = "net")]
            arc_driver
        );
        let parker_cpy = parker.clone();

        let _ = spawn(move || {
            parker.park();
            *flag_clone.lock().unwrap() = 1;
            tx.send(()).unwrap()
        });

        executor_mng_info.handles.write().unwrap().push(parker_cpy);

        executor_mng_info.wake_up_rand_one();
        rx.recv().unwrap();
        assert_eq!(*flag.lock().unwrap(), 1);
    }

    /*
     * @title  ExecutorMngInfo::wake_up_if_one_task_left() UT test
     * @design No invalid values in the input, the existence of exception branches, direct check function, return value
     * @precon Use ExecutorMngInfo::new(), get its creation object
     * @brief  Describe test case execution
     *         1、Constructs the environment, checks if there are still tasks, and if so, wakes up a thread to continue working
     * @expect Here, if the function is not correct, the thread will stay parked, if the function is correct, it will execute normally
     * @auto   true
     */
    #[test]
    fn ut_executor_mng_info_wake_up_if_one_task_left() {
        #[cfg(feature = "net")]
        let (arc_handle, arc_driver) = Driver::initialize();
        let executor_mng_info = MultiThreadScheduler::new(
            1,
            #[cfg(feature = "net")]
            arc_handle.clone()
        );

        executor_mng_info.turn_to_sleep(0, false);

        let flag = Arc::new(Mutex::new(0));
        let (tx, rx) = channel();

        let (flag_clone, tx) = (flag.clone(), tx);

        let mut parker = Parker::new(
            #[cfg(feature = "net")]
            arc_driver
        );
        let parker_cpy = parker.clone();

        let _ = spawn(move || {
            parker.park();
            *flag_clone.lock().unwrap() = 1;
            tx.send(()).unwrap()
        });

        executor_mng_info.handles.write().unwrap().push(parker_cpy);

        let builder = TaskBuilder::new();
        let exe_scheduler = Arc::downgrade(
            &Arc::new(
                MultiThreadScheduler::new(
                    1,
                    #[cfg(feature = "net")]
                    arc_handle
                )
            )
        );
        let (task, _) = Task::create_task(
            &builder,
            exe_scheduler,
            test_future(),
            VirtualTableType::Ylong,
        );

        executor_mng_info.enqueue(task, true);

        executor_mng_info.wake_up_if_one_task_left();
        rx.recv().unwrap();
        assert_eq!(*flag.lock().unwrap(), 1);
    }

    /*
     * @title  ExecutorMngInfo::from_woken_to_sleep() UT test
     * @design No invalid values in the input, the existence of exception branches, direct check function, return value
     * @precon Use ExecutorMngInfo::new(), get its creation object
     * @brief  Describe test case execution
     *         1、Construct the environment and set the state of the specified thread to park state. If the last thread is in park state, check whether there is a task, and if so, wake up this thread.
     * @expect Here, if the function is not correct, the thread will stay parked, if the function is correct, it will execute normally
     * @auto   true
     */
    #[test]
    fn ut_from_woken_to_sleep() {
        #[cfg(feature = "net")]
        let (arc_handle, arc_driver) = Driver::initialize();
        let executor_mng_info = MultiThreadScheduler::new(
            1,
            #[cfg(feature = "net")]
            arc_handle.clone()
        );

        let flag = Arc::new(Mutex::new(0));
        let (tx, rx) = channel();

        let (flag_clone, tx) = (flag.clone(), tx);

        let mut parker = Parker::new(
            #[cfg(feature = "net")]
            arc_driver
        );
        let parker_cpy = parker.clone();

        let _ = spawn(move || {
            parker.park();
            *flag_clone.lock().unwrap() = 1;
            tx.send(()).unwrap()
        });

        executor_mng_info.handles.write().unwrap().push(parker_cpy);

        let builder = TaskBuilder::new();
        let exe_scheduler = Arc::downgrade(
            &Arc::new(
                MultiThreadScheduler::new(
                    1,
                    #[cfg(feature = "net")]
                    arc_handle
                )
            )
        );
        let (task, _) = Task::create_task(
            &builder,
            exe_scheduler,
            test_future(),
            VirtualTableType::Ylong,
        );

        executor_mng_info.enqueue(task, true);
        executor_mng_info.turn_to_sleep(0, false);
        rx.recv().unwrap();
        assert_eq!(*flag.lock().unwrap(), 1);
    }

    /*
     * @title  AsyncPoolSpawner::new() UT test
     * @design No invalid values in the input, the existence of exception branches, direct check function, return value
     * @precon Use AsyncPoolSpawner::new(), get its creation object
     * @brief  Describe test case execution
     *         1、Verify the parameters of the initialization completion
     * @expect Here, if the function is not correct, the thread will stay parked, if the function is correct, it will execute normally
     * @auto   true
     */
    #[test]
    fn ut_async_pool_spawner_new() {
        let thread_pool_builder = RuntimeBuilder::new_multi_thread();
        let async_pool_spawner = AsyncPoolSpawner::new(&thread_pool_builder);
        assert_eq!(
            async_pool_spawner.inner.total,
            thread_pool_builder
                .core_thread_size
                .unwrap_or_else(get_cpu_core)
        );
        assert_eq!(
            async_pool_spawner.inner.worker_name,
            thread_pool_builder.common.worker_name
        );
        assert_eq!(
            async_pool_spawner.inner.stack_size,
            thread_pool_builder.common.stack_size
        );
        assert!(!async_pool_spawner.exe_mng_info.is_cancel.load(Acquire));
    }

    /// UT test for `create_async_thread_pool`.
    ///
    /// # Brief
    /// 1. Create an async_pool_spawner with `is_affinity` setting to false
    /// 2. Call create_async_thread_pool()
    /// 3. This UT should not panic
    #[test]
    fn ut_async_pool_spawner_create_async_thread_pool_001() {
        let thread_pool_builder = RuntimeBuilder::new_multi_thread();
        let _ = AsyncPoolSpawner::new(
            &thread_pool_builder.is_affinity(false)
        );
    }

    /// UT test for `UnboundedSender`.
    ///
    /// # Brief
    /// 1. Create an async_pool_spawner with `is_affinity` setting to true
    /// 2. Call create_async_thread_pool()
    /// 3. This UT should not panic
    #[test]
    fn ut_async_pool_spawner_create_async_thread_pool_002() {
        let thread_pool_builder = RuntimeBuilder::new_multi_thread();
        let _ = AsyncPoolSpawner::new(
            &thread_pool_builder.is_affinity(true),
        );
    }
}
