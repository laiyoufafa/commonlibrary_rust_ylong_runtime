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

use crate::executor::Schedule;
use crate::task::state::TaskState;
use crate::task::task_handle::TaskHandle;
use crate::task::{TaskBuilder, VirtualTableType};
use crate::{cfg_ffrt, ScheduleError};
use std::cell::UnsafeCell;
use std::future::Future;
use std::mem;
use std::pin::Pin;
use std::ptr::NonNull;
use std::sync::Weak;
use std::task::{Context, Poll, Waker};
cfg_ffrt! {
    use ylong_ffrt::FfrtTaskHandle;
    use crate::ffrt::ffrt_task::FfrtTaskCtx;
}

pub(crate) struct TaskVirtualTable {
    /// Task running method
    pub(crate) run: unsafe fn(NonNull<Header>) -> bool,
    /// Task scheduling method
    pub(crate) schedule: unsafe fn(NonNull<Header>, bool),
    /// Task result-getting method
    pub(crate) get_result: unsafe fn(NonNull<Header>, *mut ()),
    /// JoinHandle drop method
    pub(crate) drop_join_handle: unsafe fn(NonNull<Header>),
    /// Task reference drop method
    pub(crate) drop_ref: unsafe fn(NonNull<Header>),
    /// Task waker setting method
    pub(crate) set_waker: unsafe fn(NonNull<Header>, cur_state: usize, waker: *const ()) -> bool,
    /// Task release method
    #[cfg(not(feature = "ffrt"))]
    pub(crate) release: unsafe fn(NonNull<Header>),
    /// Task cancel method
    pub(crate) cancel: unsafe fn(NonNull<Header>),
}

pub(crate) struct Header {
    pub(crate) state: TaskState,
    #[cfg(feature = "ffrt")]
    pub(crate) task_handle: Option<FfrtTaskHandle>,
    pub(crate) vtable: &'static TaskVirtualTable,
}

#[cfg(feature = "ffrt")]
impl Header {
    fn set_task_handle(&mut self, handle: FfrtTaskHandle) {
        self.task_handle.replace(handle);
    }
}

#[derive(PartialEq, Eq, Hash)]
pub(crate) struct RawTask {
    pub(crate) ptr: NonNull<Header>,
}

impl RawTask {
    pub(crate) fn form_raw(ptr: NonNull<Header>) -> RawTask {
        RawTask { ptr }
    }

    pub(crate) fn header(&self) -> &Header {
        unsafe { self.ptr.as_ref() }
    }

    pub(crate) fn run(self) -> bool {
        let vir_table = self.header().vtable;
        unsafe { (vir_table.run)(self.ptr) }
    }

    pub(crate) unsafe fn get_result(self, res: *mut ()) {
        let vir_table = self.header().vtable;
        (vir_table.get_result)(self.ptr, res);
    }

    pub(crate) unsafe fn cancel(self) {
        let vir_table = self.header().vtable;
        (vir_table.cancel)(self.ptr)
    }

    pub(crate) unsafe fn set_waker(self, cur_state: usize, waker: *const ()) -> bool {
        let vir_table = self.header().vtable;
        (vir_table.set_waker)(self.ptr, cur_state, waker)
    }

    pub(crate) fn drop_ref(self) {
        let vir_table = self.header().vtable;
        unsafe {
            (vir_table.drop_ref)(self.ptr);
        }
    }

    pub(crate) fn drop_join_handle(self) {
        let vir_table = self.header().vtable;
        unsafe {
            (vir_table.drop_join_handle)(self.ptr);
        }
    }
}

#[cfg(feature = "ffrt")]
impl RawTask {
    fn header_mut(&mut self) -> &mut Header {
        unsafe { self.ptr.as_mut() }
    }

    pub(crate) fn set_task_handle(&mut self, handle: FfrtTaskHandle) {
        self.header_mut().set_task_handle(handle);
    }
}

#[cfg(not(feature = "ffrt"))]
impl RawTask {
    pub(super) fn shutdown(self) {
        let vir_table = self.header().vtable;
        unsafe {
            (vir_table.release)(self.ptr);
        }
    }
}

impl Copy for RawTask {}

impl Clone for RawTask {
    fn clone(&self) -> Self {
        RawTask { ptr: self.ptr }
    }
}

pub(crate) enum Stage<T: Future> {
    Executing(T),
    Executed,
    StoreData(Result<T::Output, ScheduleError>),
    UsedData,
}

