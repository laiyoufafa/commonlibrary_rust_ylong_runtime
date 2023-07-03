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

use std::{fmt, io, net};
use std::fmt::Formatter;
use std::net::SocketAddr;
use std::os::windows::io::{AsRawSocket, FromRawSocket, IntoRawSocket, RawSocket};
use crate::sys::windows::tcp::TcpSocket;
use crate::{Interest, Selector, Source, TcpStream, Token};
use crate::sys::NetState;

/// A TCP socket server, listening for connections.
pub struct TcpListener {
    pub(crate) inner: net::TcpListener,
    /// State is None if the socket has not been Registered.
    pub(crate) state: NetState,
}

impl TcpListener {
    /// Binds a new tcp Listener to the specific address to receive connection requests.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ylong_io::TcpListener;
    ///
    /// let addr = "127.0.0.1:1234".parse().unwrap();
    /// let server = TcpListener::bind(addr).unwrap();
    /// ```
    pub fn bind(addr: SocketAddr) -> io::Result<TcpListener> {
        let socket = TcpSocket::new_socket(addr)?;
        let listener = unsafe { TcpListener::from_raw_socket(socket.as_raw_socket() as _) };

        socket.bind(addr)?;
        socket.listen(1024)?;
        Ok(listener)
    }

    /// Creates `TcpListener` from raw TcpListener.
    pub fn from_std(listener: net::TcpListener) -> TcpListener {
        TcpListener {
            inner: listener,
            state: NetState::new(),
        }
    }

    /// Accepts connections and returns the `TcpStream` and the remote peer address.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ylong_io::TcpListener;
    ///
    /// let addr = "127.0.0.1:1234".parse().unwrap();
    /// let mut server = TcpListener::bind(addr).unwrap();
    /// let ret = server.accept().unwrap();
    /// ```
    pub fn accept(&self) -> io::Result<(TcpStream, SocketAddr)> {
        self.state.try_io(|inner|inner.accept(), &self.inner).map(|(stream, addr)| (TcpStream::from_std(stream), addr))
    }
}

impl Source for TcpListener {
    fn register(&mut self, selector: &Selector, token: Token, interests: Interest) -> io::Result<()> {
        self.state.register(selector, token, interests, self.inner.as_raw_socket())
    }

    fn reregister(&mut self, selector: &Selector, token: Token, interests: Interest) -> io::Result<()> {
        self.state.reregister(selector, token, interests)
    }

    fn deregister(&mut self, _selector: &Selector) -> io::Result<()> {
        self.state.deregister()
    }
}

impl fmt::Debug for TcpListener {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl IntoRawSocket for TcpListener {
    fn into_raw_socket(self) -> RawSocket {
        self.inner.into_raw_socket()
    }
}

impl AsRawSocket for TcpListener {
    fn as_raw_socket(&self) -> RawSocket {
        self.inner.as_raw_socket()
    }
}

impl FromRawSocket for TcpListener {
    /// Converts a `RawSocket` to a `TcpListener`.
    unsafe fn from_raw_socket(socket: RawSocket) -> TcpListener {
        TcpListener::from_std(FromRawSocket::from_raw_socket(socket))
    }
}