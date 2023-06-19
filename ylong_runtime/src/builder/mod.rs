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

//! A builder to configure the runtime, and thread pool of the runtime.
//!
//! Ylong-runtime provides two kinds of runtime.
//! `CurrentThread`: Runtime which runs on the current thread.
//! `MultiThread`: Runtime which runs on multiple threads.
//!
//! After configuring the builder, a call to `build` will return the actual runtime instance.
//! [`MultiThreadBuilder`] could also be used for configuring the global singleton runtime.
//!
//! For thread pool, the builder allows the user to set the thread number, stack size and
//! name prefix of each thread.

pub(crate) mod common_builder;
#[cfg(feature = "current_thread_runtime")]
pub(crate) mod current_thread_builder;
pub(crate) mod multi_thread_builder;

use std::fmt::Debug;
use std::io;
use std::sync::Arc;

pub(crate) use crate::builder::common_builder::CommonBuilder;
use crate::error::ScheduleError;
#[cfg(not(feature = "ffrt"))]
use crate::executor::async_pool::AsyncPoolSpawner;
use crate::executor::blocking_pool::BlockPoolSpawner;
#[cfg(feature = "current_thread_runtime")]
pub use current_thread_builder::CurrentThreadBuilder;
pub use multi_thread_builder::MultiThreadBuilder;

/// A callback function to be executed in different stages of a thread's life-cycle
pub type CallbackHook = Arc<dyn Fn() + Send + Sync + 'static>;

/// Schedule Policy.
#[derive(Debug, Clone, Copy, PartialOrd, PartialEq, Eq)]
pub enum ScheduleAlgo {
    /// Bounded local queues which adopts FIFO order.
    FifoBound,
}

/// Builder to build the runtime. Provides methods to customize the runtime, such
/// as setting thread pool size, worker thread stack size, work thread prefix and etc.
///
/// If `multi_instance_runtime` or `current_thread_runtime` feature is turned on:
/// After setting the RuntimeBuilder, a call to build will initialize the actual runtime
/// and returns its instance. If there is an invalid parameter during the build, an error
/// would be returned.
///
/// Otherwise:
/// RuntimeBuilder will not have the `build()` method, instead, this builder should be
/// passed to set the global executor.
///
/// # Examples
///
/// ```no run
/// #![cfg(feature = "multi_instance_runtime")]
///
/// use ylong_runtime::builder::RuntimeBuilder;
/// use ylong_runtime::executor::Runtime;
///
/// let runtime = RuntimeBuilder::new_multi_thread()
///     .worker_num(4)
///     .worker_stack_size(1024 * 300)
///     .build()
///     .unwrap();
///
/// ```
pub struct RuntimeBuilder;

impl RuntimeBuilder {
    /// Initializes a new RuntimeBuilder with current_thread settings.
    ///
    /// All tasks will run on the current thread, which means it does not create any other
    /// worker threads.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::builder::RuntimeBuilder;
    ///
    /// let runtime = RuntimeBuilder::new_current_thread()
    ///     .worker_stack_size(1024 * 3)
    ///     .max_blocking_pool_size(4);
    /// ```
    #[cfg(feature = "current_thread_runtime")]
    pub fn new_current_thread() -> CurrentThreadBuilder {
        CurrentThreadBuilder::new()
    }

    /// Initializes a new RuntimeBuilder with multi_thread settings.
    ///
    /// When running, worker threads will be created according to the builder configuration,
    /// and tasks will be allocated and run in the newly created thread pool.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::builder::RuntimeBuilder;
    ///
    /// let runtime = RuntimeBuilder::new_multi_thread()
    ///     .worker_num(8)
    ///     .max_blocking_pool_size(4);
    /// ```
    pub fn new_multi_thread() -> MultiThreadBuilder {
        MultiThreadBuilder::new()
    }
}

