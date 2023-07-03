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

use std::sync::{Arc, Mutex};
use ylong_runtime::sync::Mutex as YlongMutex;

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::{Acquire, Release};
use std::task::{Context, Poll};
use std::thread;
use std::thread::sleep;
use std::time::Duration;
use ylong_runtime::sync::Waiter;

use ylong_runtime::sync::RwLock;

const NUM: usize = 200;

pub struct TestFuture {
    value: usize,
    total: usize,
}

pub fn create_new(t: usize) -> TestFuture {
    TestFuture { value: 0, total: t }
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
    create_new(1000).await
}

/// SDV test for `Mutex`.
///
/// # Title
/// sdv_concurrency_with_mutex1
///
/// # Brief
/// 1.Create Runtime.
/// 2.Create a variable of Mutex.
/// 3.Executing an async task and change the variable.
/// 4.Check if the test results are correct.
#[test]
fn sdv_concurrency_with_mutex1() {
    ylong_runtime::block_on(async {
        let mutex1 = Arc::new(YlongMutex::new(0));
        let mutex2 = mutex1.clone();
        let mutex3 = mutex1.clone();
        let mutex4 = mutex1.clone();
        let mutex5 = mutex1.clone();

        ylong_runtime::spawn(async move {
            let mut n = mutex1.lock().await;
            *n += 1;
            assert_eq!(1, *n);
        })
        .await
        .unwrap();

        ylong_runtime::spawn(async move {
            let mut n = mutex2.lock().await;
            *n += 2;
            assert_eq!(3, *n);
        })
        .await
        .unwrap();

        ylong_runtime::spawn(async move {
            let mut n = mutex3.lock().await;
            *n += 3;
            assert_eq!(6, *n);
        })
        .await
        .unwrap();

        ylong_runtime::spawn(async move {
            let mut n = mutex4.lock().await;
            *n += 4;
            assert_eq!(10, *n);
        })
        .await
        .unwrap();

        ylong_runtime::spawn(async move {
            let mut n = mutex5.lock().await;
            *n += 5;
            assert_eq!(15, *n);
        })
        .await
        .unwrap();
    });
}

/// SDV test for `Mutex`.
///
/// # Title
/// sdv_concurrency_with_mutex2
///
/// # Brief
/// 1.Create Runtime.
/// 2.Create a variable of Mutex.
/// 3.Executing an async task and change the variable for 100 times.
/// 4.Check if the test results are correct.
#[test]
fn sdv_concurrency_with_mutex2() {
    ylong_runtime::block_on(async {
        let mutex1 = Arc::new(YlongMutex::new(0));
        for _ in 0..200 {
            let mutex = mutex1.clone();
            ylong_runtime::spawn(async move {
                let mut n = mutex.lock().await;
                *n += 1;
            })
            .await
            .unwrap();
        }
        let mutex = mutex1.clone();
        let n = mutex.lock().await;
        assert_eq!(*n, 200);
    });
}

/// SDV test for `Mutex`.
///
/// # Title
/// sdv_concurrency_with_mutex3
///
/// # Brief
/// 1.Create Runtime.
/// 2.Executing async tasks and Change the variable for 5 times
/// 3.Check if the test results are correct.
#[test]
fn sdv_concurrency_with_mutex3() {
    ylong_runtime::block_on(async {
        let mutex1 = Arc::new(YlongMutex::new(0));
        for _ in 0..5 {
            let m1 = mutex1.clone();
            ylong_runtime::spawn(async move {
                let mut n = m1.lock().await;
                sleep(Duration::new(0, 400));
                *n += 1;
            });
        }

        sleep(Duration::new(0, 200));
        loop {
            if *mutex1.lock().await >= 5 {
                break;
            }
        }
        let mutex = mutex1.clone();
        let n = mutex.lock().await;
        assert_eq!(*n, 5);
    });
}

/// SDV test for `Mutex`.
///
/// # Title
/// sdv_concurrency_with_mutex4
///
/// # Brief
/// 1.Create Runtime.
/// 2.Executing an async task which contains an async task and locking
/// 3.Check if the test results are correct.
#[test]
fn sdv_concurrency_with_mutex4() {
    let mutex1 = Arc::new(YlongMutex::new(0));
    // If test_future().await and the lock operation are put together to form a future,
    // there is a sequential relationship between the two futures,
    let mut handlers1 = Vec::with_capacity(NUM);
    let mut handlers2 = Vec::with_capacity(NUM);

    for _ in 0..200 {
        let mutex = mutex1.clone();
        handlers1.push(ylong_runtime::spawn(async move {
            //test_future() and locking do not exist concurrently.
            test_future().await;
            let mut n = mutex.lock().await;
            *n += 1;
        }));
    }

    for _ in 0..200 {
        handlers2.push(ylong_runtime::spawn(
            //test_future() and locking exist concurrently.
            test_future(),
        ));
    }

    for handler in handlers1 {
        let _ret = ylong_runtime::block_on(handler);
    }

    for handler in handlers2 {
        let _ret = ylong_runtime::block_on(handler);
    }
    ylong_runtime::block_on(async {
        let mutex = mutex1.clone();
        let n = mutex.lock().await;
        assert_eq!(*n, 200);
    });
}

