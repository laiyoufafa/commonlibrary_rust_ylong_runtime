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

//! Asynchronous counting semaphore.

use crate::sync::semaphore_inner::{SemaphoreError, SemaphoreInner};

/// Asynchronous counting semaphore. It allows more than one caller to access the shared resource.
/// Semaphore contains a set of permits. Call `acquire` method and get a permit to access the shared
/// resource. When permits are used up, new requests to acquire permit will wait until `release` method
/// is called. When no request is waiting, calling `release` method will add a permit to semaphore.
///
/// The difference between [`AutoRelSemaphore`] and [`Semaphore`] is that permit acquired from
/// [`Semaphore`] will be consumed. When permit from [`AutoRelSemaphore`] is dropped, it will be
/// assigned to another acquiring request or returned to the semaphore.
///
/// # Examples
///
/// ```
///
/// use std::sync::Arc;
/// use ylong_runtime::sync::semaphore::Semaphore;
///
///  async fn io_func() {
///     let sem = Arc::new(Semaphore::new(2).unwrap());
///     let sem2 = sem.clone();
///     let _permit1 = sem.try_acquire();
///     ylong_runtime::spawn(async move {
///         let _permit2 = sem2.acquire().await.unwrap();
///     });
///  }
///
/// ```
pub struct Semaphore {
    inner: SemaphoreInner,
}

/// Asynchronous counting semaphore. It allows more than one caller to access the shared resource.
/// semaphore contains a set of permits. Call `acquire` method and get a permit to access the shared
/// resource. The total number of permits is fixed. When no permits are available, new request to
/// acquire permit will wait until another permit is dropped. When no request is waiting and one permit
/// is **dropped**, the permit will be return to semaphore so that the number of permits in semaphore will
/// increase.
///
/// The difference between [`AutoRelSemaphore`] and [`Semaphore`] is that permit acquired from
/// [`Semaphore`] will be consumed. When permit from [`AutoRelSemaphore`] is dropped, it will be
/// assigned to another acquiring request or returned to the semaphore, in other words, permit will
/// be automatically released when it is dropped.
///
/// # Examples
///
/// ```
///
/// use std::sync::Arc;
/// use ylong_runtime::sync::semaphore::AutoRelSemaphore;
///
///  async fn io_func() {
///     let sem = Arc::new(AutoRelSemaphore::new(2).unwrap());
///     let sem2 = sem.clone();
///     let _permit1 = sem.try_acquire();
///     ylong_runtime::spawn(async move {
///         let _permit2 = sem2.acquire().await.unwrap();
///     });
///  }
///
/// ```
pub struct AutoRelSemaphore {
    inner: SemaphoreInner,
}

/// Permit acquired from `Semaphore`.
/// Consumed when dropped.
pub struct SemaphorePermit;

/// Permit acquired from `AutoRelSemaphore`.
/// Recycled when dropped.
pub struct AutoRelSemaphorePermit<'a> {
    sem: &'a AutoRelSemaphore,
}

impl Semaphore {
    /// Creates a `Semaphore` with an initial permit value.
    ///
    /// # Examples
    ///
    /// ```
    ///
    /// use ylong_runtime::sync::Semaphore;
    ///
    /// let sem = Semaphore::new(4).unwrap();
    ///
    /// ```
    pub fn new(permits: usize) -> Result<Semaphore, SemaphoreError> {
        match SemaphoreInner::new(permits) {
            Ok(inner) => Ok(Semaphore { inner }),
            Err(e) => Err(e),
        }
    }

    /// Gets the number of remaining permits.
    ///
    /// # Examples
    ///
    /// ```
    ///
    /// use ylong_runtime::sync::Semaphore;
    /// let sem = Semaphore::new(4).unwrap();
    /// assert_eq!(sem.current_permits(), 4);
    ///
    /// ```
    pub fn current_permits(&self) -> usize {
        self.inner.current_permits()
    }

    /// Adds a permit to the semaphore.
    ///
    /// # Examples
    ///
    /// ```
    ///
    /// use ylong_runtime::sync::Semaphore;
    ///
    /// let sem = Semaphore::new(4).unwrap();
    /// assert_eq!(sem.current_permits(), 4);
    /// sem.release();
    /// assert_eq!(sem.current_permits(), 5);
    ///
    /// ```
    pub fn release(&self) {
        self.inner.release();
    }

    /// Attempts to acquire a permit from semaphore.
    ///
    /// # Return value
    /// The function returns:
    ///  * `Ok(SemaphorePermit)` if acquiring a permit successfully.
    ///  * `Err(PermitError::Empty)` if no permit remaining in semaphore.
    ///  * `Err(PermitError::Closed)` if semaphore is closed.
    ///
    /// # Examples
    ///
    /// ```
    ///
    /// use ylong_runtime::sync::Semaphore;
    ///
    /// let sem = Semaphore::new(4).unwrap();
    /// assert_eq!(sem.current_permits(), 4);
    /// let permit = sem.try_acquire().unwrap();
    /// assert_eq!(sem.current_permits(), 3);
    /// drop(permit);
    /// assert_eq!(sem.current_permits(), 3);
    ///
    /// ```
    pub fn try_acquire(&self) -> Result<SemaphorePermit, SemaphoreError> {
        match self.inner.try_acquire() {
            Ok(_) => Ok(SemaphorePermit),
            Err(e) => Err(e),
        }
    }