#[cfg(not(feature = "ffrt"))]
pub(crate) fn initialize_async_spawner(
    builder: &MultiThreadBuilder,
) -> io::Result<AsyncPoolSpawner> {
    let async_spawner = AsyncPoolSpawner::new(builder);
    async_spawner.create_async_thread_pool();

    // initialize reactor
    #[cfg(any(feature = "net", feature = "time"))]
    initialize_reactor()?;
    Ok(async_spawner)
}

#[cfg(feature = "ffrt")]
pub(crate) fn initialize_ffrt_spawner(_builder: &MultiThreadBuilder) -> io::Result<()> {
    // initialize reactor
    #[cfg(any(feature = "net", feature = "time"))]
    initialize_reactor()?;
    Ok(())
}

pub(crate) fn initialize_blocking_spawner(
    builder: &CommonBuilder,
) -> Result<BlockPoolSpawner, ScheduleError> {
    let blocking_spawner = BlockPoolSpawner::new(builder);
    blocking_spawner.create_permanent_threads()?;
    Ok(blocking_spawner)
}

#[cfg(any(feature = "time", feature = "net"))]
pub(crate) fn initialize_reactor() -> io::Result<()> {
    use crate::executor::netpoller::NetLooper;
    use std::sync::Once;

    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let net_poller = NetLooper::new();
        net_poller.create_net_poller_thread();
    });
    Ok(())
}

#[cfg(all(test))]
mod test {
    use crate::builder::RuntimeBuilder;
    #[cfg(not(feature = "ffrt"))]
    use crate::builder::ScheduleAlgo;

    /*
     * @title  RuntimeBuilder::new_multi_thread() UT test
     * @design The function has no input, no exception branch, direct check function, return value
     * @precon Use RuntimeBuilder::new_multi_thread(), get its creation object
     * @brief  Describe test case execution
     *         1、Checks if the object name property is None
     *         2、Checks if the object core_pool_size property is None
     *         3、Checks if the object is_steal property is true
     *         4、Checks if the object is_affinity property is true
     *         5、Checks if the object permanent_blocking_thread_num property is 4
     *         6、Checks if the object max_pool_size property is Some(50)
     *         7、Checks if the object keep_alive_time property is None
     *         8、Checks if the object schedule_algo property is ScheduleAlgo::FifoBound
     *         9、Checks if the object stack_size property is None
     *         10、Checks if the object after_start property is None
     *         11、Checks if the object before_stop property is None
     * @expect The function has no entry, no exception branch, and the initialization parameters should correspond to each other
     * @auto   true
     */
    #[test]

    fn ut_thread_pool_builder_new() {
        let thread_pool_builder = RuntimeBuilder::new_multi_thread();
        assert_eq!(thread_pool_builder.common.worker_name, None);
        assert_eq!(thread_pool_builder.core_thread_size, None);
        assert_eq!(thread_pool_builder.common.blocking_permanent_thread_num, 0);
        assert_eq!(thread_pool_builder.common.max_blocking_pool_size, Some(50));
        assert_eq!(thread_pool_builder.common.keep_alive_time, None);
        #[cfg(not(feature = "ffrt"))]
        assert_eq!(
            thread_pool_builder.common.schedule_algo,
            ScheduleAlgo::FifoBound
        );
        assert_eq!(thread_pool_builder.common.stack_size, None);
    }

    /*
     * @title  RuntimeBuilder::name() UT test
     * @design The function has no input, no exception branch, direct check function, return value
     * @precon Use RuntimeBuilder::new_multi_thread(), get its creation object
     * @brief  Describe test case execution
     *         1、Checks if the object name property is modified value
     * @expect The function entry has no invalid value, no exception branch, and the modified name property should be Some(worker_name)
     * @auto   true
     */
    #[test]
    fn ut_thread_pool_builder_name() {
        let name = String::from("worker_name");
        let thread_pool_builder = RuntimeBuilder::new_multi_thread().worker_name(name.clone());
        assert_eq!(thread_pool_builder.common.worker_name, Some(name));
    }

