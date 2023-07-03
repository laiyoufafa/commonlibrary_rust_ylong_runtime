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

//! Utilities for tracking time.

mod driver;
mod error;
mod sleep;
mod timeout;
mod timer;
mod timer_handle;
mod wheel;

pub(crate) use driver::Driver;
pub use sleep::{sleep, sleep_until};
pub use timeout::timeout;
pub use timer::{periodic_schedule, timer, timer_at, Timer};

use crate::time::timer_handle::TimerHandle;
use std::ptr::NonNull;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;
use std::task::Waker;

// Struct for timing and waking up corresponding tasks on the timing wheel.
pub(crate) struct Clock {
    // Expected expiration time.
    expiration: u64,

    // The level to which the clock will be inserted.
    level: usize,

    // Elapsed time duration.
    duration: u64,

    // The result obtained when the corresponding Sleep structure is woken up by
    // which can be used to determine if the Future is completed correctly.
    result: AtomicBool,

    // Corresponding waker,
    // which is used to wake up sleep coroutine.
    waker: Option<Waker>,
}

impl Clock {
    // Creates a default Clock structure.
    pub(crate) fn new() -> Self {
        Self {
            expiration: 0,
            level: 0,
            duration: 0,
            result: AtomicBool::new(false),
            waker: None,
        }
    }

    // Returns the expected expiration time.
    pub(crate) fn expiration(&self) -> u64 {
        self.expiration
    }

    // Sets the expected expiration time
    pub(crate) fn set_expiration(&mut self, expiration: u64) {
        self.expiration = expiration;
    }

    // Returns the level to which the clock will be inserted.
    pub(crate) fn level(&self) -> usize {
        self.level
    }

    // Sets the level to which the clock will be inserted.
    pub(crate) fn set_level(&mut self, level: usize) {
        self.level = level;
    }

    pub(crate) fn duration(&self) -> u64 {
        self.duration
    }

    pub(crate) fn set_duration(&mut self, duration: u64) {
        self.duration = duration;
    }

    // Returns the corresponding waker.
    pub(crate) fn take_waker(&mut self) -> Option<Waker> {
        self.waker.take()
    }

    // Sets the corresponding waker.
    pub(crate) fn set_waker(&mut self, waker: Waker) {
        self.waker = Some(waker);
    }

    // Returns the result.
    pub(crate) fn result(&self) -> bool {
        self.result.load(Relaxed)
    }

    // Sets the result.
    pub(crate) fn set_result(&mut self, result: bool) {
        self.result.store(result, Relaxed);
    }

    // Creates a TimerHandle to point to the current structure.
    pub(crate) fn handle(&self) -> TimerHandle {
        TimerHandle {
            inner: NonNull::from(self),
        }
    }
}
