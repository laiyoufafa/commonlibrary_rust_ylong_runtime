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

use crate::cfg_not_ffrt;
use crate::error::ErrorKind;
/// Task state, include SCHEDULED  RUNNING  COMPLETED CLOSED and so on and transform method
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::{AcqRel, Acquire, Relaxed};

/// Task is currently running
const RUNNING: usize = 0b0001;
/// Task is in the schedule list
const SCHEDULING: usize = 0b0100;
/// Task has finished
const FINISHED: usize = 0b1000;
/// Task gets canceled
const CANCELED: usize = 0b1_0000;
/// Task needs to send the finished result back to the join handle
const CARE_JOIN_HANDLE: usize = 0b10_0000;
/// Task currently holds a waker to the join handle
const JOIN_WAKER: usize = 0b100_0000;

const RC_MASK: usize = !0b11_1111_1111;

const RC_SHIFT: usize = RC_MASK.count_zeros() as usize;

/// Reference count
const REF_ONE: usize = 1 << RC_SHIFT;

/// Initial state contains two ref count, one is held by join_handle, another one is held by
/// task itself.
const INIT: usize = CARE_JOIN_HANDLE | SCHEDULING | (REF_ONE * 2);

#[inline]
pub(crate) fn ref_count(state: usize) -> usize {
    (state & RC_MASK) >> RC_SHIFT
}

#[inline]
pub(crate) fn is_last_ref_count(prev: usize) -> bool {
    ref_count(prev) == 1
}

pub(crate) fn is_canceled(cur: usize) -> bool {
    cur & CANCELED == CANCELED
}

pub(crate) fn is_care_join_handle(cur: usize) -> bool {
    cur & CARE_JOIN_HANDLE == CARE_JOIN_HANDLE
}

pub(crate) fn is_finished(cur: usize) -> bool {
    cur & FINISHED == FINISHED
}

pub(crate) fn is_set_waker(cur: usize) -> bool {
    cur & JOIN_WAKER == JOIN_WAKER
}

pub(crate) fn is_scheduling(cur: usize) -> bool {
    cur & SCHEDULING == SCHEDULING
}

pub(crate) fn is_running(cur: usize) -> bool {
    cur & RUNNING == RUNNING
}

// A task need to satisfy these state requirements in order to get pushed back to
// the schedule list.
pub(crate) fn need_enqueue(cur: usize) -> bool {
    (cur & SCHEDULING != SCHEDULING) && (cur & RUNNING != RUNNING) && (cur & FINISHED != FINISHED)
}

pub(crate) enum StateAction {
    Success,
    Canceled(usize),
    Failed(usize),
    Enqueue,
}

pub(crate) struct TaskState(AtomicUsize);
impl TaskState {
    pub(crate) fn new() -> Self {
        TaskState(AtomicUsize::new(INIT))
    }

    pub(crate) fn dec_ref(&self) -> usize {
        self.0.fetch_sub(REF_ONE, AcqRel)
    }

    pub(crate) fn inc_ref(&self) {
        self.0.fetch_add(REF_ONE, AcqRel);
    }

    #[inline]
    pub(crate) fn get_current_state(&self) -> usize {
        self.0.load(Acquire)
    }

    /// Turns the task state into running. Contains CAS operations.
    ///
    /// Fails when the task is already running, scheduling or is already finished.
    pub(crate) fn turning_to_running(&self) -> StateAction {
        let mut cur = self.get_current_state();
        loop {
            let mut action = StateAction::Success;

            if is_running(cur) || is_finished(cur) || !is_scheduling(cur) {
                return StateAction::Failed(cur);
            }

            let mut next = cur;
            next &= !SCHEDULING;
            next |= RUNNING;
            if is_canceled(next) {
                action = StateAction::Canceled(next);
            }

            let res = self.0.compare_exchange(cur, next, AcqRel, Acquire);
            match res {
                Ok(_) => return action,
                Err(actual) => cur = actual,
            }
        }
    }

    /// Turns the task state into finished. Contains CAS operations.
    ///
    /// Fails when the task is already finished or is not running.
    pub(crate) fn turning_to_finish(&self) -> Result<usize, ErrorKind> {
        let mut cur = self.get_current_state();

        loop {
            if is_finished(cur) {
                return Err(ErrorKind::TaskShutdown);
            }

            if !is_running(cur) {
                return Err(ErrorKind::TaskStateInvalid);
            }
            let mut next = cur;
            next &= !RUNNING;
            next |= FINISHED;

            let res = self.0.compare_exchange(cur, next, AcqRel, Acquire);
            match res {
                Ok(_) => return Ok(next),
                Err(actual) => cur = actual,
            }
        }
    }

