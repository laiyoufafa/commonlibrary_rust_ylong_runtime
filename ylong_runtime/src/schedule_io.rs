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

use std::cell::UnsafeCell;
use std::future::Future;
use std::io;
use std::marker::PhantomPinned;
use std::pin::Pin;
use std::ptr::NonNull;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::{AcqRel, Acquire, Release, SeqCst};
use std::sync::Mutex;
use std::task::{Context, Poll, Waker};
use crate::util::bit::{Bit, Mask};
use ylong_io::Interest;
use crate::util::slab::Entry;
use crate::net::{LinkedList, Node, Ready, ReadyEvent};
use crate::futures::poll_fn;

const GENERATION: Mask = Mask::new(7, 24);
pub(crate) const DRIVER_TICK: Mask = Mask::new(8, 16);
pub(crate) const READINESS: Mask = Mask::new(16, 0);

// ScheduleIO::status structure
//
// | reserved | generation | driver tick | readiness |
// |----------|------------|-------------|-----------|
// |  1 bit   |   7 bits   |   8 bits    |  16 bits  |
pub(crate) struct ScheduleIO {
    /// ScheduleIO status
    pub(crate) status: AtomicUsize,

    /// Wakers that wait for this IO
    waiters: Mutex<Waiters>,
}

#[derive(Default)]
pub(crate) struct Waiters {
    list: LinkedList<UnsafeCell<Waiter>>,

    // Reader & writer wakers are for AsyncRead/AsyncWriter
    reader: Option<Waker>,

    writer: Option<Waker>,

    is_shutdown: bool,
}

pub(crate) struct Waiter {
    waker: Option<Waker>,

    interest: Interest,

    is_ready: bool,

    _p: PhantomPinned,
}

pub(crate) enum Tick {
    Set(u8),
    Clear(u8),
}

impl Default for ScheduleIO {
    fn default() -> Self {
        ScheduleIO {
            status: AtomicUsize::new(0),
            waiters: Mutex::new(Default::default()),
        }
    }
}

impl Entry for ScheduleIO {
    fn reset(&self) {
        let status_bit = Bit::from_usize(self.status.load(Acquire));

        let generation = status_bit.get_by_mask(GENERATION);
        let new_generation = generation.wrapping_add(1);
        let mut next = Bit::from_usize(0);
        next.set_by_mask(GENERATION, new_generation);
        self.status.store(next.as_usize(), Release);
    }
}

impl ScheduleIO {
    pub fn generation(&self) -> usize {
        let base = Bit::from_usize(self.status.load(Acquire));
        base.get_by_mask(GENERATION)
    }

    #[cfg(feature = "net")]
    pub(crate) fn poll_readiness(
        &self,
        cx: &mut Context<'_>,
        interest: Interest,
    ) -> Poll<ReadyEvent> {
        // Get current status and check if it contains our interest
        let curr_bit = Bit::from_usize(self.status.load(Acquire));
        let ready = Ready::from_usize(curr_bit.get_by_mask(READINESS)).intersection(interest);

        if ready.is_empty() {
            let mut waiters = self.waiters.lock().unwrap();
            // Put the waker associated with the context into the waiters
            match interest {
                Interest::WRITABLE => waiters.writer = Some(cx.waker().clone()),
                Interest::READABLE => waiters.reader = Some(cx.waker().clone()),
                _ => unreachable!(),
            }

            // Check one more time to see if any event is ready
            let ready_event = self.get_readiness(interest);
            if !waiters.is_shutdown && ready_event.ready.is_empty() {
                Poll::Pending
            } else {
                Poll::Ready(ready_event)
            }
        } else {
            let tick = curr_bit.get_by_mask(DRIVER_TICK) as u8;
            Poll::Ready(ReadyEvent::new(tick, ready))
        }
    }

    #[inline]
    pub(crate) fn get_readiness(&self, interest: Interest) -> ReadyEvent {
        let status_bit = Bit::from_usize(self.status.load(Acquire));
        let ready = Ready::from_usize(status_bit.get_by_mask(READINESS)).intersection(interest);
        let tick = status_bit.get_by_mask(DRIVER_TICK) as u8;
        ReadyEvent::new(tick, ready)
    }