pub(crate) struct Inner<T: Future, S: Schedule> {
    /// The execution stage of the future
    pub(crate) stage: UnsafeCell<Stage<T>>,
    /// The scheduler of the task queue
    pub(crate) scheduler: Weak<S>,
    /// Waker of the task waiting on this task
    pub(crate) waker: UnsafeCell<Option<Waker>>,
    /// Task in adaptive runtime
    #[cfg(feature = "ffrt")]
    pub(crate) task: UnsafeCell<Option<FfrtTaskCtx>>,
}

impl<T, S> Inner<T, S>
where
    T: Future,
    S: Schedule,
{
    fn new(task: T, scheduler: Weak<S>) -> Self {
        Inner {
            stage: UnsafeCell::new(Stage::Executing(task)),
            scheduler,
            waker: UnsafeCell::new(None),
            #[cfg(feature = "ffrt")]
            task: UnsafeCell::new(None),
        }
    }

    #[cfg(feature = "ffrt")]
    pub(crate) fn get_task_ctx(&self) {
        unsafe {
            if (*self.task.get()).is_none() {
                (*self.task.get()).replace(FfrtTaskCtx::get_current());
            }
        }
    }

    fn turning_to_executed(&self) {
        let stage = self.stage.get();
        unsafe {
            *stage = Stage::Executed;
        }
    }

    pub(crate) fn turning_to_used_data(&self) {
        let stage = self.stage.get();
        unsafe {
            *stage = Stage::UsedData;
        }
    }

    fn turning_to_store_data(&self, output: std::result::Result<T::Output, ScheduleError>) {
        let stage = self.stage.get();
        unsafe {
            *stage = Stage::StoreData(output);
        }
    }

    pub(crate) fn turning_to_get_data(&self) -> Result<T::Output, ScheduleError> {
        let stage = self.stage.get();
        let data = mem::replace(unsafe { &mut *stage }, Stage::UsedData);
        match data {
            Stage::StoreData(output) => output,
            _ => panic!("invalid task stage: the output is not stored inside the task"),
        }
    }

    pub(crate) fn poll(&self, context: &mut Context) -> Poll<T::Output> {
        let stage = self.stage.get();
        let future = match unsafe { &mut *stage } {
            Stage::Executing(future) => future,
            _ => panic!("invalid task stage: task polled while not being executed"),
        };

        let future = unsafe { Pin::new_unchecked(future) };
        let res = future.poll(context);

        // if result is received, turn the task to finished
        if res.is_ready() {
            self.turning_to_executed();
        }
        res
    }

    pub(crate) fn send_result(&self, output: Result<T::Output, ScheduleError>) {
        self.turning_to_store_data(output);
    }

    pub(crate) fn wake_join(&self) {
        let waker = self.waker.get();
        match unsafe { &*waker } {
            Some(waker) => {
                waker.wake_by_ref();
            }
            None => panic!("task waker has not been set"),
        }
    }
}

/// Manages task infos.
/// `repr(C)` is necessary because we cast a pointer of [`TaskMngInfo`] into a pointer of [`Header`].
#[repr(C)]
pub(crate) struct TaskMngInfo<T: Future, S: Schedule> {
    /// a pointer to the heap-allocated task
    header: Header,
    inner: Inner<T, S>,
}

unsafe fn run<T, S>(ptr: NonNull<Header>) -> bool
where
    T: Future,
    S: Schedule,
{
    let task_handle = TaskHandle::<T, S>::from_raw(ptr);
    task_handle.run();
    true
}

unsafe fn schedule<T, S>(ptr: NonNull<Header>, flag: bool)
where
    T: Future,
    S: Schedule,
{
    let task_handle = TaskHandle::<T, S>::from_raw(ptr);
    if flag {
        task_handle.wake();
    } else {
        task_handle.wake_by_ref();
    }
}

unsafe fn get_result<T, S>(ptr: NonNull<Header>, res: *mut ())
where
    T: Future,
    S: Schedule,
{
    let out = &mut *(res as *mut Poll<Result<T::Output, ScheduleError>>);
    let task_handle = TaskHandle::<T, S>::from_raw(ptr);
    task_handle.get_result(out);
}

unsafe fn drop_ref<T, S>(ptr: NonNull<Header>)
where
    T: Future,
    S: Schedule,
{
    let task_handle = TaskHandle::<T, S>::from_raw(ptr);
    task_handle.drop_ref();
}

unsafe fn set_waker<T, S>(ptr: NonNull<Header>, cur_state: usize, waker: *const ()) -> bool
where
    T: Future,
    S: Schedule,
{
    let waker = &*(waker as *const Waker);
    let task_handle = TaskHandle::<T, S>::from_raw(ptr);
    task_handle.set_waker(cur_state, waker)
}