    /// Turns the task state into idle. Contains CAS operations.
    ///
    /// Fails when the task is canceled or running.
    pub(crate) fn turning_to_idle(&self) -> StateAction {
        let mut cur = self.get_current_state();

        loop {
            let mut action = StateAction::Success;

            if !is_running(cur) {
                return StateAction::Failed(cur);
            }

            if is_canceled(cur) {
                return StateAction::Canceled(cur);
            }

            let mut next = cur;
            next &= !RUNNING;

            if is_scheduling(next) {
                action = StateAction::Enqueue;
            }

            let res = self.0.compare_exchange(cur, next, AcqRel, Acquire);
            match res {
                Ok(_) => return action,
                Err(actual) => cur = actual,
            }
        }
    }

    /// Turns the task state into scheduling. Returns the old state value.
    pub(crate) fn turn_to_scheduling(&self) -> usize {
        self.0.fetch_or(SCHEDULING, AcqRel)
    }

    /// Turns the task state into unset_waker. Contains CAS operations.
    ///
    /// Fails when the task is already finished.
    pub(crate) fn turn_to_un_set_waker(&self) -> Result<usize, usize> {
        let mut cur = self.get_current_state();

        loop {
            if !is_care_join_handle(cur) || !is_set_waker(cur) {
                return Err(cur);
            }

            if is_finished(cur) {
                return Err(cur);
            }

            let mut next = cur;
            next &= !JOIN_WAKER;

            let res = self.0.compare_exchange(cur, next, AcqRel, Acquire);
            match res {
                Ok(_) => return Ok(next),
                Err(actual) => cur = actual,
            }
        }
    }

    /// Turns off the Join_Waker bit of the task state. Contains CAS operations.
    ///
    /// Fails when the task is already finished.
    pub(crate) fn turn_to_set_waker(&self) -> Result<usize, usize> {
        let mut cur = self.get_current_state();

        loop {
            if !is_care_join_handle(cur) || is_set_waker(cur) {
                return Err(cur);
            }
            if is_finished(cur) {
                return Err(cur);
            }

            let mut next = cur;
            next |= JOIN_WAKER;

            let res = self.0.compare_exchange(cur, next, AcqRel, Acquire);
            match res {
                Ok(_) => return Ok(next),
                Err(actual) => cur = actual,
            }
        }
    }

    pub(crate) fn turn_to_canceled_and_scheduled(&self) -> bool {
        let mut cur = self.get_current_state();

        loop {
            if is_canceled(cur) || is_finished(cur) {
                return false;
            }

            let mut next = cur;
            let need_schedule = if is_running(cur) {
                next |= SCHEDULING;
                next |= CANCELED;
                false
            } else {
                next |= CANCELED;
                if !is_scheduling(next) {
                    next |= SCHEDULING;
                    true
                } else {
                    false
                }
            };

            let res = self.0.compare_exchange(cur, next, AcqRel, Acquire);
            match res {
                Ok(_) => return need_schedule,
                Err(actual) => cur = actual,
            }
        }
    }

    /// Turns off the CARE_JOIN_HANDLE bit of the task state. Contains CAS operations.
    ///
    /// Fails when the task is already finished.
    pub(crate) fn turn_to_un_join_handle(&self) -> Result<usize, ()> {
        let mut cur = self.get_current_state();

        loop {
            if is_finished(cur) {
                return Err(());
            }

            let mut next = cur;
            next &= !CARE_JOIN_HANDLE;

            let res = self.0.compare_exchange(cur, next, AcqRel, Acquire);
            match res {
                Ok(_) => return Ok(next),
                Err(actual) => cur = actual,
            }
        }
    }

    /// Attempts to turn off the CARE_JOIN_HANDLE bit of the task state.
    ///
    /// Returns true if successfully changed. Otherwise, returns false.
    pub(crate) fn try_turning_to_un_join_handle(&self) -> bool {
        let old = INIT;
        let new = (INIT - REF_ONE) & !CARE_JOIN_HANDLE;
        self.0.compare_exchange(old, new, Relaxed, Relaxed) == Ok(old)
    }

    cfg_not_ffrt! {
        /// Turns the task state into canceled. Returns the old state value.
        pub(crate) fn set_cancel(&self) -> usize {
            self.0.fetch_or(CANCELED, AcqRel)
        }

        /// Turns the task state into running. Returns the old state value.
        pub(crate) fn set_running(&self) {
            self.0.fetch_or(RUNNING, AcqRel);
        }
    }
}

