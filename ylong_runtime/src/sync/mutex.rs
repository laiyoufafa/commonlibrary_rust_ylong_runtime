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

//! Mutual exclusion locks

use crate::sync::semaphore_inner::SemaphoreInner;
use std::cell::UnsafeCell;
use std::error::Error;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::ops::{Deref, DerefMut};

/// An async version of [`std::sync::Mutex`]
///
/// Often it's considered as normal to use [`std::sync::Mutex`] on an asynchronous environment.
/// The primal purpose of this async mutex is to protect shared reference of io, which contains
/// a lot await point during reading and writing. If you only wants to protect a data across
/// different threads, [`std::sync::Mutex`] will probably gain you better performance.
///
/// When using across different futures, users need to wrap the mutex inside an Arc,
/// just like the use of [`std::sync::Mutex`].
pub struct Mutex<T: ?Sized> {
    /// Semaphore to provide mutual exclusion
    sem: SemaphoreInner,
    /// The data protected by this mutex
    data: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Send for Mutex<T> {}
unsafe impl<T: ?Sized + Send> Sync for Mutex<T> {}

/// Error of Mutex
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct LockError;

impl Display for LockError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Try lock error.")
    }
}

impl Error for LockError {}

impl<T: Sized> Mutex<T> {
    /// Creates a mutex protecting the data passed in.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::sync::mutex::Mutex;
    ///
    /// let _a = Mutex::new(2);
    ///
    /// ```
    pub fn new(t: T) -> Mutex<T> {
        Mutex {
            sem: SemaphoreInner::new(1).unwrap(),
            data: UnsafeCell::new(t),
        }
    }
}

impl<T: ?Sized> Mutex<T> {
    /// Locks the mutex.
    ///
    /// If the mutex is already held by others, asynchronously waits for it to release.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::sync::mutex::Mutex;
    /// use std::sync::Arc;
    ///
    /// let _res = ylong_runtime::block_on(async {
    ///     let lock = Arc::new(Mutex::new(2));
    ///     let mut n = lock.lock().await;
    ///     *n += 1;
    ///     assert_eq!(*n, 3);
    /// });
    /// ```
    pub async fn lock(&self) -> MutexGuard<'_, T> {
        // The result of `acquire()` will be `Err()` only when the semaphore is closed.
        // `Mutex` will not close, so the result of `acquire()` must be `Ok(())`.
        self.sem.acquire().await.unwrap();
        MutexGuard(self)
    }

    /// Attempts to get the mutex.
    ///
    /// If the lock is already held by others, LockError will be returned. Otherwise,
    /// the mutex guard will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::sync::mutex::Mutex;
    /// use std::sync::Arc;
    ///
    /// let _res = ylong_runtime::block_on(async {
    ///     let mutex = Arc::new(Mutex::new(0));
    ///     match mutex.try_lock() {
    ///         Ok(lock) => println!("{}",lock),
    ///         Err(_) => {}
    ///     };
    /// });
    /// ```
    pub fn try_lock(&self) -> Result<MutexGuard<'_, T>, LockError> {
        match self.sem.try_acquire() {
            Ok(_) => Ok(MutexGuard(self)),
            Err(_) => Err(LockError),
        }
    }

    /// Gets the mutable reference of the data protected by the lock without
    /// actually holding the lock.
    ///
    /// This method takes the mutable reference of the mutex, so there is no need to actually
    /// lock the mutex -- the mutable borrow statically guarantees no locks exist.
    pub fn get_mut(&mut self) -> &mut T {
        unsafe { &mut *self.data.get() }
    }
}

/// Mutex guard to access the data after holding the mutex.
pub struct MutexGuard<'a, T: ?Sized>(&'a Mutex<T>);

impl<T: ?Sized> MutexGuard<'_, T> {
    // Unlocks the mutex. Wakes the first future waiting for the mutex.
    fn unlock(&mut self) {
        self.0.sem.release();
    }
}

unsafe impl<T: ?Sized + Send + Sync> Sync for MutexGuard<'_, T> {}

/// The mutex will be released after the mutex guard is dropped.
impl<T: ?Sized> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        self.unlock();
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for MutexGuard<'_, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: ?Sized + Display> Display for MutexGuard<'_, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&**self, f)
    }
}

