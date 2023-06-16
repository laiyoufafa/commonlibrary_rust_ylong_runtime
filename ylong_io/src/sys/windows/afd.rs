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

use crate::sys::windows::iocp::CompletionPort;
use std::ffi::c_void;
use std::fs::File;
use std::mem::{size_of, zeroed};
use std::os::windows::io::{AsRawHandle, FromRawHandle, RawHandle};
use std::ptr::null_mut;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::{fmt, io};
use windows_sys::Win32::Foundation::{
    RtlNtStatusToDosError, HANDLE, INVALID_HANDLE_VALUE, NTSTATUS, STATUS_NOT_FOUND,
    STATUS_PENDING, STATUS_SUCCESS, UNICODE_STRING,
};
use windows_sys::Win32::Storage::FileSystem::{
    NtCreateFile, SetFileCompletionNotificationModes, FILE_OPEN, FILE_SHARE_READ, FILE_SHARE_WRITE,
    SYNCHRONIZE,
};
use windows_sys::Win32::System::WindowsProgramming::{
    NtDeviceIoControlFile, FILE_SKIP_SET_EVENT_ON_HANDLE, IO_STATUS_BLOCK, IO_STATUS_BLOCK_0,
    OBJECT_ATTRIBUTES,
};

pub const POLL_RECEIVE: u32 = 0x0001;
pub const POLL_RECEIVE_EXPEDITED: u32 = 0x0002;
pub const POLL_SEND: u32 = 0x0004;
pub const POLL_DISCONNECT: u32 = 0x0008;
pub const POLL_ABORT: u32 = 0x0010;
pub const POLL_LOCAL_CLOSE: u32 = 0x0020;
pub const POLL_ACCEPT: u32 = 0x0080;
pub const POLL_CONNECT_FAIL: u32 = 0x0100;

pub const ALL_EVENTS: u32 = POLL_RECEIVE
    | POLL_RECEIVE_EXPEDITED
    | POLL_SEND
    | POLL_DISCONNECT
    | POLL_ACCEPT
    | POLL_LOCAL_CLOSE
    | POLL_ABORT
    | POLL_CONNECT_FAIL;

const AFD_ATTRIBUTES: OBJECT_ATTRIBUTES = OBJECT_ATTRIBUTES {
    Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
    RootDirectory: 0,
    ObjectName: &OBJ_NAME as *const _ as *mut _,
    Attributes: 0,
    SecurityDescriptor: null_mut(),
    SecurityQualityOfService: null_mut(),
};
const OBJ_NAME: UNICODE_STRING = UNICODE_STRING {
    Length: (AFD_HELPER_NAME.len() * size_of::<u16>()) as u16,
    MaximumLength: (AFD_HELPER_NAME.len() * size_of::<u16>()) as u16,
    Buffer: AFD_HELPER_NAME.as_ptr() as *mut _,
};
const AFD_HELPER_NAME: &[u16] = &[
    '\\' as _, 'D' as _, 'e' as _, 'v' as _, 'i' as _, 'c' as _, 'e' as _, '\\' as _, 'A' as _,
    'f' as _, 'd' as _, '\\' as _, 'Y' as _, 'l' as _, 'o' as _, 'n' as _, 'g' as _,
];

static NEXT_TOKEN: AtomicUsize = AtomicUsize::new(0);
const IOCTL_AFD_POLL: u32 = 0x00012024;

#[link(name = "ntdll")]
extern "system" {
    fn NtCancelIoFileEx(
        FileHandle: HANDLE,
        IoRequestToCancel: *mut IO_STATUS_BLOCK,
        IoStatusBlock: *mut IO_STATUS_BLOCK,
    ) -> NTSTATUS;
}

/// Asynchronous file descriptor
/// Implementing a single file handle to monitor multiple Io operations using the IO multiplexing model.
#[derive(Debug)]
pub struct Afd {
    fd: File,
}

impl Afd {
    /// Creates a new Afd and add it to CompletionPort
    fn new(cp: &CompletionPort) -> io::Result<Afd> {
        let mut afd_device_handle: HANDLE = INVALID_HANDLE_VALUE;
        let mut io_status_block = IO_STATUS_BLOCK {
            Anonymous: IO_STATUS_BLOCK_0 { Status: 0 },
            Information: 0,
        };

        let fd = unsafe {
            let status = NtCreateFile(
                &mut afd_device_handle as *mut _,
                SYNCHRONIZE,
                &AFD_ATTRIBUTES as *const _ as *mut _,
                &mut io_status_block,
                null_mut(),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                FILE_OPEN,
                0,
                null_mut(),
                0,
            );

            if status != STATUS_SUCCESS {
                let raw_error = io::Error::from_raw_os_error(RtlNtStatusToDosError(status) as i32);

                let msg = format!("Failed to open \\Device\\Afd\\Ylong: {raw_error}");
                return Err(io::Error::new(raw_error.kind(), msg));
            }

            File::from_raw_handle(afd_device_handle as RawHandle)
        };

        let token = NEXT_TOKEN.fetch_add(2, Ordering::Relaxed) + 2;
        let afd = Afd { fd };
        // Add Afd to CompletionPort
        cp.add_handle(token, &afd.fd)?;

        syscall!(
            SetFileCompletionNotificationModes(
                afd_device_handle,
                FILE_SKIP_SET_EVENT_ON_HANDLE as u8,
            ),
            afd
        )
    }

