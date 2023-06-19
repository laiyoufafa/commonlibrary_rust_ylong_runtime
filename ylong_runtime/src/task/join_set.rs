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

use crate::spawn::spawn_async;
use crate::task::join_handle::CancelHandle;
use crate::task::PriorityLevel;
use crate::{JoinHandle, ScheduleError, TaskBuilder};
use std::cell::{RefCell, UnsafeCell};
use std::collections::{HashSet, LinkedList};
use std::future::Future;
use std::hash::{Hash, Hasher};
use std::mem;
use std::mem::ManuallyDrop;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

/// A collection of tasks get spawned on a Ylong runtime
///
/// A `JoinSet` will take over the `JoinHandle`s of the tasks when spawning, and it can
/// asynchronously wait for the completion of some or all of the tasks inside the set.
/// However, `JoinSet` is unordered, which means that tasks' results will be returned
/// in the order of their completion.
///
/// All the tasks spawned via a `JoinSet` must have the same return type.
///
/// # Example
///
/// ```
/// use ylong_runtime::task::JoinSet;
///
/// async fn join_set_spawn() {
///     let mut set = JoinSet::new();
///     set.spawn(async move {
///         0
///     });
///     let ret = set.join_next().await.unwrap().unwrap();
///     assert_eq!(ret, 0)
/// }
/// ```
#[derive(Default)]
pub struct JoinSet<R> {
    list: Arc<Mutex<JoinList<R>>>,
    builder: TaskBuilder,
}

unsafe impl<R: Send> Send for JoinSet<R> {}

unsafe impl<R: Send> Sync for JoinSet<R> {}

pub(crate) struct JoinList<R> {
    // Contains tasks not ready for polling
    wait_list: HashSet<Arc<JoinEntry<R>>>,
    // Contains tasks ready for polling
    done_list: LinkedList<Arc<JoinEntry<R>>>,
    // Waker of JoinSet, a ready task will wake the JoinSet it belongs to
    waker: Option<Waker>,
    len: usize,
}

impl<R> Default for JoinList<R> {
    fn default() -> Self {
        JoinList {
            wait_list: HashSet::default(),
            done_list: LinkedList::default(),
            waker: None,
            len: 0,
        }
    }
}

pub(crate) struct JoinEntry<R> {
    // The JoinHandle of the task
    handle: UnsafeCell<ManuallyDrop<JoinHandle<R>>>,
    // The JoinList this task belongs to
    list: Arc<Mutex<JoinList<R>>>,
    // A flag to indicate which list this task is in.
    // `true` means the entry is in the done list.
    in_done: RefCell<bool>,
}

impl<R> JoinSet<R> {
    /// Creates a new JoinSet.
    pub fn new() -> Self {
        Self {
            list: Default::default(),
            builder: Default::default(),
        }
    }
}

impl<R> PartialEq<Self> for JoinEntry<R> {
    fn eq(&self, other: &Self) -> bool {
        unsafe { (*(self.handle.get())).raw.eq(&(*(other.handle.get())).raw) }
    }
}

impl<R> Eq for JoinEntry<R> {}

impl<R> Hash for JoinEntry<R> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        unsafe { (*self.handle.get()).raw.hash(state) }
    }
}

impl<R> JoinEntry<R> {
    // When waking a JoinEntry, the entry will get popped out of the wait list and pushed into
    // the ready list. The corresponding in_done flag will also be changed.
    // Safety: it will take the list's lock before moving the entry, so it's concurrently safe.
    fn wake_by_ref(entry: &Arc<JoinEntry<R>>) {
        let mut list = entry.list.lock().unwrap();
        if !entry.in_done.replace(true) {
            // We couldn't find the entry, meaning that the JoinSet has been dropped already.
            // In this case, there is no need to push the entry back to the done list.
            if !list.wait_list.remove(entry) {
                return;
            }
            list.done_list.push_back(entry.clone());
            // Wake the JoinSet if an waker is set
            if let Some(waker) = list.waker.take() {
                drop(list);
                waker.wake();
            }
        }
    }
}

