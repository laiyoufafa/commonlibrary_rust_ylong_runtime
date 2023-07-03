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

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::thread::sleep;
use ylong_runtime::builder::RuntimeBuilder;

//pidstat -p `pidof block_on-0fd2269f774d8981` -t 1
//cargo test --release  test_test_schedule(fuzzy matching test name) --  --nocapture
//cargo test --color=always Run all use cases (with comments) under the current workspace
//cargo test --color=always --package ylong_runtime  Run all test cases of a crate/package
//cargo test --color=always --package ylong_runtime --test block_on Run all use cases in block_on.rs
//cargo test --color=always --package ylong_runtime --test block_on single2_block_on Run a single use case for block_on.rs
//cargo test --color=always --package ylong_runtime --test block_on single2_block_on  --nocapture --no-fail-fast Print and ignore failures
//cargo test --color=always --package ylong_runtime --test block_on single2_block_on -- --exact Run accurately and solve scenarios of the same name
//cargo test -p ylong_runtime Select the module you want to test, the test case you want to test and the sample of how to write the cargo doc

#[test]
fn sdv_single1_block_on() {
    let runtime = RuntimeBuilder::new_multi_thread().build().unwrap();
    let res = runtime.block_on(async { 1 + 2 });
    assert_eq!(res, 3);
    let res = ylong_runtime::block_on(async { 1 + 2 });
    assert_eq!(res, 3);
}

#[test]
fn sdv_nest_block_on() {
    let runtime = RuntimeBuilder::new_multi_thread().build().unwrap();
    let res = runtime.block_on(async { runtime.block_on(async { 1 + 2 }) });
    assert_eq!(res, 3);
    let res = ylong_runtime::block_on(async { ylong_runtime::block_on(async { 1 + 2 }) });
    assert_eq!(res, 3);
}

#[test]
fn sdv_block_on_nest_spawn() {
    let runtime = RuntimeBuilder::new_multi_thread().build().unwrap();
    let res = runtime.block_on(async {
        let runtime = RuntimeBuilder::new_multi_thread().build().unwrap();
        runtime.spawn(async { 1 + 2 }).await.unwrap()
    });
    assert_eq!(res, 3);
    let res = ylong_runtime::block_on(async {
        let runtime = RuntimeBuilder::new_multi_thread().build().unwrap();
        runtime.spawn(async { 1 + 2 }).await.unwrap()
    });
    assert_eq!(res, 3);
}

#[test]
fn sdv_block_on_nest_spawn_spawn_blocking() {
    use std::time;

    async fn task() -> i32 {
        let runtime = RuntimeBuilder::new_multi_thread().build().unwrap();
        runtime
            .spawn(async {
                let runtime = RuntimeBuilder::new_multi_thread().build().unwrap();
                runtime
                    .spawn_blocking(|| {
                        sleep(time::Duration::from_secs(1));
                        3
                    })
                    .await
                    .unwrap()
            })
            .await
            .unwrap()
    }

    let runtime = RuntimeBuilder::new_multi_thread().build().unwrap();
    let res = runtime.block_on(task());
    assert_eq!(res, 3);
    let res = ylong_runtime::block_on(task());
    assert_eq!(res, 3);
}

#[test]
fn sdv_block_on_nest_spawn_and_spawn() {
    use std::time;

    let runtime = RuntimeBuilder::new_multi_thread().build().unwrap();

    async fn task() -> i32 {
        let runtime = RuntimeBuilder::new_multi_thread().build().unwrap();
        let x = runtime
            .spawn(async {
                let runtime = RuntimeBuilder::new_multi_thread().build().unwrap();
                runtime
                    .spawn_blocking(|| {
                        sleep(time::Duration::from_secs(1));
                        3
                    })
                    .await
                    .unwrap()
            })
            .await
            .unwrap();
        let y = runtime
            .spawn(async {
                let runtime = RuntimeBuilder::new_multi_thread().build().unwrap();
                runtime
                    .spawn_blocking(|| {
                        sleep(time::Duration::from_secs(1));
                        7
                    })
                    .await
                    .unwrap()
            })
            .await
            .unwrap();
        x + y
    }

    let res = runtime.block_on(task());
    assert_eq!(res, 10);
    let res = ylong_runtime::block_on(task());
    assert_eq!(res, 10);
}

