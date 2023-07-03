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

use std::fmt::{Debug, Formatter};
use std::io;
use std::net::SocketAddr;
use std::task::{Context, Poll};
use ylong_io::Interest;
use crate::io::ReadBuf;
use crate::net::AsyncSource;
use std::mem::MaybeUninit;

/// Asynchronous UdpSocket.
///
/// # Examples
///
/// ```rust
/// use ylong_runtime::net::UdpSocket;
/// use std::io;
///
/// async fn io_func() -> io::Result<()>{
///     let sender_addr = "127.0.0.1:8081".parse().unwrap();
///     let receiver_addr = "127.0.0.1:8082".parse().unwrap();
///     let mut sender = UdpSocket::bind(sender_addr).await?;
///     let mut receiver = UdpSocket::bind(sender_addr).await?;
///
///     let len = sender.send_to(b"Hello", receiver_addr).await?;
///     println!("{:?} bytes sent", len);
///
///     let mut buf = [0; 1024];
///     let (len, addr) = receiver.recv_from(&mut buf).await?;
///     println!("{:?} bytes received from {:?}", len, addr);
///
///     let connected_sender = match sender.connect(receiver_addr).await {
///         Ok(socket) => socket,
///         Err(e) => {
///             assert_eq!(0, 1, "Connect UdpSocket Failed {}", e);
///             return Err(e);
///         }
///     };
///     let connected_receiver = match receiver.connect(sender_addr).await {
///         Ok(socket) => socket,
///         Err(e) => {
///             assert_eq!(0, 1, "Connect UdpSocket Failed {}", e);
///             return Err(e);
///         }
///     };
///     let len = connected_sender.send(b"Hello").await?;
///     println!("{:?} bytes sent", len);
///     let len = connected_receiver.recv(&mut buf).await?;
///     println!("{:?} bytes received from {:?}", len, sender_addr);
///     Ok(())
/// }
/// ```
pub struct UdpSocket {
    pub(crate) source: AsyncSource<ylong_io::UdpSocket>,
}

/// A connected asynchronous UdpSocket.
pub struct ConnectedUdpSocket {
    pub(crate) source: AsyncSource<ylong_io::ConnectedUdpSocket>,
}

impl Debug for UdpSocket {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.source.fmt(f)
    }
}

impl Debug for ConnectedUdpSocket {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.source.fmt(f)
    }
}