    pub(crate) async fn readiness(&self, interest: Interest) -> io::Result<ReadyEvent> {
        let mut fut = self.readiness_fut(interest);
        let mut fut = unsafe { Pin::new_unchecked(&mut fut) };

        poll_fn(|cx| {
            Pin::new(&mut fut).poll(cx)
        })
        .await
    }

    async fn readiness_fut(&self, interest: Interest) -> io::Result<ReadyEvent> {
        Readiness::new(self, interest).await
    }

    pub(crate) fn shutdown(&self) {
        self.wake0(Ready::ALL, true);
    }

    pub(crate) fn clear_readiness(&self, ready: ReadyEvent) {
        let mask_no_closed = ready.get_ready() - Ready::READ_CLOSED - Ready::WRITE_CLOSED;
        let _ = self.set_readiness(None, Tick::Clear(ready.get_tick()), |curr| {
            curr - mask_no_closed
        });
    }

    pub(crate) fn set_readiness(
        &self,
        token: Option<usize>,
        tick: Tick,
        f: impl Fn(Ready) -> Ready,
    ) -> io::Result<()> {
        let mut current = self.status.load(Acquire);
        loop {
            let current_bit = Bit::from_usize(current);
            let current_generation = current_bit.get_by_mask(GENERATION);

            // if token's generation is different from ScheduleIO's generation,
            // this token is already expired.
            if let Some(token) = token {
                if Bit::from_usize(token).get_by_mask(GENERATION) != current_generation {
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        "Token no longer valid.",
                    ));
                }
            }

            let current_readiness = Ready::from_usize(current_bit.get_by_mask(READINESS));
            let new_readiness = f(current_readiness);
            let mut new_bit = Bit::from_usize(new_readiness.as_usize());

            match tick {
                Tick::Set(t) => new_bit.set_by_mask(DRIVER_TICK, t as usize),
                // Check the tick to see if the event has already been covered.
                // If yes, clear the event.
                Tick::Clear(t) => {
                    if current_bit.get_by_mask(DRIVER_TICK) as u8 != t {
                        return Err(io::Error::new(
                            io::ErrorKind::Other,
                            "Readiness has been covered.",
                        ));
                    }
                    new_bit.set_by_mask(DRIVER_TICK, t as usize);
                }
            }

            new_bit.set_by_mask(GENERATION, current_generation);
            match self
                .status
                .compare_exchange(current, new_bit.as_usize(), AcqRel, Acquire)
            {
                Ok(_) => return Ok(()),
                // status has been changed already, so we repeats the loop
                Err(actual) => current = actual,
            }
        }
    }

    pub(crate) fn wake(&self, ready: Ready) {
        self.wake0(ready, false);
    }

    fn wake0(&self, ready: Ready, shutdown: bool) {
        let mut wakers = Vec::new();
        let mut waiters = self.waiters.lock().unwrap();
        waiters.is_shutdown |= shutdown;

        if ready.is_readable() {
            if let Some(waker) = waiters.reader.take() {
                wakers.push(Some(waker));
            }
        }

        if ready.is_writable() {
            if let Some(waker) = waiters.writer.take() {
                wakers.push(Some(waker));
            }
        }

        waiters.list.for_each_mut(|waiter| {
            let waiter = waiter.get();
            unsafe {
                if ready.satisfies((*waiter).interest) {
                    if let Some(waker) = (*waiter).waker.take() {
                        (*waiter).is_ready = true;
                        wakers.push(Some(waker));
                    }
                }
            }
        });

        drop(waiters);
        for waker in wakers.iter_mut() {
            waker.take().unwrap().wake();
        }
    }
}

impl Drop for ScheduleIO {
    fn drop(&mut self) {
        self.wake(Ready::ALL);
    }
}

unsafe impl Send for ScheduleIO {}
unsafe impl Sync for ScheduleIO {}

pub(crate) struct Readiness<'a> {
    schedule_io: &'a ScheduleIO,

    state: State,

    interest: Option<Interest>,

    waiter: Option<NonNull<Node<UnsafeCell<Waiter>>>>,
}

enum State {
    Init,
    Waiting,
    Done,
}

impl Readiness<'_> {
    pub(crate) fn new(schedule_io: &ScheduleIO, interest: Interest) -> Readiness<'_> {
        Readiness {
            schedule_io,
            state: State::Init,
            interest: Some(interest),
            waiter: None,
        }
    }
}