    /*
     * @title  RuntimeBuilder::core_pool_size() UT test
     * @design Input 1: core_pool_size
     *               Valid value range: core_pool_size >= 1 && core_pool_size <= 64
     *               Invalid value range: core_pool_size < 1 || core_pool_size > 64
     *         No abnormal branches
     * @precon Use RuntimeBuilder::new_multi_thread(), get its creation object
     * @brief  Describe test case execution
     *         1、core_pool_size set to 1, Check if the return value is Some(1)
     *         2、core_pool_size set to 64, Check if the return value is Some(64)
     *         3、core_pool_size set to 0, Check if the return value is Some(1)
     *         4、core_pool_size set to 65, Check if the return value is Some(64)
     * @expect Value of core_pool_size property after modification
     *         Some(core_pool_size) in the valid range
     *         Modified to a valid value close to the invalid value range
     * @auto   true
     */
    #[test]
    fn ut_thread_pool_builder_core_pool_size() {
        let thread_pool_builder = RuntimeBuilder::new_multi_thread().worker_num(1);
        assert_eq!(thread_pool_builder.core_thread_size, Some(1));

        let thread_pool_builder = RuntimeBuilder::new_multi_thread().worker_num(64);
        assert_eq!(thread_pool_builder.core_thread_size, Some(64));

        let thread_pool_builder = RuntimeBuilder::new_multi_thread().worker_num(0);
        assert_eq!(thread_pool_builder.core_thread_size, Some(1));

        let thread_pool_builder = RuntimeBuilder::new_multi_thread().worker_num(65);
        assert_eq!(thread_pool_builder.core_thread_size, Some(64));
    }

    /*
     * @title  RuntimeBuilder::stack_size() UT test
     * @design Input 1: stack_size
     *               Valid value range: stack_size > 0
     *               Invalid value range: stack_size = 0
     * @precon Use RuntimeBuilder::new_multi_thread(), get its creation object
     * @brief  Describe test case execution
     *         1、stack_size set to 0, Check if the return value is Some(1)
     *         2、stack_size set to 1, Check if the return value is Some(1)
     * @expect Modified stack_size property value
     *         Some(stack_size) in the range of valid values
     *         Modified to a valid value close to the invalid value range
     * @auto   true
     */
    #[test]
    fn ut_thread_pool_builder_stack_size() {
        let thread_pool_builder = RuntimeBuilder::new_multi_thread().worker_stack_size(0);
        assert_eq!(thread_pool_builder.common.stack_size.unwrap(), 1);

        let thread_pool_builder = RuntimeBuilder::new_multi_thread().worker_stack_size(1);
        assert_eq!(thread_pool_builder.common.stack_size.unwrap(), 1);
    }
}

#[cfg(all(test))]
#[cfg(feature = "current_thread_runtime")]
mod current_thread_test {
    use crate::builder::RuntimeBuilder;

    /// UT test for new_current_thread.
    ///
    /// # Title
    /// ut_thread_pool_builder_current_thread
    ///
    /// # Brief
    /// 1. Verify the result when multiple tasks are inserted to the current thread at a time.
    /// 2. Insert the task for multiple times, wait until the task is complete,
    /// verify the result, and then perform the operation again.
    /// 3. Spawn nest thread.
    #[test]
    fn ut_thread_pool_builder_current_thread() {
        let runtime = RuntimeBuilder::new_current_thread().build().unwrap();
        let mut handles = vec![];
        for index in 0..1000 {
            let handle = runtime.spawn(async move { index });
            handles.push(handle);
        }
        for (index, handle) in handles.into_iter().enumerate() {
            let result = runtime.block_on(handle).unwrap();
            assert_eq!(result, index);
        }

        let runtime = RuntimeBuilder::new_current_thread().build().unwrap();
        for index in 0..1000 {
            let handle = runtime.spawn(async move { index });
            let result = runtime.block_on(handle).unwrap();
            assert_eq!(result, index);
        }

        let runtime = RuntimeBuilder::new_current_thread().build().unwrap();
        let handle = runtime.spawn(async move {
            let runtime = RuntimeBuilder::new_current_thread().build().unwrap();
            let handle = runtime.spawn(async move { 1_usize });
            let result = runtime.block_on(handle).unwrap();
            assert_eq!(result, 1);
            result
        });
        let result = runtime.block_on(handle).unwrap();
        assert_eq!(result, 1);
    }
}

