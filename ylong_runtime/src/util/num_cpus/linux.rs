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

//! num_cpu linux wrapping

use libc::{sysconf, _SC_NPROCESSORS_CONF, _SC_NPROCESSORS_ONLN};
use std::os::raw::c_long;

/// Gets the number of CPU cores via linux syscall
///
/// # Example 2
///
/// ```rust
/// use ylong_runtime::util::num_cpus;
///
/// #[cfg(target_os = "linux")]
/// let cpus = num_cpus::linux::get_cpu_num_online();
///```
pub fn get_cpu_num_online() -> c_long {
    unsafe { sysconf(_SC_NPROCESSORS_ONLN) }
}

/// Gets the number of CPU cores via linux syscall, including disabled cpu states
///
/// # Example 2
///
/// ```rust
/// use ylong_runtime::util::num_cpus;
///
/// #[cfg(target_os = "linux")]
/// let cpus = num_cpus::linux::get_cpu_num_configured();
///```
pub fn get_cpu_num_configured() -> c_long {
    unsafe { sysconf(_SC_NPROCESSORS_CONF) }
}
