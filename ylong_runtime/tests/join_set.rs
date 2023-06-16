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

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::Arc;
use std::task::{Context, Poll};
use ylong_runtime::task::{JoinSet, PriorityLevel};

#[cfg(feature = "time")]
use std::time::Duration;
#[cfg(feature = "time")]
use ylong_runtime::error::ErrorKind;
#[cfg(feature = "time")]
use ylong_runtime::time::sleep;

/// SDV test for spawning and waiting for a simple future.
///
/// # Title
/// join_set_spawn_simple
///
/// # Brief
/// 1. Create a JoinSet
/// 2. Spawn a simple future that returns 1 immediately
/// 3. Asynchronously wait the task to finish using `join_next`
/// 4. Check the return value.
#[test]
fn sdv_join_set_spawn_simple() {
    let mut set = JoinSet::new();
    set.spawn(async move { 1 });
    let ret = ylong_runtime::block_on(set.join_next()).unwrap().unwrap();
    assert_eq!(ret, 1);
}

#[derive(Default)]
struct TestFuture {
    flag: bool,
}

impl Future for TestFuture {
    type Output = u8;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        if !this.flag {
            this.flag = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        } else {
            Poll::Ready(1)
        }
    }
}

/// SDV test for spawning and waiting for a future that returns pending once.
///
/// # Title
/// join_set_spawn_pending
///
/// # Brief
/// 1. Create a JoinSet
/// 2. Spawn a future that returns pending during first poll and ready for second poll
/// 3. Asynchronously wait the task to finish using `join_next`
/// 4. Check the return value.
#[test]
fn sdv_join_set_spawn_pending() {
    let mut set = JoinSet::new();
    set.spawn(TestFuture::default());
    let ret = ylong_runtime::block_on(set.join_next()).unwrap().unwrap();
    assert_eq!(ret, 1);
}

/// SDV test for spawning and waiting for two io tasks
///
/// # Brief
/// 1. Create a JoinSet
/// 2. Spawn a task as a tcp client
/// 3. Spawn a task as a tcp server
/// 3. Asynchronously wait the tasks to finish using `join_next`
/// 4. Check the return value.
#[cfg(feature = "net")]
#[test]
fn sdv_join_set_spawn_io() {
    use ylong_runtime::io::{AsyncReadExt, AsyncWriteExt};
    use ylong_runtime::net::{TcpListener, TcpStream};

    let mut set = JoinSet::new();
    let handle = ylong_runtime::spawn(async move {
        set.spawn(async move {
            let addr = "127.0.0.1:9001".parse().unwrap();
            let listener = TcpListener::bind(addr).await.unwrap();
            let mut server = listener.accept().await.unwrap().0;
            let mut buf = [0; 100];
            server.read_exact(&mut buf).await.unwrap();
            assert_eq!(buf, [1; 100]);
            server.write_all(&[3; 100]).await.unwrap();
            "server"
        });

        set.spawn(async move {
            let addr = "127.0.0.1:9001".parse().unwrap();
            let mut tcp = TcpStream::connect(addr).await;
            while tcp.is_err() {
                tcp = TcpStream::connect(addr).await;
            }
            let mut client = tcp.unwrap();
            client.write_all(&[1; 100]).await.unwrap();
            let mut buf = [0; 100];
            client.read_exact(&mut buf).await.unwrap();
            assert_eq!(buf, [3; 100]);
            "client"
        });

        let _ = set.join_next().await.unwrap().unwrap();
        let _ = set.join_next().await.unwrap().unwrap();
    });
    ylong_runtime::block_on(handle).unwrap();
}

/// SDV test for spawning and waiting for multiple tasks
///
/// # Brief
/// 1. Create a JoinSet
/// 2. Spawn 100 task that returns pending once
/// 3. Asynchronously wait all the tasks to finish using `join_next`
/// 4. Check the return value.
#[test]
fn sdv_join_set_spawn_multiple() {
    let mut set = JoinSet::new();
    let handle = ylong_runtime::spawn(async move {
        for _ in 0..100 {
            set.spawn(TestFuture::default());
        }
        for _ in 0..100 {
            let ret = set.join_next().await.unwrap().unwrap();
            assert_eq!(ret, 1);
        }
    });
    ylong_runtime::block_on(handle).unwrap();
}