#[cfg(all(test))]
mod test {
    use crate::task::state::{
        StateAction, TaskState, CANCELED, CARE_JOIN_HANDLE, FINISHED, INIT, JOIN_WAKER, REF_ONE,
        RUNNING, SCHEDULING,
    };
    use std::sync::atomic::Ordering::{Acquire, Release};

    /*
     * @title  TaskState::new() ut test
     * @design No entry, no exception branch
     * @precon After calling the TaskState::new() function, get its creation object
     * @brief  Describe test case execution
     *         1、Verify that the status of the initialized completed task is INIT
     * @expect The modified task status attribute value should be INIT
     * @auto   true
     */
    #[test]
    fn ut_task_state_new() {
        let task_state = TaskState::new();
        assert_eq!(task_state.0.load(Acquire), INIT);
    }

    /*
     * @title  TaskState::dec_ref() ut test
     * @design No entry, no exception branch
     * @precon After calling the TaskState::new() function, get its creation object
     * @brief  Describe test case execution
     *         1、Verify that the status of the initialized completed task is INIT.wrapping_sub(REF_ONE)
     * @expect The modified task status attribute value should be INIT.wrapping_sub(REF_ONE)
     * @auto   true
     */
    #[test]
    fn ut_task_state_dec_ref() {
        let task_state = TaskState::new();
        task_state.dec_ref();
        assert_eq!(task_state.0.load(Acquire), INIT.wrapping_sub(REF_ONE))
    }

    /*
     * @title  TaskState::inc_ref() ut test
     * @design No entry, no exception branch
     * @precon After calling the TaskState::new() function, get its creation object
     * @brief  Describe test case execution
     *         1、Verify that the status of the initialized completed task is INIT.wrapping_add(REF_ONE)
     * @expect The modified task status attribute value should be INIT.wrapping_add(REF_ONE)
     * @auto   true
     */
    #[test]
    fn ut_task_state_inc_ref() {
        let task_state = TaskState::new();
        task_state.inc_ref();
        assert_eq!(task_state.0.load(Acquire), INIT.wrapping_add(REF_ONE));
    }

    /*
     * @title  TaskState::get_current_state() ut test
     * @design No entry, no exception branch
     * @precon After calling the TaskState::new() function, get its creation object
     * @brief  Describe test case execution
     *         1、Verify that the status of the initialized completed task is INIT
     * @expect The modified task status attribute value should be current task status value
     * @auto   true
     */
    #[test]
    fn ut_task_state_get_current_state() {
        let task_state = TaskState::new();
        assert_eq!(task_state.get_current_state(), INIT);
    }

    /*
     * @title  TaskState::turning_to_running() ut test
     * @design No entry, exception branch exists
     * @precon After calling the TaskState::new() function, get its creation object
     * @brief  Describe test case execution
     *         1、(cur & RUNNING == RUNNING) || (cur & FINISHED == FINISHED) == true,
     *            Represents the current state is already running state or has ended the state, the state does not information is not correct, directly return failure
     *         2、(cur & RUNNING == RUNNING) || (cur & FINISHED == FINISHED) == false,
     *            cur & SCHEDULING != SCHEDULING == true,
     *            means the current state is not schedule state, and the status information is not correct, so it returns an error directly
     * @expect Performing a state transition in an error state will return an error, but the correct situation should cause the state to be modified to RUNNING
     * @auto   true
     */
    #[test]
    fn ut_task_state_turning_to_running() {
        let task_state = TaskState::new();
        let mut test_task_state = INIT;
        test_task_state &= !SCHEDULING;
        test_task_state |= RUNNING;

        match task_state.turning_to_running() {
            StateAction::Success => {}
            _ => panic!(),
        }

        match task_state.turning_to_running() {
            StateAction::Failed(x) => assert_eq!(x, test_task_state),
            _ => panic!(),
        }
    }