impl Future for Readiness<'_> {
    type Output = io::Result<ReadyEvent>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let (schedule_io, state, interest, waiter) = unsafe {
            let me = self.get_unchecked_mut();
            (me.schedule_io, &mut me.state, me.interest, &mut me.waiter)
        };
        loop {
            match *state {
                State::Init => {
                    let status_bit = Bit::from_usize(schedule_io.status.load(SeqCst));
                    let readiness = Ready::from_usize(status_bit.get_by_mask(READINESS));

                    let ready = readiness.intersection(interest.unwrap());

                    // if events are ready, change status to done
                    if !ready.is_empty() {
                        let tick = status_bit.get_by_mask(DRIVER_TICK) as u8;
                        *state = State::Done;
                        return Poll::Ready(Ok(ReadyEvent::new(tick, ready)));
                    }

                    let mut waiters = schedule_io.waiters.lock().unwrap();

                    let status_bit = Bit::from_usize(schedule_io.status.load(SeqCst));
                    let mut readiness = Ready::from_usize(status_bit.get_by_mask(READINESS));

                    if waiters.is_shutdown {
                        readiness = Ready::ALL;
                    }

                    let ready = readiness.intersection(interest.unwrap());

                    // check one more time to see if events are ready
                    if !ready.is_empty() {
                        let tick = status_bit.get_by_mask(DRIVER_TICK) as u8;
                        *state = State::Done;
                        return Poll::Ready(Ok(ReadyEvent::new(tick, ready)));
                    }

                    *waiter = Some(waiters.list.add_item(UnsafeCell::new(Waiter {
                        waker: Some(cx.waker().clone()),
                        interest: interest.unwrap(),
                        is_ready: false,
                        _p: PhantomPinned,
                    })));

                    *state = State::Waiting;
                }
                State::Waiting => unsafe {
                    // waiters could also be accessed in other places, so get the lock
                    let waiters = schedule_io.waiters.lock().unwrap();

                    let mut waiter = waiter.as_mut().unwrap().as_mut().get_mut().get();
                    if (*waiter).is_ready {
                        *state = State::Done;
                    } else {
                        if !(*waiter).waker.as_ref().unwrap().will_wake(cx.waker()) {
                            (*waiter).waker = Some(cx.waker().clone());
                        }
                        return Poll::Pending;
                    }
                    drop(waiters);
                },
                State::Done => {
                    let status_bit = Bit::from_usize(schedule_io.status.load(Acquire));
                    return Poll::Ready(Ok(ReadyEvent::new(
                        status_bit.get_by_mask(DRIVER_TICK) as u8,
                        Ready::from_interest(interest.unwrap()),
                    )));
                }
            }
        }
    }
}

unsafe impl Sync for Readiness<'_> {}
unsafe impl Send for Readiness<'_> {}

impl Drop for Readiness<'_> {
    fn drop(&mut self) {
        let mut waiters = self.schedule_io.waiters.lock().unwrap();

        if self.waiter.is_some() {
            waiters.list.remove_node(*self.waiter.as_mut().unwrap());
        }
    }
}

#[cfg(test)]
mod schedule_io_test {
    use std::io;
    use std::sync::atomic::Ordering::{Acquire, Release};
    use crate::util::slab::Entry;
    use crate::net::{Ready, ReadyEvent};
    use crate::schedule_io::{ScheduleIO, Tick};

    /*
    * @title  schedule_io default function ut test
    * @design Use path override
    * @precon None
    * @brief  1. Call default
    *         2. Verify the returned results
    * @expect 1. Get a ScheduleIO Instances
    * @auto  Yes
    */
    #[test]
    fn ut_schedule_io_default() {
        let mut schedule_io = ScheduleIO::default();
        let status = schedule_io.status.load(Acquire);
        assert_eq!(status, 0);
        let is_shutdown = schedule_io.waiters.get_mut().unwrap().is_shutdown;
        assert!(!is_shutdown);
    }

