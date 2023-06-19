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

use crate::time::timer_handle::TimerHandle;
use crate::time::wheel::Wheel;
use std::convert::TryInto;
use std::fmt::Error;
use std::mem::MaybeUninit;
use std::sync::{Mutex, Once};
use std::task::Waker;
use std::time::Instant;

// Timer Driver
pub(crate) struct Driver {
    start_time: Instant,
    pub(crate) wheel: Mutex<Wheel>,
}

impl Driver {
    pub(crate) fn get_ref() -> &'static Self {
        static mut DRIVER: MaybeUninit<Driver> = MaybeUninit::uninit();
        static ONCE: Once = Once::new();

        unsafe {
            ONCE.call_once(|| {
                DRIVER.write(Self {
                    start_time: Instant::now(),
                    wheel: Mutex::new(Wheel::new()),
                });
            });

            &*DRIVER.as_ptr()
        }
    }

    pub(crate) fn start_time(&self) -> Instant {
        self.start_time
    }

    pub(crate) fn insert(&self, timer_handle: TimerHandle) -> Result<u64, Error> {
        let mut lock = self.wheel.lock().unwrap();
        lock.insert(timer_handle)
    }

    pub(crate) fn run(&self) {
        let now = Instant::now();
        let now = now
            .checked_duration_since(self.start_time())
            .unwrap()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX);

        let mut waker_list: [Option<Waker>; 32] = Default::default();
        let mut waker_idx = 0;

        let mut lock = self.wheel.lock().unwrap();

        while let Some(timer_handle) = lock.poll(now) {
            let elapsed = lock.elapsed();
            lock.set_last_elapsed(elapsed);

            // Unsafe access to timer_handle is only unsafe when Sleep Drop,
            // but does not let `Sleep` go to `Ready` before access to timer_handle fetched by poll.
            let timer_handle = unsafe { timer_handle.inner().as_mut() };
            waker_list[waker_idx] = timer_handle.take_waker();
            waker_idx += 1;

            timer_handle.set_result(true);

            if waker_idx == waker_list.len() {
                for waker in waker_list.iter_mut() {
                    waker.take().unwrap().wake();
                }

                waker_idx = 0;
            }
        }

        drop(lock);
        for waker in waker_list[0..waker_idx].iter_mut() {
            waker.take().unwrap().wake();
        }
    }
}
