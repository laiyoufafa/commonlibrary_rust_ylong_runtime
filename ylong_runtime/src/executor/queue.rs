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


/// Schedule strategy implementation, includes FIFO LIFO priority and work-stealing
/// work-stealing strategy include stealing half of every worker or the largest amount of worker
use crate::task::Task;
use std::borrow::Borrow;
use std::cell::UnsafeCell;
use std::collections::linked_list::LinkedList;
use std::mem::MaybeUninit;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering::{AcqRel, Acquire, Relaxed, Release};
use std::sync::atomic::{AtomicU16, AtomicUsize};
use std::sync::{Arc, Mutex};
use std::{cmp, ptr};

unsafe fn non_atomic_load(data: &AtomicU16) -> u16 {
    ptr::read(data as *const AtomicU16 as *const u16)
}

/// Capacity of the local queue
pub(crate) const LOCAL_QUEUE_CAP: usize = 256;
const MASK: u16 = LOCAL_QUEUE_CAP as u16 - 1;

/// Local queue of the worker
pub(crate) struct LocalQueue {
    pub(crate) inner: Arc<InnerBuffer>,
}

unsafe impl Send for LocalQueue {}
unsafe impl Sync for LocalQueue {}

unsafe impl Send for InnerBuffer {}
unsafe impl Sync for InnerBuffer {}

impl LocalQueue {
    pub(crate) fn new() -> Self {
        LocalQueue {
            inner: Arc::new(InnerBuffer::new(LOCAL_QUEUE_CAP as u16)),
        }
    }
}

#[inline]
fn unwrap(num: u32) -> (u16, u16) {
    let head_pos = num & u16::MAX as u32;
    let steal_pos = num >> 16;
    (steal_pos as u16, head_pos as u16)
}

#[inline]
fn wrap(steal_pos: u16, head_pos: u16) -> u32 {
    (head_pos as u32) | ((steal_pos as u32) << 16)
}

impl LocalQueue {
    #[inline]
    pub(crate) fn pop_front(&self) -> Option<Task> {
        self.inner.pop_front()
    }

    #[inline]
    pub(crate) fn push_back(&self, task: Task, global: &GlobalQueue) {
        self.inner.push_back(task, global);
    }

    #[inline]
    pub(crate) fn steal_into(&self, dst: &LocalQueue) -> Option<Task> {
        self.inner.steal_into(dst)
    }

    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    #[inline]
    pub(crate) fn remaining(&self) -> u16 {
        self.inner.remaining()
    }
}

pub(crate) struct InnerBuffer {
    /// Front stores the position of both head and steal
    front: AtomicU32,
    rear: AtomicU16,
    cap: u16,
    buffer: Box<[UnsafeCell<MaybeUninit<Task>>]>,
}

impl InnerBuffer {
    fn new(cap: u16) -> Self {
        let mut buffer = Vec::with_capacity(cap as usize);

        for _ in 0..cap {
            buffer.push(UnsafeCell::new(MaybeUninit::uninit()));
        }
        InnerBuffer {
            front: AtomicU32::new(0),
            rear: AtomicU16::new(0),
            cap,
            buffer: buffer.into(),
        }
    }

    /// Checks whether the queue is empty
    fn is_empty(&self) -> bool {
        let (_, head) = unwrap(self.front.load(Acquire));
        let rear = self.rear.load(Acquire);
        head == rear
    }

    pub(crate) fn pop_front(&self) -> Option<Task> {
        let mut head = self.front.load(Acquire);

        let pos = loop {
            let (steal_pos, real_pos) = unwrap(head);

            // it's a spmc queue, so the queue could read its own tail non-atomically
            let tail_pos = unsafe { non_atomic_load(&self.rear) };

            // return none if the queue is empty
            if real_pos == tail_pos {
                return None;
            }

            let next_real = real_pos.wrapping_add(1);
            let next = if steal_pos == real_pos {
                wrap(next_real, next_real)
            } else {
                wrap(steal_pos, next_real)
            };

            let res = self
                .front
                .compare_exchange_weak(head, next, AcqRel, Acquire);
            match res {
                Ok(_) => break real_pos,
                Err(actual) => head = actual,
            }
        };

        let task = self.buffer[(pos & MASK) as usize].get();

        Some(unsafe { ptr::read(task).assume_init() })
    }

