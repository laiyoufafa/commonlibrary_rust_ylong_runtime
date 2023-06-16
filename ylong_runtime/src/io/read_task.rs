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

use crate::futures::poll_fn;
use crate::io::async_buf_read::AsyncBufRead;
use crate::io::async_read::AsyncRead;
use crate::io::poll_ready;
use crate::io::read_buf::ReadBuf;
use std::future::Future;
use std::mem::MaybeUninit;
use std::pin::Pin;
use std::slice::from_raw_parts_mut;
use std::string::FromUtf8Error;
use std::task::{Context, Poll};
use std::{io, mem};

macro_rules! take_reader {
    ($self: expr) => {
        match $self.reader.take() {
            Some(reader) => reader,
            None => panic!("read: poll after finished"),
        }
    };
}

/// A future for reading available data from the source into a buffer.
///
/// Returned by [`crate::io::AsyncReadExt::read`]
pub struct ReadTask<'a, R: ?Sized> {
    reader: Option<&'a mut R>,
    buf: &'a mut [u8],
}

impl<'a, R: ?Sized> ReadTask<'a, R> {
    #[inline(always)]
    pub(crate) fn new(reader: &'a mut R, buf: &'a mut [u8]) -> ReadTask<'a, R> {
        ReadTask {
            reader: Some(reader),
            buf,
        }
    }
}

impl<'a, R> Future for ReadTask<'a, R>
where
    R: AsyncRead + Unpin,
{
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut reader = take_reader!(self);

        let mut buf = ReadBuf::new(self.buf);
        match Pin::new(&mut reader).poll_read(cx, &mut buf) {
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Ready(_) => Poll::Ready(Ok(buf.filled_len())),
            Poll::Pending => {
                self.reader = Some(reader);
                Poll::Pending
            }
        }
    }
}

/// A future for reading every data from the source into a vector.
///
/// Returned by [`crate::io::AsyncReadExt::read_to_end`]
pub struct ReadToEndTask<'a, R: ?Sized> {
    reader: &'a mut R,
    buf: &'a mut Vec<u8>,
    r_len: usize,
}

impl<'a, R: ?Sized> ReadToEndTask<'a, R> {
    #[inline(always)]
    pub(crate) fn new(reader: &'a mut R, buf: &'a mut Vec<u8>) -> ReadToEndTask<'a, R> {
        ReadToEndTask {
            reader,
            buf,
            r_len: 0,
        }
    }
}

fn poll_read_to_end<R: AsyncRead + Unpin>(
    buf: &mut Vec<u8>,
    mut reader: &mut R,
    read_len: &mut usize,
    cx: &mut Context<'_>,
) -> Poll<io::Result<usize>> {
    loop {
        // Allocate 32 bytes every time, if the remaining capacity is larger than 32 bytes,
        // this will do nothing.
        buf.reserve(32);
        let len = buf.len();
        let mut read_buf = ReadBuf::uninit(unsafe {
            from_raw_parts_mut(buf.as_mut_ptr() as *mut MaybeUninit<u8>, buf.capacity())
        });
        read_buf.assume_init(len);
        read_buf.set_filled(len);

        let poll = Pin::new(&mut reader).poll_read(cx, &mut read_buf);
        let new_len = read_buf.filled_len();
        match poll {
            Poll::Pending => {
                return Poll::Pending;
            }
            Poll::Ready(Ok(())) if (new_len - len) == 0 => {
                return Poll::Ready(Ok(mem::replace(read_len, 0)))
            }
            Poll::Ready(Ok(())) => {
                *read_len += new_len - len;
                unsafe {
                    buf.set_len(new_len);
                }
            }
            Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
        }
    }
}

impl<'a, R> Future for ReadToEndTask<'a, R>
where
    R: AsyncRead + Unpin,
{
    type Output = io::Result<usize>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.get_mut();
        let (buf, reader, read_len) = (&mut me.buf, &mut me.reader, &mut me.r_len);
        poll_read_to_end(buf, *reader, read_len, cx)
    }
}

/// A future for reading every data from the source into a String.
///
/// Returned by [`crate::io::AsyncReadExt::read_to_string`]
pub struct ReadToStringTask<'a, R: ?Sized> {
    reader: &'a mut R,
    buf: Vec<u8>,
    output: &'a mut String,
    r_len: usize,
}

impl<'a, R: ?Sized> ReadToStringTask<'a, R> {
    #[inline(always)]
    pub(crate) fn new(reader: &'a mut R, dst: &'a mut String) -> ReadToStringTask<'a, R> {
        ReadToStringTask {
            reader,
            buf: mem::take(dst).into_bytes(),
            output: dst,
            r_len: 0,
        }
    }
}

