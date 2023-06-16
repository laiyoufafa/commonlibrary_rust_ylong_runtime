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

use crate::sys::windows::afd;
use crate::sys::windows::afd::{Afd, AfdGroup, AfdPollInfo};
use crate::sys::windows::events::{
    Events, ERROR_FLAGS, READABLE_FLAGS, READ_CLOSED_FLAGS, WRITABLE_FLAGS, WRITE_CLOSED_FLAGS,
};
use crate::sys::windows::io_status_block::IoStatusBlock;
use crate::sys::windows::iocp::{CompletionPort, CompletionStatus};
use crate::sys::NetInner;
use crate::{Event, Interest, Token};
use std::collections::VecDeque;
use std::ffi::c_void;
use std::io;
use std::marker::PhantomPinned;
use std::mem::size_of;
use std::os::windows::io::RawSocket;
use std::pin::Pin;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use windows_sys::Win32::Foundation::{
    ERROR_INVALID_HANDLE, ERROR_IO_PENDING, HANDLE, STATUS_CANCELLED, WAIT_TIMEOUT,
};
use windows_sys::Win32::Networking::WinSock::{
    WSAGetLastError, WSAIoctl, SIO_BASE_HANDLE, SIO_BSP_HANDLE, SIO_BSP_HANDLE_POLL,
    SIO_BSP_HANDLE_SELECT, SOCKET_ERROR,
};
use windows_sys::Win32::System::IO::OVERLAPPED;

/// An wrapper to block different OS polling system.
/// Linux: epoll
/// Windows: iocp
#[derive(Debug)]
pub struct Selector {
    inner: Arc<SelectorInner>,
}

impl Selector {
    pub(crate) fn new() -> io::Result<Selector> {
        SelectorInner::new().map(|inner| Selector {
            inner: Arc::new(inner),
        })
    }

    pub(crate) fn select(&self, events: &mut Events, timeout: Option<Duration>) -> io::Result<()> {
        self.inner.select(events, timeout)
    }

    pub(crate) fn register(
        &self,
        socket: RawSocket,
        token: Token,
        interests: Interest,
    ) -> io::Result<NetInner> {
        SelectorInner::register(&self.inner, socket, token, interests)
    }

    pub(crate) fn reregister(
        &self,
        sock_state: Pin<Arc<Mutex<SockState>>>,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        self.inner.reregister(sock_state, token, interests)
    }

    pub(crate) fn clone_cp(&self) -> Arc<CompletionPort> {
        self.inner.completion_port.clone()
    }
}

#[derive(Debug)]
pub(crate) struct SelectorInner {
    /// IOCP Handle.
    completion_port: Arc<CompletionPort>,
    /// Registered/re-registered IO events are placed in this queue.
    update_queue: Mutex<VecDeque<Pin<Arc<Mutex<SockState>>>>>,
    /// Afd Group.
    afd_group: AfdGroup,
    /// Weather the Selector is polling.
    polling: AtomicBool,
}

impl SelectorInner {
    /// Creates a new SelectorInner
    fn new() -> io::Result<SelectorInner> {
        CompletionPort::new().map(|cp| {
            let arc_cp = Arc::new(cp);
            let cp_afd = Arc::clone(&arc_cp);

            SelectorInner {
                completion_port: arc_cp,
                update_queue: Mutex::new(VecDeque::new()),
                afd_group: AfdGroup::new(cp_afd),
                polling: AtomicBool::new(false),
            }
        })
    }

    /// Start poll
    fn select(&self, events: &mut Events, timeout: Option<Duration>) -> io::Result<()> {
        events.clear();

        match timeout {
            None => loop {
                let len = self.select_inner(events, timeout)?;
                if len != 0 {
                    return Ok(());
                }
            },
            Some(_) => {
                self.select_inner(events, timeout)?;
                Ok(())
            }
        }
    }

