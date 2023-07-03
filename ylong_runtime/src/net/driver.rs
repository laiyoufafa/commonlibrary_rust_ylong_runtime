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

use crate::cfg_ffrt;
use crate::net::{Ready, ScheduleIO, Tick};
use std::io;
use std::ops::Deref;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use crate::util::bit::{Bit, Mask};
use ylong_io::{EventTrait, Events, Interest, Poll, Source, Token};
use crate::util::slab::{Address, Ref, Slab};

const DRIVER_TICK_INIT: u8 = 0;

const EVENTS_MAX_CAPACITY: usize = 1024;

// Token structure
// | reserved | generation | address |
// |----------|------------|---------|
// |   1 bit  |   7 bits   | 24 bits |
//const RESERVED: Mask = Mask::new(1, 31);
const GENERATION: Mask = Mask::new(7, 24);
const ADDRESS: Mask = Mask::new(24, 0);

const WAKE_TOKEN: Token = Token(1 << 31);

/// IO reactor that listens to fd events and wakes corresponding tasks.
pub(crate) struct Driver {
    /// Stores every IO source that is ready
    resources: Option<Slab<ScheduleIO>>,

    /// Stores IO events that need to be handled
    events: Option<Events>,

    /// Counter used for slab struct to compact
    tick: u8,

    /// Used for epoll
    poll: Arc<Poll>,
}

pub(crate) struct Handle {
    inner: Arc<Inner>,
    #[cfg(not(feature = "ffrt"))]
    pub(crate) waker: ylong_io::Waker,
}

cfg_ffrt!(
    use std::men::MaybeUninit;
    static mut DRIVER: MaybeUninit<Mutex<Driver>> = MaybeUninit::uninit();
    static mut HANDLE: MaybeUninit<Handle> = MaybeUninit::uninit();
);

#[cfg(feature = "ffrt")]
impl Handle {
    fn new(inner: Arc<Inner>) -> Self {
        Handle {
            inner,
        }
    }

    pub(crate) fn get_ref() -> &'static Self {
        Driver::initialize();
        unsafe {
            &*HANDLE.as_ptr()
        }
    }
}

#[cfg(not(feature = "ffrt"))]
impl Handle {
    fn new(inner: Arc<Inner>, waker: ylong_io::Waker) -> Self {
        Handle {
            inner,
            waker,
        }
    }

    pub(crate) fn wake(&self) {
        self.waker.wake().expect("ylong_io wake failed");
    }
}

impl Deref for Handle {
    type Target = Arc<Inner>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// In charge of two functionalities
///
/// 1）IO registration
/// 2）Resource management
pub(crate) struct Inner {
    /// When the driver gets dropped, the resources in the driver will be transmitted to here.
    /// Then all the slabs inside will get dropped when Inner's ref count clears to zero, so
    /// there is no concurrent problem when new slabs gets inserted
    resources: Mutex<Option<Slab<ScheduleIO>>>,

    /// Used to register scheduleIO into the slab
    allocator: Slab<ScheduleIO>,

    /// Used to register fd
    registry: Arc<Poll>,
}

impl Driver {
    #[cfg(not(feature = "ffrt"))]
    pub(crate) fn initialize() -> (Arc<Handle>, Arc<Mutex<Driver>>) {
            let poll = Poll::new().unwrap();
            let waker = ylong_io::Waker::new(&poll, WAKE_TOKEN)
                .expect("ylong_io waker construction failed");
            let arc_poll = Arc::new(poll);
            let events = Events::with_capacity(EVENTS_MAX_CAPACITY);
            let slab = Slab::new();
            let allocator = slab.handle();
            let inner = Arc::new(Inner {
                resources: Mutex::new(None),
                allocator,
                registry: arc_poll.clone(),
            });

            let driver = Driver {
                resources: Some(slab),
                events: Some(events),
                tick: DRIVER_TICK_INIT,
                poll: arc_poll,
            };
            
        (Arc::new(Handle::new(inner, waker)), Arc::new(Mutex::new(driver)))
    }

