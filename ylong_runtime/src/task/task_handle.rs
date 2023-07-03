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

use crate::error::{ErrorKind, ScheduleError};
use crate::executor::Schedule;
use crate::task::raw::{Header, Inner, TaskMngInfo};
use crate::task::state::StateAction;
use crate::task::waker::WakerRefHeader;
use crate::task::{state, Task};
use crate::{cfg_ffrt, cfg_not_ffrt};
use std::future::Future;
use std::panic;
use std::ptr::NonNull;
use std::task::{Context, Poll, Waker};
cfg_ffrt! {
    use std::ffi::c_void;
    use std::ptr::null_mut;
    use ylong_ffrt::FfrtTaskHandle;
    use crate::ffrt::ffrt_task::FfrtTaskCtx;
}
cfg_not_ffrt! {
    use crate::task::raw::Stage;
}

pub(crate) struct TaskHandle<T: Future, S: Schedule> {
    task: NonNull<TaskMngInfo<T, S>>,
}

impl<T, S> TaskHandle<T, S>
where
    T: Future,
    S: Schedule,
{
    pub(crate) unsafe fn from_raw(ptr: NonNull<Header>) -> Self {
        TaskHandle {
            task: ptr.cast::<TaskMngInfo<T, S>>(),
        }
    }

    fn header(&self) -> &Header {
        unsafe { self.task.as_ref().header() }
    }

    fn inner(&self) -> &Inner<T, S> {
        unsafe { self.task.as_ref().inner() }
    }
}

impl<T, S> TaskHandle<T, S>
where
    T: Future,
    S: Schedule,
{
    pub(crate) fn release(self) {
        unsafe { drop(Box::from_raw(self.task.as_ptr())) };
    }

    pub(crate) fn drop_ref(self) {
        let prev = self.header().state.dec_ref();
        if state::is_last_ref_count(prev) {
            self.release();
        }
    }

    fn finish(self, state: usize, output: Result<T::Output, ScheduleError>) {
        // send result if the JoinHandle is not dropped
        if state::is_care_join_handle(state) {
            self.inner().send_result(output);
        } else {
            self.inner().turning_to_used_data();
        }

        let res = self.header().state.turning_to_finish();
        let cur = match res {
            Ok(cur) => cur,
            Err(e) => panic!("{}", e.as_str()),
        };

        if state::is_set_waker(cur) {
            self.inner().wake_join();
        }
        self.drop_ref();
    }

    // Runs the task
    pub(crate) fn run(self) {
        let action = self.header().state.turning_to_running();

        match action {
            StateAction::Success => {}
            StateAction::Canceled(cur) => {
                let output = self.get_canceled();
                return self.finish(cur, Err(output));
            }
            StateAction::Failed(state) => panic!("task state invalid: {}", state),
            _ => unreachable!(),
        };

        // turn the task header into a waker
        let waker = WakerRefHeader::<'_>::new::<T>(self.header());
        let mut context = Context::from_waker(&waker);

        let res = panic::catch_unwind(panic::AssertUnwindSafe(|| {
            self.inner().poll(&mut context).map(Ok)
        }));

        let cur = self.header().state.get_current_state();
        match res {
            Ok(Poll::Ready(output)) => {
                // send result if the JoinHandle is not dropped
                self.finish(cur, output);
            }

            Ok(Poll::Pending) => match self.header().state.turning_to_idle() {
                StateAction::Enqueue => {
                    self.get_scheduled(true);
                }
                StateAction::Failed(state) => panic!("task state invalid: {}", state),
                StateAction::Canceled(state) => {
                    let output = self.get_canceled();
                    self.finish(state, Err(output));
                }
                _ => {}
            },

            Err(_) => {
                let output = Err(ScheduleError::new(ErrorKind::Panic, "panic happen"));
                self.finish(cur, output);
            }
        }
    }

    pub(crate) fn get_result(self, out: &mut Poll<std::result::Result<T::Output, ScheduleError>>) {
        *out = Poll::Ready(self.inner().turning_to_get_data());
    }

    pub(crate) fn drop_join_handle(self) {
        if self.header().state.try_turning_to_un_join_handle() {
            return;
        }

        match self.header().state.turn_to_un_join_handle() {
            Ok(_) => {}
            Err(_) => {
                self.inner().turning_to_used_data();
            }
        }
        self.drop_ref();
    }

    fn set_waker_inner(&self, des_waker: Waker, cur_state: usize) -> Result<usize, usize> {
        if !state::is_care_join_handle(cur_state) || state::is_set_waker(cur_state) {
            panic!("set waker failed: the join handle either get dropped or the task already has a waker set");
        }
        unsafe {
            let waker = self.inner().waker.get();
            *waker = Some(des_waker);
        }
        let result = self.header().state.turn_to_set_waker();
        if result.is_err() {
            unsafe {
                let waker = self.inner().waker.get();
                *waker = None;
            }
        }
        result
    }

    pub(crate) fn set_waker(self, cur: usize, des_waker: &Waker) -> bool {
        let res = if state::is_set_waker(cur) {
            let is_same_waker = unsafe {
                let waker = self.inner().waker.get();
                (*waker).as_ref().unwrap().will_wake(des_waker)
            };
            // we don't register the same waker
            if is_same_waker {
                return false;
            }
            self.header()
                .state
                .turn_to_un_set_waker()
                .and_then(|cur| self.set_waker_inner(des_waker.clone(), cur))
        } else {
            self.set_waker_inner(des_waker.clone(), cur)
        };

        if let Err(cur) = res {
            if !state::is_finished(cur) {
                panic!("setting waker should only be failed due to the task's completion");
            }
            return true;
        }

        false
    }

    pub(crate) fn wake(self) {
        self.wake_by_ref();
        self.drop_ref();
    }

    pub(crate) fn wake_by_ref(&self) {
        let prev = self.header().state.turn_to_scheduling();
        if state::need_enqueue(prev) {
            self.get_scheduled(false);
        }
    }

    // Actually cancels the task during running
    fn get_canceled(&self) -> ScheduleError {
        self.inner().turning_to_used_data();
        ErrorKind::TaskCanceled.into()
    }

    // Sets task state into canceled and scheduled
    pub(crate) fn set_canceled(&self) {
        if self.header().state.turn_to_canceled_and_scheduled() {
            self.get_scheduled(false);
        }
    }

    fn to_task(&self) -> Task {
        unsafe { Task::from_raw(self.header().into()) }
    }

    fn get_scheduled(&self, lifo: bool) {
        self.inner()
            .scheduler
            .upgrade()
            .unwrap()
            .schedule(self.to_task(), lifo);
    }
}