#[cfg(not(feature = "ffrt"))]
#[cfg(all(test))]
mod ylong_executor_test {
    use crate::builder::{RuntimeBuilder, ScheduleAlgo};

    /*
     * @title  ThreadPoolBuilder::is_affinity() UT test
     * @design The function has no input, no exception branch, direct check function, return value
     * @precon Use ThreadPoolBuilder::new(), get its creation object
     * @brief  Describe test case execution
     *         1、is_affinity set to true, check if it is a modified value
     *         2、is_affinity set to false, check if it is a modified value
     * @expect The function entry has no invalid values, no exception branches, and the value of the is_affinity property should be is_affinity after modification
     * @auto   true
     */
    #[test]
    fn ut_thread_pool_builder_is_affinity() {
        let thread_pool_builder = RuntimeBuilder::new_multi_thread().is_affinity(true);
        assert!(thread_pool_builder.common.is_affinity);

        let thread_pool_builder = RuntimeBuilder::new_multi_thread().is_affinity(false);
        assert!(!thread_pool_builder.common.is_affinity);
    }

    /*
     * @title  RuntimeBuilder::blocking_permanent_thread_num() UT test
     * @design Input 1: blocking_permanent_thread_num
     *               Valid value range: blocking_permanent_thread_num >= 1 && blocking_permanent_thread_num <= max_blocking_pool_size
     *               Invalid value range: blocking_permanent_thread_num < 1 || blocking_permanent_thread_num > max_blocking_pool_size
     * @precon Use RuntimeBuilder::new_multi_thread(), get its creation object
     * @brief  Describe test case execution
     *         1、permanent_blocking_thread_num set to 1, check if the return value is 1
     *         2、permanent_blocking_thread_num set to max_thread_num, check if the return value is max_blocking_pool_size
     *         3、permanent_blocking_thread_num set to 0, check if the return value is 1
     *         4、permanent_blocking_thread_num set to max_thread_num + 1, Check if the return value O is max_blocking_pool_size
     * @expect Modified permanent_blocking_thread_num property value
     *         In the valid range is permanent_blocking_thread_num
     *         Modified to a valid value close to the invalid value range
     * @auto   true
     */
    #[test]
    fn ut_thread_pool_builder_permanent_blocking_thread_num() {
        let thread_pool_builder =
            RuntimeBuilder::new_multi_thread().blocking_permanent_thread_num(1);
        assert_eq!(thread_pool_builder.common.blocking_permanent_thread_num, 1);

        let blocking_permanent_thread_num =
            thread_pool_builder.common.max_blocking_pool_size.unwrap();
        let thread_pool_builder = RuntimeBuilder::new_multi_thread()
            .blocking_permanent_thread_num(blocking_permanent_thread_num);
        assert_eq!(
            thread_pool_builder.common.blocking_permanent_thread_num,
            blocking_permanent_thread_num
        );

        let thread_pool_builder =
            RuntimeBuilder::new_multi_thread().blocking_permanent_thread_num(0);
        assert_eq!(thread_pool_builder.common.blocking_permanent_thread_num, 0);

        let permanent_blocking_thread_num =
            thread_pool_builder.common.max_blocking_pool_size.unwrap() + 1;
        let thread_pool_builder = RuntimeBuilder::new_multi_thread()
            .blocking_permanent_thread_num(permanent_blocking_thread_num);
        assert_eq!(
            thread_pool_builder.common.blocking_permanent_thread_num,
            thread_pool_builder.common.max_blocking_pool_size.unwrap()
        );
    }

