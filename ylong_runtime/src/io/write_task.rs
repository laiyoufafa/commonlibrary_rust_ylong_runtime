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

use crate::io::AsyncWrite;
use std::future::Future;
use std::io;
use std::io::IoSlice;
use std::pin::Pin;
use std::task::{Context, Poll};

macro_rules! take_writer {
    ($self: expr) => {
        match $self.writer.take() {
            Some(writer) => writer,
            None => panic!("write: poll after finished"),
        }
    };
}

/// A future for writing some of the buffer to the io
///
/// Returned by [`crate::io::AsyncWriteExt::write`]
pub struct WriteTask<'a, W: ?Sized> {
    writer: Option<&'a mut W>,
    buf: &'a [u8],
}

impl<'a, W: ?Sized> WriteTask<'a, W>
where
    W: AsyncWrite + Unpin,
{
    #[inline(always)]
    pub(crate) fn new(writer: &'a mut W, buf: &'a [u8]) -> WriteTask<'a, W> {
        WriteTask {
            writer: Some(writer),
            buf,
        }
    }
}

impl<'a, W> Future for WriteTask<'a, W>
where
    W: AsyncWrite + Unpin,
{
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut writer = take_writer!(self);

        match Pin::new(&mut writer).poll_write(cx, self.buf) {
            Poll::Pending => {
                self.writer = Some(writer);
                Poll::Pending
            }
            x => x,
        }
    }
}

/// A future for writing data inside a vector to the io
///
/// Returned by [`crate::io::AsyncWriteExt::write_vectored`]
pub struct WriteVectoredTask<'a, 'b, W: ?Sized> {
    writer: Option<&'a mut W>,
    bufs: &'a [IoSlice<'b>],
}

impl<'a, 'b, W: ?Sized> WriteVectoredTask<'a, 'b, W>
where
    W: AsyncWrite + Unpin,
{
    #[inline(always)]
    pub(crate) fn new(writer: &'a mut W, bufs: &'a [IoSlice<'b>]) -> WriteVectoredTask<'a, 'b, W> {
        WriteVectoredTask {
            writer: Some(writer),
            bufs,
        }
    }
}

impl<'a, 'b, W: ?Sized> Future for WriteVectoredTask<'a, 'b, W>
where
    W: AsyncWrite + Unpin,
{
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut writer = take_writer!(self);

        match Pin::new(&mut writer).poll_write_vectored(cx, self.bufs) {
            Poll::Pending => {
                self.writer = Some(writer);
                Poll::Pending
            }
            x => x,
        }
    }
}

/// A future for writing every data inside a buffer to the io
///
/// Returned by [`crate::io::AsyncWriteExt::write_all`]
pub struct WriteAllTask<'a, W: ?Sized> {
    writer: Option<&'a mut W>,
    buf: &'a [u8],
    w_len: usize,
}

impl<'a, W: ?Sized> WriteAllTask<'a, W>
where
    W: AsyncWrite + Unpin,
{
    #[inline(always)]
    pub(crate) fn new(writer: &'a mut W, buf: &'a [u8]) -> WriteAllTask<'a, W> {
        WriteAllTask {
            writer: Some(writer),
            buf,
            w_len: 0,
        }
    }
}

impl<'a, W> Future for WriteAllTask<'a, W>
where
    W: AsyncWrite + Unpin,
{
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut writer = take_writer!(self);
        while self.w_len < self.buf.len() {
            match Pin::new(&mut writer).poll_write(cx, &self.buf[self.w_len..]) {
                Poll::Ready(Ok(0)) => return Poll::Ready(Err(io::ErrorKind::WriteZero.into())),
                Poll::Ready(Ok(n)) => self.w_len += n,
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => {
                    self.writer = Some(writer);
                    return Poll::Pending;
                }
            }
        }
        Poll::Ready(Ok(()))
    }
}

pub struct FlushTask<'a, W: ?Sized> {
    writer: &'a mut W,
}

impl<'a, W: ?Sized> FlushTask<'a, W>
where
    W: AsyncWrite + Unpin,
{
    #[inline(always)]
    pub(crate) fn new(writer: &'a mut W) -> FlushTask<'a, W> {
        FlushTask { writer }
    }
}

impl<'a, W> Future for FlushTask<'a, W>
where
    W: AsyncWrite + Unpin + ?Sized,
{
    type Output = io::Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.get_mut();
        Pin::new(&mut me.writer).poll_flush(cx)
    }
}
pub struct ShutdownTask<'a, W: ?Sized> {
    writer: &'a mut W,
}

impl<'a, W: ?Sized> ShutdownTask<'a, W>
where
    W: AsyncWrite + Unpin,
{
    #[inline(always)]
    pub(crate) fn new(writer: &'a mut W) -> ShutdownTask<'a, W> {
        ShutdownTask { writer }
    }
}

impl<'a, W> Future for ShutdownTask<'a, W>
where
    W: AsyncWrite + Unpin + ?Sized,
{
    type Output = io::Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.get_mut();
        Pin::new(&mut me.writer).poll_shutdown(cx)
    }
}
