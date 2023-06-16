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

//! An asynchronous version of [`std::sync::RwLock`]

use crate::sync::semaphore_inner::SemaphoreInner;
use crate::sync::LockError;
use std::cell::UnsafeCell;
use std::fmt;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering::{AcqRel, Acquire, Release};

const MAX_READS: i64 = i64::MAX >> 2;

/// An asynchronous version of [`std::sync::RwLock`].
///
/// Rwlock allows multiple readers or a single writer to operate concurrently.
/// Readers are only allowed to read the data, but the writer is the only one
/// can change the data inside.
///
/// This Rwlock's policy is writer first, to prevent writers from starving.
///
/// # Examples
///
/// ```
/// use ylong_runtime::sync::rwlock::RwLock;
///
///
/// ylong_runtime::block_on(async {
///     let lock = RwLock::new(0);
///
///     // Can have multiple read locks at the same time
///     let r1 = lock.read().await;
///     let r2 = lock.read().await;
///     assert_eq!(*r1, 0);
///     assert_eq!(*r2, 0);
///     drop((r1, r2));
///
///     // Only one write lock at a time
///     let mut w = lock.write().await;
///     *w += 1;
///     assert_eq!(*w, 1);
///
/// });
/// ```
pub struct RwLock<T: ?Sized> {
    read_sem: SemaphoreInner,
    write_sem: SemaphoreInner,
    write_mutex: SemaphoreInner,
    read_count: AtomicI64,
    read_wait: AtomicI64,
    data: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Send for RwLock<T> {}
unsafe impl<T: ?Sized + Send + Sync> Sync for RwLock<T> {}

impl<T: Sized> RwLock<T> {
    /// Creates a new RwLock. `T` is the data that needs to be protected
    /// by this RwLock.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::sync::rwlock::RwLock;
    ///
    /// let lock = RwLock::new(0);
    /// ```
    pub fn new(t: T) -> RwLock<T> {
        RwLock {
            read_sem: SemaphoreInner::new(0).unwrap(),
            write_sem: SemaphoreInner::new(0).unwrap(),
            write_mutex: SemaphoreInner::new(1).unwrap(),
            read_count: AtomicI64::new(0),
            read_wait: AtomicI64::new(0),
            data: UnsafeCell::new(t),
        }
    }
}

impl<T: ?Sized> RwLock<T> {
    /// Asynchronously acquires the read lock.
    ///
    /// If there is a writer holding the write lock, then this method will wait asynchronously
    /// for the write lock to get released.
    ///
    /// But if the write lock is not held, it's ok for multiple readers to hold the read lock
    /// concurrently.
    ///
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::sync::rwlock::RwLock;
    ///
    /// ylong_runtime::block_on(async {
    ///     let lock = RwLock::new(0);
    ///     let r1 = lock.read().await;
    ///     assert_eq!(*r1, 0);
    /// });
    /// ```
    pub async fn read(&self) -> RwLockReadGuard<'_, T> {
        if self.read_count.fetch_add(1, Release) < 0 {
            // The result of `acquire()` will be `Err()` only when the semaphore is closed.
            // `RwLock` will not close, so the result of `acquire()` must be `Ok(())`.
            self.read_sem.acquire().await.unwrap();
        }
        RwLockReadGuard(self)
    }

