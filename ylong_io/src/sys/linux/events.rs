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

use crate::{EventTrait, Token};

/// A vector of events
pub type Events = Vec<Event>;

/// An io event
pub type Event = libc::epoll_event;

impl EventTrait for Event {
    fn token(&self) -> Token {
        Token(self.u64 as usize)
    }

    fn is_readable(&self) -> bool {
        (self.events as libc::c_int & libc::EPOLLIN) != 0
            || (self.events as libc::c_int & libc::EPOLLPRI) != 0
    }

    fn is_writable(&self) -> bool {
        (self.events as libc::c_int & libc::EPOLLOUT) != 0
    }

    fn is_read_closed(&self) -> bool {
        self.events as libc::c_int & libc::EPOLLHUP != 0
            || (self.events as libc::c_int & libc::EPOLLIN != 0
                && self.events as libc::c_int & libc::EPOLLRDHUP != 0)
    }

    fn is_write_closed(&self) -> bool {
        self.events as libc::c_int & libc::EPOLLHUP != 0
            || (self.events as libc::c_int & libc::EPOLLOUT != 0
                && self.events as libc::c_int & libc::EPOLLERR != 0)
            || self.events as libc::c_int == libc::EPOLLERR
    }

    fn is_error(&self) -> bool {
        (self.events as libc::c_int & libc::EPOLLERR) != 0
    }
}