    pub(crate) fn remaining(&self) -> u16 {
        let front = self.front.load(Acquire);

        let (steal_pos, _real_pos) = unwrap(front);
        // it's a spmc queue, so the queue could read its own tail non-atomically
        let rear = unsafe { non_atomic_load(&self.rear) };

        self.cap - (rear.wrapping_sub(steal_pos))
    }

    pub(crate) fn push_back(&self, mut task: Task, global: &GlobalQueue) {
        loop {
            let front = self.front.load(Acquire);

            let (steal_pos, real_pos) = unwrap(front);
            // it's a spmc queue, so the queue could read its own tail non-atomically
            let rear = unsafe { non_atomic_load(&self.rear) };

            // if the local queue is full, push the task into the global queue
            if rear.wrapping_sub(steal_pos) < self.cap {
                let idx = (rear & MASK) as usize;
                let ptr = self.buffer[idx].get();
                unsafe {
                    ptr::write((*ptr).as_mut_ptr(), task);
                }
                self.rear.store(rear.wrapping_add(1), Release);
                return;
            } else {
                match self.push_overflowed(task, global, real_pos) {
                    Ok(_) => return,
                    Err(ret) => task = ret,
                }
            }
        }
    }

    #[allow(unused_assignments)]
    pub(crate) fn push_overflowed(
        &self,
        task: Task,
        global: &GlobalQueue,
        front: u16,
    ) -> Result<(), Task> {
        // get the number of tasks the worker has stolen
        let count = LOCAL_QUEUE_CAP / 2;
        let prev = wrap(front, front);
        let next = wrap(
            front.wrapping_add(count as u16),
            front.wrapping_add(count as u16),
        );

        match self.front.compare_exchange(prev, next, Release, Relaxed) {
            Ok(_) => {}
            Err(_) => return Err(task),
        }

        let (mut src_front_steal, _src_front_real) = unwrap(prev);

        let mut tmp_buf = Vec::with_capacity(count);
        for _ in 0..count {
            tmp_buf.push(UnsafeCell::new(MaybeUninit::uninit()));
        }

        for dst_ptr in tmp_buf.iter().take(count) {
            let src_idx = (src_front_steal & MASK) as usize;
            let task_ptr = self.buffer[src_idx].get();
            let task = unsafe { ptr::read(task_ptr).assume_init() };
            unsafe {
                ptr::write((*dst_ptr.get()).as_mut_ptr(), task);
            }
            src_front_steal = src_front_steal.wrapping_add(1);
        }

        global.push_batch(tmp_buf, task);
        Ok(())
    }

