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

use std::num::NonZeroU8;

/// The interested events, such as readable, writeable.
#[derive(Copy, PartialEq, Eq, Clone, PartialOrd, Ord)]
pub struct Interest(NonZeroU8);

const READABLE: u8 = 0b0001;
const WRITABLE: u8 = 0b0010;

/// A wrapper that wraps around fd events
impl Interest {
    /// An interest for readable events
    pub const READABLE: Interest = Interest(unsafe { NonZeroU8::new_unchecked(READABLE) });
    /// An interest for writeable events
    pub const WRITABLE: Interest = Interest(unsafe { NonZeroU8::new_unchecked(WRITABLE) });

    /// Combines two Interest into one.
    pub const fn add(self, other: Interest) -> Interest {
        Interest(unsafe { NonZeroU8::new_unchecked(self.0.get() | other.0.get()) })
    }

    /// Checks if the interest is for readable events.
    pub const fn is_readable(self) -> bool {
        (self.0.get() & READABLE) != 0
    }

    /// Checks if the interest is for writeable events.
    pub const fn is_writable(self) -> bool {
        (self.0.get() & WRITABLE) != 0
    }
}
