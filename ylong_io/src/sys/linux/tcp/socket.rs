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

use super::{TcpListener, TcpStream};
use libc::{
    c_int, c_void, socklen_t, AF_INET, AF_INET6, SOCK_CLOEXEC, SOCK_NONBLOCK,
    SOCK_STREAM, SOL_SOCKET, SO_REUSEADDR,
};
use std::io;
use std::mem::{self, size_of};
use std::net::{self, SocketAddr};
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use super::super::socket_addr::socket_addr_trans;

pub(crate) struct TcpSocket {
    socket: c_int,
}

impl TcpSocket {
    pub(crate) fn new_socket(addr: SocketAddr) -> io::Result<TcpSocket> {
        if addr.is_ipv4() {
            TcpSocket::create_socket(AF_INET, SOCK_STREAM)
        } else {
            TcpSocket::create_socket(AF_INET6, SOCK_STREAM)
        }
    }

    pub(crate) fn create_socket(domain: c_int, socket_type: c_int) -> io::Result<TcpSocket> {
        let socket_type = socket_type | SOCK_NONBLOCK | SOCK_CLOEXEC;
        match syscall!(socket(domain, socket_type, 0)) {
            Ok(socket) => Ok(TcpSocket {
                socket: socket as c_int,
            }),
            Err(err) => {
                Err(err)
            }
        }
    }

    pub(crate) fn set_reuse(&self, is_reuse: bool) -> io::Result<()> {
        let set_value: c_int = i32::from(is_reuse);

        match syscall!(setsockopt(
            self.socket,
            SOL_SOCKET,
            SO_REUSEADDR,
            &set_value as *const c_int as *const c_void,
            size_of::<c_int>() as socklen_t
        )) {
            Err(err) => {
                Err(err)
            }
            Ok(_) => Ok(()),
        }
    }

    pub(crate) fn bind(&self, addr: SocketAddr) -> io::Result<()> {
        let (raw_addr, addr_length) = socket_addr_trans(&addr);
        match syscall!(bind(self.socket, raw_addr.as_ptr(), addr_length)) {
            Err(err) => {
                Err(err)
            }
            Ok(_) => Ok(()),
        }
    }

    pub(crate) fn listen(self, max_connect: c_int) -> io::Result<TcpListener> {
        syscall!(listen(self.socket, max_connect))?;

        let tcp_listener = Ok(TcpListener {
            inner: unsafe { net::TcpListener::from_raw_fd(self.socket) },
        });

        mem::forget(self);

        tcp_listener
    }

    pub(crate) fn connect(self, addr: SocketAddr) -> io::Result<TcpStream> {
        let (raw_addr, addr_length) = socket_addr_trans(&addr);
        match syscall!(connect(self.socket, raw_addr.as_ptr(), addr_length)) {
            Err(err) if err.raw_os_error() != Some(libc::EINPROGRESS) => {
                Err(err)
            }
            _ => {
                let tcp_stream = Ok(TcpStream {
                    inner: unsafe { net::TcpStream::from_raw_fd(self.socket) },
                });
                mem::forget(self);
                tcp_stream
            }
        }
    }

    pub(crate) fn close(&self) {
        let _ = unsafe { net::TcpStream::from_raw_fd(self.socket) };
    }
}

impl AsRawFd for TcpSocket {
    fn as_raw_fd(&self) -> RawFd {
        self.socket
    }
}

impl FromRawFd for TcpSocket {
    unsafe fn from_raw_fd(fd: RawFd) -> TcpSocket {
        TcpSocket { socket: fd }
    }
}

impl Drop for TcpSocket {
    fn drop(&mut self) {
        self.close();
    }
}
