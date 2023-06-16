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

//! Wakes tasks to wake up

use crate::sync::semaphore_inner::SemaphoreInner;

/// Wakes one or multiple tasks to wake up.
///
/// `Waiter` itself does not protect any data. Its only purpose is to signal
/// other tasks to perform an operation.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use ylong_runtime::sync::waiter::Waiter;
///
/// let waiter = Arc::new(Waiter::new());
/// let waiter2 = waiter.clone();
///
/// let _ = ylong_runtime::block_on(async {
///      let handle = ylong_runtime::spawn(async move {
///         waiter2.wait().await;
///      });
///      waiter.wake_one();
///      let _ = handle.await;
///  });
/// ```
pub struct Waiter {
    sem: SemaphoreInner,
}

impl Waiter {
    /// Creates a new Waiter.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::sync::waiter::Waiter;
    ///
    /// let waiter = Waiter::new();
    /// ```
    // Prevent to increase binary size and thus mask this warning.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Waiter {
        Waiter {
            sem: SemaphoreInner::new(0).unwrap(),
        }
    }

    /// Asynchronously waits for this Waiter to get signaled.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use ylong_runtime::sync::waiter::Waiter;
    ///
    /// let waiter = Arc::new(Waiter::new());
    /// let waiter2 = waiter.clone();
    /// let _ = ylong_runtime::block_on(async {
    ///     let handle = ylong_runtime::spawn(async move {
    ///         waiter2.wait().await;
    ///     });
    ///     waiter.wake_one();
    ///     let _ = handle.await;
    ///  });
    /// ```
    pub async fn wait(&self) {
        // The result of `acquire()` will be `Err()` only when the semaphore is closed.
        // `Waiter` will not close, so the result of `acquire()` must be `Ok(())`.
        self.sem.acquire().await.unwrap();
    }

    /// Notifies one task waiting on it.
    ///
    /// If this method gets called when there is no task waiting on this Waiter,
    /// then the next task called `wait` on it will not get blocked.
    ///
    /// If this method gets called multiple times, only one task will get passed straightly
    /// when calling `wait`. Any other task still has to asynchronously wait for it to be
    /// released.
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use ylong_runtime::sync::waiter::Waiter;
    ///
    /// let waiter = Arc::new(Waiter::new());
    /// let waiter2 = waiter.clone();
    /// let _ = ylong_runtime::block_on(async {
    ///     let handle = ylong_runtime::spawn(async move {
    ///         waiter2.wait().await;
    ///     });
    ///     waiter.wake_one();
    ///     let _ = handle.await;
    ///  });
    /// ```
    pub fn wake_one(&self) {
        self.sem.release_notify();
    }

    /// Notifies all tasks waiting on it.
    ///
    /// Unlike `wake_one`, if this method gets called when there is no task waiting on this wake,
    /// then the next task called `wait` on it will `still` get blocked.
    ///
    /// # Examples
    ///
    /// ```no run
    /// use std::sync::Arc;
    /// use ylong_runtime::sync::waiter::Waiter;
    ///
    /// let waiter = Arc::new(Waiter::new());
    /// let waiter2 = waiter.clone();
    /// let waiter3 = waiter.clone();
    /// let _ = ylong_runtime::block_on(async {
    ///     let handle = ylong_runtime::spawn(async move {
    ///         waiter2.wait().await;
    ///     });
    ///     let handle2 = ylong_runtime::spawn(async move {
    ///         waiter3.wait().await;
    ///     });
    ///     waiter.wake_all();
    /// });
    /// ```
    pub fn wake_all(&self) {
        self.sem.release_all();
    }
}
