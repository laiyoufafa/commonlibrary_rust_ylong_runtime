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

use std::{fmt, io, net};
use std::fmt::Formatter;
use std::io::{IoSlice, IoSliceMut, Read, Write};
use std::net::{Shutdown, SocketAddr};
use std::os::windows::io::{AsRawSocket, FromRawSocket, IntoRawSocket, RawSocket};
use crate::{Interest, Selector, Source, Token};
use crate::sys::{ NetState};
use crate::sys::windows::tcp::TcpSocket;

/// A non-blocking TCP Stream between a local socket and a remote socket.
pub struct TcpStream {
    /// Raw TCP socket
    pub(crate) inner: net::TcpStream,
    /// State is None if the socket has not been Registered.
    pub(crate) state: NetState,
}

impl TcpStream {
    /// Connects address to form TcpStream
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let addr = "127.0.0.1:1234".parse().unwrap();
    /// let stream = ylong_io::TcpStream::connect(addr).unwrap();
    /// ```
    pub fn connect(addr: SocketAddr) -> io::Result<TcpStream> {
        let socket = TcpSocket::new_socket(addr)?;
        let stream = unsafe { TcpStream::from_raw_socket(socket.as_raw_socket() as _) };

        socket.connect( addr)?;
        Ok(stream)
    }

    /// Creates `TcpStream` from raw TcpStream.
    pub fn from_std(stream: net::TcpStream) -> TcpStream {
        TcpStream {
            inner: stream,
            state: NetState::new(),
        }
    }

    /// Clones the TcpStream.
    pub fn try_clone(&self) -> Self {
        TcpStream {
            inner: self.inner.try_clone().unwrap(),
            state: self.state.clone(),
        }
    }

    /// Returns the socket address of the local half of this TCP connection.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::net::{IpAddr, Ipv4Addr};
    /// use ylong_io::TcpStream;
    ///
    /// let addr = "127.0.0.1:1234".parse().unwrap();
    /// let stream = TcpStream::connect(addr)
    ///                        .expect("Couldn't connect to the server...");
    /// assert_eq!(stream.local_addr().unwrap().ip(),
    ///            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
    /// ```
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }

    /// Returns the socket address of the remote half of this TCP connection.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
    /// use ylong_io::TcpStream;
    ///
    /// let addr = "127.0.0.1:1234".parse().unwrap();
    /// let stream = TcpStream::connect(addr)
    ///                        .expect("Couldn't connect to the server...");
    /// assert_eq!(stream.peer_addr().unwrap(),
    ///            SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 1234)));
    /// ```
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.inner.peer_addr()
    }

    /// Shuts down the read, write, or both halves of this connection.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::net::{Shutdown};
    /// use ylong_io::TcpStream;
    ///
    /// let addr = "127.0.0.1:1234".parse().unwrap();
    /// let stream = TcpStream::connect(addr)
    ///                        .expect("Couldn't connect to the server...");
    /// stream.shutdown(Shutdown::Both).expect("shutdown call failed");
    /// ```
    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.inner.shutdown(how)
    }

    /// Sets the value of the `TCP_NODELAY`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ylong_io::TcpStream;
    ///
    /// let addr = "127.0.0.1:1234".parse().unwrap();
    /// let stream = TcpStream::connect(addr)
    ///                        .expect("Couldn't connect to the server...");
    /// stream.set_nodelay(true).expect("set_nodelay call failed");
    /// ```
    pub fn set_nodelay(&self, nodelay: bool) -> io::Result<()> {
        self.inner.set_nodelay(nodelay)
    }

    /// Gets the value of the `TCP_NODELAY`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ylong_io::TcpStream;
    ///
    /// let addr = "127.0.0.1:1234".parse().unwrap();
    /// let stream = TcpStream::connect(addr)
    ///                        .expect("Couldn't connect to the server...");
    /// stream.set_nodelay(true).expect("set_nodelay call failed");
    /// assert_eq!(stream.nodelay().unwrap_or(false), true);
    /// ```
    pub fn nodelay(&self) -> io::Result<bool> {
        self.inner.nodelay()
    }

    /// Sets the value for the `IP_TTL`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ylong_io::TcpStream;
    ///
    /// let addr = "127.0.0.1:1234".parse().unwrap();
    /// let stream = TcpStream::connect(addr)
    ///                        .expect("Couldn't connect to the server...");
    /// stream.set_ttl(100).expect("set_ttl call failed");
    /// ```
    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        self.inner.set_ttl(ttl)
    }

    /// Gets the value of the `IP_TTL`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ylong_io::TcpStream;
    ///
    /// let addr = "127.0.0.1:1234".parse().unwrap();
    /// let stream = TcpStream::connect(addr)
    ///                        .expect("Couldn't connect to the server...");
    /// stream.set_ttl(100).expect("set_ttl call failed");
    /// assert_eq!(stream.ttl().unwrap_or(0), 100);
    /// ```
    pub fn ttl(&self) -> io::Result<u32> {
        self.inner.ttl()
    }

    /// Get the value of the `SO_ERROR`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ylong_io::TcpStream;
    ///
    /// let addr = "127.0.0.1:1234".parse().unwrap();
    /// let stream = TcpStream::connect(addr)
    ///                        .expect("Couldn't connect to the server...");
    /// stream.take_error().expect("No error was expected...");
    /// ```
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.inner.take_error()
    }

    /// Same as std::net::TcpStream::peek().
    ///
    /// Receives data on the socket from the remote address to which it is
    /// connected, without removing that data from the queue. On success,
    /// returns the number of bytes peeked.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ylong_io::TcpStream;
    ///
    /// let addr = "127.0.0.1:1234".parse().unwrap();
    /// let stream = TcpStream::connect(addr)
    ///                        .expect("Couldn't connect to the server...");
    /// let mut buf = [0; 10];
    /// let len = stream.peek(&mut buf).expect("peek failed");
    /// ```
    pub fn peek(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.peek(buf)
    }
}