    pub(crate) fn steal_into(&self, dst: &LocalQueue) -> Option<Task> {
        // it's a spmc queue, so the queue could read its own tail non-atomically
        let mut dst_rear = unsafe { non_atomic_load(&dst.inner.rear) };
        let (des_steal_pos, _des_front_pos) = unwrap(dst.inner.front.load(Acquire));
        if dst_rear.wrapping_sub(des_steal_pos) > LOCAL_QUEUE_CAP as u16 / 2 {
            return None;
        }

        let mut src_next_front;
        let mut src_prev_front = self.front.load(Acquire);

        // get the number of tasks the worker has stolen
        let mut count = loop {
            let (src_front_steal, src_front_real) = unwrap(src_prev_front);

            // if these two values are not equal, it means another worker has stolen from this
            // queue, therefore abort this steal.
            if src_front_steal != src_front_real {
                return None;
            };

            let src_rear = self.rear.load(Acquire);

            // steal half of the tasks from the queue
            let mut n = src_rear.wrapping_sub(src_front_real);
            n = n - n / 2;
            if n == 0 {
                return None;
            }

            let src_steal_to = src_front_real.wrapping_add(n);
            src_next_front = wrap(src_front_steal, src_steal_to);

            let res =
                self.front
                    .compare_exchange_weak(src_prev_front, src_next_front, AcqRel, Acquire);
            match res {
                Ok(_) => break n,
                Err(actual) => src_prev_front = actual,
            }
        };

        // transfer the tasks
        let (mut src_front_steal, _src_front_real) = unwrap(src_next_front);
        count -= 1;
        for _ in 0..count {
            let src_idx = (src_front_steal & MASK) as usize;
            let des_idx = (dst_rear & MASK) as usize;

            let task_ptr = self.buffer[src_idx].get();

            let task = unsafe { ptr::read(task_ptr).assume_init() };
            let ptr = dst.inner.buffer[des_idx].get();
            unsafe {
                ptr::write((*ptr).as_mut_ptr(), task);
            }
            src_front_steal = src_front_steal.wrapping_add(1);
            dst_rear = dst_rear.wrapping_add(1);
        }

        let src_idx = (src_front_steal & MASK) as usize;

        let task_ptr = self.buffer[src_idx].get();
        let task = unsafe { ptr::read(task_ptr).assume_init() };
        if count != 0 {
            dst.inner.rear.store(dst_rear, Release);
        }

        src_prev_front = src_next_front;
        loop {
            let (_src_front_steal, src_front_real) = unwrap(src_prev_front);
            let src_next_front = wrap(src_front_real, src_front_real);
            let res = self
                .front
                .compare_exchange(src_prev_front, src_next_front, AcqRel, Acquire);

            match res {
                Ok(_) => {
                    break;
                }
                Err(actual) => {
                    let (actual_steal_pos, actual_real_pos) = unwrap(actual);
                    if actual_steal_pos == actual_real_pos {
                        panic!("steal_pos and real_pos should not be the same");
                    }
                    src_prev_front = actual;
                }
            }
        }
        Some(task)
    }
}

impl Drop for InnerBuffer {
    fn drop(&mut self) {
        while self.pop_front().is_some() {}
    }
}

pub(crate) struct GlobalQueue {
    len: AtomicUsize,
    globals: Mutex<LinkedList<Task>>,
}

impl GlobalQueue {
    pub(crate) fn new() -> Self {
        GlobalQueue {
            len: AtomicUsize::new(0_usize),
            globals: Mutex::new(LinkedList::new()),
        }
    }
    pub(super) fn is_empty(&self) -> bool {
        self.len.load(Acquire) == 0
    }

    pub(super) fn push_batch(&self, tasks: Vec<UnsafeCell<MaybeUninit<Task>>>, task: Task) {
        let mut list = self.globals.lock().unwrap();
        let len = tasks.len() + 1;
        for task_ptr in tasks {
            let task = unsafe { ptr::read(task_ptr.get()).assume_init() };
            list.push_back(task);
        }
        list.push_back(task);
        self.len.fetch_add(len, AcqRel);
    }

    pub(super) fn pop_batch(
        &self,
        worker_num: usize,
        queue: &LocalQueue,
        limit: usize,
    ) -> Option<Task> {
        let len = self.len.load(Acquire);
        let num = cmp::min(len / worker_num, limit);

        let inner_buf = &queue.inner;
        // it's a spmc queue, so the queue could read its own tail non-atomically
        let rear = unsafe { non_atomic_load(&inner_buf.rear) };
        let mut curr = rear;

        let mut list = self.globals.lock().unwrap();
        let first_task = list.pop_front()?;

        let mut count = 1;

        for _ in 1..num {
            if let Some(task) = list.pop_front() {
                let idx = (curr & MASK) as usize;
                let ptr = inner_buf.buffer[idx].get();
                unsafe {
                    ptr::write((*ptr).as_mut_ptr(), task);
                }
                curr = curr.wrapping_add(1);
                count += 1;
            } else {
                break;
            }
        }
        drop(list);
        self.len.fetch_sub(count, AcqRel);
        inner_buf.rear.store(curr, Release);
        Some(first_task)
    }

    pub(super) fn pop_front(&self) -> Option<Task> {
        if self.is_empty() {
            return None;
        }
        let mut list = self.globals.lock().unwrap();
        let task = list.pop_front();
        drop(list);
        if task.is_some() {
            self.len.fetch_sub(1, AcqRel);
        }
        task
    }

