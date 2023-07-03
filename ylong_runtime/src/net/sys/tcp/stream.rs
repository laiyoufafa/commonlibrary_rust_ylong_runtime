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

use crate::net::AsyncSource;
use ylong_io::Interest;
use std::io;
use std::io::{IoSlice};
use std::net::{Shutdown, SocketAddr};
use crate::io::{AsyncRead, ReadBuf, AsyncWrite};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::fmt::{Debug, Formatter};

/// An asynchronous version of [`std::net::TcpStream`]
///
/// After creating a `TcpStream` by either connecting to a remote host or accepting a
/// connection on a `TcpListener`, data can be transmitted asynchronously
/// by reading and writing to it.
///
///
/// # Example
/// ```rust
/// use ylong_runtime::net::TcpStream;
/// use std::io::IoSlice;
/// use std::io::IoSliceMut;
/// use std::io;
/// use ylong_runtime::io::{AsyncReadExt, AsyncWriteExt};
///
/// async fn io_func() -> io::Result<()> {
///     let addr = "127.0.0.1:8080".parse().unwrap();
///     let mut stream = TcpStream::connect(addr).await?;
///
///     let _ = stream.write(b"hello client").await?;
///     let _ = stream.write_vectored(&[IoSlice::new(b"hello client")]).await?;
///
///     let mut read_buf = [0 as u8; 1024];
///     let _ = stream.read(&mut read_buf).await?;
///     let _ = stream.read(&mut read_buf).await?;
///     Ok(())
/// }
/// ```
pub struct TcpStream {
    pub(crate) source: AsyncSource<ylong_io::TcpStream>,
}

impl Debug for TcpStream {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.source.fmt(f)
    }
}

impl TcpStream {
    /// Opens a TCP connection to a remote host asynchronously.
    ///
    /// # Example
    /// ```rust
    /// use ylong_runtime::net::TcpStream;
    /// use std::io;
    ///
    /// async fn io_func() -> io::Result<()> {
    ///     let addr = "127.0.0.1:8080".parse().unwrap();
    ///     let mut stream = TcpStream::connect(addr).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn connect(addr: SocketAddr) -> io::Result<Self> {
        let stream = TcpStream::new(ylong_io::TcpStream::connect(addr)?)?;
        stream
            .source
            .async_process(
                // Wait until the stream is writable
                Interest::WRITABLE,
                || Ok(())
            )
            .await?;

        if let Some(e) = stream.source.take_error()? {
            return Err(e);
        }
        Ok(stream)
    }

    // Registers the ylong_io::TcpStream's fd to the reactor, and returns async TcpStream.
    pub(crate) fn new(stream: ylong_io::TcpStream) -> io::Result<Self> {
        let source = AsyncSource::new(stream, None)?;
        Ok(TcpStream { source })
    }

    // todo: make this async
    /// Shutdown TcpStream
    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.source.shutdown(how)
    }
}

impl AsyncRead for TcpStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>
    ) -> Poll<io::Result<()>> {
        self.source.poll_read(cx, buf)
    }
}

impl AsyncWrite for TcpStream {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        self.source.poll_write(cx, buf)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        self.source.poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        true
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.source.shutdown(std::net::Shutdown::Write)?;
        Poll::Ready(Ok(()))
    }
}
