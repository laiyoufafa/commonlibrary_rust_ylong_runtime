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

use std::mem;
use std::net::SocketAddr;
use windows_sys::Win32::Networking::WinSock::{
    AF_INET, AF_INET6, IN6_ADDR, IN6_ADDR_0, IN_ADDR, IN_ADDR_0, SOCKADDR, SOCKADDR_IN,
    SOCKADDR_IN6, SOCKADDR_IN6_0,
};

#[repr(C)]
pub(crate) union SocketAddrWin {
    v4: SOCKADDR_IN,
    v6: SOCKADDR_IN6,
}

impl SocketAddrWin {
    pub(crate) fn as_ptr(&self) -> *const SOCKADDR {
        self as *const _ as *const SOCKADDR
    }
}

pub(crate) fn socket_addr_trans(addr: &SocketAddr) -> (SocketAddrWin, i32) {
    match addr {
        SocketAddr::V4(ref addr) => {
            let sin_addr = unsafe {
                let mut in_addr_0 = mem::zeroed::<IN_ADDR_0>();
                in_addr_0.S_addr = u32::from_ne_bytes(addr.ip().octets());
                IN_ADDR { S_un: in_addr_0 }
            };

            let sockaddr_in = SOCKADDR_IN {
                sin_family: AF_INET,
                sin_port: addr.port().to_be(),
                sin_addr,
                sin_zero: [0; 8],
            };

            (
                SocketAddrWin { v4: sockaddr_in },
                mem::size_of::<SOCKADDR_IN>() as i32,
            )
        }

        SocketAddr::V6(ref addr) => {
            let sin_addr6 = unsafe {
                let mut u = mem::zeroed::<IN6_ADDR_0>();
                u.Byte = addr.ip().octets();
                IN6_ADDR { u }
            };

            let sockaddr_in6_0 = unsafe {
                let mut si0 = mem::zeroed::<SOCKADDR_IN6_0>();
                si0.sin6_scope_id = addr.scope_id();
                si0
            };

            let sockaddr_in6 = SOCKADDR_IN6 {
                sin6_family: AF_INET6,
                sin6_port: addr.port().to_be(),
                sin6_flowinfo: addr.flowinfo(),
                sin6_addr: sin_addr6,
                Anonymous: sockaddr_in6_0,
            };

            (
                SocketAddrWin { v6: sockaddr_in6 },
                mem::size_of::<SOCKADDR_IN6>() as i32,
            )
        }
    }
}
