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

mod helpers;
use helpers::*;
use ylong_runtime::builder::RuntimeBuilder;
use ylong_runtime::task::TaskBuilder;

#[cfg(feature = "time")]
use ylong_runtime::time;
const SPAWN_NUM: usize = 100;
const THREAD_NUM: usize = 10;

/// SDV test for concurrently spawning tasks on the singleton runtime, through runtime instance.
///
/// # Title
/// sdv_concurrently_runtime_spawn_async_in_async_task
///
/// # Brief
/// 1. Spawn multiple threads and build runtime together.
/// 2. Spawn async tasks that have other async calls in them.
/// 3. Check if they are built and run successfully.
#[test]
fn sdv_concurrently_runtime_spawn_async_in_async_task() {
    let mut thread_handlers = vec![];
    for _ in 0..THREAD_NUM {
        let thread_handler = std::thread::spawn(move || {
            let mut handles = Vec::with_capacity(SPAWN_NUM);
            for i in 0..SPAWN_NUM {
                handles.push(ylong_runtime::spawn(test_async_in_async(i, i + 1)));
            }
            for (times, handle) in handles.into_iter().enumerate() {
                let ret = ylong_runtime::block_on(handle);
                assert_eq!(ret.unwrap(), (times, times + 1));
            }
        });
        thread_handlers.push(thread_handler);
    }
    for h in thread_handlers {
        h.join().unwrap();
    }
}

/// SDV test for concurrently spawning tasks on the singleton runtime, through ylong_runtime::spawn.
///
/// # Title
/// sdv_concurrently_spawn_async_in_async_task
///
/// # Brief
/// 1. Spawn multiple threads and build runtime together.
/// 2. Spawn async tasks that have other async calls in them.
/// 3. Check if they are built and run successfully.
#[test]
fn sdv_concurrently_spawn_async_in_async_task() {
    let mut thread_handlers = vec![];
    for _ in 0..THREAD_NUM {
        let thread_handler = std::thread::spawn(move || {
            let mut handles = Vec::with_capacity(SPAWN_NUM);
            for i in 0..SPAWN_NUM {
                handles.push(ylong_runtime::spawn(test_async_in_async(i, i + 1)));
            }
            for (times, handle) in handles.into_iter().enumerate() {
                let ret = ylong_runtime::block_on(handle);
                assert_eq!(ret.unwrap(), (times, times + 1));
            }
        });
        thread_handlers.push(thread_handler);
    }
    for h in thread_handlers {
        h.join().unwrap();
    }
}

/// SDV test for concurrently spawning tasks on the singleton runtime, through task builder instance.
///
/// # Title
/// sdv_concurrently_task_builder_spawn_async_in_async_task
///
/// # Brief
/// 1. Spawn multiple threads and build runtime together.
/// 2. Spawn async tasks that have other async calls in them.
/// 3. Check if they are built and run successfully.
#[test]
fn sdv_concurrently_task_builder_spawn_async_in_async_task() {
    let mut thread_handlers = vec![];
    for _ in 0..THREAD_NUM {
        let thread_handler = std::thread::spawn(move || {
            let mut handles = Vec::with_capacity(SPAWN_NUM);
            let task_builder = TaskBuilder::new();
            for i in 0..SPAWN_NUM {
                handles.push(task_builder.spawn(test_async_in_async(i, i + 1)));
            }
            for (times, handle) in handles.into_iter().enumerate() {
                let ret = ylong_runtime::block_on(handle);
                assert_eq!(ret.unwrap(), (times, times + 1));
            }
        });
        thread_handlers.push(thread_handler);
    }
    for h in thread_handlers {
        h.join().unwrap();
    }
}

/// SDV test for blocking on a time sleep without initializing the runtime.
///
/// # Brief
/// 1. Construct a future that calls time::sleep
/// 2. Use ylong_runtime::block_on to await this future to completion
/// 3. Check future's return value
#[cfg(feature = "time")]
#[test]
fn sdv_global_block_on() {
    let ret = ylong_runtime::block_on(async move {
        time::sleep(std::time::Duration::from_millis(10)).await;
        0
    });
    assert_eq!(ret, 0)
}

/// SDV for setting the global runtime after starting the runtime
///
/// # Brief
/// 1. Use global runtime to spawn a task
/// 2. Configures the global runtime
/// 3. Check the error
#[test]
fn sdv_build_global_failed() {
    let _ = ylong_runtime::block_on(ylong_runtime::spawn(async move { 1 }));
    let ret = RuntimeBuilder::new_multi_thread()
        .worker_num(2)
        .max_blocking_pool_size(1)
        .build_global();
    assert!(ret.is_err());
}
