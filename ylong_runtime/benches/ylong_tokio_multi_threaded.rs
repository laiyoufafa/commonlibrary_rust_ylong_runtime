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

//! Benchmarks for the threaded scheduler.
//!
//! Designs of ylong_runtime benchmarks:
//! - Mainly follow designs of tokio multi threaded tests.
//!
//! Designs of tokio benchmarks:
//! - Reference: [tokio/benches/multi_threaded.rs](https://github.com/tokio-rs/tokio/blob/master/benches/rt_multi_threaded.rs)
//! - Get changed to keep benchmarks in the same manner.
//! - Move ping-pong tests to channel related benchmarks.

#![feature(test)]

pub mod task_helpers;

#[macro_export]
macro_rules! runtime_spawn_function {
    () => {
        fn runtime_spawn_many(runtime: &Runtime, sender: mpsc::SyncSender<()>) {
            runtime.spawn(async move {
                sender.send(()).unwrap();
            });
        }

        fn runtime_yield_many(runtime: &Runtime, sender: mpsc::SyncSender<()>) {
            runtime.spawn(async move {
                for _ in 0..YIELD_NUM {
                    yield_now().await;
                }
                sender.send(()).unwrap();
            });
        }
    };
}

// Change not to use atomic variables, and thus avoid those costs.
#[macro_export]
macro_rules! spawn_yield_many {
    ($runtime: ident, $test: ident, $fn: ident) => {
        #[bench]
        fn $test(b: &mut Bencher) {
            let runtime = $runtime();

            let (send, recv) = mpsc::sync_channel(TASK_NUM);

            b.iter(black_box(|| {
                let task = || async {
                    for _ in 0..TASK_NUM {
                        $fn(&runtime, send.clone());
                    }
                };

                runtime.block_on(task());

                for _ in 0..TASK_NUM {
                    let _ = recv.recv().unwrap();
                }
            }));
        }
    };
}

#[cfg(test)]
mod ylong_multi_threaded {
    extern crate test;
    use crate::task_helpers::*;
    use test::Bencher;

    use std::hint::black_box;
    use std::sync::{mpsc, Arc};

    use ylong_runtime::executor::Runtime;
    use ylong_runtime::task::yield_now;

    runtime_spawn_function!();

    spawn_yield_many!(ylong_runtime, spawn_many, runtime_spawn_many);
    spawn_yield_many!(ylong_runtime, yield_many, runtime_yield_many);

    #[bench]
    fn chained_spawn(b: &mut Bencher) {
        fn iter(runtime: Arc<Runtime>, sender: mpsc::SyncSender<()>, n: usize) {
            if n == 0 {
                sender.send(()).unwrap();
            } else {
                let runtime_clone = runtime.clone();
                runtime.spawn(async move { iter(runtime_clone, sender, n - 1) });
            }
        }

        let runtime = Arc::new(ylong_runtime());
        let (send, recv) = mpsc::sync_channel(TASK_NUM);

        let runtime_clone = runtime.clone();
        b.iter(black_box(move || {
            let sender = send.clone();
            runtime.block_on(async {
                let runtime_iter_clone = runtime_clone.clone();
                runtime_clone.spawn(async move { iter(runtime_iter_clone, sender, TASK_NUM) });

                recv.recv().unwrap();
            });
        }));
    }
}

#[cfg(test)]
mod tokio_multi_threaded {
    extern crate test;
    use test::Bencher;

    use crate::task_helpers::*;

    use std::hint::black_box;
    use std::sync::mpsc;

    use tokio::runtime::Runtime;
    use tokio::task::yield_now;

    runtime_spawn_function!();

    spawn_yield_many!(tokio_runtime, spawn_many, runtime_spawn_many);
    spawn_yield_many!(tokio_runtime, yield_many, runtime_yield_many);

    #[bench]
    fn chained_spawn(b: &mut Bencher) {
        fn iter(sender: mpsc::SyncSender<()>, n: usize) {
            if n == 0 {
                sender.send(()).unwrap();
            } else {
                tokio::spawn(async move { iter(sender, n - 1) });
            }
        }

        let runtime = tokio_runtime();
        let (send, recv) = mpsc::sync_channel(TASK_NUM);

        b.iter(black_box(move || {
            let sender = send.clone();
            runtime.block_on(async {
                tokio::spawn(async move { iter(sender, TASK_NUM) });

                recv.recv().unwrap();
            });
        }));
    }
}