impl<R> JoinSet<R> {
    /// Spawns a task via a `JoinSet` onto a Ylong runtime. The task will start immediately
    /// when `spawn` is called.
    ///
    /// # Panics
    /// This method panics when calling outside of Ylong runtime.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::task::JoinSet;
    /// ylong_runtime::block_on(async move {
    ///     let mut set = JoinSet::new();
    ///     let cancel_handle = set.spawn(async move {1});
    ///     cancel_handle.cancel();
    /// });
    /// ```
    pub fn spawn<T>(&mut self, task: T) -> CancelHandle
    where
        T: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        self.spawn_inner(task, None)
    }

    fn spawn_inner<T>(&mut self, task: T, builder: Option<&TaskBuilder>) -> CancelHandle
    where
        T: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        let handle = match builder {
            None => spawn_async(&self.builder, task),
            Some(builder) => builder.spawn(task),
        };
        let cancel = handle.get_cancel_handle();
        let entry = Arc::new(JoinEntry {
            handle: UnsafeCell::new(ManuallyDrop::new(handle)),
            list: self.list.clone(),
            in_done: RefCell::new(false),
        });
        let mut list = self.list.lock().unwrap();
        list.len += 1;
        list.wait_list.insert(entry.clone());
        drop(list);
        let waker = entry_into_waker(&entry);
        unsafe {
            (*entry.handle.get()).set_waker(&waker);
        }
        cancel
    }

    /// Waits until one task inside the `JoinSet` completes and returns its output.
    ///
    /// Returns `None` if there is no task inside the set.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::task::JoinSet;
    /// ylong_runtime::block_on(async move {
    ///     let mut set = JoinSet::new();
    ///     set.spawn(async move {1});
    ///     let ret = set.join_next().await.unwrap().unwrap();
    ///     assert_eq!(ret, 1);
    ///     // no more task, so this `join_next` will return none
    ///     let ret = set.join_next().await;
    ///     assert!(ret.is_none());
    /// });
    /// ```
    pub async fn join_next(&mut self) -> Option<Result<R, ScheduleError>> {
        use crate::futures::poll_fn;
        poll_fn(|cx| self.poll_join_next(cx)).await
    }

    /// Waits for all tasks inside the set to finish.
    pub async fn join_all(&mut self) -> Result<(), ScheduleError> {
        // todo: take the lock only once
        let count = self.list.lock().unwrap().len;
        for _ in 0..count {
            match self.join_next().await {
                None => return Ok(()),
                Some(Ok(_)) => {}
                Some(Err(e)) => return Err(e),
            }
        }
        Ok(())
    }

    /// Cancels every tasks inside the JoinSet.
    ///
    /// If [`JoinSet::join_next`] is called after calling `cancel_all`, then it would return
    /// `TaskCanceled` error.
    ///
    /// # Examples
    /// ```
    /// use ylong_runtime::task::JoinSet;
    /// ylong_runtime::block_on(async move {
    ///     let mut set = JoinSet::new();
    ///     set.spawn(async move {1});
    ///     set.cancel_all();
    /// });
    /// ```
    pub fn cancel_all(&mut self) {
        let list = self.list.lock().unwrap();
        for item in &list.done_list {
            unsafe { (*item.handle.get()).cancel() }
        }
        for item in &list.wait_list {
            unsafe { (*item.handle.get()).cancel() }
        }
    }

    /// Cancels every tasks inside and clears all entries inside
    ///
    /// # Examples
    /// ```
    /// use ylong_runtime::task::JoinSet;
    /// ylong_runtime::block_on(async move {
    ///     let mut set = JoinSet::new();
    ///     set.spawn(async move {1});
    ///     set.shutdown();
    /// });
    /// ```
    pub async fn shutdown(&mut self) {
        self.cancel_all();
        while self.join_next().await.is_some() {}
    }

    /// Creates a builder that configures task attributes. This builder could spawn tasks
    /// with its attributes onto the JoinSet.
    ///
    /// # Examples
    /// ```
    /// use ylong_runtime::task::JoinSet;
    /// ylong_runtime::block_on(async move {
    ///     let mut set = JoinSet::new();
    ///     let mut builder = set.build_task().name("hello".into());
    ///     builder.spawn(async move {1});
    /// });
    /// ```
    pub fn build_task(&mut self) -> Builder<'_, R> {
        Builder::new(self)
    }

    fn poll_join_next(&mut self, cx: &mut Context<'_>) -> Poll<Option<Result<R, ScheduleError>>> {
        let mut list = self.list.lock().unwrap();

        // quick path: check if the set is empty, return none if true
        if list.len == 0 {
            return Poll::Ready(None);
        }

        // set the joinset's waker if it's not set
        let is_same_waker = match list.waker.as_ref() {
            None => false,
            Some(waker) => cx.waker().will_wake(waker),
        };

        if !is_same_waker {
            list.waker = Some(cx.waker().clone());
        }

        // pop a ready task from the done list and poll it
        if let Some(entry) = list.done_list.pop_front() {
            drop(list);
            let waker = entry_into_waker(&entry);
            let mut ctx = Context::from_waker(&waker);
            // We have to dereference the JoinHandle from the UnsafeCell in order to poll it.
            // The lifetime of the handle is valid here since it's wrapped by a ManuallyDrop.
            // It will only get dropped when the task returns ready, and by the time, the entry
            // is also dropped, and could never be popped from the done list once again.
            unsafe {
                match Pin::new(&mut **(entry.handle.get())).poll(&mut ctx) {
                    Poll::Ready(res) => {
                        let mut list = self.list.lock().unwrap();
                        list.len -= 1;
                        drop(list);
                        // drop the JoinHandle and return it's result
                        drop(ManuallyDrop::take(&mut *entry.handle.get()));
                        Poll::Ready(Some(res))
                    }
                    Poll::Pending => {
                        let mut list = self.list.lock().unwrap();
                        // The future hasn't finished, push it back to wait-list
                        let _ = entry.in_done.replace(false);
                        list.wait_list.insert(entry);
                        drop(list);
                        cx.waker().wake_by_ref();
                        Poll::Pending
                    }
                }
            }
        } else {
            // there is no task, return none
            if list.len == 0 {
                Poll::Ready(None)
            } else {
                // no ready task, return pending
                Poll::Pending
            }
        }
    }
}

