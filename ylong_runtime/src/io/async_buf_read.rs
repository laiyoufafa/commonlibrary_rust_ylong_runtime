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

use crate::io::read_task::{LinesTask, ReadLineTask, ReadUtilTask, SplitTask};
use crate::io::AsyncRead;
use std::io;
use std::ops::DerefMut;
use std::pin::Pin;
use std::task::{Context, Poll};

/// It is an asynchronous version of [`std::io::BufRead`].
///
/// A `AsyncBufRead` is a type of `AsyncRead`er which has an internal buffer, allowing it
/// to perform extra ways of reading, such as `read_line`.
pub trait AsyncBufRead: AsyncRead {
    /// Returns the contents of the internal buffer, trying to fill it with more data
    /// from the inner reader if it is empty.
    ///
    /// This method is non-blocking. If the underlying reader is unable to perform
    /// a read at the time, this method would return a `Poll::Pending`. If there is data inside
    /// the buffer or the read is successfully performed, then it would return a
    /// `Poll::Ready(&[u8])`.
    ///
    /// This method is a low-level call. It needs to be paired up with calls to
    /// [`Self::consume`] method to function properly.
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>>;

    /// Tells this buffer that `amt` bytes have been consumed from the buffer, so they should
    /// no longer be returned in calls to `read`.
    ///
    /// This method is a low-level call. It has to be called after a call to
    /// [`Self::poll_fill_buf`] in order to function properly.
    ///
    /// The `amt` must be `<=` the number of bytes in the buffer returned by
    /// [`Self::poll_fill_buf`]
    fn consume(self: Pin<&mut Self>, amt: usize);
}

// Auto-implementation for Box object
impl<T: AsyncBufRead + Unpin + ?Sized> AsyncBufRead for Box<T> {
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        Pin::new(&mut **self.get_mut()).poll_fill_buf(cx)
    }

    fn consume(mut self: Pin<&mut Self>, amt: usize) {
        Pin::new(&mut **self).consume(amt)
    }
}

// Auto-implementation for mutable reference.
impl<T: AsyncBufRead + Unpin + ?Sized> AsyncBufRead for &mut T {
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        Pin::new(&mut **self.get_mut()).poll_fill_buf(cx)
    }

    fn consume(mut self: Pin<&mut Self>, amt: usize) {
        Pin::new(&mut **self).consume(amt)
    }
}

// Auto-implementation for Pinned object.
impl<T> AsyncBufRead for Pin<T>
where
    T: DerefMut + Unpin,
    T::Target: AsyncBufRead,
{
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        self.get_mut().as_mut().poll_fill_buf(cx)
    }

    fn consume(self: Pin<&mut Self>, amt: usize) {
        self.get_mut().as_mut().consume(amt)
    }
}

/// An external trait that is automatically implemented for any object that has the AsyncBufRead trait.
/// Provides std-like reading methods such as `read_until`, `read_line`, `split`, `lines`.
/// Every method in this trait returns a future object. Awaits on the future will complete the
/// task, but it doesn't guarantee whether the task will finished immediately or asynchronously.
pub trait AsyncBufReadExt: AsyncBufRead {
    /// Asynchronously reads data from the underlying stream into the `buf` until the
    /// desired delimiter appears or EOF is reached.
    ///
    /// If successful, this function will return the total number of bytes read.
    ///
    /// # Examples
    /// ```no run
    /// let mut file = File::open("foo.txt").await?;
    /// let mut res = vec![];
    /// let mut buf_reader = AsyncBufReader::new(file);
    /// let ret = buf_reader.read_until(b':', &mut res).await?;
    /// ```
    fn read_until<'a>(&'a mut self, byte: u8, buf: &'a mut Vec<u8>) -> ReadUtilTask<'a, Self>
    where
        Self: Unpin,
    {
        ReadUtilTask::new(self, byte, buf)
    }

    /// Asynchronously reads data from the underlying stream into the `buf` until the
    /// delimiter '\n' appears or EOF is reached.
    ///
    /// If successful, this function will return the total number of bytes read.
    ///
    /// # Examples
    /// ```no run
    /// let mut file = File::open("foo.txt").await?;
    /// let mut res = String::new();
    /// let mut buf_reader = AsyncBufReader::new(file);
    /// let ret = buf_reader.read_line(&mut res).await?;
    /// ```
    fn read_line<'a>(&'a mut self, buf: &'a mut String) -> ReadLineTask<'a, Self>
    where
        Self: Unpin,
    {
        ReadLineTask::new(self, buf)
    }

    /// Asynchronously reads data from the underlying stream until EOF is reached
    /// and splits it on a delimiter.
    ///
    /// # Examples
    /// ```no run
    /// let mut file = File::open("foo.txt").await?;
    /// let mut buf_reader = AsyncBufReader::new(file);
    /// let mut segments = buf_reader.split(b'-');
    /// assert!(segments.next().await?.is_some());
    /// ```
    fn split(self, byte: u8) -> SplitTask<Self>
    where
        Self: Sized + Unpin,
    {
        SplitTask::new(self, byte)
    }

    /// Asynchronously reads data from the underlying stream until EOF is reached
    /// and splits it on a delimiter.
    ///
    /// # Examples
    /// ```no run
    /// let mut file = File::open("foo.txt").await?;
    /// let mut buf_reader = AsyncBufReader::new(file);
    /// let mut segments = buf_reader.lines();
    /// assert!(segments.next_line().await?.is_some());
    /// ```
    fn lines(self) -> LinesTask<Self>
    where
        Self: Sized,
    {
        LinesTask::new(self)
    }
}

impl<R: AsyncBufRead + ?Sized> AsyncBufReadExt for R {}
