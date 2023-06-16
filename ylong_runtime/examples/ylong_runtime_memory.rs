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

//! Benchmarks for memory usage, computed by difference between virtual rss printed.

#[cfg(unix)]
use std::process;
#[cfg(unix)]
use std::process::Command;
#[cfg(unix)]
use ylong_runtime::builder::RuntimeBuilder;
#[cfg(unix)]
use ylong_runtime::executor::Runtime;

#[cfg(unix)]
fn ylong_runtime() -> Runtime {
    RuntimeBuilder::new_multi_thread().build().unwrap()
}

#[cfg(unix)]
fn get_memory_info() {
    let pid = process::id();
    println!("pid {pid:}");
    let cmd = format!("/proc/{pid:}/status");
    println!("cmd {cmd}");
    let result = Command::new("cat")
        .arg(cmd)
        .output()
        .expect("fail to execute");
    let out = std::str::from_utf8(&result.stdout).unwrap();
    println!("status: \n{out}");
}

#[cfg(unix)]
macro_rules! memory {
    ($runtime: ident) => {
        println!("Before building runtime: ");
        get_memory_info();
        let runtime = $runtime();
        println!("After building runtime: ");
        get_memory_info();

        runtime.block_on(async move {
            println!("hello world");
        });
    };
}

fn main() {
    println!("ylong:");
    #[cfg(unix)]
    memory!(ylong_runtime);
}
