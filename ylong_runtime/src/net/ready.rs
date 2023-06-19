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

use core::ops;
use ylong_io::{Event, EventTrait, Interest};

const READABLE: usize = 0b0_01;
const WRITABLE: usize = 0b0_10;
const READ_CLOSED: usize = 0b0_0100;
const WRITE_CLOSED: usize = 0b0_1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd)]
pub(crate) struct Ready(usize);

#[derive(Debug)]
pub(crate) struct ReadyEvent {
    tick: u8,
    pub(crate) ready: Ready,
}

impl Ready {
    pub const EMPTY: Ready = Ready(0);

    pub const READABLE: Ready = Ready(READABLE);

    pub const WRITABLE: Ready = Ready(WRITABLE);

    pub const READ_CLOSED: Ready = Ready(READ_CLOSED);

    pub const WRITE_CLOSED: Ready = Ready(WRITE_CLOSED);

    pub const ALL: Ready = Ready(READABLE | WRITABLE | READ_CLOSED | WRITE_CLOSED);

    pub(crate) fn from_event(event: &Event) -> Ready {
        let mut ready = Ready::EMPTY;

        if event.is_readable() {
            ready |= Ready::READABLE;
        }

        if event.is_writable() {
            ready |= Ready::WRITABLE;
        }

        if event.is_read_closed() {
            ready |= Ready::READ_CLOSED;
        }

        if event.is_write_closed() {
            ready |= Ready::WRITE_CLOSED;
        }
        ready
    }

    pub fn is_empty(self) -> bool {
        self == Ready::EMPTY
    }

    pub fn is_readable(self) -> bool {
        (self & Ready::READABLE).0 != 0 || self.is_read_closed()
    }

    pub fn is_writable(self) -> bool {
        (self & Ready::WRITABLE).0 != 0 || self.is_write_closed()
    }

    pub fn is_read_closed(self) -> bool {
        (self & Ready::READ_CLOSED).0 != 0
    }

    pub fn is_write_closed(self) -> bool {
        (self & Ready::WRITE_CLOSED).0 != 0
    }

    pub(crate) fn from_usize(val: usize) -> Ready {
        Ready(val & Ready::ALL.as_usize())
    }

    pub(crate) fn as_usize(self) -> usize {
        self.0
    }

    pub(crate) fn from_interest(interest: Interest) -> Ready {
        let mut ready = Ready::EMPTY;

        if interest.is_readable() {
            ready |= Ready::READABLE;
            ready |= Ready::READ_CLOSED;
        }

        if interest.is_writable() {
            ready |= Ready::WRITABLE;
            ready |= Ready::WRITE_CLOSED;
        }

        ready
    }

    pub(crate) fn intersection(self, interest: Interest) -> Ready {
        Ready(self.0 & Ready::from_interest(interest).0)
    }

    pub(crate) fn satisfies(self, interest: Interest) -> bool {
        self.0 & Ready::from_interest(interest).0 != 0
    }
}

impl ops::BitOr<Ready> for Ready {
    type Output = Ready;

    #[inline]
    fn bitor(self, other: Ready) -> Self::Output {
        Ready(self.0 | other.0)
    }
}

impl ops::BitOrAssign<Ready> for Ready {
    #[inline]
    fn bitor_assign(&mut self, other: Ready) {
        self.0 |= other.0
    }
}

impl ops::BitAnd<Ready> for Ready {
    type Output = Ready;

    #[inline]
    fn bitand(self, other: Ready) -> Self::Output {
        Ready(self.0 & other.0)
    }
}

impl ops::Sub<Ready> for Ready {
    type Output = Ready;

    #[inline]
    fn sub(self, other: Ready) -> Self::Output {
        Ready(self.0 & !other.0)
    }
}

impl ReadyEvent {
    pub fn new(tick: u8, ready: Ready) -> Self {
        ReadyEvent { tick, ready }
    }

    pub fn get_tick(&self) -> u8 {
        self.tick
    }

    pub fn get_ready(&self) -> Ready {
        self.ready
    }
}