impl UdpSocket {
    /// Creates a new UDP socket and attempts to bind it to the address provided,
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ylong_runtime::net::UdpSocket;
    /// use std::io;
    ///
    /// async fn io_func() -> io::Result<()> {
    ///     let addr = "127.0.0.1:8080".parse().unwrap();
    ///     let mut sock = UdpSocket::bind(addr).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn bind(addr: SocketAddr) -> io::Result<Self> {
        UdpSocket::new(ylong_io::UdpSocket::bind(addr)?)
    }

    /// Internal interfaces.
    /// Creates new ylong_runtime::net::UdpSocket according to the incoming ylong_io::UdpSocket.
    pub(crate) fn new(socket: ylong_io::UdpSocket) -> io::Result<Self> {
        let source = AsyncSource::new(socket, None)?;
        Ok(UdpSocket { source })
    }

    /// Sets the default address for the UdpSocket and limits packets to
    /// those that are read via recv from the specific address.
    ///
    /// Returns the connected UdpSocket if succeeds.
    /// # Examples
    ///
    /// ```rust
    /// use ylong_runtime::net::UdpSocket;
    /// use std::io;
    ///
    /// async fn io_func() -> io::Result<()> {
    ///     let local_addr = "127.0.0.1:8080".parse().unwrap();
    ///     let sock = UdpSocket::bind(local_addr).await?;
    ///     let remote_addr = "127.0.0.1:8081".parse().unwrap();
    ///     let connected_sock = match sock.connect(remote_addr).await {
    ///         Ok(socket) => socket,
    ///         Err(e) => {
    ///             return Err(e);
    ///         }
    ///     };
    ///     Ok(())
    /// }
    /// ```
    pub async fn connect(self, addr: SocketAddr) -> io::Result<ConnectedUdpSocket> {
        let local_addr = self.local_addr().unwrap();
        drop(self);
        let socket = ylong_io::UdpSocket::bind(local_addr)?;
        let connected_socket = match socket.connect(addr) {
            Ok(socket) => socket,
            Err(e) => return Err(e)
        };
        ConnectedUdpSocket::new(connected_socket)
    }

    /// Returns the local address that this socket is bound to.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ylong_runtime::net::UdpSocket;
    /// use std::io;
    ///
    /// async fn io_func() -> io::Result<()> {
    ///     let addr = "127.0.0.1:8080".parse().unwrap();
    ///     let mut sock = UdpSocket::bind(addr).await?;
    ///     let local_addr = sock.local_addr()?;
    ///     Ok(())
    /// }
    /// ```
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.source.local_addr()
    }

    /// Sends data on the socket to the given address. On success, returns the number of bytes written.
    /// This will return an error when the IP version of the local socket does not match that returned from SocketAddr.
    ///
    /// # Return value
    /// The function returns:
    /// * `Ok(n)` n is the number of bytes sent.
    /// * `Err(e)` if an error is encountered.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ylong_runtime::net::UdpSocket;
    /// use std::io;
    ///
    /// async fn io_func() -> io::Result<()> {
    ///     let local_addr = "127.0.0.1:8080".parse().unwrap();
    ///     let sock = UdpSocket::bind(local_addr).await?;
    ///     let remote_addr = "127.0.0.1:8081".parse().unwrap();
    ///     let len = sock.send_to(b"hello world", remote_addr).await?;
    ///     println!("Sent {} bytes", len);
    ///     Ok(())
    /// }
    /// ```
    pub async fn send_to(&self, buf: &[u8], target: SocketAddr) -> io::Result<usize> {
        self.source
            .async_process(Interest::WRITABLE, || self.source.send_to(buf, target))
            .await
    }

    /// Attempts to send data on the socket to the given address.
    ///
    /// The function is usually paired with `writable`.
    ///
    /// # Return value
    /// The function returns:
    /// * `Ok(n)` n is the number of bytes sent.
    /// * `Err(e)` if an error is encountered.
    /// When the remote cannot receive the message, an [`ErrorKind::WouldBlock`] will be returned.
    /// This will return an error If the IP version of the local socket does not match
    /// that returned from SocketAddr.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ylong_runtime::net::UdpSocket;
    /// use std::io;
    ///
    /// async fn io_func() -> io::Result<()> {
    ///     let local_addr = "127.0.0.1:8080".parse().unwrap();
    ///     let sock = UdpSocket::bind(local_addr).await?;
    ///     let remote_addr = "127.0.0.1:8081".parse().unwrap();
    ///     let len = sock.try_send_to(b"hello world", remote_addr)?;
    ///     Ok(())
    /// }
    /// ```
    pub fn try_send_to(&self, buf: &[u8], target: SocketAddr) -> io::Result<usize> {
        self.source
            .try_io(Interest::WRITABLE, || self.source.send_to(buf, target))
    }

    /// Attempts to send data on the socket to a given address.
    /// Note that on multiple calls to a poll_* method in the send direction,
    /// only the Waker from the Context passed to the most recent call will be scheduled to receive a wakeup
    ///
    /// # Return value
    /// The function returns:
    /// * `Poll::Pending` if the socket is not ready to write
    /// * `Poll::Ready(Ok(n))` n is the number of bytes sent.
    /// * `Poll::Ready(Err(e))` if an error is encountered.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ylong_runtime::net::UdpSocket;
    /// use ylong_runtime::futures::poll_fn;
    /// use std::io;
    ///
    /// async fn io_func() -> io::Result<()> {
    ///     let local_addr = "127.0.0.1:8080".parse().unwrap();
    ///     let sock = UdpSocket::bind(local_addr).await?;
    ///     let remote_addr = "127.0.0.1:8081".parse().unwrap();
    ///     let len = poll_fn(|cx| sock.poll_send_to(cx, b"Hello", remote_addr)).await?;
    ///     println!("Sent {} bytes", len);
    ///     Ok(())
    /// }
    /// ```
    pub fn poll_send_to(
        &self,
        cx: &mut Context<'_>,
        buf: &[u8],
        target: SocketAddr,
    ) -> Poll<io::Result<usize>> {
        self.source.poll_write_io(cx, || self.source.send_to(buf, target))
    }

    /// Receives a single datagram message on the socket. On success, returns the number of bytes
    /// read and the origin. The function must be called with valid byte array buf of sufficient
    /// size to hold the message bytes. If a message is too long to fit in the supplied buffer,
    /// excess bytes may be discarded.
    ///
    /// # Return value
    /// The function returns:
    /// * `Ok((n, addr))` n is the number of bytes received, addr is the address of sender.
    /// * `Err(e)` if an error is encountered.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ylong_runtime::net::UdpSocket;
    /// use std::io;
    ///
    /// async fn io_func() -> io::Result<()> {
    ///     let local_addr = "127.0.0.1:8080".parse().unwrap();
    ///     let sock = UdpSocket::bind(local_addr).await?;
    ///     let mut recv_buf = [0_u8; 12];
    ///     let (len, addr) = sock.recv_from(&mut recv_buf).await?;
    ///     println!("received {:?} bytes from {:?}", len, addr);
    ///     Ok(())
    /// }
    /// ```
    pub async fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.source.
            async_process(Interest::READABLE, || self.source.recv_from(buf))
            .await
    }

    /// Attempts to receive a single datagram message on the socket.
    ///
    /// The function is usually paired with `readable` and must be called with valid byte array
    /// buf of sufficient size to hold the message bytes. If a message is too long to fit in the
    /// supplied buffer, excess bytes may be discarded.
    ///
    /// # Return value
    /// The function returns:
    /// * `Ok(n, addr)` n is the number of bytes received, addr is the address of the remote.
    /// * `Err(e)` if an error is encountered.
    /// If there is no pending data, an [`ErrorKind::WouldBlock`] will be returned.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ylong_runtime::net::UdpSocket;
    /// use std::io;
    ///
    /// async fn io_func() -> io::Result<()> {
    ///     let local_addr = "127.0.0.1:8080".parse().unwrap();
    ///     let sock = UdpSocket::bind(local_addr).await?;
    ///     let mut recv_buf = [0_u8; 12];
    ///     let (len, addr) = sock.try_recv_from(&mut recv_buf)?;
    ///     Ok(())
    /// }
    /// ```
    pub fn try_recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.source
            .try_io(Interest::READABLE, || self.source.recv_from(buf))
    }

    /// Waits for the socket to become readable.
    ///
    /// This function is usually paired up with [`UdpSocket::try_recv_from`]
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::net::UdpSocket;
    /// use std::io;
    ///
    /// async fn io_func() -> io::Result<()> {
    ///     let local_addr = "127.0.0.1:8080".parse().unwrap();
    ///     let sock = UdpSocket::bind(local_addr).await?;
    ///     sock.readable().await?;
    ///     let mut buf = [0; 12];
    ///     let (len, addr) = sock.try_recv_from(&mut buf)?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn readable(&self) -> io::Result<()> {
        self.source.entry.readiness(Interest::READABLE).await?;
        Ok(())
    }

    /// Waits for the socket to become writable.
    ///
    /// This function is usually paired up with [`UdpSocket::try_send_to`]
    ///
    /// # Examples
    /// ```
    /// use ylong_runtime::net::UdpSocket;
    /// use std::io;
    ///
    /// async fn io_func() -> io::Result<()> {
    ///     let local_addr = "127.0.0.1:8080".parse().unwrap();
    ///     let remote_addr = "127.0.0.1:8080".parse().unwrap();
    ///     let sock = UdpSocket::bind(local_addr).await?;
    ///     sock.writable().await?;
    ///     let len = sock.try_send_to(b"hello", remote_addr)?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn writable(&self) -> io::Result<()> {
        self.source.entry.readiness(Interest::WRITABLE).await?;
        Ok(())
    }

    /// Attempts to receive a single datagram on the socket.
    /// Note that on multiple calls to a poll_* method in the recv direction,
    /// only the Waker from the Context passed to the most recent call will be scheduled to receive a wakeup.
    ///
    /// # Return value
    /// The function returns:
    /// * `Poll::Pending` if the socket is not ready to read
    /// * `Poll::Ready(Ok(addr))` reads data from addr into ReadBuf if the socket is ready
    /// * `Poll::Ready(Err(e))` if an error is encountered.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ylong_runtime::net::UdpSocket;
    /// use std::io;
    /// use ylong_runtime::futures::poll_fn;
    /// use ylong_runtime::io::ReadBuf;
    ///
    /// async fn io_func() -> io::Result<()> {
    ///     let local_addr = "127.0.0.1:8080".parse().unwrap();
    ///     let sock = UdpSocket::bind(local_addr).await?;
    ///     let mut recv_buf = [0_u8; 12];
    ///     let mut read = ReadBuf::new(&mut recv_buf);
    ///     let addr = poll_fn(|cx| sock.poll_recv_from(cx, &mut read)).await?;
    ///     println!("received {:?} from {:?}", read.filled(), addr);
    ///     Ok(())
    /// }
    /// ```
    pub fn poll_recv_from(
        &self,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<SocketAddr>> {
        let ret = self.source.poll_read_io(cx, || unsafe {
            let slice = &mut *(buf.unfilled_mut() as *mut [MaybeUninit<u8>] as *mut [u8]);
            self.source.recv_from(slice)
        });
        let (r_len, r_addr) = match ret {
            Poll::Ready(Ok(x)) => x,
            Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
            Poll::Pending => return Poll::Pending,
        };
        buf.assume_init(r_len);
        buf.advance(r_len);

        Poll::Ready(Ok(r_addr))
    }

    /// Sets the value of the `SO_BROADCAST` option for this socket.
    /// When enabled, this socket is allowed to send packets to a broadcast address.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::io;
    /// use ylong_runtime::net::UdpSocket;
    ///
    /// async fn io_func() -> io::Result<()> {
    ///     let local_addr = "127.0.0.1:8080".parse().unwrap();
    ///     let broadcast_socket = UdpSocket::bind(local_addr).await?;
    ///     if broadcast_socket.broadcast()? == false {
    ///         broadcast_socket.set_broadcast(true)?;
    ///     }
    ///     assert_eq!(broadcast_socket.broadcast()?, true);
    ///     Ok(())
    /// }
    /// ```
    pub fn set_broadcast(&self, on: bool) -> io::Result<()> {
        self.source.set_broadcast(on)
    }

    /// Gets the value of the `SO_BROADCAST` option for this socket.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::io;
    /// use ylong_runtime::net::UdpSocket;
    ///
    /// async fn io_func() -> io::Result<()> {
    ///     let local_addr = "127.0.0.1:8080".parse().unwrap();
    ///     let broadcast_socket = UdpSocket::bind(local_addr).await?;
    ///     assert_eq!(broadcast_socket.broadcast()?, false);
    ///     Ok(())
    /// }
    /// ```
    pub fn broadcast(&self) -> io::Result<bool> {
        self.source.broadcast()
    }
}

