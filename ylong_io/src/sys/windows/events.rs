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

use crate::sys::windows::afd::{
    POLL_ABORT, POLL_ACCEPT, POLL_CONNECT_FAIL, POLL_DISCONNECT, POLL_RECEIVE, POLL_SEND,
};
use crate::sys::windows::iocp::CompletionStatus;
use crate::{EventTrait, Token};
use std::fmt;

/// An io event
#[derive(Debug)]
pub struct Event {
    pub(crate) flags: u32,
    pub(crate) data: u64,
}

impl Event {
    /// Creates a new `Event` with `Token`.
    pub(crate) fn new(token: Token) -> Event {
        Event {
            flags: 0,
            data: usize::from(token) as u64,
        }
    }

    /// Sets `Event` as readable.
    pub(super) fn set_readable(&mut self) {
        self.flags |= POLL_RECEIVE
    }

    /// Converts `Event` to `CompletionStatus`.
    pub(super) fn as_completion_status(&self) -> CompletionStatus {
        CompletionStatus::new(self.flags, self.data as usize, std::ptr::null_mut())
    }

    /// Converts `CompletionStatus` to `Event`.
    pub(super) fn from_completion_status(status: &CompletionStatus) -> Event {
        Event {
            flags: status.bytes_transferred(),
            data: status.token() as u64,
        }
    }
}

/// Storage completed events
pub struct Events {
    pub(crate) status: Box<[CompletionStatus]>,
    pub(crate) events: Vec<Event>,
}

impl Events {
    /// Creates a new `Events`.
    pub fn with_capacity(cap: usize) -> Events {
        Events {
            status: vec![CompletionStatus::zero(); cap].into_boxed_slice(),
            events: Vec::with_capacity(cap),
        }
    }

    /// Clear the Events
    pub fn clear(&mut self) {
        self.events.clear();
        for status in self.status.iter_mut() {
            *status = CompletionStatus::zero();
        }
    }

    /// Returns an iterator over the slice.
    /// The iterator yields all items from start to end.
    pub fn iter(&self) -> IterEvent {
        IterEvent {
            iter: &self.events,
            index: 0,
        }
    }

    /// Returns true if the vector contains no elements.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

impl fmt::Debug for Events {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self).finish()
    }
}

impl<'a> IntoIterator for &'a Events {
    type Item = &'a Event;
    type IntoIter = IterEvent<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

pub struct IterEvent<'a> {
    iter: &'a Vec<Event>,
    index: usize,
}

impl<'a> Iterator for IterEvent<'a> {
    type Item = &'a Event;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.iter.len() {
            self.index += 1;
            Some(&self.iter[self.index - 1])
        } else {
            None
        }
    }

    fn count(self) -> usize {
        self.iter.len()
    }
}

pub(crate) const READABLE_FLAGS: u32 =
    POLL_RECEIVE | POLL_DISCONNECT | POLL_ABORT | POLL_ACCEPT | POLL_CONNECT_FAIL;
pub(crate) const WRITABLE_FLAGS: u32 = POLL_SEND | POLL_ABORT | POLL_CONNECT_FAIL;
pub(crate) const READ_CLOSED_FLAGS: u32 = POLL_DISCONNECT | POLL_ABORT | POLL_CONNECT_FAIL;
pub(crate) const WRITE_CLOSED_FLAGS: u32 = POLL_ABORT | POLL_CONNECT_FAIL;
pub(crate) const ERROR_FLAGS: u32 = POLL_CONNECT_FAIL;

impl EventTrait for Event {
    fn token(&self) -> Token {
        Token(self.data as usize)
    }

    fn is_readable(&self) -> bool {
        self.flags & READABLE_FLAGS != 0
    }

    fn is_writable(&self) -> bool {
        (self.flags & WRITABLE_FLAGS) != 0
    }

    fn is_read_closed(&self) -> bool {
        self.flags & READ_CLOSED_FLAGS != 0
    }

    fn is_write_closed(&self) -> bool {
        self.flags & WRITE_CLOSED_FLAGS != 0
    }

    fn is_error(&self) -> bool {
        self.flags & ERROR_FLAGS != 0
    }
}