/*
* @title  ready from_event function ut test
* @design conditions of use override
* @precon none
* @brief  1. Create an event
*         2. Call from_event
*         3. Verify the returned results
* @expect 1. Event readable to get readable Ready instances
*         2. Event writable, call writable Ready instances
*         3. Event Read Close, Call Read Close Ready Instance
*         4. Event Write Close, Call Write Close Ready Instance
* @auto  Yes
*/
#[test]
#[cfg(feature = "tcp")]
fn ut_ready_from_event() {
    ut_ready_from_event_01();
    ut_ready_from_event_02();
    ut_ready_from_event_03();
    ut_ready_from_event_04();

    // Readable
    fn ut_ready_from_event_01() {
        let mut event = libc::epoll_event {
            events: 0b00,
            u64: 0,
        };
        event.events |= libc::EPOLLIN as u32;
        let ready = Ready::from_event(&event);
        assert_eq!(ready.0, 0b01);
    }

    // Writable
    fn ut_ready_from_event_02() {
        let mut event = libc::epoll_event {
            events: 0b00,
            u64: 0,
        };
        event.events |= libc::EPOLLOUT as u32;
        let ready = Ready::from_event(&event);
        assert_eq!(ready.0, 0b10);
    }

    // Read off
    fn ut_ready_from_event_03() {
        let mut event = libc::epoll_event {
            events: 0b00,
            u64: 0,
        };
        event.events |= (libc::EPOLLIN | libc::EPOLLRDHUP) as u32;
        let ready = Ready::from_event(&event);
        assert_eq!(ready.0, 0b101);
    }

    // Write Off
    fn ut_ready_from_event_04() {
        let mut event = libc::epoll_event {
            events: 0x00,
            u64: 0,
        };
        event.events |= (libc::EPOLLOUT | libc::EPOLLERR) as u32;
        let ready = Ready::from_event(&event);
        assert_eq!(ready.0, 0b1010);
    }
}

/*
* @title  ready from_usize function ut test
* @design Use path override
* @precon None
* @brief  1. Enter a usize, call from_usize
*         2. Verify the returned results
* @expect 1. Generate a Ready that is passed into usize
* @auto  Yes
*/
#[test]
fn ut_ready_from_usize() {
    let ready = Ready::from_usize(0x01);
    assert_eq!(ready.0, 0x01);
}

/*
* @title  ready is_empty function ut test
* @design Use path override
* @precon None
* @brief  1. Create a Ready
*         2. Call is_empty
*         3. Verify the returned results
* @expect 1. Empty Ready instances, return true
*         2. Non-empty Ready instances, return false
* @auto  Yes
*/
#[test]
fn ut_ready_is_empty() {
    let ready = Ready::from_usize(0x00);
    assert!(ready.is_empty());

    let ready = Ready::from_usize(0x01);
    assert!(!ready.is_empty());
}

/*
* @title  ready is_readable function ut test
* @design Use path override
* @precon None
* @brief  1. Create a Ready
*         2. Call is_readable
*         3. Verify the returned results
* @expect 1. Readable Ready instances that return true
*         2. Non-readable Ready instance that returns false
* @auto  Yes
*/
#[test]
fn ut_ready_is_readable() {
    let ready = Ready::from_usize(0x01);
    assert!(ready.is_readable());

    let ready = Ready::from_usize(0x02);
    assert!(!ready.is_readable());
}

/*
* @title  ready is_writable function ut test
* @design Use path override
* @precon None
* @brief  1. Create a Ready
*         2. Call is_writable
*         3. Verify the returned results
* @expect 1. Writable Ready instances that return true
*         2. Non-writable Ready instance that returns false
* @auto  Yes
*/
#[test]
fn ut_ready_is_writable() {
    let ready = Ready::from_usize(0x02);
    assert!(ready.is_writable());

    let ready = Ready::from_usize(0x01);
    assert!(!ready.is_writable());
}

/*
* @title  ready is_read_closed function ut test
* @design Use path override
* @precon None
* @brief  1. Create a Ready
*         2. Call is_read_closed
*         3. Verify the returned results
* @expect 1. Read the closed Ready instance and return true
*         2. Ready instances that are not read off, return false
* @auto  Yes
*/
#[test]
fn ut_ready_is_read_closed() {
    let ready = Ready::from_usize(0x04);
    assert!(ready.is_read_closed());

    let ready = Ready::from_usize(0x01);
    assert!(!ready.is_read_closed());
}

/*
* @title  ready is_write_closed function ut test
* @design Use path override
* @precon None
* @brief  1. Create a Ready
*         2. Call is_write_closed
*         3. Verify the returned results
* @expect 1. Write closed Ready instances, return true
*         2. Ready instances that are not written off return false
* @auto  Yes
*/
#[test]
fn ut_ready_is_write_closed() {
    let ready = Ready::from_usize(0x08);
    assert!(ready.is_write_closed());

    let ready = Ready::from_usize(0x01);
    assert!(!ready.is_write_closed());
}

/*
* @title  ready as_usize function ut test
* @design Use path override
* @precon None
* @brief  1. Create a Ready
*         2. Call as_usize
*         3. Verify the returned results
* @expect 1. Get the usize in Ready
* @auto  Yes
*/
#[test]
fn ut_ready_as_usize() {
    let ready = Ready::from_usize(0x08);
    assert_eq!(ready.as_usize(), 0x08);
}

/*
* @title  ready from_interest function ut test
* @design Use path override
* @precon None
* @brief  1. Create a Interest instances
*         2. Call from_interest
*         3. Verify the returned results
* @expect 1. Create a readable Interest instances, get readable + read off Ready instances
*         2. Create a writable Interest instances, get writable + write off Ready instances
* @auto  Yes
*/
#[test]
fn ut_ready_from_interest() {
    let interest = Interest::READABLE;
    let ready = Ready::from_interest(interest);
    assert_eq!(ready.as_usize(), 0b101);

    let interest = Interest::WRITABLE;
    let ready = Ready::from_interest(interest);
    assert_eq!(ready.as_usize(), 0b1010);
}

