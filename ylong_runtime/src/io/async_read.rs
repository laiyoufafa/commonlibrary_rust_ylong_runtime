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

use crate::io::read_buf::ReadBuf;
use crate::io::read_task::{ReadExactTask, ReadTask, ReadToEndTask, ReadToStringTask};
use std::ops::DerefMut;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::{cmp, io};

/// An async version of the `std::io::Read` trait. Provides all necessary reading methods in an
/// asynchronous style.
pub trait AsyncRead {
    /// Attempts to reads bytes from the an I/O source into the buffer passed in.
    ///
    /// If succeeds, this method will return `Poll::Ready(Ok(n))` where `n` indicates the number of
    /// bytes that have been successfully read. It's guaranteed that `n <= buf.len()`.
    ///
    /// If returns `Poll::Ready(Ok(0))`, one of the two scenarios below might have occurred
    ///     1. The underlying stream has been shut down and no longer transfers any bytes.
    ///     2. The buf passed in is empty
    ///
    /// If `Poll::Pending` is returned, it means that the input source is currently not ready
    /// for reading. In this case, this task will be put to sleep until the underlying stream
    /// becomes readable or closed.
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>>;
}

// Auto-implementation for Box object
impl<T: AsyncRead + Unpin + ?Sized> AsyncRead for Box<T> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut **self).poll_read(cx, buf)
    }
}

// Auto-implementation for mutable reference.
impl<T: AsyncRead + Unpin + ?Sized> AsyncRead for &mut T {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut **self).poll_read(cx, buf)
    }
}

// Auto-implementation for Pinned object.
impl<T> AsyncRead for Pin<T>
where
    T: DerefMut + Unpin,
    T::Target: AsyncRead,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        self.get_mut().as_mut().poll_read(cx, buf)
    }
}

/// An external trait that is automatically implemented for any object that has the AsyncRead trait.
/// Provides std-like reading methods such as `read`, `read_exact`, `read_to_end`.
/// Every method in this trait returns a future object. Awaits on the future will complete the
/// task, but it doesn't guarantee whether the task will finished immediately or asynchronously.
pub trait AsyncReadExt: AsyncRead {
    /// Reads data from the I/O source into the buffer.
    ///
    /// On success, `Ok(n)` will be returned, where `n` indicates the number of bytes
    /// that have been successfully read into the buffer. It guarantees `0 <= n < buf.len()`, and if
    /// `n == 0`, then one of the two scenarios below might have been occurred.
    ///     1. The reader has reaches `end of file` and no more bytes will be produced.
    ///     2. The length of the buffer passed in is 0.
    ///
    /// `Err(e)` will be returned when encounters a fatal error during the read procedure.
    /// This method should not read anything into the buffer if an error has occurred.
    ///
    /// # Examples
    /// ```no run
    /// let mut io = File::open("foo.txt").await?;
    /// let mut buf = [0; 3];
    /// let n = io.read(&mut buf).await?;
    /// ```
    fn read<'a>(&'a mut self, buf: &'a mut [u8]) -> ReadTask<'a, Self> {
        ReadTask::new(self, buf)
    }

    /// Reads data from the I/O source into the buffer until the buffer is entirely filled.
    ///
    /// On success, `Ok(())` will be returned, indicating the `buf` has been filled entirely.
    /// If the I/O connection closes before filling the entire buffer, io::Error::UnexpectedEof
    /// will be returned.
    /// If a read error occurs during the process, this method will finish immediately,
    /// the number of bytes that has been read is unspecified.
    ///
    /// # Examples
    /// ```no run
    /// let mut io = File::open("foo.txt").await?;
    /// let mut buf = [0; 16384];
    /// let n = io.read_exact(&mut buf).await?;
    /// ```
    fn read_exact<'a>(&'a mut self, buf: &'a mut [u8]) -> ReadExactTask<'a, Self> {
        ReadExactTask::new(self, buf)
    }

    /// Reads all data from the I/O source into the buffer until EOF.
    ///
    /// On success, `Ok(())` will be returned, indicating all data from the I/O source has been
    /// append to the buffer.
    ///
    /// If a read error occurs during the read process, this method will finish immediately,
    /// the number of bytes that has been read is unspecified.
    ///
    /// # Examples
    /// ```no run
    /// let mut io = File::open("foo.txt").await?;
    /// let mut buf = Vec::new();
    /// let n = io.read_to_end(&mut buf).await?;
    /// ```
    fn read_to_end<'a>(&'a mut self, buf: &'a mut Vec<u8>) -> ReadToEndTask<'a, Self> {
        ReadToEndTask::new(self, buf)
    }

    /// Reads all string data from the I/O source into the buffer until EOF.
    ///
    /// On success, `Ok(())` will be returned, indicating all data from the I/O source has been
    /// append to the buffer.
    ///
    /// If a read error occurs during the read process, this method will finish immediately,
    /// the number of bytes that has been read is unspecified.
    ///
    /// # Examples
    /// ```no run
    /// let mut io = File::open("foo.txt").await?;
    /// let mut buf = String::new();
    /// let n = io.read_to_string(&mut buf).await?;
    /// ```
    fn read_to_string<'a>(&'a mut self, dst: &'a mut String) -> ReadToStringTask<'a, Self> {
        ReadToStringTask::new(self, dst)
    }
}

/// AsyncRead is implemented for `&[u8]` by copying from the slice.
///
/// Note that reading updates the slice to point to the yet unread part.
/// The slice will be empty when EOF is reached.
impl AsyncRead for &[u8] {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let amt = cmp::min(buf.remaining(), self.len());
        let (a, b) = self.split_at(amt);
        buf.append(a);
        *self = b;
        Poll::Ready(Ok(()))
    }
}

impl<R: AsyncRead + ?Sized> AsyncReadExt for R {}