/// SDV test for RwLock.
///
/// # Title
/// sdv_rwlock_multi_threads
///
/// # Brief
/// 1.Create producer_lock.
/// 2.Write for 100000 times.
/// 3.Two read thread to read for 100000 times.
/// 4.Check if the test results are correct.
#[test]
fn sdv_rwlock_multi_threads() {
    // Put the counter initial value into a read/write lock encapsulated
    // in an atomic reference to allow secure sharing.
    let producer_lock = Arc::new(std::sync::RwLock::new(1));
    // Create a clone for each thread
    let consumer_id_lock = producer_lock.clone();
    let consumer_square_lock = producer_lock.clone();
    let producer_thread = thread::spawn(move || {
        for _i in 0..100000 {
            // The write() method blocks the thread until exclusive access is available.
            if let Ok(mut write_guard) = producer_lock.write() {
                *write_guard += 1;
            }
        }
    });

    let mutex = Arc::new(Mutex::new(0));
    let mutex1 = mutex.clone();
    let mutex2 = mutex.clone();
    // read thread
    let consumer_id_thread = thread::spawn(move || {
        for _i in 0..100000 {
            // The read() method blocks only when the producer_thread thread holds a lock
            let guard = consumer_id_lock.read();
            drop(guard);

            let mut n = mutex1.lock().unwrap();
            *n += 1;
        }
    });

    // Another read thread
    let consumer_square_thread = thread::spawn(move || {
        for _i in 0..10000 {
            let guard = consumer_square_lock.read();
            drop(guard);
            let mut n = mutex2.lock().unwrap();
            *n += 1;
        }
    });
    let _ = producer_thread.join();
    let _ = consumer_id_thread.join();
    let _ = consumer_square_thread.join();
    assert_eq!(*mutex.lock().unwrap(), 110000);
}

/// SDV test for RwLock read.
///
/// # Title
/// sdv_rwlock_with_read1
///
/// # Brief
/// 1.Create a variable of Rwlock.
/// 2.Create Runtime.
/// 3.Executing an async task for 200 times.
/// 4.Check if the test results are correct.
#[test]
fn sdv_rwlock_with_read1() {
    let rwlock = Arc::new(RwLock::new(5));
    let mut future0_handlers = Vec::with_capacity(NUM);
    let mut future1_handlers = Vec::with_capacity(NUM);

    for _ in 0..200 {
        let rwlock = rwlock.clone();
        future0_handlers.push(ylong_runtime::spawn(async move {
            let n = rwlock.read().await;
            assert_eq!(5, *n);
        }));
    }
    for _ in 0..200 {
        future1_handlers.push(ylong_runtime::spawn(test_future()));
    }
    for handler in future0_handlers {
        let _ret = ylong_runtime::block_on(handler);
    }
    for handler in future1_handlers {
        let _ret = ylong_runtime::block_on(handler);
    }
}

/// SDV test for read lock.
///
/// # Title
/// sdv_rwlock_with_read2
///
/// # Brief
/// 1.Create a variable of Rwlock.
/// 2.Create Runtime.
/// 3.Read for 200 times.
/// 4.Check if the test results are correct.
#[test]
fn sdv_rwlock_with_read2() {
    let rwlock = Arc::new(RwLock::new(5));
    let mut handlers = Vec::with_capacity(NUM);

    for _ in 0..200 {
        let rwlock = rwlock.clone();
        handlers.push(ylong_runtime::spawn(async move {
            let n = rwlock.read().await;
            assert_eq!(5, *n);
        }));
    }
    for handler in handlers {
        ylong_runtime::block_on(handler).expect("block_on failed");
    }
}

