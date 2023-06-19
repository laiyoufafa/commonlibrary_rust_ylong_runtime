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

//! Yields the current task and wakes it for a reschedule.
#[cfg(not(feature = "ffrt"))]
use crate::executor::worker;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Yields the current task and wakes it for a reschedule.
/// # Examples
///
/// ```
/// use ylong_runtime::task::*;
///
/// let res = ylong_runtime::block_on(
///    ylong_runtime::spawn(async { yield_now().await; })
/// ).unwrap();
/// assert_eq!(res, ());
/// ```
pub async fn yield_now() {
    YieldTask(false).await
}

struct YieldTask(bool);

impl Future for YieldTask {
    type Output = ();

    #[cfg(not(feature = "ffrt"))]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if !self.0 {
            self.0 = true;
            let ctx = worker::get_current_ctx();

            // Under worker context, we push the waker into the yielded list owned by the worker
            // to avoid waking the waker immediately. This is because waking the waker in a worker
            // context will put the task in the lifo slot, we don't want that.
            if let Some(ctx) = ctx {
                let mut yielded = ctx.yielded.borrow_mut();
                yielded.push(cx.waker().clone());
            } else {
                cx.waker().wake_by_ref();
            }
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }

    #[cfg(feature = "ffrt")]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if !self.0 {
            self.0 = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
}

#[cfg(not(feature = "ffrt"))]
pub(crate) fn wake_yielded_tasks() {
    let ctx = worker::get_current_ctx().expect("not in a worker ctx");
    let mut yielded = ctx.yielded.borrow_mut();
    if yielded.is_empty() {
        return;
    }
    for waker in yielded.drain(..) {
        waker.wake();
    }
}

#[cfg(test)]
mod test {
    use crate::task::yield_now;

    /// ut for yield.
    ///
    /// # Brief
    /// 1. Create two tasks that adds a number to 1000
    /// 2. Make the tasks yield during adding
    /// 3. Check if value is correct
    #[test]
    fn ut_yield_now() {
        let handle = crate::spawn(async move {
            let mut i = 0;
            for _ in 0..1000 {
                i += 1;
                yield_now().await;
            }
            i
        });
        let handle2 = crate::spawn(async move {
            let mut i = 0;
            for _ in 0..1000 {
                i += 1;
                yield_now().await;
            }
            i
        });
        let ret = crate::block_on(handle).unwrap();
        assert_eq!(ret, 1000);
        let ret = crate::block_on(handle2).unwrap();
        assert_eq!(ret, 1000);
    }
}
