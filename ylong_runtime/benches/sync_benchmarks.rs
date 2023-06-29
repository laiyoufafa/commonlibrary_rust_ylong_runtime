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

//! Benchmarks for sync operations as performance baselines.

#![feature(test)]

pub mod task_helpers;

#[cfg(test)]
mod sync_benchmarks {
    extern crate test;
    use crate::task_helpers::*;
    #[cfg(unix)]
    use std::fs::File;
    use std::hint::black_box;
    #[cfg(unix)]
    use std::io::prelude::*;
    use std::sync::mpsc;
    use test::Bencher;

    #[bench]
    fn single_thread_run_1000_fibbo(b: &mut Bencher) {
        b.iter(black_box(|| {
            let handlers: Vec<[u8; TASK_NUM]> = black_box(Vec::with_capacity(TASK_NUM));
            for _ in 0..TASK_NUM {
                let ret = black_box(fibbo(30));
                assert_eq!(ret, FIBBO_ANS);
            }
            assert_eq!(handlers.len(), 0);
        }));
    }

    #[bench]
    fn single_thread_run_task(b: &mut Bencher) {
        b.iter(black_box(|| {
            let result = work();
            assert_eq!(result, 3);
        }))
    }

    #[cfg(unix)]
    #[bench]
    fn std_read_file(b: &mut Bencher) {
        b.iter(|| {
            let mut file = File::open(READ_FILE).unwrap();

            for _i in 0..TASK_NUM {
                unsafe {
                    file.read_exact(&mut READ_BUFFER).unwrap();
                }
            }
        });
    }

    #[cfg(unix)]
    #[bench]
    fn std_read_file_by_chars(b: &mut Bencher) {
        b.iter(|| {
            let mut file = File::open(READ_FILE).unwrap();
            let mut buffer = [0_u8];

            unsafe {
                for tar in READ_BUFFER.iter_mut().take(TASK_NUM) {
                    file.read_exact(&mut buffer).unwrap();
                    *tar = buffer[0];
                }
            }
        });
    }

    #[cfg(unix)]
    #[bench]
    fn std_write_file(b: &mut Bencher) {
        init_write_buffer();

        b.iter(black_box(|| {
            let mut file = File::create(WRITE_FILE).unwrap();
            for _i in 0..TASK_NUM {
                unsafe {
                    let _ = file.write(&WRITE_BUFFER).unwrap();
                }
            }
        }));
    }

    #[bench]
    fn std_multi_threaded_ping(b: &mut Bencher) {
        let (send, recv) = mpsc::sync_channel(TASK_NUM);

        b.iter(black_box(|| {
            let sender = send.clone();
            let _ = std::thread::spawn(move || {
                for _ in 0..TASK_NUM {
                    sender.send(()).unwrap();
                }
            });

            for _ in 0..TASK_NUM {
                recv.recv().unwrap();
            }
        }));
    }

    #[bench]
    fn std_multi_threaded_ping_pong(b: &mut Bencher) {
        let (send, recv) = mpsc::sync_channel(TASK_NUM);

        b.iter(black_box(|| {
            let done_send = send.clone();
            std::thread::spawn(move || {
                for _ in 0..TASK_NUM {
                    let (send1, recv1) = mpsc::sync_channel(TASK_NUM);
                    let (send2, recv2) = mpsc::sync_channel(TASK_NUM);

                    std::thread::spawn(move || {
                        recv1.recv().unwrap();
                        send2.send(()).unwrap();
                    });

                    send1.send(()).unwrap();
                    recv2.recv().unwrap();

                    done_send.send(()).unwrap();
                }
            });

            for _ in 0..TASK_NUM {
                recv.recv().unwrap();
            }
        }));
    }
}
