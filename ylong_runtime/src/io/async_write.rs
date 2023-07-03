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

use crate::io::write_task::{FlushTask, ShutdownTask, WriteAllTask, WriteTask, WriteVectoredTask};
use std::io;
use std::io::IoSlice;
use std::ops::DerefMut;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Async version of the `std::io::Write` trait. Provides all necessary writing methods in an
/// asynchronous style.
pub trait AsyncWrite {
    /// Attempts to write bytes from buffer into an I/O source.
    ///
    /// If succeeds, this method will return `Poll::Ready(Ok(n))` where `n` indicates the number of
    /// bytes that have been successfully written. It's guaranteed that `n <= buf.len()`.
    ///
    /// If returns `Poll::Ready(Ok(0))`, one of the two scenarios below might have occurred
    ///     1. The underlying stream has been shut down and no longer accepts any bytes.
    ///     2. The buf passed in is empty
    ///
    /// If `Poll::Pending` is returned, it means that the output stream is currently not ready
    /// for writing. In this case, this task will be put to sleep until the underlying stream
    /// becomes writable or closed.
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>>;

    /// Attempts to write bytes from a slice of buffers into an I/O source.
    ///
    /// This default implementation writes the first none empty buffer, or writes an empty one
    /// if all buffers are empty.
    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        let buf = bufs
            .iter()
            .find(|b| !b.is_empty())
            .map_or(&[][..], |b| &**b);
        self.poll_write(cx, buf)
    }

    /// Indicates whether this AsyncWrite implementation has an efficient `write_vectored`.
    /// The default implementation is not.
    fn is_write_vectored(&self) -> bool {
        false
    }

    /// Attempts to flush the I/O source, ensuring that any buffered data has been sent to
    /// their destination.
    ///
    /// If succeeds, `Poll::Ready(Ok(()))` will be returned
    ///
    /// If `Poll::Pending` is returned, it means the stream cannot be flushed immediately.
    /// The task will continue once its waker receives a notification indicating the stream is
    /// ready.
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>>;

    /// Attempts to shut down the writer, returns `Poll::Ready(Ok(()))` when the underlying I/O
    /// connection is completely closed and therefore safe to drop.
    ///
    /// This method is designed for asynchronous shutdown of the I/O connection. For protocols like
    /// TLS or TCP, this is the place to do a last flush of data and gracefully turn off the
    /// connection.
    ///
    /// If `Poll::Ready(Err(e))` is returned, it indicates a fatal error has been occurred during
    /// the shutdown procedure. It typically means the I/O source is already broken.
    ///
    /// If `Poll::Pending` is returned, it indicates the I/O connection is not ready to shut
    /// down immediately, it may have another final data to be flushed.
    /// This task will be continued once the waker receives a ready notification from the connection.
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>>;
}

macro_rules! async_write_deref {
    () => {
        /// A default poll_write implementation for an object that could be deref to an
        /// AsyncWrite object.
        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<Result<usize, io::Error>> {
            Pin::new(&mut **self).poll_write(cx, buf)
        }

        /// A default poll_write_vectored implementation for an object that could be deref to an
        /// AsyncWrite object.
        fn poll_write_vectored(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            bufs: &[IoSlice<'_>],
        ) -> Poll<Result<usize, io::Error>> {
            Pin::new(&mut **self).poll_write_vectored(cx, bufs)
        }

        /// A default is_write_vectored implementation for an object that could be deref to an
        /// AsyncWrite object.
        fn is_write_vectored(&self) -> bool {
            (**self).is_write_vectored()
        }

        /// A default poll_flush implementation for an object that could be deref to an
        /// AsyncWrite object.
        fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Pin::new(&mut **self).poll_flush(cx)
        }

        /// A default poll_shutdown implementation for an object that could be deref to an
        /// AsyncWrite object.
        fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Pin::new(&mut **self).poll_shutdown(cx)
        }
    };
}

impl<T: AsyncWrite + Unpin + ?Sized> AsyncWrite for Box<T> {
    async_write_deref!();
}

impl<T: AsyncWrite + Unpin + ?Sized> AsyncWrite for &mut T {
    async_write_deref!();
}

impl<T> AsyncWrite for Pin<T>
where
    T: DerefMut<Target = dyn AsyncWrite> + Unpin,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        Pin::as_mut(self.get_mut()).poll_write(cx, buf)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<Result<usize, io::Error>> {
        Pin::as_mut(self.get_mut()).poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        (**self).is_write_vectored()
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::as_mut(self.get_mut()).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::as_mut(self.get_mut()).poll_shutdown(cx)
    }
}

