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

use super::{TcpSocket, TcpStream};
use crate::{Interest, Source, Selector, Token};
use libc::{
    c_int, sockaddr_in, sockaddr_in6, sockaddr_storage, socklen_t, SOCK_CLOEXEC, SOCK_NONBLOCK,
};
use std::io;
use std::mem::{size_of, MaybeUninit};
use std::net::{self, SocketAddr};
use std::os::unix::io::{AsRawFd, FromRawFd};

/// A socket server.
pub struct TcpListener {
    pub(crate) inner: net::TcpListener,
}

impl TcpListener {
    /// Binds a new tcp Listener to the specific address to receive connection requests.
    ///
    /// The socket will be set to `SO_REUSEADDR`.
    pub fn bind(addr: SocketAddr) -> io::Result<TcpListener> {
        let socket = TcpSocket::new_socket(addr)?;
        socket.set_reuse(true)?;
        socket.bind(addr)?;
        socket.listen(1024)
    }

    /// Accepts connections and returns the `TcpStream` and the remote peer address.
    ///
    /// # Error
    /// This may return an `Err(e)` where `e.kind()` is `io::ErrorKind::WouldBlock`.
    /// This means a stream may be ready at a later point and one should wait for an event
    /// before calling `accept` again.
    pub fn accept(&self) -> io::Result<(TcpStream, SocketAddr)> {
        let mut addr: MaybeUninit<sockaddr_storage> = MaybeUninit::uninit();
        let mut length = size_of::<sockaddr_storage>() as socklen_t;

        let stream = match syscall!(accept4(
            self.inner.as_raw_fd(),
            addr.as_mut_ptr() as *mut _,
            &mut length,
            SOCK_CLOEXEC | SOCK_NONBLOCK,
        )) {
            Ok(socket) => unsafe { net::TcpStream::from_raw_fd(socket) },
            Err(err) => {
                return Err(err);
            }
        };

        let ret = unsafe { trans_addr_2_socket(addr.as_ptr()) };

        match ret {
            Ok(sockaddr) => Ok((TcpStream::from_std(stream), sockaddr)),
            Err(err) => Err(err),
        }
    }
}

pub(crate) unsafe fn trans_addr_2_socket(
    storage: *const libc::sockaddr_storage,
) -> io::Result<SocketAddr> {
    match (*storage).ss_family as c_int {
        libc::AF_INET => Ok(SocketAddr::V4(*(storage as *const sockaddr_in as *const _))),
        libc::AF_INET6 => Ok(SocketAddr::V6(
            *(storage as *const sockaddr_in6 as *const _),
        )),
        _ => {
            let err = io::Error::new(io::ErrorKind::Other, "Cannot transfer address into socket.");
            Err(err)
        }
    }
}

impl Source for TcpListener {
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

impl std::fmt::Debug for TcpListener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}
