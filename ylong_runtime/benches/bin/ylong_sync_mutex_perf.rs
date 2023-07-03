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

//! Mutex usage in ylong_runtime.

use std::sync::Arc;
use std::time::Instant;
use ylong_runtime::sync::Mutex;

fn main() {
    let mutex = Arc::new(Mutex::new(0));
    // Testing custom future logic
    let handle = ylong_runtime::spawn(async move {
        let start = Instant::now();
        for _ in 0..400_0000 {
            let mut lock = mutex.lock().await;
            *lock += 1;
        }
        let end = Instant::now();
        println!("{:?}", end - start);
    });
    ylong_runtime::block_on(handle).unwrap();
}