impl ConnectedUdpSocket {
    /// Internal interfaces.
    /// Creates new ylong_runtime::net::ConnectedUdpSocket according to the incoming ylong_io::UdpSocket.
    pub(crate) fn new(socket: ylong_io::ConnectedUdpSocket) -> io::Result<Self> {
        let source = AsyncSource::new(socket, None)?;
        Ok(ConnectedUdpSocket { source })
    }

    /// Returns the local address that this socket is bound to.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ylong_runtime::net::UdpSocket;
    /// use std::io;
    ///
    /// async fn io_func() -> io::Result<()> {
    ///     let addr = "127.0.0.1:8080".parse().unwrap();
    ///     let mut sock = UdpSocket::bind(addr).await?;
    ///     let remote_addr = "127.0.0.1:8081".parse().unwrap();
    ///     let connected_sock = match sock.connect(remote_addr).await {
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
        self.source.local_addr()
    }

    /// Returns the socket address of the remote peer this socket was connected to.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ylong_runtime::net::UdpSocket;
    /// use std::io;
    ///
    /// async fn io_func() -> io::Result<()> {
    ///     let addr = "127.0.0.1:8080".parse().unwrap();
    ///     let peer_addr = "127.0.0.1:8081".parse().unwrap();
    ///     let mut sock = UdpSocket::bind(addr).await?;
    ///     let connected_sock = match sock.connect(peer_addr).await {
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
        self.source.peer_addr()
    }

    /// Sends data on the socket to the remote address that the socket is connected to.
    /// The connect method will connect this socket to a remote address.
    /// This method will fail if the socket is not connected.
    ///
    /// # Return value
    /// On success, the number of bytes sent is returned, otherwise, the encountered error is returned.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ylong_runtime::net::UdpSocket;
    /// use std::io;
    ///
    /// async fn io_func() -> io::Result<()> {
    ///     let local_addr = "127.0.0.1:8080".parse().unwrap();
    ///     let sock = UdpSocket::bind(local_addr).await?;
    ///     let remote_addr = "127.0.0.1:8081".parse().unwrap();
    ///     let connected_sock = match sock.connect(remote_addr).await {
    ///         Ok(socket) => socket,
    ///         Err(e) => {
    ///             return Err(e);
    ///         }
    ///     };
    ///     connected_sock.send(b"Hello").await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.source
            .async_process(Interest::WRITABLE, || self.source.send(buf))
            .await
    }

    /// Attempts to send data on the socket to the remote address that the socket is connected to.
    /// This method will fail if the socket is not connected.
    ///
    /// The function is usually paired with `writable`.
    ///
    /// # Return value
    /// The function returns:
    /// * `Ok(n)` n is the number of bytes sent.
    /// * `Err(e)` if an error is encountered.
    /// When the remote cannot receive the message, an [`ErrorKind::WouldBlock`] will be returned.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ylong_runtime::net::UdpSocket;
    /// use std::io;
    ///
    /// async fn io_func() -> io::Result<()> {
    ///     let local_addr = "127.0.0.1:8080".parse().unwrap();
    ///     let sock = UdpSocket::bind(local_addr).await?;
    ///     let remote_addr = "127.0.0.1:8081".parse().unwrap();
    ///     let connected_sock = match sock.connect(remote_addr).await {
    ///         Ok(socket) => socket,
    ///         Err(e) => {
    ///             return Err(e);
    ///         }
    ///     };
    ///     connected_sock.try_send(b"Hello")?;
    ///     Ok(())
    /// }
    /// ```
    pub fn try_send(&self, buf: &[u8]) -> io::Result<usize> {
        self.source
            .try_io(Interest::WRITABLE, || self.source.send(buf))
    }


    /// Attempts to send data on the socket to the remote address to which it was previously connected.
    /// The connect method will connect this socket to a remote address.
    /// This method will fail if the socket is not connected.
    /// Note that on multiple calls to a poll_* method in the send direction,
    /// only the Waker from the Context passed to the most recent call will be scheduled to receive a wakeup.
    ///
    /// # Return value
    /// The function returns:
    /// * `Poll::Pending` if the socket is not available to write
    /// * `Poll::Ready(Ok(n))` `n` is the number of bytes sent
    /// * `Poll::Ready(Err(e))` if an error is encountered.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ylong_runtime::net::UdpSocket;
    /// use std::io;
    /// use ylong_runtime::futures::poll_fn;
    ///
    /// async fn io_func() -> io::Result<()> {
    ///     let local_addr = "127.0.0.1:8080".parse().unwrap();
    ///     let sock = UdpSocket::bind(local_addr).await?;
    ///     let remote_addr = "127.0.0.1:8081".parse().unwrap();
    ///     let connected_sock = match sock.connect(remote_addr).await {
    ///         Ok(socket) => socket,
    ///         Err(e) => {
    ///             return Err(e);
    ///         }
    ///     };
    ///     poll_fn(|cx| connected_sock.poll_send(cx, b"Hello")).await?;
    ///     Ok(())
    /// }
    /// ```
    pub fn poll_send(&self, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        self.source.poll_write_io(cx, || self.source.send(buf))
    }

    /// Receives a single datagram message on the socket from the remote address to which it is connected.
    /// On success, returns the number of bytes read.
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
    /// use ylong_runtime::net::UdpSocket;
    /// use std::io;
    ///
    /// async fn io_func() -> io::Result<()> {
    ///     let local_addr = "127.0.0.1:8080".parse().unwrap();
    ///     let sock = UdpSocket::bind(local_addr).await?;
    ///     let remote_addr = "127.0.0.1:8081".parse().unwrap();
    ///     let connected_sock = match sock.connect(remote_addr).await {
    ///         Ok(socket) => socket,
    ///         Err(e) => {
    ///             return Err(e);
    ///         }
    ///     };
    ///     let mut recv_buf = [0_u8; 12];
    ///     let n = connected_sock.recv(&mut recv_buf[..]).await?;
    ///     println!("received {} bytes {:?}", n, &recv_buf[..n]);
    ///     Ok(())
    /// }
    /// ```
    pub async fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.source
            .async_process(Interest::READABLE, || self.source.recv(buf))
            .await
    }

    /// Attempts to receive a single datagram message on the socket from the remote address
    /// to which it is connected.
    /// On success, returns the number of bytes read. The function must be called with valid byte
    /// array buf of sufficient size to hold the message bytes. If a message is too long to fit in
    /// the supplied buffer, excess bytes may be discarded.
    /// This method will fail if the socket is not connected.
    ///
    /// # Return value
    /// The function returns:
    /// * `Ok(n, addr)` n is the number of bytes received, addr is the address of the remote.
    /// * `Err(e)` if an error is encountered.
    /// If there is no pending data, an [`ErrorKind::WouldBlock`] will be returned.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ylong_runtime::net::UdpSocket;
    /// use std::io;
    ///
    /// async fn io_func() -> io::Result<()> {
    ///     let local_addr = "127.0.0.1:8080".parse().unwrap();
    ///     let sock = UdpSocket::bind(local_addr).await?;
    ///     let remote_addr = "127.0.0.1:8081".parse().unwrap();
    ///     let connected_sock = match sock.connect(remote_addr).await {
    ///         Ok(socket) => socket,
    ///         Err(e) => {
    ///             return Err(e);
    ///         }
    ///     };
    ///     let mut recv_buf = [0_u8; 12];
    ///     let n = connected_sock.try_recv(&mut recv_buf[..])?;
    ///     println!("received {} bytes {:?}", n, &recv_buf[..n]);
    ///     Ok(())
    /// }
    /// ```
    pub fn try_recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.source
            .try_io(Interest::READABLE, || self.source.recv(buf))
    }

    /// Attempts to receive a single datagram message on the socket from the remote address to which it is connected.
    /// The connect method will connect this socket to a remote address.
    /// This method resolves to an error if the socket is not connected.
    /// Note that on multiple calls to a poll_* method in the recv direction,
    /// only the Waker from the Context passed to the most recent call will be scheduled to receive a wakeup.
    ///
    /// # Return value
    /// The function returns:
    ///
    /// * `Poll::Pending` if the socket is not ready to read
    /// * `Poll::Ready(Ok(()))` reads data ReadBuf if the socket is ready
    /// * `Poll::Ready(Err(e))` if an error is encountered.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ylong_runtime::net::UdpSocket;
    /// use std::io;
    /// use ylong_runtime::futures::poll_fn;
    /// use ylong_runtime::io::ReadBuf;
    ///
    /// async fn io_func() -> io::Result<()> {
    ///     let local_addr = "127.0.0.1:8080".parse().unwrap();
    ///     let sock = UdpSocket::bind(local_addr).await?;
    ///     let remote_addr = "127.0.0.1:8081".parse().unwrap();
    ///     let connected_sock = match sock.connect(remote_addr).await {
    ///         Ok(socket) => socket,
    ///         Err(e) => {
    ///             return Err(e);
    ///         }
    ///     };
    ///     let mut recv_buf = [0_u8; 12];
    ///     let mut read = ReadBuf::new(&mut recv_buf);
    ///     poll_fn(|cx| connected_sock.poll_recv(cx, &mut read)).await?;
    ///     println!("received : {:?}", read.filled());
    ///     Ok(())
    /// }
    /// ```
    pub fn poll_recv(&self, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        let ret = self.source.poll_read_io(cx, || unsafe {
            let slice = &mut *(buf.unfilled_mut() as *mut [MaybeUninit<u8>] as *mut [u8]);
            self.source.recv(slice)
        });
        let r_len = match ret {
            Poll::Ready(Ok(x)) => x,
            Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
            Poll::Pending => return Poll::Pending,
        };
        buf.assume_init(r_len);
        buf.advance(r_len);

        Poll::Ready(Ok(()))
    }

    /// Waits for the socket to become readable.
    ///
    /// This function is usually paired up with [`UdpSocket::try_recv_from`]
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::net::{UdpSocket, ConnectedUdpSocket};
    /// use std::io;
    ///
    /// async fn io_func() -> io::Result<()> {
    ///     let local_addr = "127.0.0.1:8080".parse().unwrap();
    ///     let sock = UdpSocket::bind(local_addr).await?;
    ///     let remote_addr = "127.0.0.1:8081".parse().unwrap();
    ///     let connected_sock = match sock.connect(remote_addr).await {
    ///         Ok(socket) => socket,
    ///         Err(e) => {
    ///             return Err(e);
    ///         }
    ///     };
    ///     connected_sock.readable().await?;
    ///     let mut buf = [0; 12];
    ///     let len = connected_sock.try_recv(&mut buf)?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn readable(&self) -> io::Result<()> {
        self.source.entry.readiness(Interest::READABLE).await?;
        Ok(())
    }

    /// Waits for the socket to become writable.
    ///
    /// This function is usually paired up with [`UdpSocket::try_send_to`]
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::net::{UdpSocket, ConnectedUdpSocket};
    /// use std::io;
    ///
    /// async fn io_func() -> io::Result<()> {
    ///     let local_addr = "127.0.0.1:8080".parse().unwrap();
    ///     let sock = UdpSocket::bind(local_addr).await?;
    ///     let remote_addr = "127.0.0.1:8081".parse().unwrap();
    ///     let connected_sock = match sock.connect(remote_addr).await {
    ///         Ok(socket) => socket,
    ///         Err(e) => {
    ///             return Err(e);
    ///         }
    ///     };
    ///     connected_sock.writable().await?;
    ///     let mut buf = [0; 12];
    ///     let len = connected_sock.try_send(&mut buf)?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn writable(&self) -> io::Result<()> {
        self.source.entry.readiness(Interest::WRITABLE).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::io::ReadBuf;
    use crate::net::UdpSocket;
    use crate::futures::poll_fn;
    use crate::{block_on, spawn};

    /// UT test for `poll_send()` and `poll_recv()`.
    ///
    /// # Title
    /// test_send_recv_poll
    ///
    /// # Brief
    /// 1.Create UdpSocket and connect to the remote address.
    /// 2.Sender calls poll_fn() to send message first.
    /// 3.Receiver calls poll_fn() to receive message.
    /// 4.Check if the test results are correct.
    #[test]
    fn test_send_recv_poll() {
        let sender_addr = "127.0.0.1:8083".parse().unwrap();
        let receiver_addr = "127.0.0.1:8084".parse().unwrap();
        let handle = spawn(async move {
            let sender = match UdpSocket::bind(sender_addr).await {
                Ok(socket) => socket,
                Err(e) => {
                    panic!("Bind Socket Failed {}", e);
                }
            };

            let receiver = match UdpSocket::bind(receiver_addr).await {
                Ok(socket) => socket,
                Err(e) => {
                    panic!("Bind Socket Failed {}", e);
                }
            };

            let connected_sender = match sender.connect(receiver_addr).await {
                Ok(socket) => socket,
                Err(e) => {
                    panic!("Connect Socket Failed {}", e);
                }
            };
            let connected_receiver = match receiver.connect(sender_addr).await {
                Ok(socket) => socket,
                Err(e) => {
                    panic!("Connect Socket Failed {}", e);
                }
            };

            match poll_fn(|cx| connected_sender.poll_send(cx, b"Hello")).await {
                Ok(n) => {
                    assert_eq!(n, "Hello".len());
                }
                Err(e) => {
                    panic!("Sender Send Failed {}", e);
                }
            }

            let mut recv_buf = [0_u8; 12];
            let mut read = ReadBuf::new(&mut recv_buf);
            poll_fn(|cx| connected_receiver.poll_recv(cx, &mut read))
                .await
                .unwrap();

            assert_eq!(read.filled(), b"Hello");
        });
        block_on(handle).expect("block_on failed");
    }

    /// UT test for `poll_send_to()` and `poll_recv_from()`.
    ///
    /// # Title
    /// test_send_to_recv_from_poll
    ///
    /// # Brief
    /// 1.Create UdpSocket.
    /// 2.Sender calls poll_fn() to send message to the specified address.
    /// 3.Receiver calls poll_fn() to receive message and return the address the message from.
    /// 4.Check if the test results are correct.
    #[test]
    fn test_send_to_recv_from_poll() {
        let sender_addr = "127.0.0.1:8087".parse().unwrap();
        let receiver_addr = "127.0.0.1:8088".parse().unwrap();
        let handle = spawn(async move {
            let sender = match UdpSocket::bind(sender_addr).await {
                Ok(socket) => socket,
                Err(e) => {
                    panic!("Bind Socket Failed {}", e);
                }
            };

            let receiver = match UdpSocket::bind(receiver_addr).await {
                Ok(socket) => socket,
                Err(e) => {
                    panic!("Bind Socket Failed {}", e);
                }
            };

            match poll_fn(|cx| sender.poll_send_to(cx, b"Hello", receiver_addr)).await {
                Ok(n) => {
                    assert_eq!(n, "Hello".len());
                }
                Err(e) => {
                    panic!("Sender Send Failed {}", e);
                }
            }

            let mut recv_buf = [0_u8; 12];
            let mut read = ReadBuf::new(&mut recv_buf);
            let addr = poll_fn(|cx| receiver.poll_recv_from(cx, &mut read))
                .await
                .unwrap();
            assert_eq!(read.filled(), b"Hello");
            assert_eq!(addr, sender_addr);
        });
        block_on(handle).expect("block_on failed");
    }

    /// UT test for `broadcast()` and `set_broadcast()`.
    ///
    /// # Title
    /// ut_set_get_broadcast
    ///
    /// # Brief
    /// 1.Create UdpSocket.
    /// 2.Sender calls set_broadcast() to set broadcast.
    /// 3.Sender calls broadcast() to get broadcast.
    /// 4.Check if the test results are correct.
    #[test]
    fn ut_set_get_broadcast() {
        let local_addr = "127.0.0.1:8091".parse().unwrap();

        let handle = spawn(async move {
            let broadcast_socket = match UdpSocket::bind(local_addr).await {
                Ok(socket) => socket,
                Err(e) => {
                    panic!("Bind Socket Failed {}", e);
                }
            };
            broadcast_socket.set_broadcast(true).expect("set_broadcast failed");

            assert!(broadcast_socket.broadcast().expect("get broadcast failed"));
        });
        block_on(handle).expect("block_on failed");
    }

    /// UT test for `local_addr()`.
    ///
    /// # Title
    /// ut_get_local_addr
    ///
    /// # Brief
    /// 1.Create UdpSocket.
    /// 2.Sender calls local_addr() to get local address.
    /// 3.Check if the test results are correct.
    #[test]
    fn ut_get_local_addr() {
        let local_addr = "127.0.0.1:8092".parse().unwrap();
        let remote_addr = "127.0.0.1:8093".parse().unwrap();

        let handle = spawn(async move {
            let sock = match UdpSocket::bind(local_addr).await {
                Ok(socket) => socket,
                Err(e) => {
                    panic!("Bind Socket Failed {}", e);
                }
            };
            let connected_sock = match sock.connect(remote_addr).await {
                Ok(socket) => socket,
                Err(e) => {
                    panic!("Connect Socket Failed {}", e);
                }
            };
            let local_addr1 = connected_sock.local_addr().expect("local_addr failed");
            assert_eq!(local_addr1, local_addr);
        });
        block_on(handle).expect("block_on failed");
    }

    /// UT test for `peer_addr()`.
    ///
    /// # Title
    /// ut_get_peer_addr
    ///
    /// # Brief
    /// 1.Create UdpSocket.
    /// 2.Sender calls peer_addr() to get the socket address of the remote peer.
    /// 3.Check if the test results are correct.
    #[test]
    fn ut_get_peer_addr() {
        let local_addr = "127.0.0.1:8094".parse().unwrap();
        let peer_addr = "127.0.0.1:8095".parse().unwrap();
        let handle = spawn(async move {
            let sock = match UdpSocket::bind(local_addr).await {
                Ok(socket) => socket,
                Err(e) => {
                    panic!("Bind Socket Failed {}", e);
                }
            };
            let connected_sock = match sock.connect(peer_addr).await {
                Ok(socket) => socket,
                Err(e) => {
                    panic!("Connect Socket Failed {}", e);
                }
            };
            assert_eq!(connected_sock.peer_addr().expect("peer_addr failed"), peer_addr);
        });
        block_on(handle).expect("block_on failed");
    }
}