    fn select_inner(&self, events: &mut Events, timeout: Option<Duration>) -> io::Result<usize> {
        // We can only poll once at the same time.
        if self.polling.swap(true, Ordering::AcqRel) {
            panic!("Can't be polling twice at same time!");
        }

        unsafe { self.update_sockets_events() }?;

        let results = self
            .completion_port
            .get_results(&mut events.status, timeout);

        self.polling.store(false, Ordering::Relaxed);

        match results {
            Ok(iocp_events) => Ok(unsafe { self.feed_events(&mut events.events, iocp_events) }),
            Err(ref e) if e.raw_os_error() == Some(WAIT_TIMEOUT as i32) => Ok(0),
            Err(e) => Err(e),
        }
    }

    /// Process completed operation and put them into events; regular AFD events are put back into VecDeque
    unsafe fn feed_events(
        &self,
        events: &mut Vec<Event>,
        iocp_events: &[CompletionStatus],
    ) -> usize {
        let mut epoll_event_count = 0;
        let mut update_queue = self.update_queue.lock().unwrap();
        for iocp_event in iocp_events.iter() {
            if iocp_event.overlapped().is_null() {
                events.push(Event::from_completion_status(iocp_event));
                epoll_event_count += 1;
                continue;
            } else if iocp_event.token() % 2 == 1 {
                // Non-AFD event, including pipe.
                let callback = (*(iocp_event.overlapped() as *mut super::Overlapped)).callback;

                let len = events.len();
                callback(iocp_event.entry(), Some(events));
                epoll_event_count += events.len() - len;
                continue;
            }

            // General asynchronous IO event.
            let sock_state = from_overlapped(iocp_event.overlapped());
            let mut sock_guard = sock_state.lock().unwrap();
            if let Some(event) = sock_guard.sock_feed_event() {
                events.push(event);
                epoll_event_count += 1;
            }

            // Reregister the socket.
            if !sock_guard.is_delete_pending() {
                update_queue.push_back(sock_state.clone());
            }
        }

        self.afd_group.release_unused_afd();
        epoll_event_count
    }

    /// Updates each SockState in the Deque, started only when Poll::poll() is called externally
    unsafe fn update_sockets_events(&self) -> io::Result<()> {
        let mut update_queue = self.update_queue.lock().unwrap();
        for sock in update_queue.iter_mut() {
            let mut sock_internal = sock.lock().unwrap();
            if !sock_internal.delete_pending {
                sock_internal.update(sock)?;
            }
        }
        // Deletes events which has been updated successful.
        update_queue.retain(|sock| sock.lock().unwrap().has_error());

        self.afd_group.release_unused_afd();
        Ok(())
    }

    /// No actual system call is made at register, it only starts at Poll::poll().
    /// Return Arc<NetInternal> and put it in the asynchronous IO structure
    pub(crate) fn register(
        this: &Arc<Self>,
        raw_socket: RawSocket,
        token: Token,
        interests: Interest,
    ) -> io::Result<NetInner> {
        // Creates Afd
        let afd = this.afd_group.acquire()?;
        let mut sock_state = SockState::new(raw_socket, afd)?;

        let flags = interests_to_afd_flags(interests);
        sock_state.set_event(flags, token.0 as u64);

        let pin_sock_state = Arc::pin(Mutex::new(sock_state));

        let net_internal = NetInner::new(this.clone(), token, interests, pin_sock_state.clone());

        // Adds SockState to VecDeque
        this.queue_state(pin_sock_state);

        if this.polling.load(Ordering::Acquire) {
            unsafe { this.update_sockets_events()? }
        }

        Ok(net_internal)
    }

    /// Re-register, put SockState back into VecDeque
    pub(crate) fn reregister(
        &self,
        state: Pin<Arc<Mutex<SockState>>>,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        let flags = interests_to_afd_flags(interests);
        state.lock().unwrap().set_event(flags, token.0 as u64);

        // Put back in the update queue VecDeque
        self.queue_state(state);

        if self.polling.load(Ordering::Acquire) {
            unsafe { self.update_sockets_events() }
        } else {
            Ok(())
        }
    }

