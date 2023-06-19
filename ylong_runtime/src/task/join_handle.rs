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

//! Joinhandle for asynchronous tasks.
//!
//! [`JoinHandle`] is similar to a JoinHandle for a thread. It could be used to await an
//! asynchronous task to finish to get its result.

use crate::error::ScheduleError;
use crate::task::raw::RawTask;
use crate::task::state;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};
#[cfg(feature = "ffrt")]
use ylong_ffrt::FfrtTaskHandle;

/// A handle to the actual spawned task.
///
/// This can be considered as the equivalent of [`std::thread::JoinHandle`]
/// for a ylong task rather than a thread.
///
/// It could be used to join the corresponding task or cancel it.
/// If a `JoinHandle` is dropped, then the task continues executing in the background
/// and its return value is lost. There is no way to join the task after its JoinHandle is
/// dropped.
///
/// # Examples
///
/// ```
/// let handle = ylong_runtime::spawn(async {
///     let handle2 = ylong_runtime::spawn(async {1});
///     assert_eq!(handle2.await.unwrap(), 1);
/// });
/// ylong_runtime::block_on(handle).unwrap();
/// ```
pub struct JoinHandle<R> {
    pub(crate) raw: RawTask,
    marker: PhantomData<R>,
}

unsafe impl<R: Send> Send for JoinHandle<R> {}
unsafe impl<R: Send> Sync for JoinHandle<R> {}

impl<R> JoinHandle<R> {
    pub(crate) fn new(raw: RawTask) -> JoinHandle<R> {
        JoinHandle {
            raw,
            marker: PhantomData,
        }
    }

    /// Cancels the task associating with this JoinHandle. If the task has already finished,
    /// this method does nothing.
    ///
    /// When successfully canceled, `.await` on this JoinHandle will return a `TaskCanceled` error.
    pub fn cancel(&self) {
        unsafe {
            self.raw.cancel();
        }
    }

    pub(crate) fn get_cancel_handle(&self) -> CancelHandle {
        CancelHandle::new(self.raw)
    }

    pub(crate) fn set_waker(&mut self, waker: &Waker) {
        let cur = self.raw.header().state.get_current_state();
        unsafe {
            if self.raw.set_waker(cur, waker as *const Waker as *const ()) {
                // Task already finished, wake the waker immediately
                waker.wake_by_ref();
            }
        }
    }

    #[cfg(feature = "ffrt")]
    pub(crate) fn set_task_handle(&mut self, handle: FfrtTaskHandle) {
        self.raw.set_task_handle(handle);
    }
}

impl<R> Unpin for JoinHandle<R> {}

impl<R> Future for JoinHandle<R> {
    // The type of the output needs to match with Stage::StorageData
    type Output = Result<R, ScheduleError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut res = Poll::Pending;

        let cur = self.raw.header().state.get_current_state();
        if !state::is_care_join_handle(cur) {
            panic!("JoinHandle should not be polled after it's dropped");
        }
        if state::is_finished(cur) {
            unsafe {
                self.raw.get_result(&mut res as *mut _ as *mut ());
            }
        } else {
            unsafe {
                let is_finished = self.raw.set_waker(cur, cx.waker() as *const _ as *mut ());
                // Setting the waker may happen concurrently with task finishing.
                // Therefore we check one more time to see if the task is finished.
                if is_finished {
                    self.raw.get_result(&mut res as *mut _ as *mut ());
                }
            }
        }
        res
    }
}

impl<R> Drop for JoinHandle<R> {
    fn drop(&mut self) {
        self.raw.drop_join_handle();
    }
}

/// A handle to cancel the spawned task.
///
/// `CancelHandle` cannot await the task's completion, it can only terminate it.
pub struct CancelHandle {
    raw: RawTask,
}

impl CancelHandle {
    pub(crate) fn new(raw: RawTask) -> CancelHandle {
        raw.header().state.inc_ref();
        CancelHandle { raw }
    }

    /// Cancels the task associated with this handle.
    ///
    /// If the task has been already finished or it is currently running and about to finish,
    /// then this method will do nothing.
    pub fn cancel(&self) {
        unsafe { self.raw.cancel() }
    }

    /// Checks whether the task associated with this handle has finished executing.
    pub fn is_finished(&self) -> bool {
        let state = self.raw.header().state.get_current_state();
        state::is_finished(state)
    }
}

impl Drop for CancelHandle {
    fn drop(&mut self) {
        self.raw.drop_ref()
    }
}

unsafe impl Send for CancelHandle {}
unsafe impl Sync for CancelHandle {}
