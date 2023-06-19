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

use crate::io::async_buf_read::AsyncBufRead;
use crate::io::buffered::DEFAULT_BUF_SIZE;
use crate::io::{poll_ready, AsyncRead, AsyncSeek, AsyncWrite, ReadBuf};
use std::cmp;
use std::io::{IoSlice, SeekFrom};
use std::pin::Pin;
use std::task::{Context, Poll};

/// This is an asynchronous version of [`std::io::BufReader`]
///
/// The `AsyncBufReader<R>` struct adds buffering to any reader that implements AsyncRead.
/// It is suitable to perform large, infrequent reads on the underlying [`AsyncRead`] object
/// and maintains an in-memory buffer of the results.
///
/// When the `AsyncBufReader<R>` is dropped, the contents inside its buffer will be discarded.
/// Creating multiple instances of `AsyncBufReader<R>` on the same [`AsyncRead`] stream may cause
/// data loss.
pub struct AsyncBufReader<R> {
    inner: R,
    buf: Box<[u8]>,
    pos: usize,
    filled: usize,
}

impl<R: AsyncRead> AsyncBufReader<R> {
    /// Creates a new `AsyncBufReader<R>` with a default buffer capacity.
    /// The default buffer capacity is 8 KB, which is the same as [`std::io::BufReader`]
    ///
    /// # Examples
    ///
    /// ```no run
    /// use ylong_runtime::fs::File;
    ///
    /// async fn main() -> std::io::Result<()> {
    ///     use ylong_runtime::io::AsyncBufReader;
    ///     let f = File::open("test.txt").await?;
    ///     let reader = AsyncBufReader::new(f);
    ///     Ok(())
    /// }
    /// ```
    pub fn new(inner: R) -> AsyncBufReader<R> {
        AsyncBufReader::with_capacity(DEFAULT_BUF_SIZE, inner)
    }

    /// Creates a new `AsyncBufReader<R>` with a specific buffer capacity.
    ///
    /// # Examples
    ///
    /// ```no run
    /// use ylong_runtime::fs::File;
    ///
    /// async fn main() -> std::io::Result<()> {
    ///     use ylong_runtime::io::AsyncBufReader;
    ///     let f = File::open("test.txt").await?;
    ///     let reader = AsyncBufReader::with_capacity(1000, f);
    ///     Ok(())
    /// }
    pub fn with_capacity(capacity: usize, inner: R) -> AsyncBufReader<R> {
        AsyncBufReader {
            inner,
            buf: vec![0; capacity].into_boxed_slice(),
            pos: 0,
            filled: 0,
        }
    }
}

impl<R> AsyncBufReader<R> {
    /// Gets a reference to the underlying reader.
    ///
    /// # Examples
    ///
    /// ```no run
    /// use ylong_runtime::fs::File;
    ///
    /// async fn main() -> std::io::Result<()> {
    ///     use ylong_runtime::io::AsyncBufReader;
    ///     let f = File::open("test.txt").await?;
    ///     let reader = AsyncBufReader::new(f);
    ///     let reader_ref = reader.get_ref();
    ///     Ok(())
    /// }
    /// ```
    pub fn get_ref(&self) -> &R {
        &self.inner
    }

    /// Gets the mutable reference to the underlying reader.
    ///
    /// # Examples
    ///
    /// ```no run
    /// use ylong_runtime::fs::File;
    ///
    /// async fn main() -> std::io::Result<()> {
    ///     use ylong_runtime::io::AsyncBufReader;
    ///     let f = File::open("test.txt").await?;
    ///     let mut reader = AsyncBufReader::new(f);
    ///     let reader_ref = reader.get_mut();
    ///     Ok(())
    /// }
    /// ```
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.inner
    }

    /// Returns a reference to the internally buffered data.
    ///
    /// Only returns the filled part of the buffer instead of the whole buffer.
    ///
    /// # Examples
    ///
    /// ```no run
    /// use ylong_runtime::fs::File;
    ///
    /// async fn main() -> std::io::Result<()> {
    ///     use ylong_runtime::io::AsyncBufReader;
    ///     let f = File::open("test.txt").await?;
    ///     let reader = AsyncBufReader::new(f);
    ///     let read_buf = reader.buffer();
    ///     assert!(read_buf.is_empty());
    ///     Ok(())
    /// }
    /// ```
    pub fn buffer(&self) -> &[u8] {
        &self.buf[self.pos..self.filled]
    }

    /// Returns the capacity of the internal buffer.
    ///
    /// # Examples
    ///
    /// ```no run
    /// use ylong_runtime::fs::File;
    ///
    /// async fn main() -> std::io::Result<()> {
    ///     use ylong_runtime::io::AsyncBufReader;
    ///     let f = File::open("test.txt").await?;
    ///     let reader = AsyncBufReader::with_capacity(10, f);
    ///     let capacity = reader.capacity();
    ///     assert_eq!(capacity, 10);
    ///     Ok(())
    /// }
    /// ```
    pub fn capacity(&self) -> usize {
        self.buf.len()
    }

    /// Unwraps this `AsyncBufReader<R>`, returning the underlying reader.
    ///
    /// Any leftover data inside the internal buffer of the `AsyncBufReader` is lost.
    pub fn into_inner(self) -> R {
        self.inner
    }

    /// Invalidates all data in the internal buffer.
    fn discard_buffer(&mut self) {
        self.pos = 0;
        self.filled = 0;
    }
}

