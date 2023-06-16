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
use std::net::SocketAddr;
use crate::{Interest, Selector, Source, Token};
use std::os::unix::io::AsRawFd;
use super::UdpSock;

/// UdpSocket. The bottom layer uses std::net::UdpSocket。
/// UdpSocket supports bind\connect\send\recv\send_to\recv_from\broadcast.
///
/// # Examples
///
/// ```rust
/// use std::io;
/// use ylong_io::UdpSocket;
///
/// async fn io_func() -> io::Result<()> {
///     let sender_addr = "127.0.0.1:8081".parse().unwrap();
///     let receiver_addr = "127.0.0.1:8082".parse().unwrap();
///     let mut sender = UdpSocket::bind(sender_addr)?;
///     let mut receiver = UdpSocket::bind(receiver_addr)?;
///
///     let len = sender.send_to(b"Hello", receiver_addr)?;
///     println!("{:?} bytes sent", len);
///
///     let mut buf = [0; 1024];
///     let (len, addr) = receiver.recv_from(&mut buf)?;
///     println!("{:?} bytes received from {:?}", len, addr);
///
///     let connected_sender = match sender.connect(receiver_addr) {
///         Ok(socket) => socket,
///         Err(e) => {
///             return Err(e);
///         }
///     };
///     let connected_receiver = match receiver.connect(sender_addr) {
///         Ok(socket) => socket,
///         Err(e) => {
///             return Err(e);
///         }
///     };
///     let len = connected_sender.send(b"Hello")?;
///     println!("{:?} bytes sent", len);
///     let len = connected_receiver.recv(&mut buf)?;
///     println!("{:?} bytes received from {:?}", len, sender_addr);
///     Ok(())
/// }
/// ```
pub struct UdpSocket {
    pub(crate) inner: net::UdpSocket,
}

impl UdpSocket {
    /// Creates a UDP socket from the given address.
    /// return io::Error if errors happen.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::io;
    /// use ylong_io::UdpSocket;
    ///
    /// async fn io_func() -> io::Result<()> {
    ///     let addr = "127.0.0.1:8080".parse().unwrap();
    ///     let mut sock = match UdpSocket::bind(addr) {
    ///         Ok(new_socket) => new_socket,
    ///         Err(e) => {
    ///             panic!("Failed to bind socket. {:?}", e);
    ///         }
    ///     };
    ///     Ok(())
    /// }
    /// ```
    pub fn bind(addr: SocketAddr) -> io::Result<UdpSocket> {
        let socket = UdpSock::new_socket(addr)?;
        socket.bind(addr)
    }

    /// Creates a new `UdpSocket` from a standard `net::UdpSocket`.
    ///
    /// This function is intended to be used to wrap a UDP socket from the
    /// standard library in the io equivalent. The conversion assumes nothing
    /// about the underlying socket; it is left up to the user to set it in
    /// non-blocking mode.
    pub fn from_std(socket: net::UdpSocket) -> UdpSocket {
        UdpSocket{
            inner: socket
        }
    }

