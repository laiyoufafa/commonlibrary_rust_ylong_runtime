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

use crate::time::error::TimerError;
use crate::time::sleep;
use crate::time::sleep::Sleep;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

/// Requires a future to be completed by a set deadline.
///
/// # Examples
///
/// ```
/// use std::time::Duration;
/// use ylong_runtime::time::timeout;
///
/// let handle = ylong_runtime::spawn(timeout(Duration::from_secs(1), async {1}));
/// let result = ylong_runtime::block_on(handle).unwrap().unwrap();
/// assert_eq!(result, 1);
/// ```
pub fn timeout<F>(duration: Duration, future: F) -> Timeout<F>
where
    F: Future,
{
    let sleep = sleep(duration);
    Timeout::new(future, sleep)
}

/// [`Timeout`](Timeout) is a structure that implements Future.
///
/// [`Timeout`](Timeout) will be returned by func ['timeout'](timeout).
pub struct Timeout<T> {
    value: T,
    sleep: Sleep,
}

impl<T> Timeout<T> {
    fn new(value: T, sleep: Sleep) -> Timeout<T> {
        Self { value, sleep }
    }
}

impl<T> Future for Timeout<T>
where
    T: Future,
{
    type Output = Result<T::Output, TimerError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let timeout = unsafe { self.get_unchecked_mut() };

        let value = unsafe { Pin::new_unchecked(&mut timeout.value) };
        if let Poll::Ready(result) = value.poll(cx) {
            return Poll::Ready(Ok(result));
        }

        let sleep = unsafe { Pin::new_unchecked(&mut timeout.sleep) };
        match sleep.poll(cx) {
            Poll::Ready(_) => Poll::Ready(Err(TimerError::Elapsed)),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[cfg(test)]
mod test {
    use crate::time::timeout;
    use crate::{block_on, spawn};
    use std::time::Duration;

    /// timeout ut test case.
    ///
    /// # Title
    /// ut_timeout_test
    ///
    /// # Brief
    /// 1. Use timeout to create a Timeout Struct.
    /// 2. Use block_on to test not elapsed timeout.
    /// 3. Use block_on to test elapsed timeout.
    #[test]
    #[cfg(feature = "sync")]
    fn ut_timeout_test() {
        use crate::sync::oneshot;
        async fn not_elapsed() -> bool {
            let (tx, rx) = oneshot::channel();
            tx.send(()).unwrap();

            // Wrap the future with a `Timeout` set to expire in 10 milliseconds.
            timeout(Duration::from_millis(10), rx).await.is_ok()
        }

        async fn elapsed() -> bool {
            let (_tx, rx) = oneshot::channel::<()>();

            // Wrap the future with a `Timeout` set to expire in 10 milliseconds.
            timeout(Duration::from_millis(10), rx).await.is_ok()
        }

        let handle = spawn(not_elapsed());
        let result = block_on(handle).unwrap();
        assert!(result);

        let handle = spawn(elapsed());
        let result = block_on(handle).unwrap();
        assert!(!result);
    }

    /// timeout ut test case.
    ///
    /// # Title
    /// ut_timeout_test_002
    ///
    /// # Brief
    /// 1. Use timeout to create a Timeout Struct.
    /// 2. Test simple example.
    #[test]
    fn ut_timeout_test_002() {
        async fn simple() {
            let result = timeout(Duration::from_millis(100), async { 1 }).await;
            assert_eq!(result.unwrap(), 1);
        }

        let handle = spawn(simple());
        block_on(handle).unwrap();
    }
}
