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

//! Benchmarks for mutex.
//!
//! Designs of ylong_runtime benchmarks:
//! - Multiple threads are mutually exclusive to obtain the mutex and then rewrite the contents of the mutex for 10 times.
//! - Multiple threads are mutually exclusive to obtain the mutex and then rewrite the contents of the mutex for 100 times.
//! - Multiple threads are mutually exclusive to obtain the mutex and then rewrite the contents of the mutex for 1000 times.

#![feature(test)]

pub mod task_helpers;

#[macro_export]
macro_rules! tokio_mutex_task {
    ($runtime: expr, $bench: ident, $mutex: ident, $num: literal) => {
        #[bench]
        fn $bench(b: &mut Bencher) {
            let runtime = $runtime;

            b.iter(black_box(|| {
                let mutex = Arc::new($mutex::new(0));
                let mut handlers = Vec::with_capacity($num);
                for _ in 0..$num {
                    let mutex1 = mutex.clone();
                    handlers.push(runtime.spawn(async move {
                        let mut lock = mutex1.lock().await;
                        *lock += 1;
                    }));
                }

                for handler in handlers {
                    let _ = runtime.block_on(handler).unwrap();
                }
                let _res = runtime.block_on(async {
                    let n = mutex.lock().await;
                    assert_eq!(*n, $num);
                });
            }));
        }
    };
}

#[macro_export]
macro_rules! ylong_mutex_task {
    ($bench: ident, $mutex: ident, $num: literal) => {
        #[bench]
        fn $bench(b: &mut Bencher) {
            b.iter(black_box(|| {
                let mutex = Arc::new($mutex::new(0));
                let mut handlers = Vec::with_capacity($num);
                for _ in 0..$num {
                    let mutex1 = mutex.clone();
                    handlers.push(ylong_runtime::spawn(async move {
                        let mut lock = mutex1.lock().await;
                        *lock += 1;
                    }));
                }

                for handler in handlers {
                    let _ = ylong_runtime::block_on(handler).unwrap();
                }
                let _res = ylong_runtime::block_on(async {
                    let n = mutex.lock().await;
                    assert_eq!(*n, $num);
                });
            }));
        }
    };
}

#[cfg(test)]
mod mutex_bench {
    extern crate test;

    pub use crate::task_helpers::tokio_runtime;
    use std::hint::black_box;
    use std::sync::Arc;
    use test::Bencher;

    use tokio::sync::Mutex;
    use ylong_runtime::sync::Mutex as YlongMutex;

    ylong_mutex_task!(ylong_mutex_10, YlongMutex, 10);
    ylong_mutex_task!(ylong_mutex_100, YlongMutex, 100);
    ylong_mutex_task!(ylong_mutex_1000, YlongMutex, 1000);
    tokio_mutex_task!(tokio_runtime(), tokio_mutex_10, Mutex, 10);
    tokio_mutex_task!(tokio_runtime(), tokio_mutex_100, Mutex, 100);
    tokio_mutex_task!(tokio_runtime(), tokio_mutex_1000, Mutex, 1000);
}