    pub(super) fn push_back(&self, task: Task) {
        let mut list = self.globals.lock().unwrap();
        list.push_back(task);
        drop(list);
        self.len.fetch_add(1, AcqRel);
    }

    pub(super) fn get_global(&self) -> &Mutex<LinkedList<Task>> {
        self.globals.borrow()
    }
}

#[cfg(feature = "multi_instance_runtime")]
#[cfg(all(test))]
mod test {
    use crate::executor::async_pool::MultiThreadScheduler;
    use crate::executor::queue::{unwrap, GlobalQueue, InnerBuffer, LocalQueue, LOCAL_QUEUE_CAP};
    use crate::task::{Task, TaskBuilder, VirtualTableType};
    #[cfg(feature = "net")]
    use crate::net::Driver;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::atomic::Ordering::Acquire;
    use std::sync::Arc;
    use std::task::{Context, Poll};
    use std::thread::park;

    impl InnerBuffer {
        fn len(&self) -> u16 {
            let front = self.front.load(Acquire);
            let (_, real_pos) = unwrap(front);

            let rear = self.rear.load(Acquire);
            rear.wrapping_sub(real_pos)
        }
    }

    impl LocalQueue {
        pub fn len(&self) -> u16 {
            self.inner.len()
        }
    }

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
                //unsafe {
                //Pin::get_unchecked_mut(self).value += 1;

                //}
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

    #[test]
    fn ut_inner_buffer() {
        ut_inner_buffer_new();
        ut_inner_buffer_len();
        ut_inner_buffer_is_empty();
        ut_inner_buffer_push_back();
        ut_inner_buffer_pop_front();
        ut_inner_buffer_steal_into();
    }

    /*
     * @title  InnerBuffer::new() UT test
     * @design The function has no invalid values in the input, no exception branch, direct check function, return value
     * @precon After calling InnerBuffer::new() function, get its created object
     * @brief  Describe test case execution
     *         1、Checking the parameters after initialization is completed
     * @expect The function entry has no invalid value, no exception branch, and the property value should be related to the entry after the initialization is completed
     * @auto   true
     */
    fn ut_inner_buffer_new() {
        let inner_buffer = InnerBuffer::new(LOCAL_QUEUE_CAP as u16);
        assert_eq!(inner_buffer.cap, LOCAL_QUEUE_CAP as u16);
        assert_eq!(inner_buffer.buffer.len(), LOCAL_QUEUE_CAP);
    }

    /*
     * @title  InnerBuffer::is_empty() UT test
     * @design The function has no invalid values in the input, no exception branch, direct check function, return value
     * @precon After calling InnerBuffer::new() function, get its created object
     * @brief  Describe test case execution
     *         1、Checking the parameters after initialization is completed
     *         2、After entering a task into the queue space, determine again whether it is empty or not, and it should be non-empty
     * @expect The function entry has no invalid value, no exception branch, and the property value should be related to the entry after the initialization is completed
     * @auto   true
     */
    fn ut_inner_buffer_is_empty() {
        let inner_buffer = InnerBuffer::new(LOCAL_QUEUE_CAP as u16);
        assert!(inner_buffer.is_empty());

        let builder = TaskBuilder::new();

        #[cfg(feature = "net")]
        let (arc_handle, _) = Driver::initialize();

        let exe_scheduler = Arc::downgrade(
            &Arc::new(
                MultiThreadScheduler::new(
                    1,
                    #[cfg(feature = "net")]
                    arc_handle
                )
            )
        );
        let (task, _) = Task::create_task(&builder, exe_scheduler, test_future(), VirtualTableType::Ylong);
        let global_queue = GlobalQueue::new();
        let inner_buffer = InnerBuffer::new(LOCAL_QUEUE_CAP as u16);
        inner_buffer.push_back(task, &global_queue);
        assert!(!inner_buffer.is_empty());
    }

