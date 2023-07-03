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

use crate::mutex::MutexGuard;
use libc::{c_int, c_void, timespec};
use std::cell::UnsafeCell;
use std::ffi::c_long;
use std::time::Duration;

#[repr(C)]
struct FfrtCondvar {
    storage: [u8; 512],
}

/// FFRT condvar
pub struct Condvar {
    inner: UnsafeCell<FfrtCondvar>,
}

impl Condvar {
    /// Creates a condvar.
    pub fn new() -> Condvar {
        let mut inner = FfrtCondvar { storage: [0; 512] };
        unsafe {
            let _ = ffrt_cnd_init(&mut inner as _); // how to handle return value?
        }
        Condvar {
            inner: UnsafeCell::new(inner),
        }
    }

    /// Notifies all tasks that wait on this condvar.
    pub fn notify_all(&self) {
        unsafe {
            let _ = ffrt_cnd_broadcast(self.inner.get()); // ignore return result
        }
    }

    /// Notifies one task that waits on this condvar.
    pub fn notify_one(&self) {
        unsafe {
            let _ = ffrt_cnd_signal(self.inner.get()); // ignore return result
        }
    }

    /// Waits on this condvar.
    pub fn wait<T>(&self, guard: MutexGuard<'_, T>) {
        unsafe {
            let _ = ffrt_cnd_wait(self.inner.get(), guard.lock.inner.get() as _);
        }
    }

    /// Waits on this condvar with a time limit.
    pub fn wait_timeout<T>(&self, guard: MutexGuard<'_, T>, d: Duration) {
        let t = timespec {
            tv_sec: d.as_secs() as i64,
            tv_nsec: d.subsec_nanos() as c_long,
        };
        unsafe {
            let _ = ffrt_cnd_timedwait(self.inner.get(), guard.lock.inner.get() as _, &t as _);
        }
    }
}

impl Default for Condvar {
    fn default() -> Self {
        Condvar::new()
    }
}

impl Drop for Condvar {
    fn drop(&mut self) {
        unsafe {
            ffrt_cnd_destroy(self.inner.get());
        }
    }
}

unsafe impl Send for Condvar {}
unsafe impl Sync for Condvar {}

#[link(name = "ffrt")]
// condition_variable.h
extern "C" {
    fn ffrt_cnd_init(cnd: *mut FfrtCondvar) -> c_int;
    fn ffrt_cnd_signal(cnd: *mut FfrtCondvar) -> c_int;
    fn ffrt_cnd_broadcast(cnd: *mut FfrtCondvar) -> c_int;
    fn ffrt_cnd_wait(cnd: *mut FfrtCondvar, mtx: *mut c_void) -> c_int;
    fn ffrt_cnd_timedwait(
        cnd: *mut FfrtCondvar,
        mtx: *mut c_void,
        time_point: *const timespec,
    ) -> c_int;
    fn ffrt_cnd_destroy(cnd: *mut FfrtCondvar);
}
