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

use crate::time::timer_handle::TimerHandle;
use std::collections::VecDeque;
use std::fmt::Error;
use std::mem::MaybeUninit;

// In a slots, the number of slot.
const SLOTS_NUM: usize = 64;

// In a levels, the number of level.
const LEVELS_NUM: usize = 6;

// Maximum sleep duration.
pub(crate) const MAX_DURATION: u64 = (1 << (6 * LEVELS_NUM)) - 1;

pub(crate) struct Wheel {
    // Since the wheel started,
    // the number of milliseconds elapsed.
    elapsed: u64,

    // Since the time wheel started,
    // to the end of the last future run,
    // the number of milliseconds elapsed.
    last_elapsed: u64,

    // The time wheel levels are similar to a multi-layered dial.
    //
    // levels:
    //
    // 1  ms slots == 64 ms range
    // 64 ms slots ~= 4 sec range
    // 4 sec slots ~= 4 min range
    // 4 min slots ~= 4 hr range
    // 4 hr slots ~= 12 day range
    // 12 day slots ~= 2 yr range
    levels: Vec<Level>,

    // These corresponding timers have expired,
    // and are ready to be triggered.
    trigger: VecDeque<TimerHandle>,
}

impl Wheel {
    // Creates a new timing wheel.
    pub(crate) fn new() -> Self {
        let levels = (0..LEVELS_NUM).map(Level::new).collect();

        Self {
            elapsed: 0,
            last_elapsed: 0,
            levels,
            trigger: Default::default(),
        }
    }

    // Return the elapsed.
    pub(crate) fn elapsed(&self) -> u64 {
        self.elapsed
    }

    // Set the elapsed.
    pub(crate) fn set_elapsed(&mut self, elapsed: u64) {
        self.elapsed = elapsed;
    }

    // Return the last_elapsed.
    pub(crate) fn last_elapsed(&self) -> u64 {
        self.last_elapsed
    }

    // Set the last_elapsed.
    pub(crate) fn set_last_elapsed(&mut self, last_elapsed: u64) {
        self.last_elapsed = last_elapsed;
    }

    // Compare the timing wheel elapsed with the expiration,
    // from which to decide which level to insert.
    pub(crate) fn find_level(&self, expiration: u64) -> usize {
        // 0011 1111
        const SLOT_MASK: u64 = (1 << 6) - 1;

        // Use the time difference value to find at which level.
        let mut masked = (expiration - self.elapsed()) | SLOT_MASK;

        // 1111 1111 1111 1111 1111 1111 1111 1111 1111
        if masked >= MAX_DURATION {
            masked = MAX_DURATION - 1;
        }

        let leading_zeros = masked.leading_zeros() as usize;
        // Calculate how many valid bits there are.
        let significant = 63 - leading_zeros;

        // One level per 6 bit,
        // one slots has 2^6 slots.
        significant / 6
    }

    // Insert the corresponding TimerHandle into the specified position in the timing wheel.
    pub(crate) fn insert(&mut self, timer_handle: TimerHandle) -> Result<u64, Error> {
        let expiration = unsafe { timer_handle.inner().as_ref().expiration() };

        if expiration <= self.elapsed() {
            // This means that the timeout period has passed,
            // and the time should be triggered immediately.
            return Err(Error::default());
        }

        let level = self.find_level(expiration);
        // Unsafe access to timer_handle is only unsafe when Sleep Drop,
        // `Sleep` here does not go into `Ready`.
        unsafe { timer_handle.inner().as_mut().set_level(level) };

        self.levels[level].insert(timer_handle, self.elapsed);

        Ok(expiration)
    }

    pub(crate) fn cancel(&mut self, timer_handle: &TimerHandle) {
        // Unsafe access to timer_handle is only unsafe when Sleep Drop,
        // `Sleep` here does not go into `Ready`.
        let level = unsafe { timer_handle.inner().as_ref().level() };
        self.levels[level].cancel(timer_handle);
        for (index, handle) in self.trigger.iter().enumerate() {
            if handle == timer_handle {
                self.trigger.remove(index).unwrap();
                break;
            }
        }
    }

