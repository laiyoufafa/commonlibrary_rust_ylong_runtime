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

use crate::executor::PlaceholderScheduler;
use crate::task::{JoinHandle, Task, VirtualTableType};
use crate::TaskBuilder;
use libc::c_void;
use std::future::Future;
use std::ptr::null;
use std::sync::Weak;
use ylong_ffrt::FfrtRet::{FfrtCoroutinePending, FfrtCoroutineReady};
use ylong_ffrt::{ffrt_submit_h_coroutine, FfrtRet, FfrtTaskAttr, FfrtTaskHandle};

pub(crate) fn ffrt_submit(t: Task, attr: &FfrtTaskAttr) -> FfrtTaskHandle {
    extern "C" fn exec_future(data: *mut c_void) -> FfrtRet {
        unsafe {
            match (*(data as *mut Task)).0.run() {
                true => FfrtCoroutineReady,
                false => FfrtCoroutinePending,
            }
        }
    }

    extern "C" fn exec_drop(data: *mut c_void) {
        unsafe {
            drop(Box::from_raw(data as *mut Task));
        }
    }

    let t: Box<Task> = Box::new(t);
    let data = Box::into_raw(t) as *mut c_void;
    let handle = unsafe {
        ffrt_submit_h_coroutine(
            data,
            exec_future,
            exec_drop,
            null(),
            null(),
            attr as *const FfrtTaskAttr,
        )
    };
    FfrtTaskHandle::new(handle)
}

pub fn spawn<F>(task: F, builder: &TaskBuilder) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    let scheduler: Weak<PlaceholderScheduler> = Weak::new();
    let (task, mut join_handle) =
        Task::create_task(builder, scheduler, task, VirtualTableType::Ffrt);
    let attr = FfrtTaskAttr::new();

    let handle = ffrt_submit(task, &attr);
    join_handle.set_task_handle(handle);
    join_handle
}
