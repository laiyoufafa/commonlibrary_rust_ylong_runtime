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

use super::TcpSocket;
use crate::{Interest, Source, Selector, Token};
use std::io::{self, IoSlice, IoSliceMut, Read, Write};
use std::net::{self, SocketAddr, Shutdown};
use std::os::unix::io::AsRawFd;

/// A non-blocking TCP Stream between a local socket and a remote socket.
pub struct TcpStream {
    /// Raw TCP socket
    pub inner: net::TcpStream,
}

impl TcpStream {
    /// Create a new TCP stream and issue a non-blocking connect to the
    /// specified address.
    pub fn connect(addr: SocketAddr) -> io::Result<TcpStream> {
        let socket = TcpSocket::new_socket(addr)?;
        socket.connect(addr)
    }

    /// Creates a new `TcpStream` from a standard `net::TcpStream`.
    pub fn from_std(stream: net::TcpStream) -> TcpStream {
        TcpStream { inner: stream }
    }

    /// Clones the TcpStream.
    pub fn try_clone(&self) -> Self {
        TcpStream {
            inner: self.inner.try_clone().unwrap(),
        }
    }

    /// Gets the value of the `SO_ERROR` option on this socket.
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.inner.take_error()
    }

    /// Shuts down the read, write, or both halves of this connection.
    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.inner.shutdown(how)
    }
}

impl Read for TcpStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }

    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        self.inner.read_vectored(bufs)
    }
}

impl Write for TcpStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.inner.write_vectored(bufs)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl Read for &TcpStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut inner = &self.inner;
        inner.read(buf)
    }

    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        let mut inner = &self.inner;
        inner.read_vectored(bufs)
    }
}

impl Write for &TcpStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut inner = &self.inner;
        inner.write(buf)
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        let mut inner = &self.inner;
        inner.write_vectored(bufs)
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut inner = &self.inner;
        inner.flush()
    }
}

impl std::fmt::Debug for TcpStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}

impl Source for TcpStream {
    fn register(
        &mut self,
        selector: &Selector,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        selector.register(self.inner.as_raw_fd(), token, interests)
    }

    fn reregister(
        &mut self,
        selector: &Selector,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        selector.reregister(self.inner.as_raw_fd(), token, interests)
    }

    fn deregister(&mut self, selector: &Selector) -> io::Result<()> {
        selector.deregister(self.inner.as_raw_fd())
    }
}