    // Return where the next expiration is located, and its deadline.
    pub(crate) fn next_expiration(&self) -> Option<(usize, usize, u64)> {
        for level in 0..LEVELS_NUM {
            if let Some(expiration) =
                self.levels[level].next_expiration(self.elapsed() - self.last_elapsed())
            {
                return Some(expiration);
            }
        }

        None
    }

    // Retrieve the corresponding expired TimerHandle.
    pub(crate) fn process_expiration(&mut self, expiration: &(usize, usize, u64)) {
        let mut handles = self.levels[expiration.0].take_slot(expiration.1);
        while let Some(item) = handles.pop_back() {
            self.trigger.push_front(item);
        }
    }

    // Determine which timers have timed out at the current time.
    pub(crate) fn poll(&mut self, now: u64) -> Option<TimerHandle> {
        loop {
            if let Some(handle) = self.trigger.pop_back() {
                return Some(handle);
            }

            let expiration = self.next_expiration().and_then(|expiration| {
                if expiration.2 > now - self.last_elapsed {
                    None
                } else {
                    Some(expiration)
                }
            });

            match expiration {
                Some(ref expiration) if expiration.2 > now - self.last_elapsed() => return None,
                Some(ref expiration) => {
                    self.process_expiration(expiration);
                    self.set_elapsed(now);
                }
                None => {
                    self.set_elapsed(now);
                    break;
                }
            }
        }

        self.trigger.pop_back()
    }
}

// Level in the wheel.
// All level contains 64 slots.
pub struct Level {
    // current level
    level: usize,

    // Determine which slot contains entries based on occupied bit.
    occupied: u64,

    // slots in a level.
    slots: [VecDeque<TimerHandle>; SLOTS_NUM],
}

impl Level {
    // Specify the level and create a Level structure.
    pub(crate) fn new(level: usize) -> Self {
        let mut slots: [MaybeUninit<VecDeque<TimerHandle>>; SLOTS_NUM] =
            unsafe { MaybeUninit::uninit().assume_init() };

        for slot in slots.iter_mut() {
            *slot = MaybeUninit::new(Default::default());
        }

        unsafe {
            let slots = std::mem::transmute::<_, [VecDeque<TimerHandle>; SLOTS_NUM]>(slots);
            Self {
                level,
                occupied: 0,
                slots,
            }
        }
    }

    // Based on the elapsed which the current time wheel is running,
    // and the expected expiration time of the timer_handle,
    // find the corresponding slot and insert it.
    pub(crate) fn insert(&mut self, timer_handle: TimerHandle, elapsed: u64) {
        let duration = unsafe { timer_handle.inner().as_ref().expiration() } - elapsed;
        // Unsafe access to timer_handle is only unsafe when Sleep Drop,
        // `Sleep` here does not go into `Ready`.
        unsafe { timer_handle.inner().as_mut().set_duration(duration) };

        let slot = ((duration >> (self.level * LEVELS_NUM)) % SLOTS_NUM as u64) as usize;

        self.slots[slot].push_front(timer_handle);

        self.occupied |= 1 << slot;
    }

    pub(crate) fn cancel(&mut self, timer_handle: &TimerHandle) {
        // Unsafe access to timer_handle is only unsafe when Sleep Drop,
        // `Sleep` here does not go into `Ready`.
        let duration = unsafe { timer_handle.inner().as_ref().duration() };

        let slot = ((duration >> (self.level * LEVELS_NUM)) % SLOTS_NUM as u64) as usize;

        for (index, handle) in self.slots[slot].iter().enumerate() {
            if handle == timer_handle {
                self.slots[slot].remove(index).unwrap();
                break;
            }
        }

        if self.slots[slot].is_empty() {
            // Unset the bit
            self.occupied ^= 1 << slot;
        }
    }