    /*
     * @title  InnerBuffer::len() UT test
     * @design The function entry has no invalid value, there is an exception branch, direct check function, return value
     * @precon After calling InnerBuffer::new() function, get its created object
     * @brief  Describe test case execution
     *         1、Checking the parameters after initialization is completed
     *         2、Insert tasks up to their capacity into the local queue, checking the local queue length
     *         3、Insert tasks into the local queue that exceed its capacity, checking the local queue length as well as the global queue length
     * @expect The function entry has no invalid value, no exception branch, and the property value should be related to the entry after the initialization is completed
     * @auto   true
     */
    fn ut_inner_buffer_len() {
        let inner_buffer = InnerBuffer::new(LOCAL_QUEUE_CAP as u16);
        assert_eq!(inner_buffer.len(), 0);

        let inner_buffer = InnerBuffer::new(LOCAL_QUEUE_CAP as u16);
        let global_queue = GlobalQueue::new();
        let builder = TaskBuilder::new();

        #[cfg(feature = "net")]
        let (arc_handle, _) = Driver::initialize();

        let exe_scheduler = Arc::downgrade(
            &Arc::new(
                MultiThreadScheduler::new(
                    1,
                    #[cfg(feature = "net")]
                    arc_handle.clone()
                )
            )
        );
        let (task, _) = Task::create_task(&builder, exe_scheduler, test_future(), VirtualTableType::Ylong);
        inner_buffer.push_back(task, &global_queue);
        assert_eq!(inner_buffer.len(), 1);

        let inner_buffer = InnerBuffer::new(LOCAL_QUEUE_CAP as u16);
        let global_queue = GlobalQueue::new();
        for _ in 0..LOCAL_QUEUE_CAP + 1 {
            let exe_scheduler = Arc::downgrade(
                &Arc::new(
                    MultiThreadScheduler::new(
                        1,
                        #[cfg(feature = "net")]
                        arc_handle.clone()
                    )
                )
            );
            let (task, _) = Task::create_task(&builder, exe_scheduler, test_future(), VirtualTableType::Ylong);
            inner_buffer.push_back(task, &global_queue);
        }
        assert_eq!(
            inner_buffer.len() as usize,
            LOCAL_QUEUE_CAP - LOCAL_QUEUE_CAP / 2
        );
        assert_eq!(global_queue.len.load(Acquire), 1 + LOCAL_QUEUE_CAP / 2);
    }

    /*
     * @title  InnerBuffer::push_back() UT test
     * @design The function entry has no invalid value, there is an exception branch, direct check function, return value
     * @precon After calling InnerBuffer::new() function, get its created object
     * @brief  Describe test case execution
     *         1、Insert tasks up to capacity into the local queue, verifying that they are functionally correct
     *         2、Insert tasks that exceed the capacity into the local queue and verify that they are functionally correct
     * @expect The function entry has no invalid value, there is an exception branch, after the initialization is completed the property value should be related to the entry
     * @auto   true
     */
    fn ut_inner_buffer_push_back() {
        // 1、Insert tasks up to capacity into the local queue, verifying that they are functionally correct
        let local_queue = LocalQueue::new();
        let global_queue = GlobalQueue::new();

        #[cfg(feature = "net")]
        let (arc_handle, _) = Driver::initialize(); 

        let builder = TaskBuilder::new();
        for _ in 0..LOCAL_QUEUE_CAP / 2 {
            let exe_scheduler = Arc::downgrade(
                &Arc::new(
                    MultiThreadScheduler::new(
                        2,
                        #[cfg(feature = "net")]                
                        arc_handle.clone()
                    )
                )
            );
            let (task, _) = Task::create_task(&builder, exe_scheduler, test_future(), VirtualTableType::Ylong);
            local_queue.push_back(task, &global_queue);
        }

        for _ in 0..LOCAL_QUEUE_CAP / 2 {
            let exe_scheduler = Arc::downgrade(
                &Arc::new(
                    MultiThreadScheduler::new(
                        2,
                        #[cfg(feature = "net")]                
                        arc_handle.clone()
                    )
                )
            );
            let (task, _) = Task::create_task(&builder, exe_scheduler, test_future(), VirtualTableType::Ylong);
            local_queue.push_back(task, &global_queue);
        }

        assert_eq!(local_queue.len(), 256);

        // 2、Insert tasks that exceed the capacity into the local queue and verify that they are functionally correct
        let local_queue = LocalQueue::new();
        let global_queue = GlobalQueue::new();

        #[cfg(feature = "net")]
        let (arc_handle, _) = Driver::initialize(); 

        for _ in 0..LOCAL_QUEUE_CAP / 2 + 1 {
            let exe_scheduler = Arc::downgrade(
                &Arc::new(
                    MultiThreadScheduler::new(
                        2,
                        #[cfg(feature = "net")]                
                        arc_handle.clone()
                    )
                )
            );
            let (task, _) = Task::create_task(&builder, exe_scheduler, test_future(), VirtualTableType::Ylong);
            local_queue.push_back(task, &global_queue);
        }

        for _ in 0..LOCAL_QUEUE_CAP / 2 {
            let exe_scheduler = Arc::downgrade(
                &Arc::new(
                    MultiThreadScheduler::new(
                        2,
                        #[cfg(feature = "net")]                
                        arc_handle.clone()
                    )
                )
            );
            let (task, _) = Task::create_task(&builder, exe_scheduler, test_future(), VirtualTableType::Ylong);
            local_queue.push_back(task, &global_queue);
        }

        assert_eq!(
            local_queue.len() as usize,
            LOCAL_QUEUE_CAP - LOCAL_QUEUE_CAP / 2
        );
        assert_eq!(global_queue.len.load(Acquire), 1 + LOCAL_QUEUE_CAP / 2);
    }

