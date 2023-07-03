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

use crate::builder::common_builder::impl_common;
use crate::builder::CommonBuilder;
#[cfg(feature = "multi_instance_runtime")]
use crate::executor::{AsyncHandle, Runtime};
use std::io;
use std::sync::Mutex;

pub(crate) static GLOBAL_BUILDER: Mutex<Option<MultiThreadBuilder>> = Mutex::new(None);

/// Runtime builder that configures a multi-threaded runtime, or the global runtime.
pub struct MultiThreadBuilder {
    pub(crate) common: CommonBuilder,

    /// Maximum thread number for core thread pool
    pub(crate) core_thread_size: Option<u8>,
}

impl MultiThreadBuilder {
    pub(crate) fn new() -> Self {
        MultiThreadBuilder {
            common: CommonBuilder::new(),
            core_thread_size: None,
        }
    }

    /// Configures the global runtime.
    ///
    /// # Error
    /// If the global runtime is already running or this method has been called before, then
    /// it will return an `AlreadyExists` error.
    pub fn build_global(self) -> io::Result<()> {
        let mut builder = GLOBAL_BUILDER.lock().unwrap();
        match *builder {
            None => *builder = Some(self),
            Some(_) => return Err(io::ErrorKind::AlreadyExists.into()),
        }
        Ok(())
    }

    /// Initializes the runtime and returns its instance.
    #[cfg(all(feature = "multi_instance_runtime", not(feature = "ffrt")))]
    pub fn build(&mut self) -> io::Result<Runtime> {
        use crate::builder::initialize_async_spawner;

        #[cfg(feature = "net")]
        let (arc_handle, arc_driver) = crate::net::Driver::initialize();

        let async_spawner = initialize_async_spawner(
            self,
            #[cfg(feature = "net")]
            (arc_handle.clone(), arc_driver),
        )?;

        Ok(Runtime {
            async_spawner: AsyncHandle::MultiThread(async_spawner),
            #[cfg(feature = "net")]
            handle: arc_handle,
        })
    }

    /// Sets the number of core worker threads.
    ///
    ///
    /// The boundary of thread number is 1-64:
    /// If sets a number smaller than 1, then thread number would be set to 1.
    /// If sets a number larger than 64, then thread number would be set to 64.
    /// The default value is the number of cores of the cpu.
    ///
    /// # Examples
    /// ```
    /// use crate::ylong_runtime::builder::RuntimeBuilder;
    ///
    /// let runtime = RuntimeBuilder::new_multi_thread()
    ///     .worker_num(8);
    /// ```
    pub fn worker_num(mut self, core_pool_size: u8) -> Self {
        if core_pool_size < 1 {
            self.core_thread_size = Some(1);
        } else if core_pool_size > 64 {
            self.core_thread_size = Some(64);
        } else {
            self.core_thread_size = Some(core_pool_size);
        }
        self
    }
}

impl_common!(MultiThreadBuilder);

#[cfg(feature = "full")]
#[cfg(test)]
mod test {
    use crate::builder::RuntimeBuilder;
    use crate::executor::{global_default_async, AsyncHandle};

    /// UT for blocking on a time sleep without initializing the runtime.
    ///
    /// # Brief
    /// 1. Configure the global runtime to make it have six core threads
    /// 2. Get the global runtime
    /// 3. Check the core thread number of the runtime
    /// 4. Call build_global once more
    /// 5. Check the error
    #[test]
    fn ut_build_global() {
        let ret = RuntimeBuilder::new_multi_thread()
            .worker_num(6)
            .max_blocking_pool_size(3)
            .build_global();
        assert!(ret.is_ok());

        let async_pool = global_default_async();
        match &async_pool.async_spawner {
            AsyncHandle::CurrentThread(_) => unreachable!(),
            AsyncHandle::MultiThread(x) => {
                assert_eq!(x.inner.total, 6);
            }
        }

        let ret = RuntimeBuilder::new_multi_thread()
            .worker_num(2)
            .max_blocking_pool_size(3)
            .build_global();
        assert!(ret.is_err());
    }
}
