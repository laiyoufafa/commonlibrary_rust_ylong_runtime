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

use super::Handle;
use crate::sys::Overlapped;
use std::os::windows::io::{AsRawHandle, IntoRawHandle, RawHandle};
use std::time::Duration;
use std::{cmp, io};
use windows_sys::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::IO::{
    CreateIoCompletionPort, GetQueuedCompletionStatusEx, PostQueuedCompletionStatus, OVERLAPPED,
    OVERLAPPED_ENTRY,
};

/// IOCP's HANDLE.
#[derive(Debug)]
pub(crate) struct CompletionPort {
    handle: Handle,
}

impl CompletionPort {
    /// Creates a new CompletionPort
    pub(crate) fn new() -> io::Result<CompletionPort> {
        let handle = unsafe { CreateIoCompletionPort(INVALID_HANDLE_VALUE, 0, 0, 0) };
        if handle == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(CompletionPort {
                handle: Handle::new(handle),
            })
        }
    }
    /// Add t to CompletionPort
    pub(crate) fn add_handle<T: AsRawHandle + ?Sized>(
        &self,
        token: usize,
        t: &T,
    ) -> io::Result<()> {
        syscall!(
            CreateIoCompletionPort(t.as_raw_handle() as HANDLE, self.handle.raw(), token, 0),
            ()
        )
    }

    /// Gets the completed events in the IOCP, and can get multiple events at the same time.
    /// Return the set of completed events
    pub(crate) fn get_results<'a>(
        &self,
        list: &'a mut [CompletionStatus],
        timeout: Option<Duration>,
    ) -> io::Result<&'a mut [CompletionStatus]> {
        let len = cmp::min(list.len(), u32::MAX as usize) as u32;
        let mut removed = 0;
        let timeout = match timeout {
            Some(dur) => {
                let dur_ms = (dur + Duration::from_nanos(999_999)).as_millis();
                cmp::min(dur_ms, u32::MAX as u128) as u32
            }
            None => u32::MAX,
        };

        syscall!(
            GetQueuedCompletionStatusEx(
                self.handle.raw(),
                list.as_ptr() as *mut _,
                len,
                &mut removed,
                timeout,
                0,
            ),
            &mut list[..removed as usize]
        )
    }

    /// Posts an I/O completion packet to an I/O completion port.
    pub(crate) fn post(&self, status: CompletionStatus) -> io::Result<()> {
        syscall!(
            PostQueuedCompletionStatus(
                self.handle.raw(),
                status.0.dwNumberOfBytesTransferred,
                status.0.lpCompletionKey,
                status.0.lpOverlapped
            ),
            ()
        )
    }
}

impl IntoRawHandle for CompletionPort {
    fn into_raw_handle(self) -> RawHandle {
        self.handle.into_raw()
    }
}

/// Includes OVERLAPPED_ENTRY struct which contains operation result of IOCP
#[derive(Clone, Copy)]
pub struct CompletionStatus(OVERLAPPED_ENTRY);

unsafe impl Send for CompletionStatus {}
unsafe impl Sync for CompletionStatus {}

impl CompletionStatus {
    /// Creates a new `CompletionStatus`.
    pub(crate) fn new(bytes: u32, token: usize, overlapped: *mut Overlapped) -> Self {
        CompletionStatus(OVERLAPPED_ENTRY {
            dwNumberOfBytesTransferred: bytes,
            lpCompletionKey: token,
            lpOverlapped: overlapped as *mut _,
            Internal: 0,
        })
    }

    /// Creates a CompletionStatus with 0.
    pub fn zero() -> Self {
        Self::new(0, 0, std::ptr::null_mut())
    }

    /// Returns dwNumberOfBytesTransferred of OVERLAPPED_ENTRY.
    pub fn bytes_transferred(&self) -> u32 {
        self.0.dwNumberOfBytesTransferred
    }

    /// Returns lpCompletionKey of OVERLAPPED_ENTRY.
    pub fn token(&self) -> usize {
        self.0.lpCompletionKey
    }

    /// Returns lpOverlapped of OVERLAPPED_ENTRY.
    pub fn overlapped(&self) -> *mut OVERLAPPED {
        self.0.lpOverlapped
    }

    /// Returns OVERLAPPED_ENTRY struct.
    pub fn entry(&self) -> &OVERLAPPED_ENTRY {
        &self.0
    }
}
