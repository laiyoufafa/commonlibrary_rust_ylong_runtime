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

use super::FfrtResult;
use libc::{c_int, c_void};
use std::ptr::null_mut;

/// A handle for thread.
pub struct ThreadHandle(FfrtThrd);

/// Spawns tasks to be executed on a thread.
pub fn thread_spawn<F>(f: F) -> FfrtResult<ThreadHandle>
where
    F: FnOnce() -> i32 + Send + 'static,
{
    extern "C" fn exec<F>(data: *mut c_void) -> c_int
    where
        F: FnOnce() -> i32,
    {
        unsafe {
            let f = Box::from_raw(data as *mut F);
            f()
        }
    }

    let f: Box<F> = Box::new(f);
    let data = Box::into_raw(f) as *mut c_void;
    unsafe {
        let mut h = ThreadHandle(null_mut());
        let r = ffrt_thrd_create(&mut h.0 as _, exec::<F>, data);
        if r != 0 {
            return Err(r);
        }
        let r = ffrt_thrd_detach(&mut h.0 as _);
        if r != 0 {
            return Err(r);
        }
        Ok(h)
    }
}

impl ThreadHandle {
    /// Joins the thread.
    pub fn join(self) -> FfrtResult<()> {
        let mut h = self;
        unsafe {
            // Question: how to handle this return value?
            let r = ffrt_thrd_join(&mut h.0 as _);
            if r != 0 {
                return Err(r);
            }
            ffrt_thrd_exit(&mut h.0);
            Ok(())
        }
    }
}

impl Drop for ThreadHandle {
    fn drop(&mut self) {
        unsafe {
            let r = ffrt_thrd_join(&mut self.0 as _);
            if r != 0 {
                return;
            }
            ffrt_thrd_exit(&mut self.0);
        }
    }
}

// because ThreadHandle has ownership of inner data
unsafe impl Send for ThreadHandle {}
unsafe impl Sync for ThreadHandle {}

type FfrtThreadStart = extern "C" fn(*mut c_void) -> c_int;
type FfrtThrd = *mut c_void;

#[link(name = "ffrt")]
// thread.h
extern "C" {
    fn ffrt_thrd_create(thr: *mut FfrtThrd, func: FfrtThreadStart, arg: *mut c_void) -> c_int;
    fn ffrt_thrd_join(thr: *mut FfrtThrd) -> c_int;
    fn ffrt_thrd_detach(thr: *mut FfrtThrd) -> c_int;
    fn ffrt_thrd_exit(thr: *mut FfrtThrd);
}
