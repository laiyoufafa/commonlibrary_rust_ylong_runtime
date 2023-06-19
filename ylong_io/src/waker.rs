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

use crate::sys::WakerInner;
use crate::{Poll, Token};
use std::io;

/// Waker allows cross-thread waking of Poll.
#[derive(Debug)]
pub struct Waker {
    inner: WakerInner,
}

impl Waker {
    /// Creates a new Waker
    pub fn new(poll: &Poll, token: Token) -> io::Result<Self> {
        WakerInner::new(poll.selector(), token).map(|inner| Waker { inner })
    }
    /// Wakes up the [`Poll`] associated with this `Waker`
    pub fn wake(&self) -> io::Result<()> {
        self.inner.wake()
    }
}