#[test]
fn sdv_block_on_nest_spawn_nest_spawn() {
    use std::time;

    async fn task() -> i32 {
        let runtime = RuntimeBuilder::new_multi_thread().build().unwrap();
        let x = runtime
            .spawn(async {
                let runtime = RuntimeBuilder::new_multi_thread().build().unwrap();
                runtime.spawn(async { 7 }).await.unwrap()
            })
            .await
            .unwrap();
        let y = runtime
            .spawn(async {
                let runtime = RuntimeBuilder::new_multi_thread().build().unwrap();
                runtime
                    .spawn_blocking(|| {
                        sleep(time::Duration::from_secs(1));
                        7
                    })
                    .await
                    .unwrap()
            })
            .await
            .unwrap();
        x + y
    }

    let runtime = RuntimeBuilder::new_multi_thread().build().unwrap();
    let res = runtime.block_on(task());
    assert_eq!(res, 14);
    let res = ylong_runtime::block_on(task());
    assert_eq!(res, 14);
}

#[test]
fn sdv_block_on_nest_spawn_nest_spawn2() {
    async fn task() -> i32 {
        let runtime = RuntimeBuilder::new_multi_thread().build().unwrap();
        runtime
            .spawn(async {
                let runtime = RuntimeBuilder::new_multi_thread().build().unwrap();
                runtime.spawn(async { 7 }).await.unwrap()
            })
            .await
            .unwrap()
    }

    let runtime = RuntimeBuilder::new_multi_thread().build().unwrap();
    let res = runtime.block_on(task());
    assert_eq!(res, 7);
    let res = ylong_runtime::block_on(task());
    assert_eq!(res, 7);
}

#[test]
fn sdv_block_on_nest_batch_spawn() {
    async fn task() -> i32 {
        let cnt = 100;
        let mut res = 0;
        for _ in 0..cnt {
            let runtime = RuntimeBuilder::new_multi_thread().build().unwrap();
            res += runtime
                .spawn(async {
                    let runtime = RuntimeBuilder::new_multi_thread().build().unwrap();
                    runtime.spawn(async { 7 }).await.unwrap()
                })
                .await
                .unwrap();
        }
        res
    }

    let runtime = RuntimeBuilder::new_multi_thread().build().unwrap();
    let res = runtime.block_on(task());
    assert_eq!(res, 7 * 100);
    let res = ylong_runtime::block_on(task());
    assert_eq!(res, 7 * 100);
}

#[test]
fn sdv_block_on_nest_await_spawn() {
    async fn task() -> usize {
        let cnt = 100;
        let mut res = 0;
        for _ in 0..cnt {
            let runtime = RuntimeBuilder::new_multi_thread()
                .worker_num(2)
                .build()
                .unwrap();
            res += runtime
                .spawn(async {
                    let runtime = RuntimeBuilder::new_multi_thread().build().unwrap();
                    runtime.spawn(test_future()).await.unwrap()
                })
                .await
                .unwrap();
        }
        res
    }

    let runtime = RuntimeBuilder::new_multi_thread().build().unwrap();
    let res = runtime.block_on(task());
    assert_eq!(res, 100 * 1000);
    let res = ylong_runtime::block_on(task());
    assert_eq!(res, 100 * 1000);
}

// Problem 1: When all the above use cases run concurrently with the following use cases, there is a segment error. The simple analysis is that someone pops at the same time when stealing, which causes the fetching task to be empty, resulting in a segment error.
// Problem 2: When there are many concurrent use cases, such as more than 1000, it is impossible to wake up the thread where block_on is located.
#[test]
fn sdv_block_on_nest_await_spawn_bug_test() {
    async fn task() -> usize {
        let cnt = 1000;
        let mut res = 0;
        for _ in 0..cnt {
            let runtime = RuntimeBuilder::new_multi_thread().build().unwrap();
            res += runtime.spawn(test_future()).await.unwrap();
        }
        res
    }

    let runtime = RuntimeBuilder::new_multi_thread().build().unwrap();
    let res = runtime.block_on(task());
    assert_eq!(res, 1000 * 1000);
    let res = ylong_runtime::block_on(task());
    assert_eq!(res, 1000 * 1000);
}

pub struct TestFuture {
    value: usize,
    total: usize,
}

pub fn create_new() -> TestFuture {
    TestFuture {
        value: 0,
        total: 1000,
    }
}

impl Future for TestFuture {
    type Output = usize;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.total > self.value {
            self.get_mut().value += 1;
            cx.waker().wake_by_ref();
            Poll::Pending
        } else {
            Poll::Ready(self.total)
        }
    }
}

async fn test_future() -> usize {
    create_new().await
}