    /// Returns the local address that this socket is bound to.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::io;
    /// use ylong_io::UdpSocket;
    ///
    /// async fn io_func() -> io::Result<()> {
    ///     let addr = "127.0.0.1:8080".parse().unwrap();
    ///     let mut sock = UdpSocket::bind(addr)?;
    ///     let local_addr = sock.local_addr()?;
    ///     Ok(())
    /// }
    /// ```
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }

    /// Sends data on the socket to the given address. On success, returns the number of bytes written.
    ///
    /// # Return value
    /// The function returns:
    /// * `Ok(n)` n is the number of bytes sent.
    /// * `Err(e)` if an error is encountered.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::io;
    /// use ylong_io::UdpSocket;
    ///
    /// async fn io_func() -> io::Result<()> {
    ///     let local_addr = "127.0.0.1:8080".parse().unwrap();
    ///     let sock = UdpSocket::bind(local_addr)?;
    ///     let remote_addr = "127.0.0.1:8081".parse().unwrap();
    ///     let len = sock.send_to(b"hello world", remote_addr)?;
    ///     println!("Sent {} bytes", len);
    ///     Ok(())
    /// }
    /// ```
    pub fn send_to(&self, buf: &[u8], target: SocketAddr) -> io::Result<usize> {
        let inner = &self.inner;
        inner.send_to(buf, target)
    }

    /// Receives a single datagram message on the socket. On success, returns the number of bytes read and the origin.
    /// The function must be called with valid byte array buf of sufficient size to hold the message bytes.
    /// If a message is too long to fit in the supplied buffer, excess bytes may be discarded.
    ///
    /// # Return value
    /// The function returns:
    /// * `Ok((n, addr))` n is the number of bytes received, addr is the address of sender.
    /// * `Err(e)` if an error is encountered.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::io;
    /// use ylong_io::UdpSocket;
    ///
    /// async fn io_func() -> io::Result<()> {
    ///     let local_addr = "127.0.0.1:8080".parse().unwrap();
    ///     let sock = UdpSocket::bind(local_addr)?;
    ///     let mut recv_buf = [0_u8; 12];
    ///     let (len, addr) = sock.recv_from(&mut recv_buf)?;
    ///     println!("received {:?} bytes from {:?}", len, addr);
    ///     Ok(())
    /// }
    /// ```
    pub fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        let inner = &self.inner;
        inner.recv_from(buf)
    }

    /// Connects the UDP socket setting the default destination for send()
    /// and limiting packets that are read via recv from the address specified in addr.
    /// return io::Error if errors happen.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::io;
    /// use ylong_io::UdpSocket;
    ///
    /// async fn io_func() -> io::Result<()> {
    ///     let local_addr = "127.0.0.1:8080".parse().unwrap();
    ///     let sock = UdpSocket::bind(local_addr)?;
    ///     let remote_addr = "127.0.0.1:8081".parse().unwrap();
    ///     let connected_sock = match sock.connect(remote_addr) {
    ///         Ok(socket) => socket,
    ///         Err(e) => {
    ///             return Err(e);
    ///         }
    ///     };
    ///     Ok(())
    /// }
    /// ```
    pub fn connect(self, addr: SocketAddr) -> io::Result<ConnectedUdpSocket> {
        let socket = ConnectedUdpSocket::from_std(self.inner);
        socket.inner.connect(addr)?;
        Ok(socket)
    }

    /// Sets the value of the `SO_BROADCAST` option for this socket.
    /// When enabled, this socket is allowed to send packets to a broadcast address.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::io;
    /// use ylong_io::UdpSocket;
    ///
    /// async fn io_func() -> io::Result<()> {
    ///     let local_addr = "127.0.0.1:8080".parse().unwrap();
    ///     let broadcast_socket = UdpSocket::bind(local_addr)?;
    ///     if broadcast_socket.broadcast()? == false {
    ///         broadcast_socket.set_broadcast(true)?;
    ///     }
    ///     assert_eq!(broadcast_socket.broadcast()?, true);
    ///     Ok(())
    /// }
    /// ```
    pub fn set_broadcast(&self, on: bool) ->io::Result<()> {
        self.inner.set_broadcast(on)
    }

    /// Gets the value of the `SO_BROADCAST` option for this socket.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::io;
    /// use ylong_io::UdpSocket;
    ///
    /// async fn io_func() -> io::Result<()> {
    ///     let local_addr = "127.0.0.1:8080".parse().unwrap();
    ///     let broadcast_socket = UdpSocket::bind(local_addr)?;
    ///     assert_eq!(broadcast_socket.broadcast()?, false);
    ///     Ok(())
    /// }
    /// ```
    pub fn broadcast(&self) -> io::Result<bool> {
        self.inner.broadcast()
    }
}

/// An already connected, non-blocking UdpSocket
pub struct ConnectedUdpSocket {
    pub(crate) inner: net::UdpSocket,
}

impl ConnectedUdpSocket {
    /// Creates a new `UdpSocket` from a standard `net::UdpSocket`.
    ///
    /// This function is intended to be used to wrap a UDP socket from the
    /// standard library in the io equivalent. The conversion assumes nothing
    /// about the underlying socket; it is left up to the user to set it in
    /// non-blocking mode.
    pub fn from_std(socket: net::UdpSocket) -> ConnectedUdpSocket {
        ConnectedUdpSocket{
            inner: socket
        }
    }