/// An external trait that is automatically implemented for any object that has the AsyncWrite trait.
/// Provides std-like writing methods such as `write`, `write_vectored`, 'write_all'.
/// Every method in this trait returns a future object. Awaits on the future will complete the task
/// but it doesn't guarantee whether the task will finished immediately or asynchronously.
pub trait AsyncWriteExt: AsyncWrite {
    /// Writes data from the buffer into the I/O source.
    ///
    /// On success, `Ok(n)` will be returned, where `n` indicates the number of bytes
    /// that have been successfully written into the buffer. It guarantees `0 <= n < buf.len()`,
    /// and if `n == 0`, then one of the two scenarios below might have been occurred.
    ///     1. The underlying I/O no longer accepts any bytes.
    ///     2. The length of the buffer passed in is 0.
    ///
    /// `Err(e)` will be returned when encounters a fatal error during the write procedure.
    /// This method should not write anything into the buffer if an error has occurred.
    ///
    /// Not writing the entire buffer into the I/O is not an error.
    /// # Examples
    /// ```no run
    /// let mut io = File::create("foo.txt").await?;
    /// let buf = [1, 2, 3];
    /// let n = io.write(&buf).await?;
    /// ```
    fn write<'a>(&'a mut self, buf: &'a [u8]) -> WriteTask<'a, Self>
    where
        Self: Unpin,
    {
        WriteTask::new(self, buf)
    }

    /// Writes data from a slice of buffers into the I/O source.
    ///
    /// Data is copied from each buffer in order, with the final buffer
    /// read from possibly being only partially consumed. This method must
    /// behave as a call to [`write`] with the buffers concatenated would.
    ///
    /// Return values of this method are the same as [`write`].
    ///
    /// # Examples
    /// ```no run
    /// let mut data1 = [1, 2, 3];
    /// let mut data2 = [4, 5, 6];
    /// let slice1 = IoSlice::new(&mut data1);
    /// let slice2 = IoSlice::new(&mut data2);
    /// let mut io = Filre::create("foo.txt").await?;
    /// let n = io.write_vectored(&[slice1, slice2]).await?;
    ///
    /// ```
    fn write_vectored<'a, 'b>(
        &'a mut self,
        bufs: &'a [IoSlice<'b>],
    ) -> WriteVectoredTask<'a, 'b, Self>
    where
        Self: Unpin,
    {
        WriteVectoredTask::new(self, bufs)
    }

    /// Writes all data from the buffer into the I/O source.
    ///
    /// On success, `Ok(())` will be returned, indicating all data from the buffer has been
    /// written into the I/O.
    ///
    /// If a write error occurs during the process, this method will finish immediately,
    /// the number of bytes that has been written is unspecified.
    ///
    /// # Examples
    /// ```no run
    /// let mut io = File::create("foo.txt").await?;
    /// let buf = [0; 16384];
    /// let n = io.read_to_end(&buf).await?;
    /// ```
    fn write_all<'a>(&'a mut self, buf: &'a [u8]) -> WriteAllTask<'a, Self>
    where
        Self: Unpin,
    {
        WriteAllTask::new(self, buf)
    }

    /// Flushes the stream to ensure that all data reach the destination.
    ///
    /// `Err(e)` will be returned when the I/O error occurring or EOF being reached.
    ///
    /// # Examples
    /// ```no run
    /// let mut io = File::create("foo.txt").await?;
    /// let buf = [1, 2, 3];
    /// let n = io.write(&buf).await?;
    /// io.flush().await?;
    /// ```
    fn flush(&mut self) -> FlushTask<'_, Self>
    where
        Self: Unpin,
    {
        FlushTask::new(self)
    }

    /// Shuts down the stream.
    ///
    /// # Examples
    /// ```no run
    /// let mut io = File::create("foo.txt").await?;
    /// let buf = [1, 2, 3];
    /// let n = io.write(&buf).await?;
    /// io.shutdown().await?;
    /// ```
    fn shutdown(&mut self) -> ShutdownTask<'_, Self>
    where
        Self: Unpin,
    {
        ShutdownTask::new(self)
    }
}

impl<R: AsyncWrite + ?Sized> AsyncWriteExt for R {}
