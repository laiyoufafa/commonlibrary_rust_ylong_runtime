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

use std::{
    cell::{Cell, UnsafeCell},
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use libc::c_int;

use crate::FfrtResult;

#[repr(C)]
pub(crate) struct FfrtMutex {
    storage: [u8; 128],
}

/// The API is similar with parking_lot::Mutex without poison.
pub struct Mutex<T: ?Sized> {
    pub(crate) inner: UnsafeCell<FfrtMutex>,
    data: UnsafeCell<T>,
}

/// Mutexguard used for operating the value inside the mutex.
pub struct MutexGuard<'a, T: ?Sized + 'a> {
    pub(crate) lock: &'a Mutex<T>,
    _phantom: PhantomData<Cell<()>>, // Not Send, Not Sync
}

#[repr(C)]
#[allow(dead_code)]
enum FfrtMutexType {
    Plain = 0,
    Recursive,
    Timed,
}

impl<T> Mutex<T> {
    /// Creates a new mutex protecting a value.
    pub fn new(val: T) -> FfrtResult<Mutex<T>> {
        unsafe {
            let mut inner = FfrtMutex { storage: [0; 128] };
            let r = ffrt_mtx_init(&mut inner as _, FfrtMutexType::Plain as c_int);
            if r != 0 {
                return Err(r);
            }
            Ok(Mutex {
                inner: UnsafeCell::new(inner),
                data: UnsafeCell::new(val),
            })
        }
    }

    /// Locks the mutex and gets its MutexGuard.
    pub fn lock(&self) -> FfrtResult<MutexGuard<'_, T>> {
        unsafe {
            let r = ffrt_mtx_lock(self.inner.get());
            if r != 0 {
                return Err(r);
            }
            Ok(MutexGuard {
                lock: self,
                _phantom: PhantomData,
            })
        }
    }

    /// Attempts to lock the mutex.
    pub fn try_lock(&self) -> FfrtResult<MutexGuard<'_, T>> {
        unsafe {
            let r = ffrt_mtx_trylock(self.inner.get());
            if r != 0 {
                return Err(r);
            }
            Ok(MutexGuard {
                lock: self,
                _phantom: PhantomData,
            })
        }
    }

    /// Get the mutable reference of the value inside.
    pub fn get_mut(&mut self) -> &mut T {
        self.data.get_mut()
    }

    /// Unlocks the mutex.
    pub fn unlock(guard: MutexGuard<'_, T>) {
        drop(guard);
    }
}

impl<'a, T: ?Sized> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        unsafe {
            let _ = ffrt_mtx_unlock(self.lock.inner.get()); // ignore result
        }
    }
}

impl<T: ?Sized> Drop for Mutex<T> {
    fn drop(&mut self) {
        unsafe {
            let _ = ffrt_mtx_destroy(self.inner.get()); // ignore result
        }
    }
}

impl<'a, T: ?Sized> Deref for MutexGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T: ?Sized> DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.data.get() }
    }
}

unsafe impl<T: ?Sized + Sync> Sync for MutexGuard<'_, T> {}

unsafe impl<T: ?Sized + Send> Sync for Mutex<T> {}
unsafe impl<T: ?Sized + Send> Send for Mutex<T> {}

#[link(name = "ffrt")]
// mutex.h
extern "C" {
    fn ffrt_mtx_init(m: *mut FfrtMutex, ty: c_int) -> c_int;
    fn ffrt_mtx_lock(m: *mut FfrtMutex) -> c_int;
    fn ffrt_mtx_unlock(m: *mut FfrtMutex) -> c_int;
    fn ffrt_mtx_trylock(m: *mut FfrtMutex) -> c_int;
    fn ffrt_mtx_destroy(m: *mut FfrtMutex) -> c_int;
}
