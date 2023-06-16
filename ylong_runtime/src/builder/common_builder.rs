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

use crate::builder::CallbackHook;
#[cfg(not(feature = "ffrt"))]
use crate::builder::ScheduleAlgo;
use crate::executor::blocking_pool::BLOCKING_MAX_THEAD_NUM;
use std::time::Duration;

const BLOCKING_PERMANENT_THREAD_NUM: u8 = 0;

pub(crate) struct CommonBuilder {
    /// Name prefix of worker threads
    pub(crate) worker_name: Option<String>,

    /// Core affinity, default set to true
    #[cfg(not(feature = "ffrt"))]
    pub(crate) is_affinity: bool,

    /// How long the blocking thread will be kept alive after becoming idle
    pub(crate) keep_alive_time: Option<Duration>,

    /// Maximum thread number for blocking thread pool
    pub(crate) max_blocking_pool_size: Option<u8>,

    /// Schedule policy, default set to FIFO
    #[cfg(not(feature = "ffrt"))]
    pub(crate) schedule_algo: ScheduleAlgo,

    /// Maximum number of permanent threads
    pub(crate) blocking_permanent_thread_num: u8,

    /// Worker thread stack size
    pub(crate) stack_size: Option<usize>,

    /// A callback function to be called after starting a worker thread
    pub(crate) after_start: Option<CallbackHook>,

    /// A callback function to be called before stopping a worker thread
    pub(crate) before_stop: Option<CallbackHook>,
}

impl CommonBuilder {
    pub(crate) fn new() -> Self {
        CommonBuilder {
            worker_name: None,
            #[cfg(not(feature = "ffrt"))]
            is_affinity: true,
            blocking_permanent_thread_num: BLOCKING_PERMANENT_THREAD_NUM,
            max_blocking_pool_size: Some(BLOCKING_MAX_THEAD_NUM),
            #[cfg(not(feature = "ffrt"))]
            schedule_algo: ScheduleAlgo::FifoBound,
            stack_size: None,
            after_start: None,
            before_stop: None,
            keep_alive_time: None,
        }
    }
}

macro_rules! impl_common {
    ($self:ident) => {
        #[cfg(not(feature = "ffrt"))]
        use crate::builder::ScheduleAlgo;
        #[cfg(not(feature = "ffrt"))]
        use std::sync::Arc;
        use std::time::Duration;

        impl $self {
            /// Sets the name prefix for all worker threads.
            pub fn worker_name(mut self, name: String) -> Self {
                self.common.worker_name = Some(name);
                self
            }

            /// Sets the core affinity of the worker threads
            #[cfg(not(feature = "ffrt"))]
            pub fn is_affinity(mut self, is_affinity: bool) -> Self {
                self.common.is_affinity = is_affinity;
                self
            }

            /// Sets the maximum number of permanent threads in blocking thread pool
            pub fn blocking_permanent_thread_num(
                mut self,
                blocking_permanent_thread_num: u8,
            ) -> Self {
                if blocking_permanent_thread_num > self.common.max_blocking_pool_size.unwrap() {
                    self.common.blocking_permanent_thread_num =
                        self.common.max_blocking_pool_size.unwrap();
                } else {
                    self.common.blocking_permanent_thread_num = blocking_permanent_thread_num;
                }
                self
            }

            /// Sets the number of threads that the runtime could spawn additionally besides the core
            /// thread pool.
            ///
            /// The boundary is 1-64.
            pub fn max_blocking_pool_size(mut self, max_blocking_pool_size: u8) -> Self {
                if max_blocking_pool_size < 1 {
                    self.common.max_blocking_pool_size = Some(1);
                } else if max_blocking_pool_size > 64 {
                    self.common.max_blocking_pool_size = Some(64);
                } else {
                    self.common.max_blocking_pool_size = Some(max_blocking_pool_size);
                }
                self
            }

            /// Sets the schedule policy.
            #[cfg(not(feature = "ffrt"))]
            pub fn schedule_algo(mut self, schedule_algo: ScheduleAlgo) -> Self {
                self.common.schedule_algo = schedule_algo;
                self
            }

            /// Sets the stack size for every worker thread that gets spawned by the runtime.
            /// The minimum stack size is 1.
            pub fn worker_stack_size(mut self, stack_size: usize) -> Self {
                if stack_size < 1 {
                    self.common.stack_size = Some(1);
                } else {
                    self.common.stack_size = Some(stack_size);
                }
                self
            }

            /// Sets how long will the thread be kept alive inside the blocking pool after
            /// it becomes idle.
            pub fn keep_alive_time(mut self, keep_alive_time: Duration) -> Self {
                self.common.keep_alive_time = Some(keep_alive_time);
                self
            }

            /// Sets the callback function to be called when a worker thread starts.
            #[cfg(not(feature = "ffrt"))]
            pub fn after_start<F>(mut self, f: F) -> Self
            where
                F: Fn() + Send + Sync + 'static,
            {
                self.common.after_start = Some(Arc::new(f));
                self
            }

            /// Sets the callback function to be called when a worker thread stops.
            #[cfg(not(feature = "ffrt"))]
            pub fn before_stop<F>(mut self, f: F) -> Self
            where
                F: Fn() + Send + Sync + 'static,
            {
                self.common.before_stop = Some(Arc::new(f));
                self
            }
        }
    };
}

pub(crate) use impl_common;
