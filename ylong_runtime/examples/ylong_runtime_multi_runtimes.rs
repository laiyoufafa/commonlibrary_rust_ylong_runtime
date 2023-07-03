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

//! Multi_thread RuntimeBuilder usage.
#![cfg(not(feature = "ffrt"))]
use ylong_runtime::builder::RuntimeBuilder;

// Simple asynchronous tasks
async fn test_future(num: usize) -> usize {
    num
}

fn main() {
    // Creates multi-instance asynchronous thread pools
    // Sets the maximum capacity of the asynchronous thread pool
    let core_pool_size = 4;
    // Sets whether the asynchronous thread pool is tied for core processing
    let is_affinity = true;
    // Creates runtime environment first time
    let runtime_one = RuntimeBuilder::new_multi_thread()
        .is_affinity(is_affinity)
        .worker_num(core_pool_size)
        .build()
        .unwrap();

    // Sets the maximum capacity of the asynchronous thread pool
    let core_pool_size = 4;
    // Sets whether the asynchronous thread pool is tied for core processing
    let is_affinity = true;
    // Creates runtime environment second time (only asynchronous thread pools are supported)
    let runtime_two = RuntimeBuilder::new_multi_thread()
        .is_affinity(is_affinity)
        .worker_num(core_pool_size)
        .build()
        .unwrap();

    // Gets the respective task hooks
    let handle_one = runtime_one.spawn(test_future(1));
    let handle_two = runtime_two.spawn(test_future(2));

    // Gets task execution results via hooks
    let _result_one = runtime_one.block_on(handle_one).unwrap();
    let _result_two = runtime_two.block_on(handle_two).unwrap();
}