    /// Attempts to get the read lock. If another writer is holding the write lock, then
    /// None will be returned. Otherwise, the ReadMutexGuard will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::sync::rwlock::RwLock;
    ///
    /// let lock = RwLock::new(0);
    /// let r1 = lock.try_read().unwrap();
    /// assert_eq!(*r1, 0);
    /// ```
    pub fn try_read(&self) -> Result<RwLockReadGuard<'_, T>, LockError> {
        let mut read_count = self.read_count.load(Acquire);
        loop {
            if read_count < 0 {
                return Err(LockError);
            } else {
                match self.read_count.compare_exchange_weak(
                    read_count,
                    read_count + 1,
                    AcqRel,
                    Acquire,
                ) {
                    Ok(_) => {
                        return Ok(RwLockReadGuard(self));
                    }
                    Err(curr) => {
                        read_count = curr;
                    }
                }
            }
        }
    }

    /// Asynchronously acquires the write lock.
    ///
    /// If there is other readers or writers, then this method will wait asynchronously
    /// for them to get released.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::sync::rwlock::RwLock;
    ///
    /// ylong_runtime::block_on(async {
    ///     let lock = RwLock::new(0);
    ///     let mut r1 = lock.write().await;
    ///     *r1 += 1;
    ///     assert_eq!(*r1, 1);
    /// });
    /// ```
    pub async fn write(&self) -> RwLockWriteGuard<'_, T> {
        // The result of `acquire()` will be `Err()` only when the semaphore is closed.
        // `RwLock` will not close, so the result of `acquire()` must be `Ok(())`.
        self.write_mutex.acquire().await.unwrap();
        let read_count = self.read_count.fetch_sub(MAX_READS, Release);
        // If the `read_count` is not 0, it indicates that there is currently a reader holding
        // a read lock. If the `read_wait` is 0 after addition, it indicates that all readers have
        // been dropped.
        if read_count >= 0 && self.read_wait.fetch_add(read_count, Release) != -read_count {
            self.write_sem.acquire().await.unwrap();
        }
        RwLockWriteGuard(self)
    }

    /// Attempts to acquire the write lock.
    ///
    /// If any other task holds the read/write lock, None will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::sync::rwlock::RwLock;
    ///
    /// let lock = RwLock::new(0);
    /// let mut r1 = lock.try_write().unwrap();
    /// *r1 += 1;
    /// assert_eq!(*r1, 1);
    /// ```
    pub fn try_write(&self) -> Result<RwLockWriteGuard<'_, T>, LockError> {
        if self.write_mutex.try_acquire().is_err() {
            return Err(LockError);
        }
        match self
            .read_count
            .compare_exchange(0, -MAX_READS, AcqRel, Acquire)
        {
            Ok(_) => Ok(RwLockWriteGuard(self)),
            Err(_) => {
                self.write_mutex.release();
                Err(LockError)
            }
        }
    }

    /// Consumes the lock, and returns the data protected by it.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::sync::rwlock::RwLock;
    ///
    /// let lock = RwLock::new(0);
    /// assert_eq!(lock.into_inner(), 0);
    /// ```
    pub fn into_inner(self) -> T
    where
        T: Sized,
    {
        self.data.into_inner()
    }

    /// Gets the mutable reference of the data protected by the lock.
    ///
    /// This method takes the mutable reference of the RwLock, so there is no need to actually
    /// lock the RwLock -- the mutable borrow statically guarantees no locks exist.
    /// ```
    /// use ylong_runtime::sync::rwlock::RwLock;
    ///
    /// ylong_runtime::block_on(async {
    ///     let mut lock = RwLock::new(0);
    ///     *lock.get_mut() = 10;
    ///     assert_eq!(*lock.write().await, 10);
    /// });
    /// ```
    pub fn get_mut(&mut self) -> &mut T {
        unsafe { &mut *self.data.get() }
    }
}

/// Read guard to access the data after holding the mutex.
pub struct RwLockReadGuard<'a, T: ?Sized>(&'a RwLock<T>);

unsafe impl<T: ?Sized + Send> Send for RwLockReadGuard<'_, T> {}
unsafe impl<T: ?Sized + Sync> Sync for RwLockReadGuard<'_, T> {}

/// Releases the read lock. Wakes any waiting writer if it's the last one holding the read lock.
impl<T: ?Sized> RwLockReadGuard<'_, T> {
    fn unlock(&mut self) {
        if self.0.read_count.fetch_sub(1, Release) < 0
            && self.0.read_wait.fetch_sub(1, Release) == 1
        {
            self.0.write_sem.release();
        }
    }
}

/// Unlock the read lock when ReadGuard is dropped.
impl<T: ?Sized> Drop for RwLockReadGuard<'_, T> {
    fn drop(&mut self) {
        self.unlock();
    }
}

impl<T: ?Sized> Deref for RwLockReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.0.data.get() }
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for RwLockReadGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for RwLockReadGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

/// RwLock write guard
pub struct RwLockWriteGuard<'a, T: ?Sized>(&'a RwLock<T>);

