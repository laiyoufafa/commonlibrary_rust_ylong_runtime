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

/// Configuration for ffrt executor.
pub struct Config(*mut c_void);

impl Config {
    /// Initializes a new ffrt config.
    pub fn new() -> Config {
        unsafe {
            let p = ffrt_config_create();
            ffrt_config_init(p); // should check return value
            Config(p)
        }
    }

    /// Set the number of workers for the runtime.
    pub fn set_cpu_worker_num(&mut self, num: i32) -> &mut Self {
        unsafe {
            ffrt_config_set_cpu_worker_num(self.0, num as c_int);
        }
        self
    }

    /// Set the stack size for each worker threads.
    pub fn set_stack_size(&mut self, stack_size: i32) -> &mut Self {
        unsafe {
            ffrt_config_set_stack_size(self.0, stack_size as c_int);
        }
        self
    }

    /// Sets stack memory protect for worker threads.
    pub fn set_stack_protect(&mut self, protect_type: i32) -> &mut Self {
        unsafe {
            ffrt_config_set_stack_protect(self.0, protect_type as c_int);
        }
        self
    }

    /// Set the schedule policy for worker threads.
    pub fn set_sched_policy(&mut self, sched_policy: i32) -> &mut Self {
        unsafe {
            ffrt_config_set_sched_policy(self.0, sched_policy as c_int);
        }
        self
    }
}

impl Drop for Config {
    fn drop(&mut self) {
        unsafe {
            ffrt_config_destroy(self.0);
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Config::new()
    }
}

// because Config has ownership for inner data
unsafe impl Send for Config {}

#[link(name = "ffrt")]
// config.h
extern "C" {
    fn ffrt_config_create() -> *mut c_void;
    fn ffrt_config_set_cpu_worker_num(cfg: *mut c_void, cpu_worker_num: c_int);
    fn ffrt_config_set_stack_size(cfg: *mut c_void, stack_size: c_int);
    fn ffrt_config_set_stack_protect(cfg: *mut c_void, stack_protect_type: c_int);
    fn ffrt_config_set_sched_policy(cfg: *mut c_void, sched_policy: c_int);
    fn ffrt_config_destroy(cfg: *mut c_void);
    fn ffrt_config_init(cfg: *mut c_void) -> c_int;
}