    #[cfg(feature = "ffrt")]
    fn initialize() {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| unsafe {
            let poll = Poll::new().unwrap();
            let arc_poll = Arc::new(poll);
            let events = Events::with_capacity(EVENTS_MAX_CAPACITY);
            let slab = Slab::new();
            let allocator = slab.handle();
            let inner = Arc::new(Inner {
                resources: Mutex::new(None),
                allocator,
                registry: arc_poll.clone(),
            });

            let driver = Driver {
                resources: Some(slab),
                events: Some(events),
                tick: DRIVER_TICK_INIT,
                poll: arc_poll,
            };
            HANDLE = MaybeUninit::new(Handle::new(inner));
            DRIVER = MaybeUninit::new(Mutex::new(driver));
        });
    }

    /// Initializes the single instance IO driver.
    #[cfg(feature = "ffrt")]
    pub(crate) fn try_get_mut() -> Option<MutexGuard<'static, Driver>>{
        Driver::initialize();
        unsafe {
            & *DRIVER.as_ptr()
        }.try_lock().ok()
    }

    /// Runs the driver. This method will blocking wait for fd events to come in and then
    /// wakes the corresponding tasks through the events.
    ///
    /// In linux environment, the driver uses epoll.
    pub(crate) fn drive(&mut self, time_out: Option<Duration>) -> io::Result<bool> {
        // For every 255 ticks, cleans the redundant entries inside the slab
        const COMPACT_INTERVAL: u8 = 255;

        self.tick = self.tick.wrapping_add(1);

        if self.tick == COMPACT_INTERVAL {
            unsafe {
                self.resources.as_mut().unwrap().compact();
            }
        }

        let mut events = match self.events.take() {
            Some(ev) => ev,
            None => {
                let err = io::Error::new(io::ErrorKind::Other, "driver event store missing.");
                return Err(err);
            }
        };

        match self.poll.poll(&mut events, time_out) {
            Ok(_) => {}
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(err) => return Err(err),
        }


        let has_events = !events.is_empty();

        for event in events.iter() {
            let token = event.token();
            if token == WAKE_TOKEN {
                continue;
            }
            let ready = Ready::from_event(event);
            self.dispatch(token, ready);
        }

        self.events = Some(events);
        Ok(has_events)
    }

    /// IO dispatch function. Wakes the task through the token getting from the epoll events.
    fn dispatch(&mut self, token: Token, ready: Ready) {
        let addr_bit = Bit::from_usize(token.0);
        let addr = addr_bit.get_by_mask(ADDRESS);

        let io = match self
            .resources
            .as_mut()
            .unwrap()
            .get(Address::from_usize(addr))
        {
            Some(io) => io,
            None => return,
        };

        if io
            .set_readiness(Some(token.0), Tick::Set(self.tick), |curr| curr | ready)
            .is_err()
        {
            return;
        }

        // Wake the io task
        io.wake(ready)
    }
}

impl Inner {
    /// Registers the fd of the `Source` object
    pub(crate) fn register_source(
        &self,
        io: &mut impl Source,
        interest: Interest,
    ) -> io::Result<Ref<ScheduleIO>> {
        // Allocates space for the slab. If reaches maximum capacity, error will be returned
        let (addr, schedule_io) = unsafe {
            self.allocator.allocate().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::Other,
                    "driver at max registered I/O resources.",
                )
            })?
        };

        // Initializes the token for finding the task in the slab.
        let mut base = Bit::from_usize(0);
        base.set_by_mask(GENERATION, schedule_io.generation());
        base.set_by_mask(ADDRESS, addr.as_usize());
        let token = base.as_usize();

        self.registry
            .register(io, Token::from_usize(token), interest)?;
        Ok(schedule_io)
    }

    /// Deregisters the fd of the `Source` object.
    pub(crate) fn deregister_source(&self, io: &mut impl Source) -> io::Result<()> {
        self.registry.deregister(io)
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        let resources = self.resources.lock().unwrap().take();

        if let Some(mut slab) = resources {
            slab.for_each(|io| {
                io.shutdown();
            });
        }
    }
}