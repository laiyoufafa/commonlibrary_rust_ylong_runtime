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

use crate::time::Clock;
use crate::time::Driver;
use std::convert::TryInto;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

const TEN_YEARS: Duration = Duration::from_secs(86400 * 365 * 10);

/// Waits until 'instant' has reached.
pub fn sleep_until(instant: Instant) -> Sleep {
    let start_time = Driver::get_ref().start_time();
    let instant = if instant < start_time {
        start_time
    } else {
        instant
    };
    Sleep::new_timeout(instant)
}

/// Waits until 'duration' has elapsed.
pub fn sleep(duration: Duration) -> Sleep {
    // If the time reaches the maximum value,
    // then set the default timing time to 10 years.
    match Instant::now().checked_add(duration) {
        Some(deadline) => Sleep::new_timeout(deadline),
        None => Sleep::new_timeout(Instant::now() + TEN_YEARS),
    }
}

/// [`Sleep`](Sleep) is a structure that implements Future.
///
/// [`Sleep`](Sleep) will be returned by func ['sleep'](sleep).
///
/// # Examples
///
/// ```
/// use std::time::Duration;
/// use ylong_runtime::time::sleep;
///
/// async fn sleep_test() {
///     let sleep = sleep(Duration::from_secs(2)).await;
///     println!("2 secs have elapsed");
/// }
/// ```
pub struct Sleep {
    // During the polling of this structure, no repeated insertion.
    need_insert: bool,

    // The time at which the structure should end.
    deadline: Instant,

    // Corresponding Timer structure.
    timer: Clock,
}

impl Sleep {
    // Creates a Sleep structure based on the given deadline.
    fn new_timeout(deadline: Instant) -> Self {
        let timer = Clock::new();
        Self {
            need_insert: true,
            deadline,
            timer,
        }
    }

    // Returns the deadline of the Sleep
    pub(crate) fn deadline(&self) -> Instant {
        self.deadline
    }

    // Resets the deadline of the Sleep
    pub(crate) fn reset(&mut self, new_deadline: Instant) {
        self.need_insert = true;
        self.deadline = new_deadline;
        self.timer.set_result(false);
    }

    // Cancels the Sleep
    fn cancel(&mut self) {
        let driver = Driver::get_ref();
        let mut lock = driver.wheel.lock().unwrap();
        lock.cancel(&self.timer.handle());
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let driver = Driver::get_ref();
        if self.need_insert {
            let ms = self
                .deadline
                .checked_duration_since(driver.start_time())
                .unwrap()
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX);
            self.timer.set_expiration(ms);
            self.timer.set_waker(cx.waker().clone());

            match driver.insert(self.timer.handle()) {
                Ok(_) => self.need_insert = false,
                Err(_) => {
                    // Even if the insertion fails, there is no need to insert again here,
                    // it is a timeout clock and needs to be triggered immediately at the next poll.
                    self.need_insert = false;
                    self.timer.set_result(true);
                }
            }
        }

        if self.timer.result() {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

impl Drop for Sleep {
    fn drop(&mut self) {
        // For some uses, for example, Timeout,
        // `Sleep` enters the `Pending` state first and inserts the `TimerHandle` into the `DRIVER`,
        // the future of timeout returns `Ready` in advance of the next polling,
        // as a result, the `TimerHandle` pointer in the `DRIVER` is invalid.
        // need to cancel the `TimerHandle` operation during `Sleep` drop.
        self.cancel()
    }
}

#[cfg(test)]
mod test {
    use crate::time::sleep;
    use crate::{block_on, spawn};
    use std::time::Duration;

    /// sleep ut test case.
    ///
    /// # Title
    /// new_sleep
    ///
    /// # Brief
    /// 1. Uses sleep to create a Sleep Struct.
    /// 2. Uses block_on to test different sleep duration.
    #[test]
    fn new_timer_sleep() {
        block_on(async move {
            sleep(Duration::new(0, 20_000_000)).await;
            sleep(Duration::new(0, 20_000_000)).await;
            sleep(Duration::new(0, 20_000_000)).await;
        });

        let handle_one = spawn(async {
            sleep(Duration::new(0, 20_000_000)).await;
        });
        let handle_two = spawn(async {
            sleep(Duration::new(0, 20_000_000)).await;
        });
        let handle_three = spawn(async {
            sleep(Duration::new(0, 20_000_000)).await;
        });
        block_on(handle_one).unwrap();
        block_on(handle_two).unwrap();
        block_on(handle_three).unwrap();
    }
}
