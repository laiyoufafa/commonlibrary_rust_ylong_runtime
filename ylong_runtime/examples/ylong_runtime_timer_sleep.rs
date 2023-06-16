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

//! Sleep usage in ylong_runtime.

use std::time::{Duration, Instant};
use ylong_runtime::builder::RuntimeBuilder;
#[cfg(feature = "time")]
use ylong_runtime::time::sleep;
use ylong_runtime::{block_on, spawn};

fn main() {
    let _runtime = RuntimeBuilder::new_multi_thread().build().unwrap();

    let handle_one = spawn(async {
        let start = Instant::now();
        sleep(Duration::new(1, 0)).await;
        println!("{:?}", Instant::now() - start);
    });
    let handle_two = spawn(async {
        let start = Instant::now();
        sleep(Duration::new(2, 0)).await;
        println!("{:?}", Instant::now() - start);
    });
    let handle_three = spawn(async {
        let start = Instant::now();
        sleep(Duration::new(3, 0)).await;
        println!("{:?}", Instant::now() - start);
    });
    block_on(handle_one).unwrap();
    block_on(handle_two).unwrap();
    block_on(handle_three).unwrap();

    println!("-------------------");

    block_on(async move {
        let start = Instant::now();
        let mut times = 10;
        while times > 0 {
            sleep(Duration::new(0, 200_000_000)).await;
            println!("{:?}", Instant::now() - start);
            times -= 1;
        }
    });
}
