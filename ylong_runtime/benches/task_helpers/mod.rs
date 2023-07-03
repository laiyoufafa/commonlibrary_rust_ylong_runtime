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

#![allow(dead_code)]
use std::hint::black_box;
use std::sync::Arc;
use tokio::sync::RwLock;
#[cfg(feature = "multi_instance_runtime")]
use ylong_runtime::builder::RuntimeBuilder;
#[cfg(feature = "multi_instance_runtime")]
use ylong_runtime::executor::Runtime;
#[cfg(feature = "sync")]
use ylong_runtime::sync::RwLock as YlongRwlock;

pub const TASK_NUM: usize = 10;
pub const FIBBO_ANS: u64 = 1346268;

const BUFFER_SIZE: usize = 4096;
pub const READ_FILE: &str = "/dev/zero";
pub const WRITE_FILE: &str = "/dev/null";
pub static mut READ_BUFFER: [u8; BUFFER_SIZE] = [0_u8; BUFFER_SIZE];
pub static mut WRITE_BUFFER: [u8; BUFFER_SIZE] = [0_u8; BUFFER_SIZE];

pub const YIELD_NUM: usize = 1_000;

pub type FA<R> = fn() -> R;

#[cfg(feature = "multi_instance_runtime")]
pub fn ylong_runtime() -> Runtime {
    RuntimeBuilder::new_multi_thread().build().unwrap()
}

#[cfg(feature = "multi_instance_runtime")]
pub fn ylong_runtime_set_threads(threads_count: u8) -> Runtime {
    RuntimeBuilder::new_multi_thread()
        .worker_num(threads_count)
        .build()
        .unwrap()
}

pub fn tokio_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
}

pub fn tokio_runtime_set_threads(threads_count: usize) -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(threads_count)
        .enable_all()
        .build()
        .unwrap()
}

pub fn work() -> u64 {
    black_box(3)
}

pub fn recur_fibbo(a: u64) -> u64 {
    if a == 0 || a == 1 {
        return a;
    }
    recur_fibbo(a - 1) + recur_fibbo(a - 2)
}

pub fn fibbo(upper: usize) -> u64 {
    let mut ret = 0;
    for j in 1..upper {
        ret += black_box(recur_fibbo(j as u64));
    }
    black_box(ret)
}

pub fn init_write_buffer() {
    let mut buffer = [0; BUFFER_SIZE];
    for (i, tar) in buffer.iter_mut().enumerate().take(BUFFER_SIZE) {
        *tar = (i % 256) as u8;
    }
    unsafe {
        WRITE_BUFFER
            .iter_mut()
            .zip(buffer.as_slice().iter())
            .for_each(|(tar, src)| *tar = *src);
    }
}

#[cfg(feature = "sync")]
pub async fn ylong_rwlock_write_task(lock: Arc<YlongRwlock<()>>) {
    let _read = lock.write().await;
}

#[cfg(feature = "sync")]
pub async fn ylong_rwlock_task(lock: Arc<YlongRwlock<()>>) {
    let _read = lock.read().await;
}

pub async fn tokio_rwlock_write_task(lock: Arc<RwLock<()>>) {
    let _read = lock.write().await;
}

pub async fn tokio_rwlock_task(lock: Arc<RwLock<()>>) {
    let _read = lock.read().await;
}
