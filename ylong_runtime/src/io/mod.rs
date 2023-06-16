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

//! Usability Traits and helpers for asynchronous I/O functionality.
//!
//! This module is an asynchronous version of [`std::io`]

mod async_buf_read;
mod async_read;
mod async_seek;
mod async_write;
mod buffered;
mod read_buf;
mod read_task;
mod seek_task;
mod write_task;

pub use async_buf_read::{AsyncBufRead, AsyncBufReadExt};
pub use async_read::{AsyncRead, AsyncReadExt};
pub use async_seek::{AsyncSeek, AsyncSeekExt};
pub use async_write::{AsyncWrite, AsyncWriteExt};
pub use buffered::{AsyncBufReader, AsyncBufWriter};
pub use read_buf::ReadBuf;
pub use read_task::ReadTask;
pub use write_task::WriteTask;

macro_rules! poll_ready {
    ($e:expr) => {
        match $e {
            std::task::Poll::Ready(t) => t,
            std::task::Poll::Pending => return Poll::Pending,
        }
    };
}

pub(crate) use poll_ready;
