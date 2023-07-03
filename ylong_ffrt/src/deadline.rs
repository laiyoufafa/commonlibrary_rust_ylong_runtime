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

use super::Qos;
use libc::c_void;

#[must_use]
/// Qos Interval.
pub struct QosInterval(*mut c_void);

impl QosInterval {
    /// Creates a new QosInterval.
    pub fn new(deadline_us: u64, qos: Qos) -> Self {
        unsafe {
            let p = ffrt_qos_interval_create(deadline_us, qos);
            ffrt_qos_interval_begin(p);
            QosInterval(p)
        }
    }

    /// Updates the interval with a new deadline.
    pub fn update(&mut self, new_deadline_us: u64) {
        unsafe {
            ffrt_qos_interval_update(self.0, new_deadline_us);
        }
    }
}

impl Drop for QosInterval {
    fn drop(&mut self) {
        unsafe {
            ffrt_qos_interval_end(self.0);
            ffrt_qos_interval_destroy(self.0);
        }
    }
}

unsafe impl Send for QosInterval {}

#[link(name = "ffrt")]
// deadline.h
extern "C" {
    fn ffrt_qos_interval_create(deadline_us: u64, qos: Qos) -> *mut c_void;
    fn ffrt_qos_interval_update(it: *mut c_void, new_deadline_us: u64);
    fn ffrt_qos_interval_begin(it: *mut c_void);
    fn ffrt_qos_interval_end(it: *mut c_void);
    fn ffrt_qos_interval_destroy(it: *mut c_void);
}
