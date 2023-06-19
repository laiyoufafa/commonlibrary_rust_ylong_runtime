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

use libc::c_void;
use ylong_ffrt::{ffrt_set_wake_flag, ffrt_task_get, ffrt_wake_coroutine};

type RawTaskCtx = *mut c_void;

#[derive(Clone)]
pub(crate) struct FfrtTaskCtx(RawTaskCtx);

impl FfrtTaskCtx {
    pub(crate) fn get_current() -> Self {
        let task_ctx = unsafe { ffrt_task_get() };
        FfrtTaskCtx(task_ctx)
    }

    pub(crate) fn set_waker_flag(flag: bool) {
        unsafe {
            ffrt_set_wake_flag(flag);
        }
    }

    pub(crate) fn wake_task(&self) {
        unsafe {
            ffrt_wake_coroutine(self.0);
        }
    }
}