#[cfg(not(feature = "ffrt"))]
impl<T, S> TaskHandle<T, S>
where
    T: Future,
    S: Schedule,
{
    pub(crate) unsafe fn shutdown(self) {
        self.header().state.set_cancel();
        // Check if the JoinHandle gets dropped already. If JoinHandle is still there, wakes
        // the JoinHandle.
        let cur = self.header().state.get_current_state();
        if state::is_care_join_handle(cur) {
            let stage = self.inner().stage.get();
            *stage = Stage::StoreData(Err(ErrorKind::TaskCanceled.into()));
            self.header().state.set_running();
            let _ = self.header().state.turning_to_finish();
            if state::is_set_waker(cur) {
                self.inner().wake_join();
            }
            self.drop_ref();
        }
    }
}

#[cfg(feature = "ffrt")]
impl<T, S> TaskHandle<T, S>
where
    T: Future,
    S: Schedule,
{
    fn ffrt_finish(self, state: usize, output: Result<T::Output, ScheduleError>) {
        if state::is_care_join_handle(state) {
            self.inner().send_result(output);
        } else {
            self.inner().turning_to_used_data();
        }

        let cur = match self.header().state.turning_to_finish() {
            Ok(cur) => cur,
            Err(e) => panic!("{}", e.as_str()),
        };

        if !state::is_set_waker(cur) {
            FfrtTaskCtx::set_waker_flag(false);
        }
    }

    pub(crate) fn ffrt_run(self) -> bool {
        self.inner().get_task_ctx();

        match self.header().state.turning_to_running() {
            StateAction::Failed(state) => panic!("turning to running failed: {:b}", state),
            StateAction::Canceled(cur) => {
                let output = self.ffrt_get_canceled();
                self.ffrt_finish(cur, Err(output));
                return true;
            }
            _ => {}
        }

        let waker = WakerRefHeader::<'_>::new::<T>(self.header());
        let mut context = Context::from_waker(&waker);

        let res = panic::catch_unwind(panic::AssertUnwindSafe(|| {
            self.inner().poll(&mut context).map(Ok)
        }));

        let cur = self.header().state.get_current_state();
        match res {
            Ok(Poll::Ready(output)) => {
                // send result if the JoinHandle is not dropped
                self.ffrt_finish(cur, output);
                true
            }

            Ok(Poll::Pending) => match self.header().state.turning_to_idle() {
                StateAction::Enqueue => false,
                StateAction::Failed(state) => panic!("task state invalid: {:b}", state),
                StateAction::Canceled(state) => {
                    let output = self.ffrt_get_canceled();
                    self.ffrt_finish(state, Err(output));
                    true
                }
                _ => false,
            },

            Err(_) => {
                let output = Err(ScheduleError::new(ErrorKind::Panic, "panic happen"));
                self.ffrt_finish(cur, output);
                true
            }
        }
    }

    fn ffrt_set_waker_inner(
        &self,
        des_waker: Waker,
        cur_state: usize,
        task_handle: &FfrtTaskHandle,
    ) -> Result<usize, (usize, *mut c_void)> {
        if !state::is_care_join_handle(cur_state) || state::is_set_waker(cur_state) {
            panic!("set waker failed: the join handle either get dropped or the task already has a waker set");
        }

        let waker = Box::new(des_waker);
        let waker_ptr = Box::into_raw(waker) as *mut () as *mut c_void;
        unsafe {
            task_handle.set_waker_hook(waker_hook, waker_ptr);
        }

        let result = self
            .header()
            .state
            .turn_to_set_waker()
            .map_err(|size| (size, waker_ptr));
        result
    }

    pub(crate) fn ffrt_set_waker(self, cur: usize, des_waker: &Waker) -> bool {
        let task_handle = self.header().task_handle.as_ref().unwrap();
        let res = if state::is_set_waker(cur) {
            self.header()
                .state
                .turn_to_un_set_waker()
                .map_err(|cur| (cur, null_mut()))
                .and_then(|cur| self.ffrt_set_waker_inner(des_waker.clone(), cur, task_handle))
        } else {
            self.ffrt_set_waker_inner(des_waker.clone(), cur, task_handle)
        };

        if let Err((cur, ptr)) = res {
            if !ptr.is_null() {
                let waker = ptr as *mut () as *mut Waker;
                unsafe { drop(Box::from_raw(waker)) };
            }

            if !state::is_finished(cur) {
                panic!("the state is not finished, it is wrong");
            }
            return true;
        }
        false
    }

    pub(crate) fn ffrt_wake(self) {
        self.ffrt_wake_by_ref();
        self.drop_ref();
    }

    pub(crate) fn ffrt_wake_by_ref(&self) {
        self.header().state.turn_to_scheduling();
        let ffrt_task = unsafe { (*self.inner().task.get()).as_ref().unwrap() };
        ffrt_task.wake_task();
    }

    // Actually cancels the task during running
    fn ffrt_get_canceled(&self) -> ScheduleError {
        self.inner().turning_to_used_data();
        ErrorKind::TaskCanceled.into()
    }

    // Sets task state into canceled and scheduled
    pub(crate) fn ffrt_set_canceled(&self) {
        if self.header().state.turn_to_canceled_and_scheduled() {
            let ffrt_task = unsafe { (*self.inner().task.get()).as_ref().unwrap() };
            ffrt_task.wake_task();
        }
    }
}

#[cfg(feature = "ffrt")]
extern "C" fn waker_hook(waker: *mut c_void) {
    let waker = waker as *mut () as *mut Waker;
    unsafe {
        (*waker).wake_by_ref();
        drop(Box::from_raw(waker));
    }
}