impl fmt::Debug for TcpStream {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

macro_rules! read_write {
    ($($identifier:tt)*) => {
        impl Read for $($identifier)* {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                self.state.try_io(|mut inner| inner.read(buf), &self.inner)
            }

            fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
                self.state.try_io(|mut inner| inner.read_vectored(bufs), &self.inner)
            }
        }

        impl Write for $($identifier)* {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                self.state.try_io(|mut inner| inner.write(buf), &self.inner)
            }

            fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
                self.state.try_io(|mut inner| inner.write_vectored(bufs), &self.inner)
            }

            fn flush(&mut self) -> io::Result<()> {
                self.state.try_io(|mut inner| inner.flush(), &self.inner)
            }
        }
    };
}

read_write!(TcpStream);

read_write!(&TcpStream);

impl IntoRawSocket for TcpStream {
    fn into_raw_socket(self) -> RawSocket {
        self.inner.into_raw_socket()
    }
}

impl AsRawSocket for TcpStream {
    fn as_raw_socket(&self) -> RawSocket {
        self.inner.as_raw_socket()
    }
}

impl FromRawSocket for TcpStream {
    /// Converts a `RawSocket` to a `TcpStream`.
    unsafe fn from_raw_socket(socket: RawSocket) -> Self {
        TcpStream::from_std(FromRawSocket::from_raw_socket(socket))
    }
}

impl Source for TcpStream {
    fn register(&mut self, selector: &Selector, token: Token, interests: Interest) -> io::Result<()> {
        self.state.register(selector,token,interests,self.as_raw_socket())
    }

    fn reregister(&mut self, selector: &Selector, token: Token, interests: Interest) -> io::Result<()> {
        self.state.reregister(selector,token,interests)
    }

    fn deregister(&mut self, _selector: &Selector) -> io::Result<()> {
        self.state.deregister()
    }
}