    /*
     * @title  RuntimeBuilder::max_pool_size() UT test
     * @design Input 1: max_pool_size
     *               Valid value range: max_pool_size >= 1 && max_pool_size <= 64
     *               Invalid value range: max_pool_size < 1 || max_pool_size > 64
     * @precon Use RuntimeBuilder::new_multi_thread(), get its creation object
     * @brief  Describe test case execution
     *         1、max_pool_size set to 1, check if the return value is Some(1)
     *         2、max_pool_size set to 64, check if the return value is Some(64)
     *         3、max_pool_size set to 0, check if the return value is Some(1)
     *         4、max_pool_size set to 65, check if the return value is Some(64)
     * @expect Value of max_pool_size property after modification
     *         max_pool_size within the valid values
     *         Modified to a valid value close to the invalid value range
     * @auto   true
     */
    #[test]
    fn ut_thread_pool_builder_max_pool_size() {
        let thread_pool_builder = RuntimeBuilder::new_multi_thread().max_blocking_pool_size(1);
        assert_eq!(
            thread_pool_builder.common.max_blocking_pool_size.unwrap(),
            1
        );

        let thread_pool_builder = RuntimeBuilder::new_multi_thread().max_blocking_pool_size(64);
        assert_eq!(
            thread_pool_builder.common.max_blocking_pool_size.unwrap(),
            64
        );

        let thread_pool_builder = RuntimeBuilder::new_multi_thread().max_blocking_pool_size(0);
        assert_eq!(
            thread_pool_builder.common.max_blocking_pool_size.unwrap(),
            1
        );

        let thread_pool_builder = RuntimeBuilder::new_multi_thread().max_blocking_pool_size(65);
        assert_eq!(
            thread_pool_builder.common.max_blocking_pool_size.unwrap(),
            64
        );
    }

    /*
     * @title  RuntimeBuilder::keep_alive_time() UT test
     * @design The function has no invalid values in the input, no exception branch, direct check function, return value
     * @precon Use RuntimeBuilder::new_multi_thread(), get its creation object
     * @brief  Describe test case execution
     *         1、keep_alive_time set to 0, check if the return value is Some(Duration::from_secs(0))
     *         2、keep_alive_time set to 1, check if the return value is Some(Duration::from_secs(1))
     * @expect The function entry has no invalid value, no exception branch, and the value of the keep_alive_time property should be Some(keep_alive_time) after modification
     * @auto   true
     */
    #[test]
    fn ut_thread_pool_builder_keep_alive_time() {
        use std::time::Duration;

        let keep_alive_time = Duration::from_secs(0);
        let thread_pool_builder =
            RuntimeBuilder::new_multi_thread().keep_alive_time(keep_alive_time);
        assert_eq!(
            thread_pool_builder.common.keep_alive_time.unwrap(),
            keep_alive_time
        );

        let keep_alive_time = Duration::from_secs(1);
        let thread_pool_builder =
            RuntimeBuilder::new_multi_thread().keep_alive_time(keep_alive_time);
        assert_eq!(
            thread_pool_builder.common.keep_alive_time.unwrap(),
            keep_alive_time
        );
    }

    /*
     * @title  RuntimeBuilder::schedule_algo() UT test
     * @design The function has no invalid values in the input, no exception branch, direct check function, return value
     * @precon Use RuntimeBuilder::new_multi_thread(), get its creation object
     * @brief  Describe test case execution
     *         1、schedule_algo set to FifoBound, check if it is the modified value
     *         2、schedule_algo set to FifoUnbound, check if it is the modified value
     *         3、schedule_algo set to Priority, check if it is the modified value
     * @expect The function entry has no invalid values, no exception branches, and the modified schedule_algo property should be schedule_algo
     * @auto   true
     */
    #[cfg(not(feature = "ffrt"))]
    #[test]
    fn ut_thread_pool_builder_schedule_algo_test() {
        let schedule_algo = ScheduleAlgo::FifoBound;
        let thread_pool_builder = RuntimeBuilder::new_multi_thread().schedule_algo(schedule_algo);
        assert_eq!(thread_pool_builder.common.schedule_algo, schedule_algo);
    }
}