/// A TaskBuilder for tasks that get spawned on a specific JoinSet
pub struct Builder<'a, R> {
    builder: TaskBuilder,
    set: &'a mut JoinSet<R>,
}

impl<'a, R> Builder<'a, R> {
    pub(crate) fn new(set: &'a mut JoinSet<R>) -> Builder<'a, R> {
        Builder {
            builder: TaskBuilder::new(),
            set,
        }
    }

    /// Sets the name for the tasks that are going to get spawned by this JoinSet Builder
    ///
    /// # Examples
    /// ```
    /// use ylong_runtime::task::JoinSet;
    /// ylong_runtime::block_on(async move {
    ///     let mut set = JoinSet::new();
    ///     let mut builder = set.build_task().name("hello".into());
    ///     builder.spawn(async move {1});
    /// });
    /// ```
    pub fn name(self, name: String) -> Self {
        let builder = self.builder.name(name);
        Self {
            builder,
            set: self.set,
        }
    }

    /// Sets the priority for the tasks that are going to get spawned by this JoinSet Builder
    ///
    /// # Examples
    /// ```
    /// use ylong_runtime::task::{JoinSet, PriorityLevel};
    /// ylong_runtime::block_on(async move {
    ///     let mut set = JoinSet::new();
    ///     let mut builder = set.build_task().priority(PriorityLevel::AbsHigh);
    ///     builder.spawn(async move {1});
    /// });
    /// ```
    pub fn priority(self, pri_level: PriorityLevel) -> Self {
        let builder = self.builder.priority(pri_level);
        Self {
            builder,
            set: self.set,
        }
    }

    /// Spawns a task via a `JoinSet` onto a Ylong runtime. The task will start immediately
    /// when `spawn` is called.
    ///
    /// # Panics
    /// This method panics when calling outside of Ylong runtime.
    /// # Examples
    /// ```
    /// use ylong_runtime::task::JoinSet;
    /// ylong_runtime::block_on(async move {
    ///     let mut set = JoinSet::new();
    ///     let mut builder = set.build_task();
    ///     builder.spawn(async move {1});
    /// });
    /// ```
    pub fn spawn<T>(&mut self, task: T) -> CancelHandle
    where
        T: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        self.set.spawn_inner(task, Some(&self.builder))
    }
}

/// Cancels all task inside, and frees all corresponding JoinHandle.
impl<R> Drop for JoinSet<R> {
    fn drop(&mut self) {
        let mut list = self.list.lock().unwrap();

        for item in &list.done_list {
            unsafe {
                (*item.handle.get()).cancel();
                drop(ManuallyDrop::take(&mut *item.handle.get()));
            }
        }
        for item in &list.wait_list {
            unsafe {
                (*item.handle.get()).cancel();
                drop(ManuallyDrop::take(&mut *item.handle.get()));
            }
        }
        // pop every entry inside to reduce the ref count of the list
        while list.done_list.pop_back().is_some() {}
        list.wait_list.drain();
    }
}

// Gets the vtable of the entry waker
fn get_entry_waker_table<R>() -> &'static RawWakerVTable {
    &RawWakerVTable::new(
        clone_entry::<R>,
        wake_entry::<R>,
        wake_entry_ref::<R>,
        drop_entry::<R>,
    )
}

// Converts a entry reference into a Waker
fn entry_into_waker<R>(entry: &Arc<JoinEntry<R>>) -> Waker {
    let cpy = entry.clone();
    let data = Arc::into_raw(cpy) as *const ();
    unsafe { Waker::from_raw(RawWaker::new(data, get_entry_waker_table::<R>())) }
}

unsafe fn clone_entry<R>(data: *const ()) -> RawWaker {
    // First increment the arc counter
    let entry = Arc::from_raw(data as *const JoinEntry<R>);
    mem::forget(entry.clone());
    // Construct the new waker
    let data = Arc::into_raw(entry) as *const ();
    RawWaker::new(data, get_entry_waker_table::<R>())
}

unsafe fn wake_entry<R>(data: *const ()) {
    let entry = Arc::from_raw(data as *const JoinEntry<R>);
    JoinEntry::wake_by_ref(&entry);
}

unsafe fn wake_entry_ref<R>(data: *const ()) {
    let entry = ManuallyDrop::new(Arc::from_raw(data as *const JoinEntry<R>));
    JoinEntry::wake_by_ref(&entry);
}

unsafe fn drop_entry<R>(data: *const ()) {
    drop(Arc::from_raw(data as *const JoinEntry<R>))
}
