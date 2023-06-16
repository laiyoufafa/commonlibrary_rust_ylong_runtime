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

use std::thread::sleep;
use std::time;
use ylong_runtime::builder::RuntimeBuilder;
use ylong_runtime::executor::Runtime;
use ylong_runtime::task::TaskBuilder;

fn test_spawn(runtime: &Runtime) {
    let num = 1000;

    let mut handles = Vec::with_capacity(num);
    for i in 0..num {
        handles.push(runtime.spawn_blocking(move || {
            sleep(time::Duration::from_millis(1));
            i
        }));
    }
    for (times, handle) in handles.into_iter().enumerate() {
        let ret = runtime.block_on(handle);
        assert_eq!(ret.unwrap(), times);
    }

    let mut handles = Vec::with_capacity(num);
    for i in 0..num {
        handles.push(ylong_runtime::spawn_blocking(move || {
            sleep(time::Duration::from_millis(1));
            i
        }));
    }

    for (times, handle) in handles.into_iter().enumerate() {
        let ret = runtime.block_on(handle);
        assert_eq!(ret.unwrap(), times);
    }

    let mut handles = Vec::with_capacity(num);
    let task_builder = TaskBuilder::new();
    for i in 0..num {
        handles.push(task_builder.spawn_blocking(move || {
            sleep(time::Duration::from_millis(1));
            i
        }));
    }

    for (times, handle) in handles.into_iter().enumerate() {
        let ret = runtime.block_on(handle);
        assert_eq!(ret.unwrap(), times);
    }
}

// One Core Test
#[test]
fn sdv_one_core_test() {
    let max_blocking_pool_size = 1;
    let blocking_permanent_thread_num = 1;

    let runtime = RuntimeBuilder::new_multi_thread()
        .max_blocking_pool_size(max_blocking_pool_size)
        .blocking_permanent_thread_num(blocking_permanent_thread_num)
        .build()
        .unwrap();

    test_spawn(&runtime);
}

// Second Core Test
#[test]
fn sdv_two_core_test() {
    let max_blocking_pool_size = 2;
    let blocking_permanent_thread_num = 2;

    let runtime = RuntimeBuilder::new_multi_thread()
        .max_blocking_pool_size(max_blocking_pool_size)
        .blocking_permanent_thread_num(blocking_permanent_thread_num)
        .build()
        .unwrap();

    test_spawn(&runtime);
}

// Three Core Test
#[test]
fn sdv_three_core_test() {
    let max_blocking_pool_size = 3;
    let blocking_permanent_thread_num = 3;

    let runtime = RuntimeBuilder::new_multi_thread()
        .max_blocking_pool_size(max_blocking_pool_size)
        .blocking_permanent_thread_num(blocking_permanent_thread_num)
        .build()
        .unwrap();

    test_spawn(&runtime);
}

// Four resident threads test
#[test]
fn sdv_four_core_test() {
    let max_blocking_pool_size = 4;

    let runtime = RuntimeBuilder::new_multi_thread()
        .max_blocking_pool_size(max_blocking_pool_size)
        .build()
        .unwrap();

    test_spawn(&runtime);
}

// Eight Core Test
#[test]
fn sdv_eight_core_test() {
    let max_blocking_pool_size = 8;

    let runtime = RuntimeBuilder::new_multi_thread()
        .max_blocking_pool_size(max_blocking_pool_size)
        .build()
        .unwrap();

    test_spawn(&runtime);
}

// 64-core test, which is also the maximum number of cores supported
#[test]
fn sdv_max_core_test() {
    let max_blocking_pool_size = 64;

    let runtime = RuntimeBuilder::new_multi_thread()
        .max_blocking_pool_size(max_blocking_pool_size)
        .build()
        .unwrap();

    test_spawn(&runtime);
}

#[test]
fn sdv_complex_task_test() {
    let max_blocking_pool_size = 4;

    let runtime = RuntimeBuilder::new_multi_thread()
        .max_blocking_pool_size(max_blocking_pool_size)
        .build()
        .unwrap();

    test_spawn(&runtime);
}
