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

use crate::sys::windows::selector::{SelectorInner, SockState};
use crate::{Interest, Selector, Token};
use std::os::windows::io::RawSocket;
use std::pin::Pin;
use std::sync::{Arc, Mutex, Once};
use std::{io, net};

/// Initialise the network stack for Windows.
pub(crate) fn init() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        drop(net::UdpSocket::bind("127.0.0.1:0"));
    });
}

#[derive(Clone)]
pub(crate) struct NetState {
    /// State is None if the socket has not been Registered.
    inner: Option<Box<NetInner>>,
}

impl NetState {
    /// Creates a new `NetState` with None.
    pub(crate) fn new() -> NetState {
        NetState { inner: None }
    }

    /// Register the socket to [`Selector`]
    /// If inner is Some, this function returns Err(AlreadyExists).
    /// If register success, Set the inner to Some.
    pub fn register(
        &mut self,
        selector: &Selector,
        token: Token,
        interests: Interest,
        socket: RawSocket,
    ) -> io::Result<()> {
        match self.inner {
            Some(_) => Err(io::ErrorKind::AlreadyExists.into()),
            None => selector.register(socket, token, interests).map(|state| {
                self.inner = Some(Box::new(state));
            }),
        }
    }

    /// Reregister the socket
    pub fn reregister(
        &mut self,
        selector: &Selector,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        match self.inner.as_mut() {
            Some(state) => selector
                .reregister(state.state.clone(), token, interests)
                .map(|_| {
                    state.token = token;
                    state.interests = interests;
                }),
            None => Err(io::ErrorKind::NotFound.into()),
        }
    }

    /// Deregister the socket
    pub fn deregister(&mut self) -> io::Result<()> {
        match self.inner.as_mut() {
            Some(state) => {
                {
                    let mut sock_state = state.state.lock().unwrap();
                    sock_state.start_drop();
                }
                self.inner = None;
                Ok(())
            }
            None => Err(io::ErrorKind::NotFound.into()),
        }
    }

    /// The IO operation does not really report an error when Err(WouldBlock) occurs.
    /// We need to re-register the current IO operation.
    pub(crate) fn try_io<T, F, R>(&self, task: F, io: &T) -> io::Result<R>
    where
        F: FnOnce(&T) -> io::Result<R>,
    {
        let result = task(io);
        if let Err(ref e) = result {
            if e.kind() == io::ErrorKind::WouldBlock {
                self.inner.as_ref().map_or(Ok(()), |net_inner| {
                    net_inner.selector.reregister(
                        net_inner.state.clone(),
                        net_inner.token,
                        net_inner.interests,
                    )
                })?;
            }
        }
        result
    }
}

/// This structure used to re-register the socket when Err(WouldBlock) occurs
#[derive(Clone)]
pub(crate) struct NetInner {
    selector: Arc<SelectorInner>,
    token: Token,
    interests: Interest,
    state: Pin<Arc<Mutex<SockState>>>,
}

impl NetInner {
    pub(crate) fn new(
        selector: Arc<SelectorInner>,
        token: Token,
        interests: Interest,
        state: Pin<Arc<Mutex<SockState>>>,
    ) -> NetInner {
        NetInner {
            selector,
            token,
            interests,
            state,
        }
    }
}

impl Drop for NetInner {
    fn drop(&mut self) {
        let mut sock_state = self.state.lock().unwrap();
        sock_state.start_drop();
    }
}
