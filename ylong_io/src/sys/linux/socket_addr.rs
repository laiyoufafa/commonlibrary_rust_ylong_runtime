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

use libc::{sockaddr, socklen_t};
use std::mem::size_of;
use std::net::SocketAddr;

#[repr(C)]
pub(crate) union SocketAddrLibC {
    v4: libc::sockaddr_in,
    v6: libc::sockaddr_in6,
}

impl SocketAddrLibC {
    pub(crate) fn as_ptr(&self) -> *const sockaddr {
        self as *const _ as *const sockaddr
    }
}

pub(crate) fn socket_addr_trans(addr: &SocketAddr) -> (SocketAddrLibC, socklen_t) {
    match addr {
        SocketAddr::V4(ref addr) => {
            let sockaddr_in = libc::sockaddr_in {
                sin_family: libc::AF_INET as libc::sa_family_t,
                sin_port: addr.port().to_be(),
                sin_addr: libc::in_addr {
                    s_addr: u32::from_ne_bytes(addr.ip().octets()),
                },
                sin_zero: [0; 8],
            };

            (
                SocketAddrLibC { v4: sockaddr_in },
                size_of::<libc::sockaddr_in>() as socklen_t,
            )
        }

        SocketAddr::V6(ref addr) => {
            let sin6_addr = libc::in6_addr {
                s6_addr: addr.ip().octets(),
            };

            let sockaddr_in6 = libc::sockaddr_in6 {
                sin6_family: libc::AF_INET6 as libc::sa_family_t,
                sin6_port: addr.port().to_be(),
                sin6_addr,
                sin6_flowinfo: addr.flowinfo(),
                sin6_scope_id: addr.scope_id(),
            };

            (
                SocketAddrLibC { v6: sockaddr_in6 },
                size_of::<libc::sockaddr_in6>() as socklen_t,
            )
        }
    }
}
