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

use crate::io::buffered::DEFAULT_BUF_SIZE;
use crate::io::{poll_ready, AsyncBufRead, AsyncRead, AsyncSeek, AsyncWrite, ReadBuf};
use std::io;
use std::io::{IoSlice, SeekFrom};
use std::pin::Pin;
use std::task::{Context, Poll};

/// This is an asynchronous version of [`std::io::BufWriter`]
///
/// The `AsyncBufWriter<W>` struct adds buffering to any writer that implements AsyncWrite.
/// It is suitable to perform large, infrequent writes on the underlying [`AsyncWrite`] object
/// and maintains an in-memory buffer of the results.
///
/// When the `AsyncBufWriter<W>` is dropped, the contents inside its buffer will be discarded.
/// Creating multiple instances of `AsyncBufWriter<W>` on the same [`AsyncWrite`] stream may cause
/// data loss.
pub struct AsyncBufWriter<W> {
    inner: W,
    buf: Vec<u8>,
    written: usize,
}

impl<W: AsyncWrite> AsyncBufWriter<W> {
    /// Creates a new `AsyncBufWriter<W>` with a default buffer capacity.
    /// The default buffer capacity is 8 KB, which is the same as [`std::io::BufWriter`]
    ///
    /// # Examples
    ///
    /// ```no run
    /// use ylong_runtime::fs::File;
    ///
    /// async fn main() -> std::io::Result<()> {
    ///     use ylong_runtime::io::AsyncBufWriter;
    ///     let f = File::open("test.txt").await?;
    ///     let reader = AsyncBufWriter::new(f);
    ///     Ok(())
    /// }
    /// ```
    pub fn new(inner: W) -> AsyncBufWriter<W> {
        AsyncBufWriter::with_capacity(DEFAULT_BUF_SIZE, inner)
    }

    /// Creates a new `AsyncBufWriter<W>` with a specific buffer capacity.
    ///
    /// # Examples
    ///
    /// ```no run
    /// use ylong_runtime::fs::File;
    ///
    /// async fn main() -> std::io::Result<()> {
    ///     use ylong_runtime::io::AsyncBufWriter;
    ///     let f = File::open("test.txt").await?;
    ///     let reader = AsyncBufWriter::with_capacity(1000, f);
    ///     Ok(())
    /// }
    pub fn with_capacity(cap: usize, inner: W) -> AsyncBufWriter<W> {
        AsyncBufWriter {
            inner,
            buf: Vec::with_capacity(cap),
            written: 0,
        }
    }

    /// Gets a reference to the inner writer.
    ///
    /// # Examples
    ///
    /// ```no run
    /// use ylong_runtime::fs::File;
    ///
    /// async fn main() -> std::io::Result<()> {
    ///     use ylong_runtime::io::AsyncBufWriter;
    ///     let f = File::open("test.txt").await?;
    ///     let writer = AsyncBufWriter::new(f);
    ///     let writer_ref = writer.get_ref();
    ///     Ok(())
    /// }
    /// ```
    pub fn get_ref(&self) -> &W {
        &self.inner
    }

    /// Gets the mutable reference to the inner writer.
    ///
    /// # Examples
    ///
    /// ```no run
    /// use ylong_runtime::fs::File;
    ///
    /// async fn main() -> std::io::Result<()> {
    ///     use ylong_runtime::io::AsyncBufWriter;
    ///     let f = File::open("test.txt").await?;
    ///     let mut writer = AsyncBufWriter::new(f);
    ///     let writer_ref = writer.get_mut();
    ///     Ok(())
    /// }
    /// ```
    pub fn get_mut(&mut self) -> &mut W {
        &mut self.inner
    }

    /// Unwraps this `AsyncBufWriter<R>`, returning the underlying writer.
    ///
    /// Any leftover data inside the internal buffer of the `AsyncBufWriter` is lost.
    pub fn into_inner(self) -> W {
        self.inner
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
    ///     use ylong_runtime::io::AsyncBufWriter;
    ///     let f = File::open("test.txt").await?;
    ///     let writer = AsyncBufWriter::new(f);
    ///     let writer_buf = writer.buffer();
    ///     assert!(writer_buf.is_empty());
    ///     Ok(())
    /// }
    /// ```
    pub fn buffer(&self) -> &[u8] {
        &self.buf
    }

    fn flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = unsafe { self.get_unchecked_mut() };
        let len = this.buf.len();
        let mut res = Ok(());
        while this.written < len {
            unsafe {
                match poll_ready!(
                    Pin::new_unchecked(&mut this.inner).poll_write(cx, &this.buf[this.written..])
                ) {
                    Ok(0) => {
                        res = Err(io::Error::new(
                            io::ErrorKind::WriteZero,
                            "unwritten data remains in buf",
                        ));
                        break;
                    }
                    Ok(n) => this.written += n,
                    Err(e) => {
                        res = Err(e);
                        break;
                    }
                }
            }
        }
        if this.written > 0 {
            this.buf.drain(..this.written);
            this.written = 0;
        }
        Poll::Ready(res)
    }
}

