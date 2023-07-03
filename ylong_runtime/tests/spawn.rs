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

use ylong_runtime::builder::RuntimeBuilder;

mod helpers;
use helpers::*;

// One Core Test
#[test]
fn sdv_one_core_test() {
    let core_pool_size = 1;
    let num = 1000;

    let runtime = RuntimeBuilder::new_multi_thread()
        .worker_num(core_pool_size)
        .build()
        .unwrap();

    let mut handles = Vec::with_capacity(num);

    for i in 0..num {
        handles.push(runtime.spawn(test_future(i)));
    }

    for (times, handle) in handles.into_iter().enumerate() {
        let ret = runtime.block_on(handle);
        assert_eq!(ret.unwrap(), times);
    }
}

// Two-core test
#[test]
fn sdv_two_core_test() {
    let core_pool_size = 2;
    let num = 1000;

    let runtime = RuntimeBuilder::new_multi_thread()
        .worker_num(core_pool_size)
        .build()
        .unwrap();

    let mut handles = Vec::with_capacity(num);

    for i in 0..num {
        handles.push(runtime.spawn(test_future(i)));
    }

    for (times, handle) in handles.into_iter().enumerate() {
        let ret = runtime.block_on(handle);
        assert_eq!(ret.unwrap(), times);
    }
}

// Three Core Test
#[test]
fn sdv_three_core_test() {
    let core_pool_size = 3;
    let num = 1000;

    let runtime = RuntimeBuilder::new_multi_thread()
        .worker_num(core_pool_size)
        .build()
        .unwrap();

    let mut handles = Vec::with_capacity(num);

    for i in 0..num {
        handles.push(runtime.spawn(test_future(i)));
    }

    for (times, handle) in handles.into_iter().enumerate() {
        let ret = runtime.block_on(handle);
        assert_eq!(ret.unwrap(), times);
    }
}

// Four Core Test
#[test]
fn sdv_four_core_test() {
    let core_pool_size = 4;
    let num = 1000;

    let runtime = RuntimeBuilder::new_multi_thread()
        .worker_num(core_pool_size)
        .build()
        .unwrap();

    let mut handles = Vec::with_capacity(num);

    for i in 0..num {
        handles.push(runtime.spawn(test_future(i)));
    }

    for (times, handle) in handles.into_iter().enumerate() {
        let ret = runtime.block_on(handle);
        assert_eq!(ret.unwrap(), times);
    }
}

// Eight Core Test
#[test]
fn sdv_eight_core_test() {
    let core_pool_size = 8;
    let num = 1000;

    let runtime = RuntimeBuilder::new_multi_thread()
        .worker_num(core_pool_size)
        .build()
        .unwrap();

    let mut handles = Vec::with_capacity(num);

    for i in 0..num {
        handles.push(runtime.spawn(test_future(i)));
    }

    for (times, handle) in handles.into_iter().enumerate() {
        let ret = runtime.block_on(handle);
        assert_eq!(ret.unwrap(), times);
    }
}

// 64 Core Test, It is also the largest number of cores supported
#[test]
fn sdv_max_core_test() {
    let core_pool_size = 64;
    let num = 1000;

    let runtime = RuntimeBuilder::new_multi_thread()
        .worker_num(core_pool_size)
        .build()
        .unwrap();

    let mut handles = Vec::with_capacity(num);

    for i in 0..num {
        handles.push(runtime.spawn(test_future(i)));
    }

    for (times, handle) in handles.into_iter().enumerate() {
        let ret = runtime.block_on(handle);
        assert_eq!(ret.unwrap(), times);
    }
}

// Having multiple tasks in one `async` block
#[test]
fn sdv_multi_future_in_async() {
    let core_pool_size = 4;
    let num = 1000;

    let runtime = RuntimeBuilder::new_multi_thread()
        .worker_num(core_pool_size)
        .build()
        .unwrap();

    let mut handles = Vec::with_capacity(num);

    for i in 0..num {
        handles.push(runtime.spawn(test_multi_future_in_async(i, i + 1)));
    }

    for (times, handle) in handles.into_iter().enumerate() {
        let ret = runtime.block_on(handle);
        assert_eq!(ret.unwrap(), (times, times + 1));
    }
}

// Calling other `async` blocks within an `async` block has a multiple call relationship
#[test]
fn sdv_multi_async_in_async() {
    let core_pool_size = 4;
    let num = 1000;

    let runtime = RuntimeBuilder::new_multi_thread()
        .worker_num(core_pool_size)
        .build()
        .unwrap();

    let mut handles = Vec::with_capacity(num);

    for i in 0..num {
        handles.push(runtime.spawn(test_async_in_async(i, i + 1)));
    }

    for (times, handle) in handles.into_iter().enumerate() {
        let ret = runtime.block_on(handle);
        assert_eq!(ret.unwrap(), (times, times + 1));
    }
}
