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

use std::cell::RefCell;
use std::hint;
use std::sync::atomic::AtomicU8;
use std::sync::atomic::Ordering::{AcqRel, Acquire, Release};
use std::task::Waker;

pub(crate) struct AtomicWaker {
    state: AtomicU8,
    waker: RefCell<Option<Waker>>,
}

/// Idle state
const IDLE: u8 = 0;

/// A new waker is registering.
const REGISTERING: u8 = 0b01;

/// The waker is being woken.
const WAKING: u8 = 0b10;

impl AtomicWaker {
    pub(crate) fn new() -> Self {
        Self {
            state: AtomicU8::new(IDLE),
            waker: RefCell::new(None),
        }
    }

    pub(crate) fn register_by_ref(&self, waker: &Waker) {
        match self
            .state
            .compare_exchange(IDLE, REGISTERING, Acquire, Acquire)
        {
            Ok(IDLE) => {
                self.waker.borrow_mut().replace(waker.clone());

                match self
                    .state
                    .compare_exchange(REGISTERING, IDLE, AcqRel, Acquire)
                {
                    Ok(_) => {}
                    // The state is REGISTERING | WAKING.
                    Err(_) => {
                        let waker = self.waker.borrow_mut().take().unwrap();
                        self.state.store(IDLE, Release);
                        waker.wake();
                    }
                }
            }
            Err(WAKING) => {
                waker.wake_by_ref();
                hint::spin_loop();
            }
            // The state is REGISTERING or REGISTERING | WAKING.
            _ => {}
        }
    }

    pub(crate) fn wake(&self) {
        if let Some(waker) = self.take_waker() {
            waker.wake();
        }
    }

    pub(crate) fn take_waker(&self) -> Option<Waker> {
        match self.state.fetch_or(WAKING, AcqRel) {
            IDLE => {
                let waker = self.waker.borrow_mut().take();
                self.state.fetch_and(!WAKING, Release);
                waker
            }
            // The state is REGISTERING or REGISTERING | WAKING or WAKING.
            _ => None,
        }
    }
}

unsafe impl Send for AtomicWaker {}
unsafe impl Sync for AtomicWaker {}