impl<R: AsyncRead> AsyncRead for AsyncBufReader<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        if self.filled == self.pos && buf.remaining() >= self.buf.len() {
            let this = unsafe { self.get_unchecked_mut() };
            this.discard_buffer();
            return unsafe { Pin::new_unchecked(&mut this.inner).poll_read(cx, buf) };
        }
        let rem = poll_ready!(self.as_mut().poll_fill_buf(cx))?;
        let r_len = cmp::min(rem.len(), buf.remaining());
        buf.append(&rem[..r_len]);
        self.as_mut().consume(r_len);

        Poll::Ready(Ok(()))
    }
}

impl<R: AsyncRead> AsyncBufRead for AsyncBufReader<R> {
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<&[u8]>> {
        let this = unsafe { self.get_unchecked_mut() };
        if this.pos >= this.filled {
            let mut read_buf = ReadBuf::new(&mut this.buf);
            unsafe {
                poll_ready!(Pin::new_unchecked(&mut this.inner).poll_read(cx, &mut read_buf))?;
            }
            this.pos = 0;
            this.filled = read_buf.filled_len();
        }
        Poll::Ready(Ok(&this.buf[this.pos..this.filled]))
    }

    fn consume(self: Pin<&mut Self>, amt: usize) {
        let this = unsafe { self.get_unchecked_mut() };
        this.pos = cmp::min(this.pos + amt, this.filled);
    }
}

impl<R: AsyncRead + AsyncSeek> AsyncSeek for AsyncBufReader<R> {
    fn poll_seek(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        pos: SeekFrom,
    ) -> Poll<std::io::Result<u64>> {
        let this = unsafe { self.get_unchecked_mut() };
        if let SeekFrom::Current(n) = pos {
            let remainder = (this.filled - this.pos) as i64;
            if let Some(offset) = n.checked_sub(remainder) {
                let res = unsafe {
                    poll_ready!(Pin::new_unchecked(&mut this.inner)
                        .poll_seek(cx, SeekFrom::Current(offset)))?
                };
                this.discard_buffer();
                return Poll::Ready(Ok(res));
            } else {
                unsafe {
                    poll_ready!(Pin::new_unchecked(&mut this.inner)
                        .poll_seek(cx, SeekFrom::Current(-remainder)))?;
                    this.discard_buffer();
                }
            }
        }

        let res = unsafe { poll_ready!(Pin::new_unchecked(&mut this.inner).poll_seek(cx, pos))? };
        this.discard_buffer();
        Poll::Ready(Ok(res))
    }
}

impl<R: AsyncRead + AsyncWrite> AsyncWrite for AsyncBufReader<R> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let this = unsafe { self.get_unchecked_mut() };
        unsafe { Pin::new_unchecked(&mut this.inner).poll_write(cx, buf) }
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<std::io::Result<usize>> {
        let this = unsafe { self.get_unchecked_mut() };
        unsafe { Pin::new_unchecked(&mut this.inner).poll_write_vectored(cx, bufs) }
    }

    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let this = unsafe { self.get_unchecked_mut() };
        unsafe { Pin::new_unchecked(&mut this.inner).poll_flush(cx) }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let this = unsafe { self.get_unchecked_mut() };
        unsafe { Pin::new_unchecked(&mut this.inner).poll_shutdown(cx) }
    }
}