    // Return where the next expiration is located, and its deadline.
    pub(crate) fn next_expiration(&self, now: u64) -> Option<(usize, usize, u64)> {
        let slot = self.next_occupied_slot(now)?;

        let level_range = level_range(self.level);
        let slot_range = slot_range(self.level);

        // Find the start time at this level for the current point in time.
        let level_start = now & !(level_range - 1);
        // Add the time of the last slot at this level to represent a time period.
        let deadline = level_start + slot as u64 * slot_range;

        Some((self.level, slot, deadline))
    }

    // Find the next slot that needs to be executed.
    pub(crate) fn next_occupied_slot(&self, now: u64) -> Option<usize> {
        if self.occupied == 0 {
            return None;
        }

        let now_slot = now / slot_range(self.level);
        let occupied = self.occupied.rotate_right(now_slot as u32);
        let zeros = occupied.trailing_zeros();
        let slot = (zeros as u64 + now_slot) % SLOTS_NUM as u64;

        Some(slot as usize)
    }

    // Fetch all timers in a slot of the corresponding level.
    pub(crate) fn take_slot(&mut self, slot: usize) -> VecDeque<TimerHandle> {
        self.occupied &= !(1 << slot);
        std::mem::take(&mut self.slots[slot])
    }
}

// All the slots before this level add up to approximately.
fn slot_range(level: usize) -> u64 {
    SLOTS_NUM.pow(level as u32) as u64
}

// All the slots before this level(including this level) add up to approximately.
fn level_range(level: usize) -> u64 {
    SLOTS_NUM as u64 * slot_range(level)
}

#[cfg(test)]
mod test {
    use crate::macros::cfg_io;
    use crate::time::wheel::{Wheel, LEVELS_NUM};
    cfg_io!(
        use crate::time::{sleep, timeout, Driver};
        use crate::net::UdpSocket;
        use crate::JoinHandle;
        use std::net::SocketAddr;
        use std::time::Duration;
    );

    /// Wheel::new ut test case.
    ///
    /// # Title
    /// ut_wheel_new_test
    ///
    /// # Brief
    /// 1. Use Wheel::new to create a Wheel Struct.
    /// 2. Verify the data in the Wheel Struct.
    #[test]
    fn ut_wheel_new_test() {
        let wheel = Wheel::new();
        assert_eq!(wheel.elapsed, 0);
        assert_eq!(wheel.last_elapsed, 0);
        assert_eq!(wheel.levels.len(), LEVELS_NUM);
    }

    /// Sleep Drop.
    ///
    /// # Title
    /// ut_sleep_drop
    ///
    /// # Brief
    /// 1. Use timeout to create a Timeout Struct.
    /// 2. Enable the Sleep Struct corresponding to the Timeout Struct to enter the Pending state.
    /// 3. Verify the change of the internal TimerHandle during Sleep Struct drop.
    #[test]
    #[cfg(feature = "net")]
    fn ut_sleep_drop() {
        async fn udp_sender(sender_addr: SocketAddr, receiver_addr: SocketAddr) {
            let sender = UdpSocket::bind(sender_addr).await.unwrap();
            let buf = [2; 10];
            sleep(Duration::from_secs(1)).await;
            sender.send_to(buf.as_slice(), receiver_addr).await.unwrap();
        }

        async fn udp_receiver(receiver_addr: SocketAddr) {
            let receiver = UdpSocket::bind(receiver_addr).await.unwrap();
            let mut buf = [0; 10];
            assert!(
                timeout(Duration::from_secs(2), receiver.recv_from(&mut buf[..]))
                    .await
                    .is_ok()
            );
        }

        let mut tasks: Vec<JoinHandle<()>> = Vec::new();
        let udp_sender_addr = "127.0.0.1:9093".parse().unwrap();
        let udp_receiver_addr = "127.0.0.1:9094".parse().unwrap();
        tasks.push(crate::spawn(udp_sender(udp_sender_addr, udp_receiver_addr)));
        tasks.push(crate::spawn(udp_receiver(udp_receiver_addr)));
        for t in tasks {
            let _ = crate::block_on(t);
        }
        let lock = Driver::get_ref().wheel.lock().unwrap();
        for slot in lock.levels[1].slots.iter() {
            assert!(slot.is_empty());
        }
    }
}