impl<W: AsyncWrite> AsyncWrite for AsyncBufWriter<W> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        if self.buf.len() + buf.len() > self.buf.capacity() {
            poll_ready!(self.as_mut().flush(cx))?;
        }

        let this = unsafe { self.get_unchecked_mut() };
        if buf.len() >= this.buf.capacity() {
            unsafe { Pin::new_unchecked(&mut this.inner).poll_write(cx, buf) }
        } else {
            this.buf.extend_from_slice(buf);
            Poll::Ready(Ok(buf.len()))
        }
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        mut bufs: &[IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        if self.inner.is_write_vectored() {
            let mut len: usize = 0;
            for buf in bufs {
                len = len.saturating_add(buf.len());
            }
            if len + self.buf.len() > self.buf.capacity() {
                poll_ready!(self.as_mut().flush(cx))?;
            }

            let this = unsafe { self.get_unchecked_mut() };
            if len >= this.buf.capacity() {
                unsafe { Pin::new_unchecked(&mut this.inner).poll_write_vectored(cx, bufs) }
            } else {
                for buf in bufs {
                    this.buf.extend_from_slice(buf);
                }
                Poll::Ready(Ok(len))
            }
        } else {
            if bufs.is_empty() {
                return Poll::Ready(Ok(0));
            }
            while bufs[0].len() == 0 {
                bufs = &bufs[1..];
            }
            let mut len = bufs[0].len();
            if len + self.buf.len() > self.buf.capacity() {
                poll_ready!(self.as_mut().flush(cx))?;
            }

            let this = unsafe { self.get_unchecked_mut() };
            if len >= this.buf.capacity() {
                return unsafe { Pin::new_unchecked(&mut this.inner).poll_write(cx, &bufs[0]) };
            } else {
                this.buf.extend_from_slice(&bufs[0]);
                bufs = &bufs[1..];
            }
            for buf in bufs {
                if buf.len() + this.buf.len() >= this.buf.capacity() {
                    break;
                } else {
                    this.buf.extend_from_slice(buf);
                    len += buf.len()
                }
            }
            Poll::Ready(Ok(len))
        }
    }

    fn is_write_vectored(&self) -> bool {
        true
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        poll_ready!(self.as_mut().flush(cx))?;
        let this = unsafe { self.get_unchecked_mut() };
        unsafe { Pin::new_unchecked(&mut this.inner).poll_flush(cx) }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        poll_ready!(self.as_mut().flush(cx))?;
        let this = unsafe { self.get_unchecked_mut() };
        unsafe { Pin::new_unchecked(&mut this.inner).poll_shutdown(cx) }
    }
}

impl<R: AsyncWrite + AsyncSeek> AsyncSeek for AsyncBufWriter<R> {
    fn poll_seek(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        pos: SeekFrom,
    ) -> Poll<io::Result<u64>> {
        poll_ready!(self.as_mut().flush(cx))?;
        let this = unsafe { self.get_unchecked_mut() };
        unsafe { Pin::new_unchecked(&mut this.inner).poll_seek(cx, pos) }
    }
}

impl<W: AsyncWrite + AsyncRead> AsyncRead for AsyncBufWriter<W> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = unsafe { self.get_unchecked_mut() };
        unsafe { Pin::new_unchecked(&mut this.inner).poll_read(cx, buf) }
    }
}

impl<W: AsyncWrite + AsyncBufRead> AsyncBufRead for AsyncBufWriter<W> {
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        let this = unsafe { self.get_unchecked_mut() };
        unsafe { Pin::new_unchecked(&mut this.inner).poll_fill_buf(cx) }
    }

    fn consume(self: Pin<&mut Self>, amt: usize) {
        let this = unsafe { self.get_unchecked_mut() };
        unsafe { Pin::new_unchecked(&mut this.inner).consume(amt) }
    }
}