/*
* @title  ready intersection function ut test
* @design Use path override
* @precon None
* @brief  1. Create a Interest instances and a Ready instances
*         2. Call intersection
*         3. Verify the returned results
* @expect 1. Get Ready instances of an intersection
* @auto  Yes
*/
#[test]
fn ut_ready_intersection() {
    let interest = Interest::READABLE;
    let ready = Ready::from_usize(0b1111);
    let res = ready.intersection(interest);
    assert_eq!(res.0, 0b0101);
}

/*
* @title  ready satisfies function ut test
* @design Use path override
* @precon None
* @brief  1. Create a Interest instances, and a Ready instances
*         2. Call satisfies
*         3. Verify the returned results
* @expect 1. Returns true if ready satisfies Interest
          2. Returns false if ready not satisfies Interest
* @auto  Yes
*/
#[test]
fn ut_ready_satisfies() {
    let interest = Interest::READABLE;
    let ready = Ready::from_usize(0b1111);
    assert!(ready.satisfies(interest));

    let ready = Ready::from_usize(0b0000);
    assert!(!ready.satisfies(interest));
}

/*
* @title  ready bitor function ut test
* @design Use path override
* @precon None
* @brief  1. Create two Ready instances
*         2. Call bitor or use | logical operators
*         3. Verify the returned results
* @expect 1. Get an or result of Ready
* @auto  Yes
*/
#[test]
fn ut_ready_bitor() {
    let ready1 = Ready::from_usize(0b1010);
    let ready2 = Ready::from_usize(0b0101);
    let ready3 = ready1 | ready2;
    assert_eq!(ready3.0, 0b1111);
}

/*
* @title  ready bitor_assign function ut test
* @design Use path override
* @precon None
* @brief  1. Create two Ready instances
*         2. Call bitor_assign or use |= logical operators
*         3. Verify the returned results
* @expect 1. Get a Ready of or equal to the result of the operation
* @auto  Yes
*/
#[test]
fn ut_ready_bitor_assign() {
    let mut ready1 = Ready::from_usize(0b1010);
    let ready2 = Ready::from_usize(0b0101);
    ready1 |= ready2;
    assert_eq!(ready1.0, 0b1111);
}

/*
* @title  ready bitand function ut test
* @design Use path override
* @precon None
* @brief  1. Create two Ready instances
*         2. Call bitand or use & logical operators
*         3. Verify the returned results
* @expect 1. Get a Ready with the result of the operation
* @auto  Yes
*/
#[test]
fn ut_ready_bitand() {
    let ready1 = Ready::from_usize(0b1010);
    let ready2 = Ready::from_usize(0b0101);
    let ready = ready1 & ready2;
    assert_eq!(ready.0, 0b0000);
}

/*
* @title  ready bitsub function ut test
* @design Use path override
* @precon None
* @brief  1. Create two Ready instances
*         2. Call bitsub or use - logical operators
*         3. Verify the returned results
* @expect 1. Get a Ready with the values ready1 & !ready2
* @auto  Yes
*/
#[test]
fn ut_ready_bitsub() {
    let ready1 = Ready::from_usize(0b1111);
    let ready2 = Ready::from_usize(0b0101);
    let ready = ready1 - ready2;
    assert_eq!(ready.0, 0b1010);
}

/*
* @title  ready_event new function ut test
* @design Use path override
* @precon None
* @brief  1. Call new
*         2. Verify the returned results
* @expect 1. Get a ready_event instances
* @auto  Yes
*/
#[test]
fn ut_ready_event_new() {
    let ready_event = ReadyEvent::new(1u8, Ready::from_usize(0b0101));
    assert_eq!(ready_event.tick, 1u8);
    assert_eq!(ready_event.ready.0, 0b0101);
}

/*
* @title  ready_event get_tick function ut test
* @design Use path override
* @precon None
* @brief  1. Create a ready_event
*         2. Call get_tick
*         3. Verify the returned results
* @expect 1. Get the tick of ready_event
* @auto  Yes
*/
#[test]
fn ut_ready_event_get_tick() {
    let ready_event = ReadyEvent::new(1u8, Ready::from_usize(0b0101));
    assert_eq!(ready_event.get_tick(), 1u8);
}

/*
* @title  ready_event get_ready function ut test
* @design Use path override
* @precon None
* @brief  1. Create a ready_event
*         2. Call get_ready
*         3. Verify the returned results
* @expect 1. Get the ready of ready_event
* @auto  Yes
*/
#[test]
fn ut_ready_event_get_ready() {
    let ready_event = ReadyEvent::new(1u8, Ready::from_usize(0b0101));
    assert_eq!(ready_event.get_ready().0, 0b0101);
}
