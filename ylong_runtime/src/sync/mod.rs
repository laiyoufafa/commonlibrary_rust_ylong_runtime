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

//! Synchronization primitives for asynchronous context.

pub(crate) mod atomic_waker;
pub mod error;
pub mod mpsc;
pub mod mutex;
pub mod oneshot;
pub mod rwlock;
pub mod semaphore;
pub(crate) mod semaphore_inner;
pub mod waiter;
mod wake_list;

pub use error::{RecvError, SendError};
pub use mutex::{LockError, Mutex, MutexGuard};
pub use rwlock::{RwLock, RwLockReadGuard, RwLockWriteGuard};
pub use semaphore::{AutoRelSemaphore, AutoRelSemaphorePermit, Semaphore, SemaphorePermit};
pub use semaphore_inner::SemaphoreError;
pub use waiter::Waiter;