    /*
     * @title  TaskState::turning_to_finish() ut test
     * @design No entry, exception branch exists
     * @precon After calling the TaskState::new() function, get its creation object
     * @brief  Describe test case execution
     *         1、cur & FINISHED == FINISHED == true, Represents the current state is already the end state, the state does not information is not correct, directly return failure
     *         2、cur & FINISHED == FINISHED == false, cur & RUNNING != RUNNING == true,
     *            means the current state is not running, and the status information is not correct, so the error is returned directly
     * @expect Performing a state transition in an error state will return an error, but should cause the state to be modified to FINISHED in the correct case
     * @auto   true
     */
    #[test]
    fn ut_task_state_turning_to_finish() {
        let task_state = TaskState::new();
        task_state.turning_to_running();
        let mut test_task_state = INIT;
        test_task_state &= !RUNNING;
        test_task_state |= FINISHED;
        test_task_state &= !SCHEDULING;
        let ret = task_state.turning_to_finish().unwrap();
        assert_eq!(ret, test_task_state);
        assert!(task_state.turning_to_finish().is_err());
    }

    /*
     * @title  TaskState::turning_to_idle() ut test
     * @design No entry, exception branch exists
     * @precon After calling the TaskState::new() function, get its creation object
     * @brief  Describe test case execution
     *         1、cur & FINISHED == FINISHED == true, Represents the current state is already the end state, the state does not information is not correct, directly return failure
     *         2、cur & CANCELLED == CANCELLED == false, cur & RUNNING != RUNNING == true,
     *            means that the current state is not running and the status information is not correct, directly panic
     *         3、cur & CANCELLED == CANCELLED == false,
     *            cur & RUNNING != RUNNING == false, is_scheduling(next) == false
     * @expect Performing a state transition in the wrong state will return an error, but the correct situation should cause the state to be modified to SCHEDULING
     * @auto   true
     */

    /// ut test for turning_to_idle
    ///
    /// # Brief
    /// 1. Create a TaskState, set it to Canceled & Running
    /// 2. Call turning_to_idle, check if return value equals to StateAction::canceled
    /// 3. Create a TaskState, set it to init
    /// 4. Call turning_to_idle, check if return value equals to StateAction::Failed
    /// 5. Create a TaskState, set it to Running and not scheduling
    /// 6. Call turning_to_idle, check if return value equals to StateAction::Success
    /// 7. Create a TaskState, set it to Running and scheduling
    /// 8
    #[test]
    fn ut_task_state_turning_to_idle() {
        let task_state = TaskState::new();
        let mut next_state = task_state.0.load(Acquire);
        next_state |= CANCELED;
        next_state |= RUNNING;
        task_state.0.store(next_state, Release);
        match task_state.turning_to_idle() {
            StateAction::Canceled(cur) => assert_eq!(cur, next_state),
            _ => panic!(),
        }

        let task_state = TaskState::new();
        match task_state.turning_to_idle() {
            StateAction::Failed(cur) => assert_eq!(cur, INIT),
            _ => panic!(),
        }

        let task_state = TaskState::new();
        let mut next_state = task_state.0.load(Acquire);
        next_state |= RUNNING;
        next_state &= !SCHEDULING;
        task_state.0.store(next_state, Release);
        let mut test_state = next_state;
        test_state &= !RUNNING;
        match task_state.turning_to_idle() {
            StateAction::Success => assert_eq!(task_state.0.load(Acquire), test_state),
            _ => panic!(),
        }

        let task_state = TaskState::new();
        let mut next_state = task_state.0.load(Acquire);
        next_state |= RUNNING;
        next_state |= SCHEDULING;
        task_state.0.store(next_state, Release);
        match task_state.turning_to_idle() {
            StateAction::Enqueue => {}
            _ => panic!(),
        }
    }

    /*
     * @title  TaskState::turn_to_scheduling() ut test
     * @design No entry, no exception branch
     * @precon After calling the TaskState::new() function, get its creation object
     * @brief  Describe test case execution
     *         1、Check if the state transition is SCHEDULING
     * @expect The correct case should make the state modified to SCHEDULING
     * @auto   true
     */
    #[test]
    fn ut_task_state_turning_to_scheduling() {
        let task_state = TaskState::new();
        let mut test_state = task_state.0.load(Acquire);
        test_state |= SCHEDULING;
        assert_eq!(task_state.turn_to_scheduling(), test_state);
    }