fn io_string_result(
    io_res: io::Result<usize>,
    str_res: Result<String, FromUtf8Error>,
    read_len: usize,
    output: &mut String,
) -> Poll<io::Result<usize>> {
    match (io_res, str_res) {
        (Ok(bytes), Ok(string)) => {
            *output = string;
            Poll::Ready(Ok(bytes))
        }
        (Ok(bytes), Err(trans_err)) => {
            let mut vector = trans_err.into_bytes();
            let len = vector.len() - bytes;
            vector.truncate(len);
            *output = String::from_utf8(vector).expect("Invalid utf-8 data");
            Poll::Ready(Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid utf-8 data",
            )))
        }
        (Err(io_err), Ok(string)) => {
            *output = string;
            Poll::Ready(Err(io_err))
        }
        (Err(io_err), Err(trans_err)) => {
            let mut vector = trans_err.into_bytes();
            let len = vector.len() - read_len;
            vector.truncate(len);
            *output = String::from_utf8(vector).expect("Invalid utf-8 data");
            Poll::Ready(Err(io_err))
        }
    }
}

impl<'a, R> Future for ReadToStringTask<'a, R>
where
    R: AsyncRead + Unpin,
{
    type Output = io::Result<usize>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.get_mut();
        let (buf, output, reader, read_len) =
            (&mut me.buf, &mut me.output, &mut me.reader, &mut me.r_len);
        let res = poll_ready!(poll_read_to_end(buf, *reader, read_len, cx));
        let trans = String::from_utf8(mem::take(buf));

        io_string_result(res, trans, *read_len, output)
    }
}

/// A future for reading exact amount of bytes from the source into a vector.
///
/// Returned by [`crate::io::AsyncReadExt::read_exact`]
pub struct ReadExactTask<'a, R: ?Sized> {
    reader: Option<&'a mut R>,
    buf: &'a mut [u8],
    r_len: usize,
}

impl<'a, R: ?Sized> ReadExactTask<'a, R> {
    #[inline(always)]
    pub(crate) fn new(reader: &'a mut R, buf: &'a mut [u8]) -> ReadExactTask<'a, R> {
        ReadExactTask {
            reader: Some(reader),
            buf,
            r_len: 0,
        }
    }
}

impl<'a, R> Future for ReadExactTask<'a, R>
where
    R: AsyncRead + Unpin,
{
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let r_len = self.r_len;
        let mut reader = take_reader!(self);
        let mut read_buf = ReadBuf::new(self.buf);
        read_buf.set_filled(r_len);

        loop {
            let remain = read_buf.remaining();
            if remain == 0 {
                return Poll::Ready(Ok(()));
            }
            match Pin::new(&mut reader).poll_read(cx, &mut read_buf) {
                Poll::Pending => {
                    self.r_len = read_buf.filled_len();
                    self.reader = Some(reader);
                    return Poll::Pending;
                }
                // Reach eof before filling the entire buf, return unexpected_eof
                Poll::Ready(Ok(())) if read_buf.remaining() == remain => {
                    return Poll::Ready(Err(io::ErrorKind::UnexpectedEof.into()))
                }
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                _ => {}
            }
        }
    }
}

/// A future for reading every data from the source into a vector until the
/// desired delimiter appears.
///
/// Returned by [`crate::io::AsyncBufReadExt::read_until`]
pub struct ReadUtilTask<'a, R: ?Sized> {
    reader: &'a mut R,
    r_len: usize,
    delim: u8,
    buf: &'a mut Vec<u8>,
}

impl<'a, R: ?Sized> ReadUtilTask<'a, R> {
    #[inline(always)]
    pub(crate) fn new(reader: &'a mut R, delim: u8, buf: &'a mut Vec<u8>) -> ReadUtilTask<'a, R> {
        ReadUtilTask {
            reader,
            r_len: 0,
            delim,
            buf,
        }
    }
}

fn poll_read_until<R: AsyncBufRead + Unpin>(
    buf: &mut Vec<u8>,
    mut reader: &mut R,
    delim: u8,
    read_len: &mut usize,
    cx: &mut Context<'_>,
) -> Poll<io::Result<usize>> {
    loop {
        let (done, used) = {
            let available = poll_ready!(Pin::new(&mut reader).poll_fill_buf(cx))?;

            let ret = available.iter().position(|&val| val == delim);

            match ret {
                None => {
                    buf.extend_from_slice(available);
                    (false, available.len())
                }
                Some(i) => {
                    buf.extend_from_slice(&available[..=i]);
                    (true, i + 1)
                }
            }
        };
        Pin::new(&mut reader).consume(used);
        *read_len += used;
        if done || used == 0 {
            return Poll::Ready(Ok(mem::replace(read_len, 0)));
        }
    }
}

