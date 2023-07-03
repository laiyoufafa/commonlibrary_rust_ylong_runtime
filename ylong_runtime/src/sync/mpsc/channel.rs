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

use crate::sync::mpsc::Container;
use std::ops::Deref;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::{AcqRel, Relaxed};
use std::sync::Arc;

pub(crate) struct Channel<C: Container> {
    chan: C,
    tx_cnt: AtomicUsize,
}

impl<C: Container> Channel<C> {
    fn new(chan: C) -> Channel<C> {
        Channel {
            chan,
            tx_cnt: AtomicUsize::new(1),
        }
    }
}

pub(crate) fn channel<C: Container>(chan: C) -> (Tx<C>, Rx<C>) {
    let channel = Arc::new(Channel::new(chan));
    (Tx::new(channel.clone()), Rx::new(channel))
}

pub(crate) struct Tx<C: Container> {
    inner: Arc<Channel<C>>,
}

impl<C: Container> Clone for Tx<C> {
    fn clone(&self) -> Self {
        self.inner.tx_cnt.fetch_add(1, Relaxed);
        Tx {
            inner: self.inner.clone(),
        }
    }
}

impl<C: Container> Tx<C> {
    fn new(channel: Arc<Channel<C>>) -> Tx<C> {
        Tx { inner: channel }
    }

    pub(crate) fn is_same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    pub(crate) fn close(&self) {
        if self.inner.tx_cnt.fetch_sub(1, AcqRel) == 1 {
            self.inner.chan.close();
        }
    }
}

impl<C: Container> Deref for Tx<C> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &self.inner.chan
    }
}

pub(crate) struct Rx<C: Container> {
    inner: Arc<Channel<C>>,
}

impl<C: Container> Rx<C> {
    fn new(channel: Arc<Channel<C>>) -> Rx<C> {
        Rx { inner: channel }
    }
}

impl<C: Container> Deref for Rx<C> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &self.inner.chan
    }
}
