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

use crate::{Events, Interest, Token};
use std::io;
use std::os::raw::{c_int, c_uint};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static NEXT_ID: AtomicUsize = AtomicUsize::new(1);

/// An wrapper to block different OS polling system.
/// Linux: epoll
/// Windows: iocp
pub struct Selector {
    id: usize, //selector id
    ep: i32,   //epoll fd
}

impl Selector {
    /// Creates a new Selector.
    ///
    /// # Error
    /// If the underlying syscall fails, returns the corresponding error.
    pub fn new() -> io::Result<Selector> {
        let ep = match syscall!(epoll_create1(libc::EPOLL_CLOEXEC)) {
            Ok(ep_sys) => ep_sys,
            Err(err) => {
                return Err(err);
            }
        };

        Ok(Selector {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            ep,
        })
    }

    /// Waits for io events to come within a time limit.
    pub fn select(&self, events: &mut Events, timeout: Option<Duration>) -> io::Result<()> {
        // Convert to milliseconds, if input time is none, it means the timeout is -1 and wait permanently.
        let timeout = timeout.map(|time| time.as_millis() as c_int).unwrap_or(-1);

        events.clear();

        match syscall!(epoll_wait(
            self.ep,
            events.as_mut_ptr(),
            events.capacity() as i32,
            timeout
        )) {
            Ok(n_events) => unsafe { events.set_len(n_events as usize) },
            Err(err) => {
                return Err(err);
            }
        }
        Ok(())
    }

    /// Registers the fd with specific interested events
    pub fn register(&self, fd: i32, token: Token, interests: Interest) -> io::Result<()> {
        let mut sys_event = libc::epoll_event {
            events: interests_to_io_event(interests),
            u64: usize::from(token) as u64,
        };

        match syscall!(epoll_ctl(self.ep, libc::EPOLL_CTL_ADD, fd, &mut sys_event)) {
            Ok(_) => Ok(()),
            Err(err) => Err(err),
        }
    }

    /// Re-registers the fd with specific interested events
    pub fn reregister(&self, fd: i32, token: Token, interests: Interest) -> io::Result<()> {
        let mut sys_event = libc::epoll_event {
            events: interests_to_io_event(interests),
            u64: usize::from(token) as u64,
        };

        match syscall!(epoll_ctl(self.ep, libc::EPOLL_CTL_MOD, fd, &mut sys_event)) {
            Ok(_) => Ok(()),
            Err(err) => Err(err),
        }
    }

    /// De-registers the fd.
    pub fn deregister(&self, fd: i32) -> io::Result<()> {
        match syscall!(epoll_ctl(
            self.ep,
            libc::EPOLL_CTL_DEL,
            fd,
            std::ptr::null_mut() as *mut libc::epoll_event
        )) {
            Ok(_) => Ok(()),
            Err(err) => Err(err),
        }
    }
}

fn interests_to_io_event(interests: Interest) -> c_uint {
    let mut io_event = libc::EPOLLET as u32;

    if interests.is_readable() {
        io_event |= libc::EPOLLIN as u32;
        io_event |= libc::EPOLLRDHUP as u32;
    }

    if interests.is_writable() {
        io_event |= libc::EPOLLOUT as u32;
    }

    io_event as c_uint
}

impl Drop for Selector {
    fn drop(&mut self) {
        if let Err(_err) = syscall!(close(self.ep)) {
            // todo: log the error
        }
    }
}

impl std::fmt::Debug for Selector {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(fmt, "epoll fd: {}, Select id: {}", self.ep, self.id)
    }
}
