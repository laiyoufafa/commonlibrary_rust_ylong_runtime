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

use crate::task::raw::Header;
use std::future::Future;
use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::ops::Deref;
use std::ptr::NonNull;
use std::task::{RawWaker, RawWakerVTable, Waker};

unsafe fn get_header_by_raw_ptr(ptr: *const ()) -> NonNull<Header> {
    let header = ptr as *mut Header;
    let non_header = NonNull::new(header);
    if let Some(non_header) = non_header {
        non_header
    } else {
        panic!("task header is null");
    }
}

unsafe fn clone<T>(ptr: *const ()) -> RawWaker
where
    T: Future,
{
    let header = ptr as *const Header;
    (*header).state.inc_ref();
    raw_waker::<T>(header)
}

unsafe fn wake(ptr: *const ()) {
    let header = get_header_by_raw_ptr(ptr);
    let vir_tble = header.as_ref().vtable;
    (vir_tble.schedule)(header, true);
}

unsafe fn wake_by_ref(ptr: *const ()) {
    let header = get_header_by_raw_ptr(ptr);
    let vir_tble = header.as_ref().vtable;
    (vir_tble.schedule)(header, false);
}

unsafe fn drop(ptr: *const ()) {
    let header = get_header_by_raw_ptr(ptr);
    let vir_tble = header.as_ref().vtable;
    (vir_tble.drop_ref)(header);
}

fn raw_waker<T>(header: *const Header) -> RawWaker
where
    T: Future,
{
    let ptr = header as *const ();
    let raw_waker_ref = &RawWakerVTable::new(clone::<T>, wake, wake_by_ref, drop);
    RawWaker::new(ptr, raw_waker_ref)
}

/// Warps std::task::{RawWaker, RawWakerVTable, Waker} info, implements task notify and schedule
pub(crate) struct WakerRefHeader<'a> {
    waker: ManuallyDrop<Waker>,
    _field: PhantomData<&'a Header>,
}

impl WakerRefHeader<'_> {
    pub(crate) fn new<T>(header: &Header) -> WakerRefHeader<'_>
    where
        T: Future,
    {
        let waker = unsafe { ManuallyDrop::new(Waker::from_raw(raw_waker::<T>(header))) };

        WakerRefHeader {
            waker,
            _field: PhantomData,
        }
    }
}

impl Deref for WakerRefHeader<'_> {
    type Target = Waker;

    fn deref(&self) -> &Self::Target {
        &self.waker
    }
}