    /// Returns the local address that this socket is bound to.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::io;
    /// use ylong_io::UdpSocket;
    ///
    /// async fn io_func() -> io::Result<()> {
    ///     let addr = "127.0.0.1:8080".parse().unwrap();
    ///     let mut sock = UdpSocket::bind(addr)?;
    ///     let remote_addr = "127.0.0.1:8081".parse().unwrap();
    ///     let connected_sock = match sock.connect(remote_addr) {
    ///         Ok(socket) => socket,
    ///         Err(e) => {
    ///             return Err(e);
    ///         }
    ///     };
    ///     let local_addr = connected_sock.local_addr()?;
    ///     Ok(())
    /// }
    /// ```
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }

    /// Returns the socket address of the remote peer this socket was connected to.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::io;
    /// use ylong_io::UdpSocket;
    ///
    /// async fn io_func() -> io::Result<()> {
    ///     let addr = "127.0.0.1:8080".parse().unwrap();
    ///     let peer_addr = "127.0.0.1:8081".parse().unwrap();
    ///     let mut sock = UdpSocket::bind(addr)?;
    ///     let connected_sock = match sock.connect(peer_addr) {
    ///         Ok(socket) => socket,
    ///         Err(e) => {
    ///             return Err(e);
    ///         }
    ///     };
    ///     assert_eq!(connected_sock.peer_addr()?, peer_addr);
    ///     Ok(())
    /// }
    /// ```
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.inner.peer_addr()
    }

    /// Sends data on the socket to the remote address that the socket is connected to.
    /// The connect method will connect this socket to a remote address.
    /// This method will fail if the socket is not connected.
    ///
    /// # Return
    /// On success, the number of bytes sent is returned, otherwise, the encountered error is returned.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::io;
    /// use ylong_io::UdpSocket;
    ///
    /// async fn io_func() -> io::Result<()> {
    ///     let local_addr = "127.0.0.1:8080".parse().unwrap();
    ///     let sock = UdpSocket::bind(local_addr)?;
    ///     let remote_addr = "127.0.0.1:8081".parse().unwrap();
    ///     let connected_sock = match sock.connect(remote_addr) {
    ///         Ok(socket) => socket,
    ///         Err(e) => {
    ///             return Err(e);
    ///         }
    ///     };
    ///     connected_sock.send(b"Hello")?;
    ///     Ok(())
    /// }
    /// ```
    pub fn send(&self, buf: &[u8]) -> io::Result<usize> {
        let inner = &self.inner;
        inner.send(buf)
    }

    /// Receives a single datagram message on the socket from the remote address to which it is connected. On success, returns the number of bytes read.
    /// The function must be called with valid byte array buf of sufficient size to hold the message bytes.
    /// If a message is too long to fit in the supplied buffer, excess bytes may be discarded.
    /// The connect method will connect this socket to a remote address.
    /// This method will fail if the socket is not connected.
    ///
    /// # Return value
    /// The function returns:
    /// * `Ok(n)` n is is the number of bytes received
    /// * `Err(e)` if an error is encountered.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::io;
    /// use ylong_io::UdpSocket;
    ///
    /// async fn io_func() -> io::Result<()> {
    ///     let local_addr = "127.0.0.1:8080".parse().unwrap();
    ///     let sock = UdpSocket::bind(local_addr)?;
    ///     let remote_addr = "127.0.0.1:8081".parse().unwrap();
    ///     let connected_sock = match sock.connect(remote_addr) {
    ///         Ok(socket) => socket,
    ///         Err(e) => {
    ///             return Err(e);
    ///         }
    ///     };
    ///     let mut recv_buf = [0_u8; 12];
    ///     let n = connected_sock.recv(&mut recv_buf[..])?;
    ///     println!("received {} bytes {:?}", n, &recv_buf[..n]);
    ///     Ok(())
    /// }
    /// ```
    pub fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        let inner = &self.inner;
        inner.recv(buf)
    }
}

impl fmt::Debug for UdpSocket {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl fmt::Debug for ConnectedUdpSocket {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl Source for UdpSocket {
    fn register(&mut self, selector: &Selector, token: Token, interests: Interest) -> io::Result<()> {
        selector.register(self.inner.as_raw_fd(), token, interests)
    }

    fn reregister(&mut self, selector: &Selector, token: Token, interests: Interest) -> io::Result<()> {
        selector.reregister(self.inner.as_raw_fd(), token, interests)
    }

    fn deregister(&mut self, selector: &Selector) -> io::Result<()> {
        selector.deregister(self.inner.as_raw_fd())
    }
}
impl Source for ConnectedUdpSocket {
    fn register(&mut self, selector: &Selector, token: Token, interests: Interest) -> io::Result<()> {
        selector.register(self.inner.as_raw_fd(), token, interests)
    }

    fn reregister(&mut self, selector: &Selector, token: Token, interests: Interest) -> io::Result<()> {
        selector.reregister(self.inner.as_raw_fd(), token, interests)
    }

    fn deregister(&mut self, selector: &Selector) -> io::Result<()> {
        selector.deregister(self.inner.as_raw_fd())
    }
}