/// SDV test for read-write lock.
///
/// # Title
/// sdv_rwlock_read_and_write
///
/// # Brief
/// 1.Create a variable of Rwlock.
/// 2.Create Runtime.
/// 3.Executing an async task.
/// 3.Read and write for 200 times.
/// 4.Check if the test results are correct.
#[test]
fn sdv_rwlock_read_and_write() {
    let rwlock = Arc::new(RwLock::new(5));
    let mut future0_handlers = Vec::with_capacity(NUM);
    let mut future1_handlers = Vec::with_capacity(NUM);
    for _ in 0..200 {
        let rwlock = rwlock.clone();
        future0_handlers.push(ylong_runtime::spawn(async move {
            let mut n = rwlock.write().await;
            *n += 1;
        }));
    }
    for _ in 0..200 {
        let rwlock = rwlock.clone();
        future0_handlers.push(ylong_runtime::spawn(async move {
            let _n = rwlock.read().await;
        }));
    }
    for _ in 0..200 {
        future1_handlers.push(ylong_runtime::spawn(test_future()));
    }
    for handler in future0_handlers {
        ylong_runtime::block_on(handler).expect("block_on failed");
    }
    for handler in future1_handlers {
        let _ret = ylong_runtime::block_on(handler).expect("block_on failed");
    }
    ylong_runtime::block_on(async {
        let mutex = rwlock.clone();
        let n = mutex.read().await;
        assert_eq!(*n, 205);
    });
}

/// SDV test for Rwlock.
///
/// # Title
/// sdv_rwlock_with_write1
///
/// # Brief
/// 1.Create a variable of Rwlock.
/// 2.Create Runtime.
/// 3.Write and execute another task for 200 times.
/// 4.Check if the test results are correct.
#[test]
fn sdv_rwlock_with_write1() {
    let rwlock = Arc::new(RwLock::new(5));
    let mut future0_handlers = Vec::with_capacity(NUM);
    let mut future1_handlers = Vec::with_capacity(NUM);

    for _ in 0..200 {
        let rwlock = rwlock.clone();
        future0_handlers.push(ylong_runtime::spawn(async move {
            let mut n = rwlock.write().await;
            *n += 1;
        }));
    }
    for _ in 0..200 {
        future1_handlers.push(ylong_runtime::spawn(test_future()));
    }
    for handler in future0_handlers {
        let _ret = ylong_runtime::block_on(handler);
    }
    for handler in future1_handlers {
        let _ret = ylong_runtime::block_on(handler);
    }
    ylong_runtime::block_on(async {
        let mutex = rwlock.clone();
        let n = mutex.read().await;
        assert_eq!(*n, 205);
    });
}

/// SDV test for Rwlock.
///
/// # Title
/// sdv_rwlock_with_write2
///
/// # Brief
/// 1.Create a variable of Rwlock.
/// 2.Create Runtime.
/// 3.Write for 200 times.
/// 4.Check if the test results are correct.
#[test]
fn sdv_rwlock_with_write2() {
    let rwlock = Arc::new(RwLock::new(5));
    let mut handlers = Vec::with_capacity(NUM);

    for _ in 0..200 {
        let rwlock = rwlock.clone();
        handlers.push(ylong_runtime::spawn(async move {
            let mut n = rwlock.write().await;
            *n += 1;
        }));
    }
    for handler in handlers {
        ylong_runtime::block_on(handler).expect("block_on failed");
    }
    ylong_runtime::block_on(async {
        let mutex = rwlock.clone();
        let n = mutex.read().await;
        assert_eq!(*n, 205);
    });
}

/// SDV test for `Waiter::wake_one()`.
///
/// # Title
/// sdv_waiter_with_wake_one
///
/// # Brief
/// 1.Call `wake_one` before a task calling `wait`.
/// 2.Call `wake_one` after a task calling `wait`.
#[test]
fn sdv_waiter_with_wake_one() {
    let waiter = Arc::new(Waiter::new());
    let waiter2 = waiter.clone();
    let waiter3 = waiter.clone();
    let waiter4 = waiter.clone();

    let handle = ylong_runtime::spawn(async move {
        waiter.wake_one();
        waiter2.wait().await;
    });

    let _ = ylong_runtime::block_on(handle);

    let handle2 = ylong_runtime::spawn(async move {
        waiter3.wait().await;
    });

    let handle3 = ylong_runtime::spawn(async move {
        waiter4.wake_one();
    });

    let _ = ylong_runtime::block_on(handle2);
    let _ = ylong_runtime::block_on(handle3);
}

/// SDV test for `Waiter::wake_all()`.
///
/// # Title
/// sdv_waiter_with_wake_all
///
/// # Brief
/// 1.Call `wake_all` after some tasks calling `wait`.
#[test]
fn sdv_waiter_with_wake_all() {
    let waiter = Arc::new(Waiter::new());

    let num = Arc::new(AtomicUsize::new(0));

    for _ in 0..1000 {
        let waiter3 = waiter.clone();
        let num3 = num.clone();
        ylong_runtime::spawn(async move {
            waiter3.wait().await;
            num3.fetch_add(1, Release);
        });
    }

    while num.load(Acquire) < 1000 {
        waiter.wake_all();
    }
}
