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

use crate::futures::poll_fn;
use crate::time::sleep::{sleep_until, Sleep};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

const TEN_YEARS: Duration = Duration::from_secs(86400 * 365 * 10);

/// Creates new [`Timer`] that yields with interval of `period`. The first task starts immediately.
///
/// # Examples
///
/// ```
/// use std::time::Duration;
/// use ylong_runtime::time;
///
/// async fn timer_test() {
///     let mut timer = time::timer(Duration::from_millis(10));
///
///     timer.next_period().await; // ticks immediately
///     timer.next_period().await; // ticks after 10 ms
///     timer.next_period().await; // ticks after 10 ms
/// }
///
/// let handle = ylong_runtime::spawn(timer_test());
/// ylong_runtime::block_on(handle).unwrap();
/// ```
pub fn timer(period: Duration) -> Timer {
    timer_at(Instant::now(), period)
}

/// Creates new [`Timer`] that yields with interval of `period`.
/// The first task starts at the `start` Instant.
///
/// # Examples
///
/// ```
/// use std::time::{Duration, Instant};
/// use ylong_runtime::time;
///
/// async fn timer_at_test() {
///     let mut timer = time::timer_at(
///         Instant::now() + Duration::from_millis(10),
///         Duration::from_millis(30)
///     );
///
///     timer.next_period().await; // ticks after 10 ms
///     timer.next_period().await; // ticks after 30 ms
///     timer.next_period().await; // ticks after 30 ms
/// }
///
/// let handle = ylong_runtime::spawn(timer_at_test());
/// ylong_runtime::block_on(handle).unwrap();
/// ```
pub fn timer_at(start: Instant, period: Duration) -> Timer {
    let start = sleep_until(start);
    Timer { start, period }
}

/// Automatically executes closure every 'period' for 'repeat_time' times.
/// When 'repeat_time' is `None()`, the closure will executes until drop.
///
/// # Examples
///
/// ```
/// use std::time::{Duration, Instant};
/// use ylong_runtime::{spawn, block_on, time};
/// use std::sync::{Arc, Mutex};
///
/// let x = Arc::new(Mutex::new(0));
/// let xc = x.clone();
///
/// let closure = move || {
/// let mut a = xc.lock().unwrap();
///     *a = *a + 1;
/// };
///
/// let handle = spawn(time::periodic_schedule(
///     closure,
///     Some(3),
///     Duration::from_millis(100),
/// ));
/// let _ = block_on(handle);
///
/// let x = x.lock().unwrap();
/// assert_eq!(*x, 3);
/// ```
pub async fn periodic_schedule<T>(mut closure: T, repeat_time: Option<usize>, period: Duration)
where
    T: FnMut() + Send + 'static,
{
    let mut timer = timer(period);
    match repeat_time {
        Some(times) => {
            for _ in 0..times {
                closure();
                timer.next_period().await;
            }
        }
        None => loop {
            closure();
            timer.next_period().await;
        },
    }
}

/// Struct of Timer
pub struct Timer {
    start: Sleep,
    period: Duration,
}

impl Timer {
    /// Waits until the next Instant reached.
    pub async fn next_period(&mut self) -> Instant {
        poll_fn(|cx| self.poll_next_period(cx)).await
    }

    fn poll_next_period(&mut self, cx: &mut Context<'_>) -> Poll<Instant> {
        if Pin::new(&mut self.start).poll(cx).is_pending() {
            return Poll::Pending;
        }

        let deadline = self.start.deadline();
        let next = match deadline.checked_add(self.period) {
            Some(next_out) => next_out,
            None => deadline + TEN_YEARS,
        };

        self.start.reset(next);

        Poll::Ready(next)
    }

    /// Resets Timer from now on.
    pub fn reset(&mut self) {
        self.start.reset(Instant::now() + self.period);
    }

    /// Gets period
    pub fn period(&self) -> Duration {
        self.period
    }
}

#[cfg(test)]
mod test {
    use crate::time::sleep;
    use crate::{block_on, spawn, time};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    /// time ut test case.
    ///
    /// # Title
    /// new_timer
    ///
    /// # Brief
    /// 1. Uses time to create a Timer Struct.
    /// 2. Checks whether the period, reset, and timeout policy are correct.
    #[test]
    fn ut_new_timer() {
        block_on(async move {
            let mut timer = time::timer::timer(Duration::new(1, 0));

            assert_eq!(Duration::new(1, 0), timer.period());
            timer.reset();
            assert_eq!(Duration::new(1, 0), timer.period());
        });
    }

    /// time ut test case.
    ///
    /// # Title
    /// new_timer_base
    ///
    /// # Brief
    /// 1. Uses timer_at to create a Timer Struct.
    /// 2. Uses tick() to wait until next Instant.
    #[test]
    fn ut_new_timer_base() {
        let handle = spawn(async move {
            let mut a = 0;
            let mut timer = time::timer::timer_at(
                Instant::now() + Duration::new(0, 20_000_000),
                Duration::new(0, 20_000_000),
            );
            a += 1;
            timer.next_period().await;
            a += 1;
            timer.next_period().await;
            a += 1;
            timer.next_period().await;
            assert_eq!(a, 3);
        });
        block_on(handle).unwrap();
    }

    /// time ut test case.
    ///
    /// # Title
    /// new_timer_timeout
    ///
    /// # Brief
    /// 1. Uses time to create a Timer Struct.
    /// 2. Sets TimeoutPolicy as Delay.
    /// 2. Uses tick() to wait until next Instant.
    #[test]
    fn ut_new_timer_timeout() {
        let handle = spawn(async move {
            let mut a = 0;
            let mut timer = time::timer::timer_at(
                Instant::now() + Duration::from_millis(100),
                Duration::from_millis(100),
            );
            a += 1;
            timer.next_period().await;
            sleep(Duration::new(0, 300_000_000)).await;
            a += 1;
            timer.next_period().await;
            a += 1;
            timer.next_period().await;
            assert_eq!(a, 3);
        });
        block_on(handle).unwrap();
    }

    /// time ut test case.
    ///
    /// # Title
    /// new_timer_schedule
    ///
    /// # Brief
    /// 1. Creates a closure.
    /// 2. Use period_schedule() to execute this closure for 10 times.
    /// 3. Checks if result is correct.
    #[test]
    fn ut_new_timer_schedule() {
        let x = Arc::new(Mutex::new(0));
        let xc = x.clone();

        let closure = move || {
            let mut a = xc.lock().unwrap();
            *a += 1;
        };

        let task = time::periodic_schedule(closure, Some(10), Duration::from_nanos(20_000_000));

        let handle = spawn(task);
        let _ = block_on(handle);

        let x = x.lock().unwrap();
        assert_eq!(*x, 10);
    }
}