    /// Adds SockState to VecDeque last.
    fn queue_state(&self, sock_state: Pin<Arc<Mutex<SockState>>>) {
        let mut update_queue = self.update_queue.lock().unwrap();
        update_queue.push_back(sock_state);
    }
}

impl Drop for SelectorInner {
    fn drop(&mut self) {
        loop {
            let complete_num: usize;
            let mut status: [CompletionStatus; 1024] = [CompletionStatus::zero(); 1024];

            let result = self
                .completion_port
                .get_results(&mut status, Some(Duration::from_millis(0)));

            match result {
                Ok(iocp_events) => {
                    complete_num = iocp_events.iter().len();
                    for iocp_event in iocp_events.iter() {
                        if iocp_event.overlapped().is_null() {
                            // User event
                        } else if iocp_event.token() % 2 == 1 {
                            // For pipe, dispatch the event so it can release resources
                            let callback = unsafe {
                                (*(iocp_event.overlapped() as *mut super::Overlapped)).callback
                            };

                            callback(iocp_event.entry(), None);
                        } else {
                            // Release memory of Arc reference
                            let _ = from_overlapped(iocp_event.overlapped());
                        }
                    }
                }

                Err(_) => {
                    break;
                }
            }

            if complete_num == 0 {
                // continue looping until all completion status have been drained
                break;
            }
        }

        self.afd_group.release_unused_afd();
    }
}

#[derive(Debug, PartialEq)]
enum SockPollStatus {
    /// Initial Value.
    Idle,
    /// System function called when updating sockets_events, set from Idle to Pending. Update only when polling.
    /// Only the socket of Pending can be cancelled.
    Pending,
    /// After calling the system api to cancel the sock, set it to Cancelled.
    Cancelled,
}

/// Saves all information of the socket during polling.
#[derive(Debug)]
pub struct SockState {
    iosb: IoStatusBlock,
    poll_info: AfdPollInfo,
    /// The file handle to which request is bound.
    afd: Arc<Afd>,
    /// SOCKET of the request
    base_socket: RawSocket,
    /// User Token
    user_token: u64,
    /// user Interest
    user_interests_flags: u32,
    /// When this socket is polled， save user_interests_flags in polling_interests_flags. Used for comparison during re-registration.
    polling_interests_flags: u32,
    /// Current Status. When this is Pending, System API calls must be made.
    poll_status: SockPollStatus,
    /// Mark if it is deleted.
    delete_pending: bool,
    /// Error during updating
    error: Option<i32>,

    _pinned: PhantomPinned,
}

impl SockState {
    /// Creates a new SockState with RawSocket and Afd.
    fn new(socket: RawSocket, afd: Arc<Afd>) -> io::Result<SockState> {
        Ok(SockState {
            iosb: IoStatusBlock::zeroed(),
            poll_info: AfdPollInfo::zeroed(),
            afd,
            base_socket: get_base_socket(socket)?,
            user_interests_flags: 0,
            polling_interests_flags: 0,
            user_token: 0,
            poll_status: SockPollStatus::Idle,
            delete_pending: false,

            error: None,
            _pinned: PhantomPinned,
        })
    }

