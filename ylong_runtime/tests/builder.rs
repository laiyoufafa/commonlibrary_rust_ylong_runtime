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

use std::sync::{Arc, Mutex};
use ylong_runtime::builder::RuntimeBuilder;

// async task
async fn test_future(num: usize) -> usize {
    num
}

/// SDV test for `after_start()`.
///
/// # Title
/// sdv_set_builder_after_start
///
/// # Brief
/// 1.Create Runtime.
/// 2.Sender calls after_start() to set variable x to 1.
/// 3.Executing an async task.
/// 4.Check if the test results are correct.
#[test]
fn sdv_set_builder_after_start() {
    let x = Arc::new(Mutex::new(0));
    let xc = x.clone();

    let runtime = RuntimeBuilder::new_multi_thread()
        .max_blocking_pool_size(4)
        .worker_num(8)
        .after_start(move || {
            let mut a = xc.lock().unwrap();
            *a = 1;
        })
        .build()
        .unwrap();

    let handle = runtime.spawn(test_future(1));
    let _result = runtime.block_on(handle).unwrap();

    let a = x.lock().unwrap();
    assert_eq!(*a, 1);
}

/// SDV test for `before_stop()`.
///
/// # Title
/// sdv_set_builder_before_stop
///
/// # Brief
/// 1.Create Runtime.
/// 2.Sender calls after_start() to set variable x to 1.
/// 3.Executing an async task.
/// 4.Check if the test results are correct.
#[test]
fn sdv_set_builder_before_stop() {
    let x = Arc::new(Mutex::new(0));
    let xc = x.clone();

    let runtime = RuntimeBuilder::new_multi_thread()
        .max_blocking_pool_size(4)
        .worker_num(8)
        .before_stop(move || {
            let mut a = xc.lock().unwrap();
            *a = 1;
        })
        .build()
        .unwrap();
    let handle = runtime.spawn(test_future(1));
    let _result = runtime.block_on(handle).unwrap();

    drop(runtime);
    let a = x.lock().unwrap();
    assert_eq!(*a, 1);
}
