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

//! Asynchronous file IO

mod async_dir;
mod async_file;
mod file_buf;
mod open_options;

use crate::spawn::spawn_blocking;
use crate::task::TaskBuilder;
pub use async_dir::{create_dir, create_dir_all, read_dir, remove_dir, remove_dir_all};
pub use async_file::File;
pub use open_options::OpenOptions;
use std::io;

pub(crate) async fn async_op<T, R>(task: T) -> io::Result<R>
where
    T: FnOnce() -> io::Result<R>,
    T: Send + 'static,
    R: Send + 'static,
{
    spawn_blocking(&TaskBuilder::new(), task)
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
}

macro_rules! poll_ready {
    ($e:expr) => {
        match $e {
            std::task::Poll::Ready(t) => t,
            std::task::Poll::Pending => return Poll::Pending,
        }
    };
}

pub(crate) use poll_ready;