    /*
     * @title  InnerBuffer::pop_front() UT test
     * @design The function entry has no invalid value, there is an exception branch, direct check function, return value
     * @precon After calling InnerBuffer::new() function, get its created object
     * @brief  Describe test case execution
     *         1、Multi-threaded take out task operation with empty local queue, check if the function is correct
     *         2、If the local queue is not empty, multi-threaded take out operations up to the number of existing tasks and check if the function is correct
     *         3、If the local queue is not empty, the multi-threaded operation to take out more than the number of existing tasks, check whether the function is correct
     * @expect The function entry has no invalid value, and the property value should be related to the entry after the initialization is completed
     * @auto   true
     */
    fn ut_inner_buffer_pop_front() {
        // 1、Multi-threaded take out task operation with empty local queue, check if the function is correct
        let local_queue = LocalQueue::new();
        let global_queue = GlobalQueue::new();
        assert!(local_queue.pop_front().is_none());

        // 2、If the local queue is not empty, multi-threaded take out operations up to the number of existing tasks and check if the function is correct
        let local_queue = Arc::new(LocalQueue::new());
        let builder = TaskBuilder::new();

        #[cfg(feature = "net")]
        let (arc_handle, _) = Driver::initialize(); 

        for _ in 0..LOCAL_QUEUE_CAP {
            let exe_scheduler = Arc::downgrade(
                &Arc::new(
                    MultiThreadScheduler::new(
                        2,
                        #[cfg(feature = "net")]                
                        arc_handle.clone()
                    )
                )
            );
            let (task, _) = Task::create_task(&builder, exe_scheduler, test_future(), VirtualTableType::Ylong);
            local_queue.push_back(task, &global_queue);
        }
        assert_eq!(local_queue.len(), LOCAL_QUEUE_CAP as u16);

        let local_queue_clone_one = local_queue.clone();
        let local_queue_clone_two = local_queue.clone();

        let thread_one = std::thread::spawn(move || {
            for _ in 0..LOCAL_QUEUE_CAP / 2 {
                local_queue_clone_one.pop_front();
            }
        });

        let thread_two = std::thread::spawn(move || {
            for _ in 0..LOCAL_QUEUE_CAP / 2 {
                local_queue_clone_two.pop_front();
            }
        });

        thread_one.join().expect("failed");
        thread_two.join().expect("failed");
        assert!(local_queue.is_empty());

        // 3、If the local queue is not empty, the multi-threaded operation to take out more than the number of existing tasks, check whether the function is correct
        let local_queue = Arc::new(LocalQueue::new());

        #[cfg(feature = "net")]
        let (arc_handle, _) = Driver::initialize(); 

        for _ in 0..LOCAL_QUEUE_CAP {
            let exe_scheduler = Arc::downgrade(
                &Arc::new(
                    MultiThreadScheduler::new(
                        2,
                        #[cfg(feature = "net")]                
                        arc_handle.clone()
                    )
                )
            );
            let (task, _) = Task::create_task(&builder, exe_scheduler, test_future(), VirtualTableType::Ylong);
            local_queue.push_back(task, &global_queue);
        }
        assert_eq!(local_queue.len(), LOCAL_QUEUE_CAP as u16);

        let local_queue_clone_one = local_queue.clone();
        let local_queue_clone_two = local_queue.clone();

        let thread_one = std::thread::spawn(move || {
            for _ in 0..LOCAL_QUEUE_CAP {
                local_queue_clone_one.pop_front();
            }
        });

        let thread_two = std::thread::spawn(move || {
            for _ in 0..LOCAL_QUEUE_CAP {
                local_queue_clone_two.pop_front();
            }
        });

        thread_one.join().expect("failed");
        thread_two.join().expect("failed");
        assert!(local_queue.is_empty());
    }