    /*
    * @title  schedule_io reset function ut test
    * @design Use path override
    * @precon None
    * @brief  1. Create a ScheduleIO
    *         2. Call reset
    *         3. Verify the returned results
    * @expect 1. Generation part of the ScheduleIO status bits +1
    * @auto  Yes
    */
    #[test]
    fn ut_schedule_io_reset() {
        let schedule_io = ScheduleIO::default();
        let pre_status = schedule_io.status.load(Acquire);
        assert_eq!(pre_status, 0x00);
        schedule_io.reset();
        let after_status = schedule_io.status.load(Acquire);
        assert_eq!(after_status, 0x1000000);
    }

    /*
    * @title  schedule_io generation function ut test
    * @design Use path override
    * @precon None
    * @brief  1. Create a ScheduleIO
    *         2. Call generation
    *         3. Verify the returned results
    * @expect 1. Get the generation of ScheduleIO
    * @auto  Yes
    */
    #[test]
    fn ut_schedule_io_generation() {
        let schedule_io = ScheduleIO::default();
        schedule_io.status.store(0x7f000000, Release);
        assert_eq!(schedule_io.generation(), 0x7f);
    }

    /*
    * @title  schedule_io shutdown function ut test
    * @design Use path override
    * @precon None
    * @brief  1. Create a ScheduleIO
    *         2. Call shutdown
    *         3. Verify the returned results
    * @expect 1. ScheduleIO shutdown. The is_shutdown part of the waiters is set to true
    * @auto  Yes
    */
    #[test]
    fn ut_schedule_io_shutdown() {
        let mut schedule_io = ScheduleIO::default();
        schedule_io.shutdown();
        assert!(schedule_io.waiters.get_mut().unwrap().is_shutdown);
    }

    /*
    * @title  schedule_io clear_readiness function ut test
    * @design Use path override
    * @precon None
    * @brief  1. Create a ScheduleIO
    *         2. Call clear_readiness
    *         3. Verify the returned results
    * @expect 1. ScheduleIO readiness status clear
    * @auto  Yes
    */
    #[test]
    fn ut_schedule_io_clear_readiness() {
        let schedule_io = ScheduleIO::default();
        schedule_io.status.store(0x0000000f, Release);
        schedule_io.clear_readiness(ReadyEvent::new(0, Ready::from_usize(0x1)));
        let status = schedule_io.status.load(Acquire);
        assert_eq!(status, 0x0000000e);
    }

    /*
    * @title  schedule_io set_readiness function ut test
    * @design Use path override
    * @precon None
    * @brief  1. Create a ScheduleIO
    *         2. Call set_readiness
    *         3. Verify the returned results
    * @expect 1. Constructed scenario, the generation part of the token is invalid, return failed
    *         2. Construct scene, tick for Tick::Clear property, return failure
    *         3. In a normal scenario, the ScheduleIO readiness section is modified successfully
    * @auto  Yes
    */
    #[test]
    fn ut_schedule_io_set_readiness() {
        ut_schedule_io_set_readiness_01();
        ut_schedule_io_set_readiness_02();
        ut_schedule_io_set_readiness_03();

        fn ut_schedule_io_set_readiness_01() {
            let schedule_io = ScheduleIO::default();
            let token = 0x7f000000usize;
            let ret = schedule_io.set_readiness(Some(token), Tick::Set(1), |curr| curr);
            let err = ret.err().unwrap();
            assert_eq!(err.kind(), io::ErrorKind::Other);
            assert_eq!(
                format! {"{}", err.into_inner().unwrap()},
                "Token no longer valid."
            );
        }

        fn ut_schedule_io_set_readiness_02() {
            let schedule_io = ScheduleIO::default();
            let token = 0x00000000usize;
            let ret = schedule_io.set_readiness(Some(token), Tick::Clear(1), |curr| curr);
            let err = ret.err().unwrap();
            assert_eq!(err.kind(), io::ErrorKind::Other);
            assert_eq!(
                format! {"{}", err.into_inner().unwrap()},
                "Readiness has been covered."
            );
        }

        fn ut_schedule_io_set_readiness_03() {
            let schedule_io = ScheduleIO::default();
            let token = 0x00000000usize;
            let ret = schedule_io.set_readiness(Some(token), Tick::Set(1), |curr| curr);
            assert!(ret.is_ok());
            let status = schedule_io.status.load(Acquire);
            assert_eq!(status, 0x00010000);
        }
    }
}