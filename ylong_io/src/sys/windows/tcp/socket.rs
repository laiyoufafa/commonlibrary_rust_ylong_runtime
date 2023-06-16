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

use std::{io, mem, net};
use std::net::{SocketAddr};
use std::os::windows::io::{AsRawSocket, FromRawSocket, RawSocket};
use windows_sys::Win32::Networking::WinSock::{self, closesocket, ioctlsocket, socket, SOCKET, SOCKET_ERROR, SOCK_STREAM, AF_INET, AF_INET6, ADDRESS_FAMILY, INVALID_SOCKET, FIONBIO};
use crate::sys::windows::net::init;
use crate::sys::windows::socket_addr::socket_addr_trans;

pub(crate) struct TcpSocket {
    socket: SOCKET,
}

impl TcpSocket {
    /// Gets new socket
    pub(crate) fn new_socket(addr: SocketAddr) -> io::Result<TcpSocket> {
        if addr.is_ipv4() {
            Self::create_socket(AF_INET, SOCK_STREAM)
        } else {
            Self::create_socket(AF_INET6, SOCK_STREAM)
        }
    }

    fn create_socket(domain: ADDRESS_FAMILY, socket_type: u16) -> io::Result<TcpSocket> {
        init();

        let socket = socket_syscall!(
            socket(domain as i32, socket_type as i32, 0),
            PartialEq::eq,
            INVALID_SOCKET
        )?;

        match socket_syscall!(ioctlsocket(socket, FIONBIO, &mut 1), PartialEq::ne, 0) {
            Err(err) => {
                let _ = unsafe { closesocket(socket) };
                Err(err)
            }
            Ok(_) => Ok(TcpSocket { socket: socket as SOCKET })
        }
    }

    /// System call to bind Socket.
    pub(crate) fn bind(&self, addr: SocketAddr) -> io::Result<()> {
        use WinSock::bind;

        let (raw_addr, raw_addr_length) = socket_addr_trans(&addr);
        socket_syscall!(
            bind(
                self.socket as _,
                raw_addr.as_ptr(),
                raw_addr_length
            ),
            PartialEq::eq,
            SOCKET_ERROR
        )?;
        Ok(())
    }

    /// System call to listen.
    pub(crate) fn listen(self, backlog: u32) -> io::Result<()> {
        use std::convert::TryInto;
        use WinSock::listen;

        let backlog = backlog.try_into().unwrap_or(i32::MAX);
        socket_syscall!(
            listen(self.socket as _, backlog),
            PartialEq::eq,
            SOCKET_ERROR
        )?;
        mem::forget(self);
        Ok(())
    }

    /// System call to connect.
    pub(crate) fn connect(self, addr: SocketAddr) -> io::Result<()> {
        use WinSock::connect;

        let (socket_addr, socket_addr_length) = socket_addr_trans(&addr);
        let res = socket_syscall!(
            connect(
                self.socket as _,
                socket_addr.as_ptr(),
                socket_addr_length
            ),
            PartialEq::eq,
            SOCKET_ERROR
        );

        match res {
            Err(e) if e.kind() != io::ErrorKind::WouldBlock => Err(e),
            _ => {
                mem::forget(self);
                Ok(())
            }
        }
    }

    /// Closes Socket
    pub(crate) fn close(&self) {
        let _ = unsafe { net::TcpStream::from_raw_socket(self.socket as RawSocket) };
    }
}

impl AsRawSocket for TcpSocket {
    fn as_raw_socket(&self) -> RawSocket {
        self.socket as RawSocket
    }
}

impl FromRawSocket for TcpSocket {
    unsafe fn from_raw_socket(sock: RawSocket) -> Self {
        TcpSocket { socket: sock as SOCKET }
    }
}

impl Drop for TcpSocket {
    fn drop(&mut self) {
        self.close();
    }
}