impl<'a, R> Future for ReadUtilTask<'a, R>
where
    R: AsyncBufRead + Unpin,
{
    type Output = io::Result<usize>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.get_mut();
        let (buf, reader, delim, read_len) = (&mut me.buf, &mut me.reader, me.delim, &mut me.r_len);
        poll_read_until(buf, *reader, delim, read_len, cx)
    }
}

/// A future for reading every data from the source into a vector until the
/// desired delimiter appears.
///
/// Returned by [`crate::io::AsyncBufReadExt::read_until`]
pub struct ReadLineTask<'a, R: ?Sized> {
    reader: &'a mut R,
    r_len: usize,
    buf: Vec<u8>,
    output: &'a mut String,
}

impl<'a, R: ?Sized> ReadLineTask<'a, R> {
    #[inline(always)]
    pub(crate) fn new(reader: &'a mut R, buf: &'a mut String) -> ReadLineTask<'a, R> {
        ReadLineTask {
            reader,
            r_len: 0,
            buf: mem::take(buf).into_bytes(),
            output: buf,
        }
    }
}

impl<'a, R> Future for ReadLineTask<'a, R>
where
    R: AsyncBufRead + Unpin,
{
    type Output = io::Result<usize>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.get_mut();
        let (buf, output, reader, read_len) =
            (&mut me.buf, &mut me.output, &mut me.reader, &mut me.r_len);
        let res = poll_ready!(poll_read_until(buf, *reader, b'\n', read_len, cx));
        let trans = String::from_utf8(mem::take(buf));

        io_string_result(res, trans, *read_len, output)
    }
}

/// A future for reading every data from the source into a vector and splitting it
/// into segments by a delimiter.
///
/// Returned by [`crate::io::AsyncBufReadExt::split`]
pub struct SplitTask<R> {
    reader: R,
    delim: u8,
    buf: Vec<u8>,
    r_len: usize,
}

impl<R> SplitTask<R>
where
    R: AsyncBufRead + Unpin,
{
    pub(crate) fn new(reader: R, delim: u8) -> SplitTask<R> {
        SplitTask {
            reader,
            delim,
            buf: Vec::new(),
            r_len: 0,
        }
    }

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<Option<Vec<u8>>>> {
        let me = self.get_mut();
        let (buf, reader, read_len, delim) = (&mut me.buf, &mut me.reader, &mut me.r_len, me.delim);
        let res = poll_ready!(poll_read_until(buf, reader, delim, read_len, cx))?;

        if buf.is_empty() && res == 0 {
            return Poll::Ready(Ok(None));
        }

        if buf.last() == Some(&delim) {
            buf.pop();
        }
        Poll::Ready(Ok(Some(mem::take(buf))))
    }

    pub async fn next(&mut self) -> io::Result<Option<Vec<u8>>> {
        poll_fn(|cx| Pin::new(&mut *self).poll_next(cx)).await
    }
}

/// A future for reading every data from the source into a vector and splitting it
/// into segments by row.
///
/// Returned by [`crate::io::AsyncBufReadExt::split`]
pub struct LinesTask<R> {
    reader: R,
    buf: Vec<u8>,
    output: String,
    r_len: usize,
}

impl<R> LinesTask<R>
where
    R: AsyncBufRead,
{
    pub(crate) fn new(reader: R) -> LinesTask<R> {
        LinesTask {
            reader,
            buf: Vec::new(),
            output: String::new(),
            r_len: 0,
        }
    }
}

impl<R> LinesTask<R>
where
    R: AsyncBufRead + Unpin,
{
    fn poll_next_line(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<Option<String>>> {
        let me = self.get_mut();
        let (buf, output, reader, read_len) =
            (&mut me.buf, &mut me.output, &mut me.reader, &mut me.r_len);
        let io_res = poll_ready!(poll_read_until(buf, reader, b'\n', read_len, cx));
        let str_res = String::from_utf8(mem::take(buf));

        let res = poll_ready!(io_string_result(io_res, str_res, *read_len, output))?;

        if output.is_empty() && res == 0 {
            return Poll::Ready(Ok(None));
        }

        if output.ends_with('\n') {
            output.pop();
            if output.ends_with('\r') {
                output.pop();
            }
        }
        Poll::Ready(Ok(Some(mem::take(output))))
    }

    pub async fn next_line(&mut self) -> io::Result<Option<String>> {
        poll_fn(|cx| Pin::new(&mut *self).poll_next_line(cx)).await
    }
}
