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

use super::*;
use libc::{c_char, c_void};
use std::ffi::{CStr, CString};

type FfrtHook = extern "C" fn(*mut c_void);

type FfrtExecHook = extern "C" fn(*mut c_void) -> FfrtRet;

type RawTaskCtx = *mut c_void;

type RawTaskHandle = *mut c_void;

#[repr(C)]
pub enum FfrtRet {
    FfrtCoroutinePending,
    FfrtCoroutineReady,
}

/// The task handle for the real ffrt task.
/// Users could use the task to finish through the handle.
pub struct FfrtTaskHandle(RawTaskHandle);

impl FfrtTaskHandle {
    /// Creates a new ffrt task handle.
    pub fn new(raw: RawTaskHandle) -> Self {
        FfrtTaskHandle(raw)
    }

    /// # Safety
    pub unsafe fn set_waker_hook(&self, waker: FfrtHook, wake_arg: *mut c_void) {
        ffrt_wake_by_handle(wake_arg, waker, None, self.0);
    }
}

impl Drop for FfrtTaskHandle {
    fn drop(&mut self) {
        unsafe {
            ffrt_task_handle_destroy(self.0);
        }
    }
}

impl Default for FfrtTaskAttr {
    fn default() -> Self {
        Self::new()
    }
}

impl FfrtTaskAttr {
    /// Creates a default task attribute.
    pub fn new() -> FfrtTaskAttr {
        let mut attr = FfrtTaskAttr { storage: [0; 128] };
        unsafe {
            let attr_ptr = &mut attr as *mut FfrtTaskAttr;
            ffrt_task_attr_init(attr_ptr);
            // set type to stackless coroutine
            ffrt_task_attr_set_coroutine_type(attr_ptr, CoroutineTypes::Stackless);
        }

        attr
    }

    /// Sets the name for the task attribute.
    pub fn set_name(&mut self, name: &str) -> &mut Self {
        let attr_ptr = self as *mut FfrtTaskAttr;
        let c_name = CString::new(name).expect("FfrtTaskAttr::set_name failed");
        unsafe {
            ffrt_task_attr_set_name(attr_ptr, c_name.as_ptr());
        }
        self
    }

    /// Gets the name from the task attribtue.
    pub fn get_name(&self) -> String {
        let attr_ptr = self as *const FfrtTaskAttr;
        unsafe {
            let c_name = ffrt_task_attr_get_name(attr_ptr);
            CStr::from_ptr(c_name)
                .to_str()
                .expect("FfrtTaskAttr::get_name failed")
                .to_string()
        }
    }

    /// Sets qos level for the task attribute.
    pub fn set_qos(&mut self, qos: Qos) -> &mut Self {
        unsafe {
            let ptr = self as *mut FfrtTaskAttr;
            ffrt_task_attr_set_qos(ptr, qos);
        }
        self
    }

    /// Gets the qos level from the task attribute.
    pub fn get_qos(&self) -> Qos {
        unsafe { ffrt_task_attr_get_qos(self as _) }
    }

    /// Sets the priority level for the task attributes.
    pub fn set_priority(&mut self, priority: u8) -> &mut Self {
        unsafe {
            ffrt_task_attr_set_priority(self as _, priority);
        }
        self
    }

    /// Sets the proiroty level for the task attributes.
    pub fn get_priority(&self) -> u8 {
        unsafe { ffrt_task_attr_get_priority(self as _) }
    }

    /// Sets the coroutine type.
    pub fn set_coroutine_type(&mut self, coroutine_type: CoroutineTypes) -> &mut Self {
        unsafe {
            ffrt_task_attr_set_coroutine_type(self as _, coroutine_type);
        }
        self
    }

    /// Gets the coroutine type.
    pub fn get_coroutine_type(&self) -> CoroutineTypes {
        unsafe { ffrt_task_attr_get_coroutine_type(self as _) }
    }
}

impl Drop for FfrtTaskAttr {
    fn drop(&mut self) {
        unsafe {
            ffrt_task_attr_destroy(self as _);
        }
    }
}

#[link(name = "ffrt")]
// task.h
extern "C" {
    #![allow(unused)]

    fn ffrt_task_attr_init(attr: *mut FfrtTaskAttr);
    fn ffrt_task_attr_set_name(attr: *mut FfrtTaskAttr, name: *const c_char);
    fn ffrt_task_attr_get_name(attr: *const FfrtTaskAttr) -> *const c_char;
    fn ffrt_task_attr_destroy(attr: *mut FfrtTaskAttr);
    fn ffrt_task_attr_set_qos(attr: *mut FfrtTaskAttr, qos: Qos);
    fn ffrt_task_attr_get_qos(attr: *const FfrtTaskAttr) -> Qos;
    fn ffrt_task_attr_set_priority(attr: *mut FfrtTaskAttr, priority: u8);
    fn ffrt_task_attr_get_priority(attr: *const FfrtTaskAttr) -> u8;

    fn ffrt_task_attr_set_coroutine_type(attr: *mut FfrtTaskAttr, coroutine_type: CoroutineTypes);

    fn ffrt_task_attr_get_coroutine_type(attr: *const FfrtTaskAttr) -> CoroutineTypes;

    // submit
    fn ffrt_alloc_auto_free_function_storage_base() -> *const c_void;

    /// Submits a task.
    pub fn ffrt_submit_h_coroutine(
        data: *mut c_void,         // void* callable,
        fp: FfrtExecHook,          // ffrt_function_t exec,
        destroy_fp: FfrtHook,      // ffrt_function_t destroy,
        in_deps: *const FfrtDeps,  //const ffrt_deps_t* in_deps,
        out_deps: *const FfrtDeps, //const ffrt_deps_t* out_deps,
        attr: *const FfrtTaskAttr, //const ffrt_task_attr_t* attr
    ) -> RawTaskHandle;

    // release handle
    fn ffrt_task_handle_destroy(handle: RawTaskHandle);

    /// Gets the current task context.
    pub fn ffrt_task_get() -> RawTaskCtx;

    // set waker
    fn ffrt_wake_by_handle(
        callable: *mut c_void,
        exec: FfrtHook,
        destroy: Option<FfrtHook>,
        handle: RawTaskHandle,
    );

    /// Wakes the task
    pub fn ffrt_wake_coroutine(task: RawTaskCtx);

    /// Sets the wake flag for the task.
    pub fn ffrt_set_wake_flag(flag: bool);
}
