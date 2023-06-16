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

use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::Arc;
use std::sync::{Condvar, Mutex};
use std::thread;

#[derive(Clone)]
pub(crate) struct Parker {
    inner: Arc<Inner>,
}

struct Inner {
    state: AtomicUsize,
    mutex: Mutex<bool>,
    condvar: Condvar,
}

const IDLE: usize = 0;
const PARKED_ON_CONDVAR: usize = 1;
#[cfg(feature = "net")]
const PARKED_ON_DRIVER: usize = 2;
const NOTIFIED: usize = 3;

impl Parker {
    pub(crate) fn new() -> Parker {
        Parker {
            inner: Arc::new(Inner {
                state: AtomicUsize::new(IDLE),
                mutex: Mutex::new(false),
                condvar: Condvar::new(),
            }),
        }
    }

    pub(crate) fn park(&mut self) {
        self.inner.park();
    }

    pub(crate) fn unpark(&self) {
        self.inner.unpark();
    }

    pub(crate) fn release(&self) {
        self.inner.release();
    }
}

impl Inner {
    fn park(&self) {
        // loop to reduce the chance of parking the thread
        for _ in 0..3 {
            if self
                .state
                .compare_exchange_weak(NOTIFIED, IDLE, SeqCst, SeqCst)
                .is_ok()
            {
                return;
            }
            thread::yield_now();
        }
        #[cfg(feature = "net")]
        if let Some(mut driver) = crate::net::Driver::try_get_mut() {
            self.park_on_driver(&mut driver);
            return;
        }

        self.park_on_condvar();
    }

    #[cfg(feature = "net")]
    fn park_on_driver(&self, driver: &mut crate::net::Driver) {
        match self
            .state
            .compare_exchange(IDLE, PARKED_ON_DRIVER, SeqCst, SeqCst)
        {
            Ok(_) => {}
            Err(NOTIFIED) => {
                self.state.swap(IDLE, SeqCst);
                return;
            }
            Err(actual) => panic!("inconsistent park state; actual = {}", actual),
        }

        driver.drive(None).expect("io driver failed");

        match self.state.swap(IDLE, SeqCst) {
            // got notified by real io events or not
            NOTIFIED | PARKED_ON_DRIVER => {}
            n => panic!("inconsistent park_timeout state: {}", n),
        }
    }

    fn park_on_condvar(&self) {
        let mut l = self.mutex.lock().unwrap();
        match self
            .state
            .compare_exchange(IDLE, PARKED_ON_CONDVAR, SeqCst, SeqCst)
        {
            Ok(_) => {}
            Err(NOTIFIED) => {
                // got a notification, exit parking
                self.state.swap(IDLE, SeqCst);
                return;
            }
            Err(actual) => panic!("inconsistent park state; actual = {}", actual),
        }

        loop {
            l = self.condvar.wait(l).unwrap();

            if self
                .state
                .compare_exchange(NOTIFIED, IDLE, SeqCst, SeqCst)
                .is_ok()
            {
                // got a notification, finish parking
                return;
            }
            // got spurious wakeup, go back to park again
        }
    }

    fn unpark(&self) {
        match self.state.swap(NOTIFIED, SeqCst) {
            IDLE | NOTIFIED => {}
            PARKED_ON_CONDVAR => {
                drop(self.mutex.lock());
                self.condvar.notify_one();
            }
            #[cfg(feature = "net")]
            PARKED_ON_DRIVER => crate::net::Handle::get_ref().wake(),
            actual => panic!("inconsistent state in unpark; actual = {}", actual),
        }
    }

    fn release(&self) {
        self.condvar.notify_all();
    }
}
