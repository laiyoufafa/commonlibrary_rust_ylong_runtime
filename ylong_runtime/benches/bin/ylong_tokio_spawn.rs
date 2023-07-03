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

//! Benchmarks for task memory usage, computed by difference between virtual rss printed.
use std::thread::{sleep, spawn};
use std::time::Duration;

#[cfg(unix)]
fn get_memory_info() {
    use std::process;
    use std::process::Command;

    let pid = process::id();
    println!("pid {pid}");
    let cmd = format!("{pid}");
    let result = Command::new("pmap")
        .arg("-x")
        .arg(cmd)
        .output()
        .expect("fail to execute");
    let mut out = String::from_utf8(result.stdout).unwrap();
    let pos1 = out.find("Mapping").unwrap();
    let pos2 = out.find("total").unwrap();
    out.drain(pos1 + 8..pos2);
    println!("status: \n{out}");
}

async fn async_task() {
    sleep(Duration::from_secs(5));
}

fn task() {
    sleep(Duration::from_secs(5));
}

#[cfg(unix)]
fn ylong_spawn() {
    let mut handles = Vec::with_capacity(1000);
    println!("Ylong Runtime Memory Test:");
    println!("=================Before=================");
    get_memory_info();
    for _ in 0..1000 {
        let handle = ylong_runtime::spawn(async_task());
        handles.push(handle);
    }
    println!("=================After=================");
    get_memory_info();
    for handle in handles {
        let _ = ylong_runtime::block_on(handle);
    }
}

#[cfg(unix)]
fn tokio_spawn() {
    let mut handles = Vec::with_capacity(1000);
    println!("Tokio Runtime Memory Test:");
    println!("=================Before=================");
    get_memory_info();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    for _ in 0..1000 {
        let handle = runtime.spawn(async_task());
        handles.push(handle);
    }
    println!("=================After=================");
    get_memory_info();
    for handle in handles {
        let _ = runtime.block_on(handle);
    }
}

#[cfg(unix)]
fn std_spawn() {
    let mut handles = Vec::with_capacity(1000);
    println!("Std Runtime Memory Test:");
    println!("=================Before=================");
    get_memory_info();
    for _ in 0..1000 {
        let handle = spawn(task);
        handles.push(handle);
    }
    println!("=================After=================");
    get_memory_info();
    for handle in handles {
        handle.join().unwrap();
    }
}

fn main() {
    tokio_spawn();
    ylong_spawn();
    std_spawn();
}
