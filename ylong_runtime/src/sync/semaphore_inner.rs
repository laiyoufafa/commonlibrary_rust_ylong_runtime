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

use crate::sync::wake_list::WakerList;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::{AcqRel, Acquire, Release};
use std::task::Poll::{Pending, Ready};
use std::task::{Context, Poll};

/// Maximum capacity of `Semaphore`.
const MAX_PERMITS: usize = usize::MAX >> 1;
/// The least significant bit that marks the number of permits.
const PERMIT_SHIFT: usize = 1;
/// The flag marks that Semaphore is closed.
const CLOSED: usize = 1;

pub(crate) struct SemaphoreInner {
    permits: AtomicUsize,
    waker_list: WakerList,
}

pub(crate) struct Permit<'a> {
    semaphore: &'a SemaphoreInner,
    waker_index: Option<usize>,
    enqueue: bool,
}

/// Error returned by `Semaphore`.
#[derive(Debug, Eq, PartialEq)]
pub enum SemaphoreError {
    /// The number of Permits is overflowed.
    Overflow,
    /// Semaphore doesn't have enough permits.
    Empty,
    /// Semaphore was closed.
    Closed,
}

impl Display for SemaphoreError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SemaphoreError::Overflow => write!(f, "permit overflow MAX_PERMITS : {MAX_PERMITS}"),
            SemaphoreError::Empty => write!(f, "no permits available"),
            SemaphoreError::Closed => write!(f, "semaphore has been closed"),
        }
    }
}

impl Error for SemaphoreError {}

impl SemaphoreInner {
    pub(crate) fn new(permits: usize) -> Result<SemaphoreInner, SemaphoreError> {
        if permits >= MAX_PERMITS {
            return Err(SemaphoreError::Overflow);
        }
        Ok(SemaphoreInner {
            permits: AtomicUsize::new(permits << PERMIT_SHIFT),
            waker_list: WakerList::new(),
        })
    }

    pub(crate) fn current_permits(&self) -> usize {
        self.permits.load(Acquire) >> PERMIT_SHIFT
    }

    pub(crate) fn release(&self) {
        // Get the lock first to ensure the atomicity of the two operations.
        let mut waker_list = self.waker_list.lock();
        if !waker_list.notify_one() {
            let prev = self.permits.fetch_add(1 << PERMIT_SHIFT, Release);
            assert!(
                (prev >> PERMIT_SHIFT) < MAX_PERMITS,
                "the number of permits will overflow the capacity after addition"
            );
        }
    }

    pub(crate) fn release_notify(&self) {
        // Get the lock first to ensure the atomicity of the two operations.
        let mut waker_list = self.waker_list.lock();
        if !waker_list.notify_one() {
            self.permits.store(1 << PERMIT_SHIFT, Release);
        }
    }

    pub(crate) fn release_multi(&self, mut permits: usize) {
        let mut waker_list = self.waker_list.lock();
        while permits > 0 && waker_list.notify_one() {
            permits -= 1;
        }
        let prev = self.permits.fetch_add(permits << PERMIT_SHIFT, Release);
        assert!(
            (prev >> PERMIT_SHIFT) < MAX_PERMITS,
            "the number of permits will overflow the capacity after addition"
        );
    }

    pub(crate) fn release_all(&self) {
        self.waker_list.notify_all();
    }

    pub(crate) fn try_acquire(&self) -> Result<(), SemaphoreError> {
        let mut curr = self.permits.load(Acquire);
        loop {
            if curr & CLOSED == CLOSED {
                return Err(SemaphoreError::Closed);
            }

            if curr > 0 {
                match self.permits.compare_exchange(
                    curr,
                    curr - (1 << PERMIT_SHIFT),
                    AcqRel,
                    Acquire,
                ) {
                    Ok(_) => {
                        return Ok(());
                    }
                    Err(actual) => {
                        curr = actual;
                    }
                }
            } else {
                return Err(SemaphoreError::Empty);
            }
        }
    }

    pub(crate) fn is_closed(&self) -> bool {
        self.permits.load(Acquire) & CLOSED == CLOSED
    }

    pub(crate) fn close(&self) {
        // Get the lock first to ensure the atomicity of the two operations.
        let mut waker_list = self.waker_list.lock();
        self.permits.fetch_or(CLOSED, Release);
        waker_list.notify_all();
    }

    pub(crate) fn acquire(&self) -> Permit<'_> {
        Permit::new(self)
    }

    fn poll_acquire(
        &self,
        cx: &mut Context<'_>,
        waker_index: &mut Option<usize>,
        enqueue: &mut bool,
    ) -> Poll<Result<(), SemaphoreError>> {
        let mut curr = self.permits.load(Acquire);
        if curr & CLOSED == CLOSED {
            return Ready(Err(SemaphoreError::Closed));
        } else if *enqueue {
            *enqueue = false;
            return Ready(Ok(()));
        }
        let permit_num = 1 << PERMIT_SHIFT;
        loop {
            if curr & CLOSED == CLOSED {
                return Ready(Err(SemaphoreError::Closed));
            }
            if curr >= permit_num {
                match self
                    .permits
                    .compare_exchange(curr, curr - permit_num, AcqRel, Acquire)
                {
                    Ok(_) => {
                        if *enqueue {
                            self.release();
                            return Pending;
                        }
                        return Ready(Ok(()));
                    }
                    Err(actual) => {
                        curr = actual;
                    }
                }
            } else if !(*enqueue) {
                *waker_index = Some(self.waker_list.insert(cx.waker().clone()));
                *enqueue = true;
                curr = self.permits.load(Acquire);
            } else {
                return Pending;
            }
        }
    }
}

impl Debug for SemaphoreInner {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Semaphore")
            .field("permits", &self.current_permits())
            .finish()
    }
}

impl<'a> Permit<'a> {
    fn new(semaphore: &'a SemaphoreInner) -> Permit {
        Permit {
            semaphore,
            waker_index: None,
            enqueue: false,
        }
    }
}

impl Future for Permit<'_> {
    type Output = Result<(), SemaphoreError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let (semaphore, waker_index, enqueue) = unsafe {
            let me = self.get_unchecked_mut();
            (me.semaphore, &mut me.waker_index, &mut me.enqueue)
        };

        semaphore.poll_acquire(cx, waker_index, enqueue)
    }
}

impl Drop for Permit<'_> {
    fn drop(&mut self) {
        if self.enqueue {
            //if `enqueue` is true, `waker_index` must be `Some(_)`.
            let _ = self.semaphore.waker_list.remove(self.waker_index.unwrap());
        }
    }
}