impl<T: ?Sized> Deref for MutexGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.0.data.get() }
    }
}

impl<T: ?Sized> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.0.data.get() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{block_on, spawn};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    /*
     * @title  UT test case for Mutex::new() interface
     * @design The design of this use case is carried out by the conditional coverage test method
     * @precon None
     * @brief  Test case execution steps：
     *         1. Creating a Concurrent Mutual Exclusion Lock
     *         2. Verify the state of the lock object and the size of the internal value of the lock object
     * @expect Mutex structure body state is UNLOCKED, data is preset value 10
     * @auto  Yes
     */
    #[test]
    fn ut_mutex_new_01() {
        let lock = Mutex::new(10);
        assert_eq!(lock.data.into_inner(), 10);
    }

    /*
     * @title  UT test case for Mutex::lock() interface
     * @design The design of this use case is carried out by the conditional coverage test method
     * @precon None
     * @brief  Test case execution steps：
     *         1. Creating a concurrent read/write lock
     *         2. Modification of data in the concurrent mutex lock
     *         3. Check the value in the changed concurrent mutex lock and check the status bit of the lock
     * @expect Mutex structure body state is LOCKED, data value is 11
     * @auto  Yes
     */
    #[test]
    fn ut_mutex_lock_01() {
        let mutex = Mutex::new(10);
        block_on(async {
            let mut lock = mutex.lock().await;
            *lock += 1;
            assert_eq!(*lock, 11);
        });
    }

    /*
     * @title  UT test case for Mutex::try_lock() interface
     * @design The design of this use case is carried out by the conditional coverage test method
     * @precon None
     * @brief  Test case execution steps：
     *         1. Creating a Concurrent Mutual Exclusion Lock
     *         2. Call try_lock() to try to get the lock
     *         3. Operation on in-lock values
     *         4. Calibrate in-lock values
     * @expect data value is 110
     * @auto  Yes
     */
    #[test]
    fn ut_mutex_try_lock_01() {
        let mutex = Mutex::new(10);
        let mut lock = mutex.try_lock().unwrap();
        *lock += 100;
        assert_eq!(*lock, 110);
    }

    /*
     * @title  UT test case for Mutex::try_lock() interface
     * @design The design of this use case is carried out by the conditional coverage test method
     * @precon None
     * @brief  Test case execution steps：
     *         1. Creating a Concurrent Mutual Exclusion Lock
     *         2. First build a concurrent process to hold the lock and sleep after obtaining the lock to hold the lock for a long time
     *         3. Call try_lock() to try to get a lock
     *         4. Check try_lock return value is None
     * @expect lock2 is None
     * @auto  Yes
     */
    #[test]
    fn ut_mutex_try_lock_02() {
        let mutex = Arc::new(Mutex::new(10));
        let mutex1 = mutex.clone();
        let flag = Arc::new(AtomicBool::new(true));
        let flag_clone = flag.clone();
        let lock_flag = Arc::new(AtomicBool::new(true));
        let lock_flag_clone = lock_flag.clone();

        spawn(async move {
            let _lock = mutex1.lock().await;
            lock_flag_clone.store(false, Ordering::SeqCst);
            while flag_clone.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(1));
            }
        });
        while lock_flag.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(1));
        }
        let lock2 = mutex.try_lock();
        assert!(lock2.is_err());
        flag.store(false, Ordering::SeqCst);
    }

    /*
     * @title  UT test case for Mutex::unlock() interface
     * @design The design of this use case is carried out by the conditional coverage test method
     * @precon None
     * @brief  Test case execution steps：
     *         1. Creating a Concurrent Mutual Exclusion Lock
     *         2. Perform locking operation
     *         3. Call the drop() method to release the lock
     *         4. Verify the status of the concurrent mutex lock before, after and after unlocking
     * @expect Mutex structure unlocked state is UNLOCKED, when locking state is LOCKED, after unlocking state is UNLOCKED
     * @auto  Yes
     */
    #[test]
    fn ut_mutex_unlock_01() {
        let mutex = Mutex::new(10);
        block_on(async move {
            let lock = mutex.lock().await;
            assert!(mutex.try_lock().is_err());
            drop(lock);
            assert!(mutex.try_lock().is_ok());
        });
    }
}
