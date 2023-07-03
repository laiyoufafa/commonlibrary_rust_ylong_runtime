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

use std::future::Future;
use std::mem;
use std::pin::Pin;
use std::sync::{Arc, Condvar, Mutex};
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

static BLOCK_ON_RAW_WAKER_VIRTUAL_TABLE: RawWakerVTable =
    RawWakerVTable::new(clone, wake, wake_by_ref, drop);

fn clone(ptr: *const ()) -> RawWaker {
    let thread = unsafe { Arc::from_raw(ptr as *const Parker) };

    // increment the ref count
    mem::forget(thread.clone());

    let data = Arc::into_raw(thread) as *const ();
    RawWaker::new(data, &BLOCK_ON_RAW_WAKER_VIRTUAL_TABLE)
}

fn wake(ptr: *const ()) {
    let thread = unsafe { Arc::from_raw(ptr as *const Parker) };
    thread.notify_one();
}

fn wake_by_ref(ptr: *const ()) {
    let thread = unsafe { Arc::from_raw(ptr as *const Parker) };
    thread.notify_one();
    mem::forget(thread);
}

fn drop(ptr: *const ()) {
    unsafe { mem::drop(Arc::from_raw(ptr as *const Parker)) };
}

pub(crate) struct ThreadParker {
    inner: Arc<Parker>,
}
impl ThreadParker {
    pub(crate) fn new() -> Self {
        ThreadParker {
            inner: Arc::new(Parker {
                mutex: Mutex::new(false),
                condvar: Condvar::new(),
            }),
        }
    }

    pub(crate) fn notified(&self) {
        self.inner.notified();
    }

    pub(crate) fn waker(&self) -> Waker {
        let data = Arc::into_raw(self.inner.clone()) as *const ();
        unsafe { Waker::from_raw(RawWaker::new(data, &BLOCK_ON_RAW_WAKER_VIRTUAL_TABLE)) }
    }
}

pub(crate) struct Parker {
    mutex: Mutex<bool>,
    condvar: Condvar,
}

impl Parker {
    fn notified(&self) {
        let mut guard = self.mutex.lock().unwrap();

        while !*guard {
            guard = self.condvar.wait(guard).unwrap();
        }
        *guard = false;
    }

    fn notify_one(&self) {
        let mut guard = self.mutex.lock().unwrap();
        *guard = true;
        mem::drop(guard);
        self.condvar.notify_one();
    }
}

pub(crate) fn block_on<T>(mut task: T) -> T::Output
where
    T: Future,
{
    let parker = ThreadParker::new();
    // create a waker based on the current thread
    let waker = parker.waker();
    let mut cx = Context::from_waker(&waker);

    let mut future = unsafe { Pin::new_unchecked(&mut task) };
    loop {
        if let Poll::Ready(res) = future.as_mut().poll(&mut cx) {
            return res;
        }

        parker.notified();
    }
}