unsafe impl<T: ?Sized + Send> Send for RwLockWriteGuard<'_, T> {}
unsafe impl<T: ?Sized + Sync> Sync for RwLockWriteGuard<'_, T> {}

/// Wakes all waiting readers first and releases the write lock when WriteGuard is dropped.
impl<T: ?Sized> Drop for RwLockWriteGuard<'_, T> {
    fn drop(&mut self) {
        let read_count = self.0.read_count.fetch_add(MAX_READS, Release) + MAX_READS;
        self.0.read_sem.release_multi(read_count as usize);
        self.0.write_mutex.release();
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for RwLockWriteGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for RwLockWriteGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl<T: ?Sized> Deref for RwLockWriteGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.0.data.get() }
    }
}

impl<T: ?Sized> DerefMut for RwLockWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.0.data.get() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{block_on, spawn};
    use std::sync::Arc;

    /*
     * @title  Rwlock::new() ut test
     * @design The design of this use case is carried out by the conditional coverage test method.
     * @precon Create a test structure with two members: flag, num
     * @brief  Test case execution steps：
     *         1. Create a concurrent read/write lock with structure and value as input parameters
     *         2. Verify the contents of the read/write lock
     * @expect The flag inside the Test structure in lock is true, num is 1, and the value in lock2 is 0
     * @auto  Yes
     */
    #[test]
    fn ut_rwlock_new_01() {
        pub struct Test {
            flag: bool,
            num: usize,
        }
        block_on(async {
            let lock = RwLock::new(Test { flag: true, num: 1 });
            assert!(lock.read().await.flag);
            assert_eq!(lock.read().await.num, 1);
            let lock2 = RwLock::new(0);
            assert_eq!(*lock2.read().await, 0);
        });
    }

    /*
     * @title  Rwlock::read() ut test
     * @design The design of this use case is carried out by the conditional coverage test method.
     * @precon None
     * @brief  Test case execution steps：
     *         1. Creating a concurrent read/write lock
     *         2. Calling the read() function
     *         3. Verify the value of the read() function dereference
     * @expect The value in lock is 100
     * @auto  Yes
     */
    #[test]
    fn ut_rwlock_read_01() {
        block_on(async {
            let lock = RwLock::new(100);
            let a = lock.read().await;
            assert_eq!(*a, 100);
        });
    }

    /*
     * @title  Rwlock::read() ut test
     * @design The design of this use case is carried out by the conditional coverage test method.
     * @precon None
     * @brief  Test case execution steps：
     *         1. Creating a concurrent read/write lock
     *         2. Call the write() function to make changes to the concurrent read/write lock data
     *         3. Call the read() function to verify the value in the read/write lock of the concurrent process
     * @expect The modified value in lock is 101
     * @auto  Yes
     */
    #[test]
    fn ut_rwlock_read_02() {
        let lock = Arc::new(RwLock::new(100));
        let lock2 = lock.clone();

        block_on(spawn(async move {
            let mut loopmun = lock2.write().await;
            *loopmun += 1;
        }))
        .unwrap();
        block_on(async {
            let a = lock.read().await;
            assert_eq!(*a, 101);
        });
    }

    /*
     * @title  Rwlock::try_read() ut test
     * @design The design of this use case is carried out by the conditional coverage test method.
     * @precon None
     * @brief  Test case execution steps：
     *         1. Creating a concurrent read/write lock
     *         2. Call try_read()
     *         3. Verify the value of the return value dereference
     * @expect res resolves the reference value to 100
     * @auto  Yes
     */
    #[test]
    fn ut_rwlock_try_read_01() {
        let lock = RwLock::new(100);
        let res = lock.try_read().unwrap();
        assert_eq!(*res, 100);
    }

    /*
     * @title  Rwlock::try_read() ut test
     * @design The design of this use case is carried out by the conditional coverage test method.
     * @precon None
     * @brief  Test case execution steps：
     *         1. Creating a concurrent read/write lock
     *         2. Create a thread to call the write method to hold the lock, and then sleep to hold the lock for a long time
     *         3. Call try_read() to try to get a lock
     *         4. Check the try_read return value
     * @expect res is None
     * @auto  Yes
     */
    #[test]
    fn ut_rwlock_try_read_02() {
        let lock = Arc::new(RwLock::new(100));
        let mut a = lock.try_write().unwrap();
        *a += 1;
        let res = lock.try_read();
        assert!(res.is_err());
        *a += 1;
        drop(a);
        let res2 = lock.try_read();
        assert!(res2.is_ok());
    }

    /*
     * @title  Rwlock::write() ut test
     * @design The design of this use case is carried out by the conditional coverage test method.
     * @precon None
     * @brief  Test case execution steps：
     *         1. Creating a concurrent read/write lock
     *         2. Create a call to the write interface to modify the value inside the concurrent read/write lock
     *         3. Verify the value of the concurrent read/write lock
     * @expect The unreferenced value of a is 200
     * @auto  Yes
     */
    #[test]
    fn ut_rwlock_write_01() {
        let lock = Arc::new(RwLock::new(100));
        block_on(async {
            let mut a = lock.write().await;
            *a += 100;
            assert_eq!(*a, 200);
        });
    }

    /*
     * @title  Rwlock::write() ut test
     * @design The design of this use case is carried out by the conditional coverage test method.
     * @precon None
     * @brief  Test case execution steps：
     *         1. Creating a concurrent read/write lock
     *         2. First create a thread to obtain a write lock, modify the data in the concurrent read/write lock, and then hibernate to ensure that the lock is held for a long time
     *         3. Create two co-processes one to get a read lock and one to get a write lock, so that there is both a reader and a writer requesting the lock
     *         4. Verify the value inside the concurrent read/write lock when the concurrent read/write lock is obtained
     * @expect aa's dereference is 300
     * @auto  Yes
     */
    #[test]
    fn ut_rwlock_write_test_02() {
        let lock = Arc::new(RwLock::new(100));
        let lock2 = lock.clone();
        let lock3 = lock.clone();
        let lock4 = lock;

        let handle = spawn(async move {
            let mut aa = lock2.write().await;
            *aa += 100;
        });
        let handle1 = spawn(async move {
            let mut aa = lock4.write().await;
            *aa += 100;
        });
        block_on(handle).unwrap();
        block_on(handle1).unwrap();
        let handle2 = spawn(async move {
            let aa = lock3.read().await;
            assert_eq!(*aa, 300);
        });
        block_on(handle2).unwrap();
    }

    /*
     * @title  Rwlock::try_write() ut test
     * @design The design of this use case is carried out by the conditional coverage test method.
     * @precon None
     * @brief  Test case execution steps：
     *         1. Creating a concurrent read/write lock
     *         2. Call try_write() to try to get a write lock and modify the value in it
     *         3. Verify the value in the read/write lock of the concurrent process
     * @expect The dereference of aa is 200
     * @auto  Yes
     */
    #[test]
    fn ut_rwlock_try_write_01() {
        let lock = RwLock::new(100);
        let mut aa = lock.try_write().unwrap();
        *aa += 100;
        assert_eq!(*aa, 200);
    }

    /*
     * @title  Rwlock::try_write() ut test
     * @design The design of this use case is carried out by the conditional coverage test method.
     * @precon None
     * @brief  Test case execution steps：
     *         1. Creating a concurrent read/write lock
     *         2. Execute command cargo test ut_rwlock_try_write_02
     * @expect res is None
     * @auto  Yes
     */
    #[test]
    fn ut_rwlock_try_write_02() {
        let lock = Arc::new(RwLock::new(100));
        let mut a = lock.try_write().unwrap();
        *a += 1;
        let res = lock.try_write();
        assert!(res.is_err());
        *a += 1;
        drop(a);
        let res2 = lock.try_write();
        assert!(res2.is_ok());
    }

    /*
     * @title  Rwlock::into_inner() ut test
     * @design The design of this use case is carried out by the conditional coverage test method.
     * @precon None
     * @brief  Describe the test case execution, an example of which is as follows:
     *         1. Add a temporary library path to the project directory export LD_LIBRARY_PATH=$(pwd)/platform
     *         2. Execute command cargo test ut_rwlock_into_inner_01
     * @expect The value after the lock call function is 10
     * @auto  Yes
     */
    #[test]
    fn ut_rwlock_into_inner_01() {
        let lock = RwLock::new(10);
        assert_eq!(lock.into_inner(), 10);
    }
}