unsafe fn drop_join_handle<T, S>(ptr: NonNull<Header>)
where
    T: Future,
    S: Schedule,
{
    let task_handle = TaskHandle::<T, S>::from_raw(ptr);
    task_handle.drop_join_handle();
}

#[cfg(not(feature = "ffrt"))]
unsafe fn release<T, S>(ptr: NonNull<Header>)
where
    T: Future,
    S: Schedule,
{
    let task_handle = TaskHandle::<T, S>::from_raw(ptr);
    task_handle.shutdown();
}

unsafe fn cancel<T, S>(ptr: NonNull<Header>)
where
    T: Future,
    S: Schedule,
{
    let task_handle = TaskHandle::<T, S>::from_raw(ptr);
    task_handle.set_canceled();
}

fn create_vtable<T, S>() -> &'static TaskVirtualTable
where
    T: Future,
    S: Schedule,
{
    &TaskVirtualTable {
        run: run::<T, S>,
        schedule: schedule::<T, S>,
        get_result: get_result::<T, S>,
        drop_join_handle: drop_join_handle::<T, S>,
        drop_ref: drop_ref::<T, S>,
        set_waker: set_waker::<T, S>,
        #[cfg(not(feature = "ffrt"))]
        release: release::<T, S>,
        cancel: cancel::<T, S>,
    }
}

cfg_ffrt! {
    unsafe fn ffrt_run<T, S>(ptr: NonNull<Header>) -> bool
    where
        T: Future,
        S: Schedule,
    {
        let task_handle = TaskHandle::<T, S>::from_raw(ptr);
        task_handle.ffrt_run()
    }

    unsafe fn ffrt_schedule<T, S>(ptr: NonNull<Header>, flag: bool)
    where
        T: Future,
        S: Schedule,
    {
        let task_handle = TaskHandle::<T, S>::from_raw(ptr);
        if flag {
            task_handle.ffrt_wake();
        } else {
            task_handle.ffrt_wake_by_ref();
        }
    }

    unsafe fn ffrt_set_waker<T, S>(ptr: NonNull<Header>, cur_state: usize, waker: *const ()) -> bool
    where
        T: Future,
        S: Schedule,
    {
        let waker = &*(waker as *const Waker);
        let task_handle = TaskHandle::<T, S>::from_raw(ptr);
        task_handle.ffrt_set_waker(cur_state, waker)
    }

    unsafe fn ffrt_cancel<T, S>(ptr: NonNull<Header>)
    where
        T: Future,
        S: Schedule,
    {
        let task_handle = TaskHandle::<T, S>::from_raw(ptr);
        task_handle.ffrt_set_canceled();
    }

    fn create_ffrt_vtable<T, S>() -> &'static TaskVirtualTable
    where
        T: Future,
        S: Schedule,
    {
        &TaskVirtualTable {
            run: ffrt_run::<T, S>,
            schedule: ffrt_schedule::<T, S>,
            get_result: get_result::<T, S>,
            drop_join_handle: drop_join_handle::<T, S>,
            drop_ref: drop_ref::<T, S>,
            set_waker: ffrt_set_waker::<T, S>,
            cancel: ffrt_cancel::<T, S>,
        }
    }
}

impl<T, S> TaskMngInfo<T, S>
where
    T: Future,
    S: Schedule,
{
    /// Creates non-stackful task info.
    // TODO: builder information currently is not used yet. Might use in the future (e.g. priority),
    //   so keep it now.
    pub(crate) fn new(
        _builder: &TaskBuilder,
        scheduler: Weak<S>,
        task: T,
        virtual_table_type: VirtualTableType,
    ) -> Box<Self> {
        let vtable = match virtual_table_type {
            VirtualTableType::Ylong => create_vtable::<T, S>(),
            #[cfg(feature = "ffrt")]
            VirtualTableType::Ffrt => create_ffrt_vtable::<T, S>(),
        };
        // Create the common header
        let header = Header {
            state: TaskState::new(),
            #[cfg(feature = "ffrt")]
            task_handle: None,
            vtable,
        };
        // Create task private info
        let inner = Inner::<T, S>::new(task, scheduler);
        // Allocate it onto the heap
        Box::new(TaskMngInfo { header, inner })
    }

    pub(crate) fn header(&self) -> &Header {
        &self.header
    }

    pub(crate) fn inner(&self) -> &Inner<T, S> {
        &self.inner
    }
}
