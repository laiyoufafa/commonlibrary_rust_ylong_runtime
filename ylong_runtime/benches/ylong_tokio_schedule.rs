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

//! Benchmarks for task scheduling.
//!

#![feature(test)]

mod task_helpers;

#[macro_export]
macro_rules! tokio_schedule_task {
    ($runtime: expr, $bench: ident, $num: literal, $upper: literal) => {
        #[bench]
        fn $bench(b: &mut Bencher) {
            let runtime = $runtime;
            b.iter(black_box(|| {
                let mut handlers = Vec::with_capacity($num);
                for _ in 0..$num {
                    handlers.push(runtime.spawn(async move {
                        fibbo($upper);
                        yield_now().await;
                    }));
                }

                for handler in handlers {
                    let _ = runtime.block_on(handler).unwrap();
                }
            }));
        }
    };
}

#[macro_export]
macro_rules! ylong_schedule_task {
    ($bench: ident, $num: literal, $upper: literal) => {
        #[bench]
        fn $bench(b: &mut Bencher) {
            b.iter(black_box(|| {
                let mut handlers = Vec::with_capacity($num);
                for _ in 0..$num {
                    handlers.push(ylong_runtime::spawn(async move {
                        fibbo($upper);
                        yield_now().await;
                    }));
                }

                for handler in handlers {
                    let _ = ylong_runtime::block_on(handler).unwrap();
                }
            }));
        }
    };
}

#[cfg(test)]
mod tokio_schedule_bench {
    extern crate test;

    pub use crate::task_helpers::{fibbo, tokio_runtime};
    use std::hint::black_box;
    use test::Bencher;

    use ylong_runtime::task::yield_now;

    tokio_schedule_task!(tokio_runtime(), tokio_task_10_15, 10, 15);
    tokio_schedule_task!(tokio_runtime(), tokio_task_120_15, 120, 15);
    tokio_schedule_task!(tokio_runtime(), tokio_task_10_30, 10, 30);
    tokio_schedule_task!(tokio_runtime(), tokio_task_120_30, 120, 30);
}

#[cfg(test)]
mod ylong_schedule_bench {
    extern crate test;

    pub use crate::task_helpers::{fibbo, tokio_runtime};
    use std::hint::black_box;
    use test::Bencher;

    use ylong_runtime::task::yield_now;

    ylong_schedule_task!(ylong_task_10_15, 10, 15);
    ylong_schedule_task!(ylong_task_120_15, 120, 15);
    ylong_schedule_task!(ylong_task_10_30, 10, 30);
    ylong_schedule_task!(ylong_task_120_30, 120, 30);
}
