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

//! Now `sdv_mutex` only has one test. In this test, `sleep` is called, requiring to initialize
//! time driver by `driver::Driver::get_mut_driver()`. This initialization will only be called
//! in `RuntimeBuilder::build()` when `net` feature is open. Therefore,
//! temporarily mark the whole test file so that it will only run when these features are open.
//! If more tests are added in the future, this conditionally compiling macro should be moved to
//! mark the test `sdv_mutex_lock_hold_longtime`.

#![cfg(all(feature = "sync", feature = "time"))]

use std::sync::Arc;
use std::thread;
use std::time::Duration;
use ylong_runtime::sync::Mutex;
use ylong_runtime::time;

/*
 * @title  UT test case for Mutex::lock() interface
 * @design The design of this use case is carried out by the conditional coverage test method.
 * @precon None
 * @brief  Test case execution steps：
 *         1. Create a Concurrent Mutual Exclusion Lock
 *         2. Make a concurrent process obtain a concurrent mutex lock and sleep after obtaining it to hold the lock for a long time
 *         3. The main thread creates a new concurrent thread to perform the locking operation, and then modifies the value in the lock after obtaining the lock
 *         4. Check the value in the lock
 * @expect data is 12
 * @auto   Yes
 */
#[test]
fn sdv_mutex_lock_hold_longtime() {
    let mutex = Arc::new(Mutex::new(10));
    let mutex1 = mutex.clone();
    let handle = ylong_runtime::spawn(async move {
        let mut lock = mutex1.lock().await;
        *lock += 1;
        time::sleep(Duration::new(1, 5000)).await;
    });
    thread::sleep(Duration::new(1, 00));
    ylong_runtime::block_on(async {
        let mut lock2 = mutex.lock().await;
        *lock2 += 1;
        assert_eq!(*lock2, 12);
    });
    let _ = ylong_runtime::block_on(handle);
}