    /// Update SockState in Deque, poll for each Afd.
    fn update(&mut self, self_arc: &Pin<Arc<Mutex<SockState>>>) -> io::Result<()> {
        // delete_pending must false.
        if self.delete_pending {
            panic!("SockState update when delete_pending is true, {:#?}", self);
        }

        // Make sure to reset previous error before a new update
        self.error = None;

        match self.poll_status {
            // Starts poll
            SockPollStatus::Idle => {
                // Init AfdPollInfo
                self.poll_info.exclusive = 0;
                self.poll_info.number_of_handles = 1;
                self.poll_info.timeout = i64::MAX;
                self.poll_info.handles[0].handle = self.base_socket as HANDLE;
                self.poll_info.handles[0].status = 0;
                self.poll_info.handles[0].events =
                    self.user_interests_flags | afd::POLL_LOCAL_CLOSE;

                let overlapped_ptr = into_overlapped(self_arc.clone());

                // System call to run current event.
                let result = unsafe {
                    self.afd
                        .poll(&mut self.poll_info, &mut *self.iosb, overlapped_ptr)
                };

                if let Err(e) = result {
                    let code = e.raw_os_error().unwrap();
                    if code != ERROR_IO_PENDING as i32 {
                        drop(from_overlapped(overlapped_ptr as *mut _));

                        return if code == ERROR_INVALID_HANDLE as i32 {
                            // Socket closed; it'll be dropped.
                            self.start_drop();
                            Ok(())
                        } else {
                            self.error = e.raw_os_error();
                            Err(e)
                        };
                    }
                };

                // The poll request was successfully submitted.
                self.poll_status = SockPollStatus::Pending;
                self.polling_interests_flags = self.user_interests_flags;
            }
            SockPollStatus::Pending => {
                if (self.user_interests_flags & afd::ALL_EVENTS & !self.polling_interests_flags)
                    == 0
                {
                    // All the events the user is interested in are already being monitored by
                    // the pending poll operation. It might spuriously complete because of an
                    // event that we're no longer interested in; when that happens we'll submit
                    // a new poll operation with the updated event mask.
                } else {
                    // A poll operation is already pending, but it's not monitoring for all the
                    // events that the user is interested in. Therefore, cancel the pending
                    // poll operation; when we receive it's completion package, a new poll
                    // operation will be submitted with the correct event mask.
                    if let Err(e) = self.cancel() {
                        self.error = e.raw_os_error();
                        return Err(e);
                    }
                }
            }
            // Do nothing
            SockPollStatus::Cancelled => {}
        }

        Ok(())
    }

    /// Returns true if user_interests_flags is inconsistent with polling_interests_flags.
    fn set_event(&mut self, flags: u32, token_data: u64) -> bool {
        self.user_interests_flags = flags | afd::POLL_CONNECT_FAIL | afd::POLL_ABORT;
        self.user_token = token_data;

        (self.user_interests_flags & !self.polling_interests_flags) != 0
    }

    /// Process completed IO operation.
    fn sock_feed_event(&mut self) -> Option<Event> {
        self.poll_status = SockPollStatus::Idle;
        self.polling_interests_flags = 0;

        let mut afd_events = 0;
        // Uses the status info in IO_STATUS_BLOCK to determine the socket poll status. It is unsafe to use a pointer of IO_STATUS_BLOCK.
        unsafe {
            if self.delete_pending {
                return None;
            } else if self.iosb.Anonymous.Status == STATUS_CANCELLED {
                // The poll request was cancelled by CancelIoEx.
            } else if self.iosb.Anonymous.Status < 0 {
                // The overlapped request itself failed in an unexpected way.
                afd_events = afd::POLL_CONNECT_FAIL;
            } else if self.poll_info.number_of_handles < 1 {
                // This poll operation succeeded but didn't report any socket events.
            } else if self.poll_info.handles[0].events & afd::POLL_LOCAL_CLOSE != 0 {
                // The poll operation reported that the socket was closed.
                self.start_drop();
                return None;
            } else {
                afd_events = self.poll_info.handles[0].events;
            }
        }
        // Filter out events that the user didn't ask for.
        afd_events &= self.user_interests_flags;

        if afd_events == 0 {
            return None;
        }

        // Simulates Edge-triggered behavior to match API usage.
        // Intercept all read/write from user which may cause WouldBlock usage,
        // And reregister the socket to reset the interests.
        self.user_interests_flags &= !afd_events;

        Some(Event {
            data: self.user_token,
            flags: afd_events,
        })
    }

