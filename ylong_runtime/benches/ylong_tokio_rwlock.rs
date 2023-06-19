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

//! Benchmarks for the Rwlock.
//!
//! Designs of ylong_runtime benchmarks:
//! - Mainly follow designs of tokio rwlock tests.
//!
//! Designs of tokio benchmarks:
//! - Reference: [tokio/benches/sync_rwlock.rs](https://github.com/tokio-rs/tokio/blob/master/benches/sync_rwlock.rs)

#![feature(test)]

pub mod task_helpers;

#[cfg(test)]
mod rwlock_bench {
    extern crate test;

    pub use crate::task_helpers::{
        tokio_runtime, tokio_rwlock_task, tokio_rwlock_write_task, ylong_rwlock_task,
        ylong_rwlock_write_task,
    };
    use std::hint::black_box;
    use std::sync::Arc;
    use test::Bencher;

    use tokio::sync::RwLock;
    use ylong_runtime::sync::rwlock::RwLock as YlongRwlock;

    /// Benchmark test for tokio rwlock.
    ///
    /// # Title
    /// Tokio_rwlock_read_concurrent_uncontended_multi
    ///
    /// # Brief
    /// 1.Create a runtime with 6 threads.
    /// 2.Create a rwLock variable.
    /// 3.Concurrently read data for 6 times.
    #[bench]
    fn tokio_rwlock_read(b: &mut Bencher) {
        let rt = tokio_runtime();

        let lock = Arc::new(RwLock::new(()));
        b.iter(black_box(|| {
            let mut handlers = Vec::with_capacity(6);
            for _ in 0..16 {
                let lock = lock.clone();
                handlers.push(rt.spawn(tokio_rwlock_task(lock.clone())));
            }

            for handler in handlers {
                rt.block_on(handler).unwrap();
            }
        }));
    }

    /// Benchmark test for ylong rwlock.
    ///
    /// # Title
    /// Ylong_rwlock_read_concurrent_uncontended_multi
    ///
    /// # Brief
    /// 1.Create a runtime with 6 threads.
    /// 2.Create a rwLock variable.
    /// 3.Concurrently read data for 6 times.
    #[bench]
    fn ylong_rwlock_read(b: &mut Bencher) {
        let handle = ylong_runtime::spawn(async move {});
        let _ = ylong_runtime::block_on(handle);

        let lock = Arc::new(YlongRwlock::new(()));
        b.iter(black_box(move || {
            let mut handlers = Vec::with_capacity(6);
            for _ in 0..16 {
                let lock = lock.clone();
                handlers.push(ylong_runtime::spawn(ylong_rwlock_task(lock.clone())));
            }

            for handler in handlers {
                ylong_runtime::block_on(handler).unwrap();
            }
        }));
    }

    /// Benchmark test for tokio rwlock.
    ///
    /// # Title
    /// Tokio_rwlock_read_concurrent_uncontended_multi
    ///
    /// # Brief
    /// 1.Create a runtime with 6 threads.
    /// 2.Create a rwLock variable.
    /// 3.Concurrently read data for 6 times.
    #[bench]
    fn tokio_rwlock_write(b: &mut Bencher) {
        let rt = tokio_runtime();

        let lock = Arc::new(RwLock::new(()));
        b.iter(black_box(|| {
            let mut handlers = Vec::with_capacity(6);
            for _ in 0..16 {
                let lock = lock.clone();
                handlers.push(rt.spawn(tokio_rwlock_write_task(lock.clone())));
            }

            for handler in handlers {
                rt.block_on(handler).unwrap();
            }
        }));
    }

    /// Benchmark test for ylong rwlock.
    ///
    /// # Title
    /// Ylong_rwlock_read_concurrent_uncontended_multi
    ///
    /// # Brief
    /// 1.Create a runtime with 6 threads.
    /// 2.Create a rwLock variable.
    /// 3.Concurrently read data for 6 times.
    #[bench]
    fn ylong_rwlock_write(b: &mut Bencher) {
        let handle = ylong_runtime::spawn(async move {});
        let _ = ylong_runtime::block_on(handle);

        let lock = Arc::new(YlongRwlock::new(()));
        b.iter(black_box(move || {
            let mut handlers = Vec::with_capacity(6);
            for _ in 0..16 {
                let lock = lock.clone();
                handlers.push(ylong_runtime::spawn(ylong_rwlock_write_task(lock.clone())));
            }

            for handler in handlers {
                ylong_runtime::block_on(handler).unwrap();
            }
        }));
    }

    /// Benchmark test for tokio rwlock.
    ///
    /// # Title
    /// Tokio_rwlock_read_concurrent_contended_multi
    ///
    /// # Brief
    /// 1.Create a runtime with 6 threads.
    /// 2.Create a rwLock variable.
    /// 3.Write the rwlock
    /// 4.Concurrently read data for 5 times.
    #[bench]
    fn tokio_rwlock_write_read(b: &mut Bencher) {
        let rt = tokio_runtime();

        let lock = Arc::new(RwLock::new(()));
        b.iter(black_box(|| {
            let _lock_in = lock.clone();
            let mut handlers = Vec::with_capacity(12);

            for _ in 0..16 {
                let lock_in = lock.clone();
                handlers.push(rt.spawn(tokio_rwlock_write_task(lock_in)));
            }

            for _ in 0..128 {
                let lock_in = lock.clone();
                handlers.push(rt.spawn(tokio_rwlock_task(lock_in)));
            }
            for handler in handlers {
                rt.block_on(handler).unwrap();
            }
        }));
    }

    /// Benchmark test for ylong rwlock.
    ///
    /// # Title
    /// Ylong_rwlock_read_concurrent_contended_multi
    ///
    /// # Brief
    /// 1.Create a runtime with 6 threads.
    /// 2.Create a rwLock variable.
    /// 3.Write the rwlock
    /// 4.Concurrently read data for 5 times.
    #[bench]
    fn ylong_rwlock_write_read(b: &mut Bencher) {
        let handle = ylong_runtime::spawn(async move {});
        let _ = ylong_runtime::block_on(handle);

        let lock = Arc::new(YlongRwlock::new(()));
        b.iter(black_box(|| {
            let _lock_in = lock.clone();
            let mut handlers = Vec::with_capacity(12);

            for _ in 0..16 {
                let lock_in = lock.clone();
                handlers.push(ylong_runtime::spawn(ylong_rwlock_write_task(lock_in)));
            }

            for _ in 0..128 {
                let lock_in = lock.clone();
                handlers.push(ylong_runtime::spawn(ylong_rwlock_task(lock_in)));
            }

            for handler in handlers {
                ylong_runtime::block_on(handler).unwrap();
            }
        }));
    }
}
