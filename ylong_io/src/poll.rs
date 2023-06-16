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

use crate::{Events, Interest, Selector, Source, Token};
use std::io;
use std::time::Duration;

/// Registers and deregisters an io's fd
pub struct Poll {
    selector: Selector,
}

impl Poll {
    /// Creates a new Poll.
    pub fn new() -> io::Result<Poll> {
        Selector::new().map(|selector| Poll {
            selector: { selector },
        })
    }

    /// Gets all interested io events within a time limit and writes them into `event`.
    ///
    /// If `timeout` is none, then it will block the current thread until an event arrives.
    pub fn poll(&self, events: &mut Events, timeout: Option<Duration>) -> io::Result<()> {
        self.selector.select(events, timeout)
    }

    /// Registers the I/O source's fd in order to monitor its io events.
    pub fn register<S>(&self, source: &mut S, token: Token, interests: Interest) -> io::Result<()>
    where
        S: Source + ?Sized,
    {
        source.register(&self.selector, token, interests)
    }

    /// Re-registers the I/O source's fd in order to monitor its io events.
    pub fn reregister<S>(&self, source: &mut S, token: Token, interests: Interest) -> io::Result<()>
    where
        S: Source + ?Sized,
    {
        source.reregister(&self.selector, token, interests)
    }

    /// De-registers the I/O source's fd so the Poll will no longer monitor its io events.
    pub fn deregister<S>(&self, source: &mut S) -> io::Result<()>
    where
        S: Source + ?Sized,
    {
        source.deregister(&self.selector)
    }

    /// Returns the selector of the `Poll`
    pub fn selector(&self) -> &Selector {
        &self.selector
    }
}

impl std::fmt::Debug for Poll {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(fmt, "({:?})", self.selector)
    }
}
