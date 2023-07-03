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

//! Wraps Windows core-affinity syscalls.

use std::io::Error;
use std::io::Result;
use std::os::windows::raw::HANDLE;

extern "system" {
    fn GetCurrentThread() -> HANDLE;

    fn SetThreadAffinityMask(hThread: HANDLE, dwThreadAffinityMask: usize) -> usize;
}

/// Sets the current thread's tied core cpu.
///
/// # Note
/// In order to align with linux, the `cpu` will be +1 in function,
/// so we can Use this interface as with linux
///
/// # Example
///
/// ```rust
/// use ylong_runtime::util::core_affinity;
///
/// let ret = core_affinity::set_current_affinity(0).is_ok();
/// ```
pub fn set_current_affinity(cpu: usize) -> Result<()> {
    // In Windows, dwThreadAffinityMask is start from 1, so we have to +1 to align with linux.
    let cpu = cpu + 1;
    // If the function succeeds, the return value is the thread's previous affinity mask.
    // If the function fails, the return value is 0.
    let res = unsafe {
        let handle = GetCurrentThread();
        SetThreadAffinityMask(handle, cpu) as i32
    };
    // To be consistent with linux output, the results are processed
    match res {
        0 => Err(Error::last_os_error()),
        _ => Ok(()),
    }
}

// TODO:
// Gets the cores tied to the current thread, or return all available cpu's if no cores are tied
// pub fn get_current_affinity() -> Vec<usize> {
//     return vec![0 as usize];
// }
// TODO:
// Gets the cores tied to other threads, or return all available cpu's if no cores have been tied
// pub fn get_other_thread_affinity(_pid: i32) -> Vec<usize> {
//     return vec![0 as usize];
// }