    /*
     * @title  InnerBuffer::steal_into() UT test
     * @design The function entry has no invalid value, there is an exception branch, direct check function, return value
     * @precon After calling InnerBuffer::new() function, get its created object
     * @brief  Describe test case execution
     *         1、In the single-threaded case, the local queue has more than half the number of tasks, steal from other local queues, the number of steals is 0, check whether the function is completed
     *         2、In the single-threaded case, the number of tasks already in the local queue is not more than half, steal from other local queues, the number of steals is 0, check whether the function is completed
     *         3、In the single-threaded case, the number of tasks already in the local queue is not more than half, steal from other local queues, the number of steals is not 0, check whether the function is completed
     *         4、Multi-threaded case, other queues are doing take out operations, but steal from this queue to see if the function is completed
     *         5、In the multi-threaded case, other queues are being stolen by non-local queues, steal from that stolen queue and see if the function is completed
     * @expect The function entry has no invalid value, and the property value should be related to the entry after the initialization is completed
     * @auto   true
     */
    fn ut_inner_buffer_steal_into() {
        // 1、In the single-threaded case, the local queue has more than half the number of tasks, steal from other local queues, the number of steals is 0, check whether the function is completed
        let local_queue = LocalQueue::new();
        let other_local_queue = LocalQueue::new();
        let global_queue = GlobalQueue::new();

        let builder = TaskBuilder::new();

        #[cfg(feature = "net")]
        let (arc_handle, _) = Driver::initialize(); 
        for _ in 0..LOCAL_QUEUE_CAP {
            let exe_scheduler = Arc::downgrade(
                &Arc::new(
                    MultiThreadScheduler::new(
                        1,
                        #[cfg(feature = "net")]
                        arc_handle.clone(),
                    )
                )
            );
            let (task, _) = Task::create_task(&builder, exe_scheduler, test_future(), VirtualTableType::Ylong);
            local_queue.push_back(task, &global_queue);
        }

        #[cfg(feature = "net")]
        let (arc_handle, _) = Driver::initialize(); 
        for _ in 0..LOCAL_QUEUE_CAP {
            let exe_scheduler = Arc::downgrade(
                &Arc::new(
                    MultiThreadScheduler::new(
                        1,
                        #[cfg(feature = "net")]
                        arc_handle.clone(),
                    )
                )
            );
            let (task, _) = Task::create_task(&builder, exe_scheduler, test_future(), VirtualTableType::Ylong);
            other_local_queue.push_back(task, &global_queue);
        }

        assert!(other_local_queue.steal_into(&local_queue).is_none());

        // 2、In the single-threaded case, the number of tasks already in the local queue is not more than half, steal from other local queues, the number of steals is 0, check whether the function is completed
        let local_queue = LocalQueue::new();
        let other_local_queue = LocalQueue::new();

        assert!(other_local_queue.steal_into(&local_queue).is_none());

        // 3、In the single-threaded case, the number of tasks already in the local queue is not more than half, steal from other local queues, the number of steals is not 0, check whether the function is completed
        let local_queue = LocalQueue::new();
        let other_local_queue = LocalQueue::new();
        let global_queue = GlobalQueue::new();

        #[cfg(feature = "net")]
        let (arc_handle, _) = Driver::initialize(); 
        for _ in 0..LOCAL_QUEUE_CAP {
            let exe_scheduler = Arc::downgrade(
                &Arc::new(
                    MultiThreadScheduler::new(
                        1,
                        #[cfg(feature = "net")]
                        arc_handle.clone(),
                    )
                )
            );
            let (task, _) = Task::create_task(&builder, exe_scheduler, test_future(), VirtualTableType::Ylong);
            other_local_queue.push_back(task, &global_queue);
        }

        assert!(other_local_queue.steal_into(&local_queue).is_some());
        assert_eq!(other_local_queue.len(), (LOCAL_QUEUE_CAP / 2) as u16);
        assert_eq!(local_queue.len(), (LOCAL_QUEUE_CAP / 2 - 1) as u16);

        // 4、Multi-threaded case, other queues are doing take out operations, but steal from this queue to see if the function is completed
        let local_queue = Arc::new(LocalQueue::new());
        let local_queue_clone = local_queue.clone();

        let other_local_queue = Arc::new(LocalQueue::new());
        let other_local_queue_clone_one = other_local_queue.clone();
        let other_local_queue_clone_two = other_local_queue.clone();

        let global_queue = GlobalQueue::new();

        #[cfg(feature = "net")]
        let (arc_handle, _) = Driver::initialize(); 
        for _ in 0..LOCAL_QUEUE_CAP {
            let exe_scheduler = Arc::downgrade(
                &Arc::new(
                    MultiThreadScheduler::new(
                        1,
                        #[cfg(feature = "net")]
                        arc_handle.clone(),
                    )
                )
            );
            let (task, _) = Task::create_task(&builder, exe_scheduler, test_future(), VirtualTableType::Ylong);
            other_local_queue.push_back(task, &global_queue);
        }

        let thread_one = std::thread::spawn(move || {
            for _ in 0..LOCAL_QUEUE_CAP / 2 {
                other_local_queue_clone_one.pop_front();
            }
        });

        let thread_two = std::thread::spawn(move || {
            other_local_queue_clone_two.steal_into(&local_queue_clone);
        });

        thread_one.join().expect("failed");
        thread_two.join().expect("failed");

        assert_eq!(
            other_local_queue.len() + local_queue.len() + 1,
            (LOCAL_QUEUE_CAP / 2) as u16
        );

        // 5、In the multi-threaded case, other queues are being stolen by non-local queues, steal from that stolen queue and see if the function is completed
        let local_queue_one = Arc::new(LocalQueue::new());
        let local_queue_one_clone = local_queue_one.clone();

        let local_queue_two = Arc::new(LocalQueue::new());
        let local_queue_two_clone = local_queue_two.clone();

        let other_local_queue = Arc::new(LocalQueue::new());
        let other_local_queue_clone_one = other_local_queue.clone();
        let other_local_queue_clone_two = other_local_queue.clone();

        #[cfg(feature = "net")]
        let (arc_handle, _) = Driver::initialize(); 
        for _ in 0..LOCAL_QUEUE_CAP {
            let exe_scheduler = Arc::downgrade(
                &Arc::new(
                    MultiThreadScheduler::new(
                        1,
                        #[cfg(feature = "net")]
                        arc_handle.clone(),
                    )
                )
            );
            let (task, _) = Task::create_task(&builder, exe_scheduler, test_future(), VirtualTableType::Ylong);
            other_local_queue.push_back(task, &global_queue);
        }

        let thread_one = std::thread::spawn(move || {
            park();
            other_local_queue_clone_one.steal_into(&local_queue_one_clone);
        });

        let thread_two = std::thread::spawn(move || {
            other_local_queue_clone_two.steal_into(&local_queue_two_clone);
        });

        thread_two.join().expect("failed");
        thread_one.thread().unpark();
        thread_one.join().expect("failed");

        assert_eq!(local_queue_two.len(), (LOCAL_QUEUE_CAP / 2 - 1) as u16);
        assert_eq!(local_queue_one.len(), (LOCAL_QUEUE_CAP / 4 - 1) as u16);
    }
}