    /// Starts drop SockState
    pub(crate) fn start_drop(&mut self) {
        if !self.delete_pending {
            // if it is Pending, it means SockState has been register in IOCP,
            // must system call to cancel socket.
            // else set delete_pending=true is enough.
            if let SockPollStatus::Pending = self.poll_status {
                drop(self.cancel());
            }
            self.delete_pending = true;
        }
    }

    /// Only can cancel SockState of SockPollStatus::Pending, Set to SockPollStatus::Cancelled.
    fn cancel(&mut self) -> io::Result<()> {
        // Checks poll_status again.
        if self.poll_status != SockPollStatus::Pending {
            unreachable!("Invalid poll status during cancel, {:#?}", self);
        }

        unsafe {
            self.afd.cancel(&mut *self.iosb)?;
        }

        // Only here set SockPollStatus::Cancelled, SockStates has been system called to cancel
        self.poll_status = SockPollStatus::Cancelled;
        self.polling_interests_flags = 0;

        Ok(())
    }

    fn is_delete_pending(&self) -> bool {
        self.delete_pending
    }

    fn has_error(&self) -> bool {
        self.error.is_some()
    }
}

impl Drop for SockState {
    fn drop(&mut self) {
        self.start_drop();
    }
}

fn get_base_socket(raw_socket: RawSocket) -> io::Result<RawSocket> {
    let res = base_socket_inner(raw_socket, SIO_BASE_HANDLE);
    if let Ok(base_socket) = res {
        return Ok(base_socket);
    }

    for &ioctl in &[SIO_BSP_HANDLE_SELECT, SIO_BSP_HANDLE_POLL, SIO_BSP_HANDLE] {
        if let Ok(base_socket) = base_socket_inner(raw_socket, ioctl) {
            if base_socket != raw_socket {
                return Ok(base_socket);
            }
        }
    }

    Err(io::Error::from_raw_os_error(res.unwrap_err()))
}

fn base_socket_inner(raw_socket: RawSocket, control_code: u32) -> Result<RawSocket, i32> {
    let mut base_socket: RawSocket = 0;
    let mut bytes_returned: u32 = 0;
    unsafe {
        if WSAIoctl(
            raw_socket as usize,
            control_code,
            null_mut(),
            0,
            &mut base_socket as *mut _ as *mut c_void,
            size_of::<RawSocket>() as u32,
            &mut bytes_returned,
            null_mut(),
            None,
        ) != SOCKET_ERROR
        {
            Ok(base_socket)
        } else {
            // Returns the error status for the last Windows Sockets operation that failed.
            Err(WSAGetLastError())
        }
    }
}

/// Interests convert to flags.
fn interests_to_afd_flags(interests: Interest) -> u32 {
    let mut flags = 0;

    // Sets readable flags.
    if interests.is_readable() {
        flags |= READABLE_FLAGS | READ_CLOSED_FLAGS | ERROR_FLAGS;
    }

    // Sets writable flags.
    if interests.is_writable() {
        flags |= WRITABLE_FLAGS | WRITE_CLOSED_FLAGS | ERROR_FLAGS;
    }

    flags
}

/// Converts the pointer to a `SockState` into a raw pointer.
fn into_overlapped(sock_state: Pin<Arc<Mutex<SockState>>>) -> *mut c_void {
    let overlapped_ptr: *const Mutex<SockState> =
        unsafe { Arc::into_raw(Pin::into_inner_unchecked(sock_state)) };
    overlapped_ptr as *mut _
}

/// Convert a raw overlapped pointer into a reference to `SockState`.
fn from_overlapped(ptr: *mut OVERLAPPED) -> Pin<Arc<Mutex<SockState>>> {
    let sock_ptr: *const Mutex<SockState> = ptr as *const _;
    unsafe { Pin::new_unchecked(Arc::from_raw(sock_ptr)) }
}
