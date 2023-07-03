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

//! Benchmarks for async io operations.
//!
//! Designs of ylong_runtime benchmarks:
//! - Follow designs of tokio async io tests.
//!
//! Designs of tokio benchmarks:
//! - Reference: [tokio/benches/fs.rs](https://github.com/tokio-rs/tokio/blob/master/benches/fs.rs)
//! - Add new benchmarks to test write performance.

#![feature(test)]
#![cfg(unix)]
#![cfg(feature = "fs")]

pub mod task_helpers;

#[macro_export]
macro_rules! async_read {
    ($runtime: ident) => {
        #[bench]
        fn async_read(b: &mut Bencher) {
            let runtime = $runtime();

            b.iter(black_box(|| {
                let task = || async {
                    let mut file = File::open(READ_FILE).await.unwrap();

                    for _ in 0..TASK_NUM {
                        unsafe {
                            file.read_exact(&mut READ_BUFFER).await.unwrap();
                        }
                    }
                };

                runtime.block_on(task());
            }));
        }
    };
}

#[macro_export]
macro_rules! async_read_by_chars {
    ($runtime: ident) => {
        #[bench]
        fn async_read_by_chars(b: &mut Bencher) {
            let runtime = $runtime();

            b.iter(black_box(|| {
                let task = || async {
                    let mut file = File::open(READ_FILE).await.unwrap();
                    let mut buffer = [0_u8];

                    for i in 0..TASK_NUM {
                        unsafe {
                            file.read_exact(&mut buffer).await.unwrap();
                            READ_BUFFER[i] = buffer[0];
                        }
                    }
                };

                runtime.block_on(task());
            }));
        }
    };
}

#[macro_export]
macro_rules! async_write {
    ($runtime: ident) => {
        #[bench]
        fn async_write(b: &mut Bencher) {
            init_write_buffer();
            let runtime = $runtime();

            b.iter(black_box(|| {
                let task = || async {
                    let mut file = File::create(WRITE_FILE).await.unwrap();
                    for _ in 0..TASK_NUM {
                        unsafe {
                            let _ = file.write(&WRITE_BUFFER).await.unwrap();
                        }
                    }
                };

                runtime.block_on(task());
            }));
        }
    };
}

#[cfg(test)]
mod ylong_async_file {
    extern crate test;
    use crate::task_helpers::*;
    use test::Bencher;

    use std::hint::black_box;

    use ylong_runtime::fs::File;
    use ylong_runtime::io::{AsyncReadExt, AsyncWriteExt};

    async_read!(ylong_runtime);
    async_write!(ylong_runtime);
    async_read_by_chars!(ylong_runtime);
}

#[cfg(test)]
mod tokio_async_file {
    use tokio::fs::File;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    extern crate test;
    use test::Bencher;

    use crate::task_helpers::*;
    use std::hint::black_box;

    async_read!(tokio_runtime);
    async_write!(tokio_runtime);

    // For codec benchmarks, tokio has `async_read_codec` in `fs.rs`. That's the correct
    // ways to use tokio's codec features. However, here, use the same benchmark to apply
    // the same method to test ylong, tokio, swift, and avoid new packages imported.
    async_read_by_chars!(tokio_runtime);
}