    /*
     * @title  TaskState::turn_to_un_set_waker() ut test
     * @design No entry, exception branch exists
     * @precon After calling the TaskState::new() function, get its creation object
     * @brief  Describe test case execution
     *         1、!is_care_join_handle(cur) || !is_set_waker(cur) == true, means that the current state is neither focused on hooks nor set waker
     *         2、!is_care_join_handle(cur) || !is_set_waker(cur) == false, cur & FINISHED == FINISHED == true,
     *            means the current status is FINISHED, directly return failure
     *         3、!is_care_join_handle(cur) || !is_set_waker(cur) == false, cur & FINISHED == FINISHED == false
     * @expect The correct case should make the status change to not set yet JOIN_WAKER
     * @auto   true
     */
    #[test]
    fn ut_task_state_turn_to_un_set_waker() {
        let task_state = TaskState::new();
        let mut next_state = task_state.0.load(Acquire);
        next_state &= !CARE_JOIN_HANDLE;
        next_state &= !JOIN_WAKER;
        task_state.0.store(next_state, Release);
        assert!(task_state.turn_to_un_set_waker().is_err());

        let task_state = TaskState::new();
        let mut next_state = task_state.0.load(Acquire);
        next_state |= CARE_JOIN_HANDLE;
        next_state |= JOIN_WAKER;
        next_state |= FINISHED;
        task_state.0.store(next_state, Release);
        assert!(task_state.turn_to_un_set_waker().is_err());

        let task_state = TaskState::new();
        let mut next_state = task_state.0.load(Acquire);
        next_state |= CARE_JOIN_HANDLE;
        next_state |= JOIN_WAKER;
        next_state &= !FINISHED;
        task_state.0.store(next_state, Release);
        assert!(task_state.turn_to_un_set_waker().is_ok());
    }

    /*
     * @title  TaskState::turn_to_set_waker() ut test
     * @design No entry, exception branch exists
     * @precon After calling the TaskState::new() function, get its creation object
     * @brief  Describe test case execution
     *         1、!is_care_join_handle(cur) || is_set_waker(cur) == true, means that the current state is neither concerned with hooks, has set waker
     *         2、!is_care_join_handle(cur) || is_set_waker(cur) == false, cur & FINISHED == FINISHED == true, means the current status is FINISHED, directly return failure
     *         3、!is_care_join_handle(cur) || is_set_waker(cur) == false, cur & FINISHED == FINISHED == false
     * @expect The correct case should make the status change to JOIN_WAKER
     * @auto   true
     */
    #[test]
    fn ut_task_state_turn_to_set_waker() {
        let task_state = TaskState::new();
        let mut next_state = task_state.0.load(Acquire);
        next_state &= !CARE_JOIN_HANDLE;
        next_state |= JOIN_WAKER;
        task_state.0.store(next_state, Release);
        assert!(task_state.turn_to_set_waker().is_err());

        let task_state = TaskState::new();
        let mut next_state = task_state.0.load(Acquire);
        next_state |= CARE_JOIN_HANDLE;
        next_state &= !JOIN_WAKER;
        next_state |= FINISHED;
        task_state.0.store(next_state, Release);
        assert!(task_state.turn_to_set_waker().is_err());

        let task_state = TaskState::new();
        let mut next_state = task_state.0.load(Acquire);
        next_state |= CARE_JOIN_HANDLE;
        next_state &= !JOIN_WAKER;
        next_state &= !FINISHED;
        task_state.0.store(next_state, Release);
        assert!(task_state.turn_to_set_waker().is_ok());
    }

    /*
     * @title  TaskState::turn_to_un_join_handle() ut test
     * @design No entry, exception branch exists
     * @precon After calling the TaskState::new() function, get its creation object
     * @brief  Describe test case execution
     *         1、cur & FINISHED == FINISHED == true, means the current state is FINISHED state, directly return failure
     *         2、cur & FINISHED == FINISHED == false
     * @expect The correct situation should make the status change to not set yet CARE_JOIN_HANDLE
     * @auto   true
     */
    #[test]
    fn ut_task_state_turn_to_un_join_handle() {
        let task_state = TaskState::new();
        let mut next_state = task_state.0.load(Acquire);
        next_state |= FINISHED;
        task_state.0.store(next_state, Release);
        assert!(task_state.turn_to_un_join_handle().is_err());

        let task_state = TaskState::new();
        let mut next_state = task_state.0.load(Acquire);
        next_state &= !FINISHED;
        task_state.0.store(next_state, Release);
        assert!(task_state.turn_to_un_join_handle().is_ok());
    }

    /*
     * @title  TaskState::try_turning_to_un_join_handle() ut test
     * @design No entry, no exception branch
     * @precon After calling the TaskState::new() function, get its creation object
     * @brief  Describe test case execution
     *         1、After calling this function, check if the status is modified to CARE_JOIN_HANDLE
     * @expect The correct case should make the state modified to CARE_JOIN_HANDLE
     * @auto   true
     */
    #[test]
    fn ut_task_state_turning_to_un_join_handle() {
        let task_state = TaskState::new();
        assert!(task_state.try_turning_to_un_join_handle());
    }
}
