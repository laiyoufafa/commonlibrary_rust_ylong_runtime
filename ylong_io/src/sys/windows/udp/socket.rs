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
use std::net::SocketAddr;
use std::os::windows::io::{FromRawSocket, RawSocket};
use std::os::windows::raw;
use windows_sys::Win32::Networking::WinSock::{bind as win_bind, ADDRESS_FAMILY, AF_INET, AF_INET6, closesocket, FIONBIO, INVALID_SOCKET, ioctlsocket, SOCK_DGRAM, socket, SOCKET, SOCKET_ERROR};
use crate::sys::windows::net::init;
use crate::sys::windows::socket_addr::socket_addr_trans;

#[derive(Debug)]
pub(crate) struct UdpSock {
    socket: SOCKET,
}

impl UdpSock {
    /// Gets new socket
    pub(crate) fn new_socket(addr: SocketAddr) -> io::Result<UdpSock> {
        if addr.is_ipv4() {
            Self::create_socket(AF_INET, SOCK_DGRAM)
        } else {
            Self::create_socket(AF_INET6, SOCK_DGRAM)
        }
    }

    fn create_socket(domain: ADDRESS_FAMILY, socket_type: u16) -> io::Result<UdpSock> {
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
            Ok(_) => {Ok(UdpSock{socket})}
        }
    }

    /// System call gets net::UdpSocket
    pub(crate) fn bind(self, addr: SocketAddr) -> io::Result<net::UdpSocket> {
        let socket = unsafe { net::UdpSocket::from_raw_socket(self.socket as raw::SOCKET) };

        let (raw_addr, addr_length) = socket_addr_trans(&addr);
        socket_syscall!(
            win_bind(self.socket, raw_addr.as_ptr(), addr_length),
            PartialEq::eq,
            SOCKET_ERROR
        )?;
        // must
        mem::forget(self);
        Ok(socket)
    }

    /// Closes Socket
    pub(crate) fn close(&self) {
        let _ = unsafe { net::UdpSocket::from_raw_socket(self.socket as RawSocket) };
    }
}

impl Drop for UdpSock {
    fn drop(&mut self) {
        self.close();
    }
}