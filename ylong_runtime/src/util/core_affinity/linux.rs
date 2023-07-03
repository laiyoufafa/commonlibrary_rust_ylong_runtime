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

//! Wraps Linux core-affinity syscalls.

use crate::util::num_cpus::get_cpu_num;
use libc::{cpu_set_t, sched_getaffinity, sched_setaffinity, CPU_ISSET, CPU_SET};
use std::io::Error;
use std::io::Result;
use std::mem::{size_of, zeroed};

/// Sets the tied core cpu of the current thread.
///
/// sched_setaffinity function under linux
/// # Example
///
/// ```rust
/// use ylong_runtime::util::core_affinity;
///
/// let ret = core_affinity::set_current_affinity(0).is_ok();
/// ```
pub fn set_current_affinity(cpu: usize) -> Result<()> {
    let res: i32 = unsafe {
        let mut set = new_cpu_set();
        CPU_SET(cpu, &mut set);
        sched_setaffinity(0, size_of::<cpu_set_t>(), &set)
    };
    match res {
        0 => Ok(()),
        _ => Err(Error::last_os_error()),
    }
}

/// Gets the cores tied to the current thread,
/// or returns all available cpu's if no cores have been tied.
///
/// sched_setaffinity function under linux
/// # Example 1
///
/// ```rust
/// use ylong_runtime::util::core_affinity;
///
/// let cpus:Vec<usize> = core_affinity::get_current_affinity();
/// assert!(cpus.len() > 0);
/// ```
/// # Example 2
///
/// ```rust
/// use ylong_runtime::util::core_affinity;
///
/// let ret = core_affinity::set_current_affinity(0).is_ok();
/// assert!(ret);
/// let cpus:Vec<usize> = core_affinity::get_current_affinity();
/// assert_eq!(cpus.len(), 1);
/// ```
pub fn get_current_affinity() -> Vec<usize> {
    unsafe {
        let mut vec = vec![];
        let cpus = get_cpu_num() as usize;
        let mut set = new_cpu_set();
        sched_getaffinity(0, size_of::<cpu_set_t>(), &mut set);
        for i in 0..cpus {
            if CPU_ISSET(i, &set) {
                vec.push(i);
            }
        }
        vec
    }
}

/// Gets the cores bound to the specified thread, or return all available cpu's if no cores are bound
///
/// sched_setaffinity function under linux
pub fn get_other_thread_affinity(pid: i32) -> Vec<usize> {
    unsafe {
        let mut vec = vec![];
        let cpus = get_cpu_num() as usize;
        let mut set = new_cpu_set();
        sched_getaffinity(pid, size_of::<cpu_set_t>(), &mut set);
        for i in 0..cpus {
            if CPU_ISSET(i, &set) {
                vec.push(i);
            }
        }
        vec
    }
}

/// Returns an empty cpu set
fn new_cpu_set() -> cpu_set_t {
    unsafe { zeroed::<cpu_set_t>() }
}
