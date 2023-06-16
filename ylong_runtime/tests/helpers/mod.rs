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

use libc::getpid;
use std::ffi::OsString;
use std::fs;

pub async fn test_future(num: usize) -> usize {
    num
}

pub async fn test_multi_future_in_async(i: usize, j: usize) -> (usize, usize) {
    let result_one = test_future(i).await;
    let result_two = test_future(j).await;

    (result_one, result_two)
}

pub async fn test_async_in_async(i: usize, j: usize) -> (usize, usize) {
    test_multi_future_in_async(i, j).await
}

// Gets the pid of all current threads (including the main thread)
#[allow(dead_code)]
pub(crate) unsafe fn dump_dir() -> Vec<OsString> {
    let current_pid = getpid();
    let dir = format!("/proc/{}/task", current_pid.to_string().as_str());
    let mut result = Vec::new();

    for entry in fs::read_dir(dir.as_str()).expect("read failed") {
        result.push(entry.unwrap().file_name());
    }
    result
}
