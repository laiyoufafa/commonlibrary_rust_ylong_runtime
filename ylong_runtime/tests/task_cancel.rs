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

#![cfg(feature = "time")]

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use ylong_runtime::error::ErrorKind;
use ylong_runtime::time::sleep;

/// SDV test for canceling a task.
///
/// # Title
/// sdv_task_cancel_simple
///
/// # Brief
/// 1. Configure the RuntimeBuilder and start the runtime
/// 2. Spawn a task that takes 100 seconds
/// 3. Cancel the task after the task returns pending
/// 4. Await on the canceled task
/// 5. Check return value
#[test]
fn sdv_task_cancel_simple() {
    let handle = ylong_runtime::spawn(async move {
        let task = ylong_runtime::spawn(async move {
            sleep(Duration::from_secs(100)).await;
        });
        sleep(Duration::from_millis(100)).await;
        task.cancel();
        let res = task.await.err().unwrap();
        assert_eq!(res.kind(), ErrorKind::TaskCanceled);
    });
    ylong_runtime::block_on(handle).unwrap();
}

/// SDV test for canceling a task after its finished
///
/// # Title
/// sdv_task_cancel_failed
///
/// # Brief
/// 1. Configure the RuntimeBuilder and start the runtime
/// 2. Spawn a task that returns 1 immediately
/// 3. Cancel the task after the task is finished
/// 4. Await on the canceled task
/// 5. Check return value
#[test]
fn sdv_task_cancel_failed() {
    let handle = ylong_runtime::spawn(async move {
        let task = ylong_runtime::spawn(async move { 1 });
        sleep(Duration::from_millis(100)).await;
        task.cancel();
        let res = task.await.unwrap();
        assert_eq!(res, 1);
    });
    ylong_runtime::block_on(handle).unwrap();
}

#[derive(Default)]
struct TestFuture {
    flag: usize,
}

impl Future for TestFuture {
    type Output = u8;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        if this.flag < 1000000 {
            this.flag += 1;
            cx.waker().wake_by_ref();
            Poll::Pending
        } else {
            Poll::Ready(1)
        }
    }
}

/// SDV test for multi cancel.
///
/// # Brief
/// 1. In a loop, create a long-time task and then cancel it.
/// 2. Check if the task has been canceled for each run
#[test]
fn sdv_task_cancel_multiple() {
    for _ in 0..1000 {
        ylong_runtime::block_on(async move {
            let handle = ylong_runtime::spawn(async move {
                TestFuture::default().await;
                sleep(Duration::from_secs(1000000));
                1
            });
            handle.cancel();
            let res = handle.await;
            assert!(res.is_err());
        })
    }
}
