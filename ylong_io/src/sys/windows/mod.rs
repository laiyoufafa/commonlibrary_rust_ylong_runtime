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

macro_rules! syscall {
    ($fn: ident ( $($arg: expr),* $(,)* ), $ret: expr) => {{
        let res = unsafe { $fn($($arg, )*) };
        if res == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok($ret)
        }
    }};
}

mod afd;
mod iocp;

mod selector;
pub use selector::Selector;

mod handle;
use handle::Handle;

mod events;
pub use events::{Event, Events};

mod overlapped;
pub(crate) use overlapped::Overlapped;

mod io_status_block;
mod socket_addr;

mod waker;
pub(crate) use waker::WakerInner;

mod net;
pub(crate) use net::NetInner;
cfg_net! {
    macro_rules! socket_syscall {
        ($fn: ident ( $($arg: expr),* $(,)* ), $err_fn: path, $err_val: expr) => {{
            let res = unsafe { $fn($($arg, )*) };
            if $err_fn(&res, &$err_val) {
                Err(io::Error::last_os_error())
            } else {
                Ok(res)
            }
        }};
    }
    pub(crate) use net::NetState;
}

cfg_tcp! {
    mod tcp;
    pub use self::tcp::{TcpListener, TcpStream};
}

cfg_udp! {
    mod udp;
    pub use self::udp::{UdpSocket, ConnectedUdpSocket};
}