/// SDV test for join_all
///
/// # Brief
/// 1. Create a JoinSet
/// 2. Spawn 100 tasks that fetch_add an atomic value for 10 times
/// 3. Call join_all()
/// 4. Check the atomic value.
#[test]
fn sdv_join_set_join_all() {
    let mut set = JoinSet::<u8>::new();

    let value = Arc::new(AtomicUsize::new(0));

    ylong_runtime::block_on(async move {
        for _ in 0..100 {
            let tmp = value.clone();
            set.spawn(async move {
                for _ in 0..10 {
                    tmp.fetch_add(1, Relaxed);
                }
                0
            });
        }
        set.join_all().await.unwrap();
        let val = value.load(Relaxed);
        assert_eq!(val, 1000);
    });
}

/// SDV test for CancelHandle
///
/// # Brief
/// 1. Create a JoinSet
/// 2. Spawn a task that sleeps a very long time
/// 4. Cancel the task via its CancelHandle
/// 5. Call join_next
/// 6. Check the return error
#[cfg(feature = "time")]
#[test]
fn sdv_join_set_cancel_one() {
    for _ in 0..10 {
        ylong_runtime::block_on(async move {
            let mut set = JoinSet::new();
            let handle = set.spawn(async move {
                sleep(Duration::from_secs(100000)).await;
                0
            });
            assert!(!handle.is_finished());
            handle.cancel();
            let ret = set.join_next().await.unwrap().unwrap_err();
            assert_eq!(ret.kind(), ErrorKind::TaskCanceled);
            assert!(handle.is_finished());
        });
    }
}

/// SDV test for CancelHandle
///
/// # Brief
/// 1. Create a JoinSet
/// 2. Spawn 100 tasks that sleep a very long time
/// 4. Cancel every task using cancel_all()
/// 5. Call join_next()
/// 6. Check the return error
#[test]
#[cfg(feature = "time")]
fn sdv_join_set_cancel_all() {
    let mut set = JoinSet::<u8>::new();

    ylong_runtime::block_on(async move {
        for _ in 0..100 {
            set.spawn(async move {
                sleep(Duration::from_secs(10000)).await;
                0
            });
        }
        set.cancel_all();
        for _ in 0..100 {
            let ret = set.join_next().await.unwrap().unwrap_err();
            assert_eq!(ret.kind(), ErrorKind::TaskCanceled);
        }
    });
}

/// SDV test for CancelHandle
///
/// # Brief
/// 1. Create a JoinSet
/// 2. Create a Builder
/// 3. Spawn 10 tasks via the Builder
/// 4. check return value
#[test]
fn sdv_join_set_builder() {
    let mut set = JoinSet::<u8>::new();
    ylong_runtime::block_on(async move {
        let mut builder = set
            .build_task()
            .name("hello".into())
            .priority(PriorityLevel::AbsHigh);
        for _ in 0..10 {
            let _ = builder.spawn(async move { 1 });
        }
        for _ in 0..10 {
            let ret = set.join_next().await.unwrap().unwrap();
            assert_eq!(ret, 1);
        }
    });
}

/// SDV test for CancelHandle
///
/// # Brief
/// 1. Create a JoinSet
/// 2. Spawn 100 tasks that sleep a very long time
/// 4. Shutdown the JoinSet
/// 5. Call join_next()
/// 6. Check the return error
#[cfg(feature = "time")]
#[test]
fn sdv_join_set_shutdown() {
    ylong_runtime::block_on(async move {
        let mut set = JoinSet::new();
        for _ in 0..100 {
            set.spawn(async move {
                sleep(Duration::from_secs(100000)).await;
            });
        }
        set.shutdown().await;
        let ret = set.join_next().await;
        assert!(ret.is_none());
    })
}

/// SDV test for JoinSet Drop to check memory leak
///
/// # Brief
/// 1. Create a JoinSet
/// 2. Spawn 100 tasks that sleep a very long time
/// 3. Drop the set before tasks complete
/// 4. Check asan for memory leak
#[cfg(feature = "time")]
#[test]
fn sdv_join_set_drop() {
    ylong_runtime::block_on(async move {
        let mut set = JoinSet::new();
        for _ in 0..100 {
            set.spawn(async move {
                sleep(Duration::from_secs(100000)).await;
            });
        }
        drop(set);
    });
}
