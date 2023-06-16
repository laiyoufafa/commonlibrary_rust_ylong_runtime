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

use libc::{c_int, c_void};

// Unstable interface, rust encapsulation temporarily not provided

type FfrtSysEventHandleT = *mut c_void;
type DestroyFunc = extern "C" fn(*mut c_void);

#[link(name = "ffrt")]
// sys_event.h
extern "C" {
    #![allow(unused)]
    fn ffrt_sys_event_create(ty: c_int, fd: usize, filter: usize) -> FfrtSysEventHandleT;
    fn ffrt_sys_event_wait(event: FfrtSysEventHandleT, sec: i64) -> c_int;
    fn ffrt_sys_event_destroy(event: FfrtSysEventHandleT, func: DestroyFunc, arg: *mut c_void);
}