    /// System call
    pub(crate) unsafe fn poll(
        &self,
        info: &mut AfdPollInfo,
        iosb: *mut IO_STATUS_BLOCK,
        overlapped: *mut c_void,
    ) -> io::Result<bool> {
        let afd_info = info as *mut _ as *mut c_void;
        (*iosb).Anonymous.Status = STATUS_PENDING;

        let status = NtDeviceIoControlFile(
            self.fd.as_raw_handle() as HANDLE,
            0,
            None,
            overlapped,
            iosb,
            IOCTL_AFD_POLL,
            afd_info,
            size_of::<AfdPollInfo>() as u32,
            afd_info,
            size_of::<AfdPollInfo>() as u32,
        );

        match status {
            STATUS_SUCCESS => Ok(true),
            // this is expected.
            STATUS_PENDING => Ok(false),
            _ => Err(io::Error::from_raw_os_error(
                RtlNtStatusToDosError(status) as i32
            )),
        }
    }

    /// System call to cancel File HANDLE.
    pub(crate) unsafe fn cancel(&self, iosb: *mut IO_STATUS_BLOCK) -> io::Result<()> {
        if (*iosb).Anonymous.Status != STATUS_PENDING {
            return Ok(());
        }

        let mut cancel_iosb = IO_STATUS_BLOCK {
            Anonymous: IO_STATUS_BLOCK_0 { Status: 0 },
            Information: 0,
        };
        let status = NtCancelIoFileEx(self.fd.as_raw_handle() as HANDLE, iosb, &mut cancel_iosb);
        match status {
            STATUS_SUCCESS | STATUS_NOT_FOUND => Ok(()),
            _ => Err(io::Error::from_raw_os_error(
                RtlNtStatusToDosError(status) as i32
            )),
        }
    }
}

/// A group which contains Afds.
#[derive(Debug)]
pub(crate) struct AfdGroup {
    cp: Arc<CompletionPort>,
    afd_group: Mutex<Vec<Arc<Afd>>>,
}

/// Up to 32 Arc points per Afd.
const POLL_GROUP__MAX_GROUP_SIZE: usize = 32;

impl AfdGroup {
    /// Creates a new AfdGroup.
    pub(crate) fn new(cp: Arc<CompletionPort>) -> AfdGroup {
        AfdGroup {
            afd_group: Mutex::new(Vec::new()),
            cp,
        }
    }

    /// Gets a new point to File.
    pub(crate) fn acquire(&self) -> io::Result<Arc<Afd>> {
        let mut afd_group = self.afd_group.lock().unwrap();

        // When the last File has more than 32 Arc Points, creates a new File.
        if afd_group.len() == 0
            || Arc::strong_count(afd_group.last().unwrap()) > POLL_GROUP__MAX_GROUP_SIZE
        {
            let arc = Arc::new(Afd::new(&self.cp)?);
            afd_group.push(arc);
        }

        match afd_group.last() {
            Some(arc) => Ok(arc.clone()),
            None => unreachable!(
                "Cannot acquire afd, {:#?}, afd_group: {:#?}",
                self, afd_group
            ),
        }
    }

    /// Delete Afd that is no longer in use from AfdGroup.
    pub(crate) fn release_unused_afd(&self) {
        let mut afd_group = self.afd_group.lock().unwrap();
        afd_group.retain(|g| Arc::strong_count(g) > 1);
    }
}

#[repr(C)]
pub struct AfdPollInfo {
    pub timeout: i64,
    pub number_of_handles: u32,
    pub exclusive: u32,
    pub handles: [AfdPollHandleInfo; 1],
}

impl AfdPollInfo {
    pub(crate) fn zeroed() -> AfdPollInfo {
        unsafe { zeroed() }
    }
}

impl fmt::Debug for AfdPollInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AfdPollInfo").finish()
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct AfdPollHandleInfo {
    /// SockState base_socket
    pub handle: HANDLE,
    pub events: u32,
    pub status: NTSTATUS,
}

unsafe impl Send for AfdPollHandleInfo {}
