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

use libc::timespec;
use std::ffi::{c_int, c_long};
use std::time::Duration;

/// Yields the current task.
pub fn task_yield() {
    unsafe {
        ffrt_yield();
    }
}

/// Turns the current task to sleep for a specific duration.
pub fn task_sleep(d: Duration) {
    unsafe {
        let t = timespec {
            tv_sec: d.as_secs() as i64,
            tv_nsec: d.subsec_nanos() as c_long,
        };
        ffrt_sleep(&t);
    }
}

#[link(name = "ffrt")]
// sleep.h
extern "C" {
    #![allow(unused)]
    fn ffrt_sleep(duration: *const timespec) -> c_int;
    fn ffrt_yield();
}