    /// Asynchronously acquires a permit from semaphore.
    ///
    /// # Return value
    /// The function returns:
    ///  * `Ok(SemaphorePermit)` if acquiring a permit successfully.
    ///  * `Err(PermitError::Closed)` if semaphore is closed.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::sync::Semaphore;
    /// async fn io_func() {
    ///     let sem = Semaphore::new(2).unwrap();
    ///     ylong_runtime::spawn(async move {
    ///         let _permit2 = sem.acquire().await.unwrap();
    ///     });
    ///  }
    ///
    /// ```
    pub async fn acquire(&self) -> Result<SemaphorePermit, SemaphoreError> {
        self.inner.acquire().await?;
        Ok(SemaphorePermit)
    }

    /// Checks whether semaphore is closed. If so, the semaphore could not be acquired anymore.
    ///
    /// # Examples
    ///
    /// ```
    ///
    /// use ylong_runtime::sync::semaphore::Semaphore;
    ///
    /// let sem = Semaphore::new(4).unwrap();
    /// assert!(!sem.is_closed());
    /// sem.close();
    /// assert!(sem.is_closed());
    ///
    /// ```
    pub fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }

    /// Closes the semaphore so that it could not be acquired anymore,
    /// and it notifies all requests in the waiting list.
    ///
    /// # Examples
    ///
    /// ```
    ///
    /// use ylong_runtime::sync::Semaphore;
    ///
    /// let sem = Semaphore::new(4).unwrap();
    /// assert!(!sem.is_closed());
    /// sem.close();
    /// assert!(sem.is_closed());
    ///
    /// ```
    pub fn close(&self) {
        self.inner.close();
    }
}

impl AutoRelSemaphore {
    /// Creates a semaphore with an initial capacity.
    ///
    /// # Examples
    ///
    /// ```
    ///
    /// use ylong_runtime::sync::AutoRelSemaphore;
    ///
    /// let sem = AutoRelSemaphore::new(4).unwrap();
    ///
    /// ```
    pub fn new(number: usize) -> Result<AutoRelSemaphore, SemaphoreError> {
        match SemaphoreInner::new(number) {
            Ok(inner) => Ok(AutoRelSemaphore { inner }),
            Err(e) => Err(e),
        }
    }

    /// Gets the number of remaining permits.
    ///
    /// # Examples
    ///
    /// ```
    ///
    /// use ylong_runtime::sync::AutoRelSemaphore;
    /// let sem = AutoRelSemaphore::new(4).unwrap();
    /// assert_eq!(sem.current_permits(), 4);
    ///
    /// ```
    pub fn current_permits(&self) -> usize {
        self.inner.current_permits()
    }

    /// Attempts to acquire an auto-release-permit from semaphore.
    ///
    /// # Return value
    /// The function returns:
    ///  * `Ok(OneTimeSemaphorePermit)` if acquiring a permit successfully.
    ///  * `Err(PermitError::Empty)` if no permit remaining in semaphore.
    ///  * `Err(PermitError::Closed)` if semaphore is closed.
    ///
    /// # Examples
    ///
    /// ```
    ///
    /// use ylong_runtime::sync::AutoRelSemaphore;
    ///
    /// let sem = AutoRelSemaphore::new(4).unwrap();
    /// assert_eq!(sem.current_permits(), 4);
    /// let permit = sem.try_acquire().unwrap();
    /// assert_eq!(sem.current_permits(), 3);
    /// drop(permit);
    /// assert_eq!(sem.current_permits(), 4);
    ///
    /// ```
    pub fn try_acquire(&self) -> Result<AutoRelSemaphorePermit<'_>, SemaphoreError> {
        match self.inner.try_acquire() {
            Ok(_) => Ok(AutoRelSemaphorePermit { sem: self }),
            Err(e) => Err(e),
        }
    }

    /// Asynchronously acquires an auto-release-permit from semaphore.
    ///
    /// # Return value
    /// The function returns:
    ///  * `Ok(OneTimeSemaphorePermit)` if acquiring a permit successfully.
    ///  * `Err(PermitError::Closed)` if semaphore is closed.
    ///
    /// # Examples
    ///
    /// ```
    ///
    /// use ylong_runtime::sync::AutoRelSemaphore;
    ///
    /// async fn io_func() {
    ///     let sem = AutoRelSemaphore::new(2).unwrap();
    ///     ylong_runtime::spawn(async move {
    ///         let _permit2 = sem.acquire().await.unwrap();
    ///     });
    ///  }
    ///
    /// ```
    pub async fn acquire(&self) -> Result<AutoRelSemaphorePermit<'_>, SemaphoreError> {
        self.inner.acquire().await?;
        Ok(AutoRelSemaphorePermit { sem: self })
    }

    /// Checks whether the state of semaphore is closed, if so, the semaphore could not acquire
    /// permits anymore.
    ///
    /// # Examples
    ///
    /// ```
    ///
    /// use ylong_runtime::sync::AutoRelSemaphore;
    ///
    /// let sem = AutoRelSemaphore::new(4).unwrap();
    /// assert!(!sem.is_closed());
    /// sem.close();
    /// assert!(sem.is_closed());
    ///
    /// ```
    pub fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }

    /// Turns the state of semaphore to be closed so that semaphore could not acquire
    /// permits anymore, and notify all request in the waiting list.
    ///
    /// # Examples
    ///
    /// ```
    ///
    /// use ylong_runtime::sync::AutoRelSemaphore;
    ///
    /// let sem = AutoRelSemaphore::new(4).unwrap();
    /// assert!(!sem.is_closed());
    /// sem.close();
    /// assert!(sem.is_closed());
    ///
    /// ```
    pub fn close(&self) {
        self.inner.close();
    }
}

impl Drop for AutoRelSemaphorePermit<'_> {
    fn drop(&mut self) {
        self.sem.inner.release();
    }
}
