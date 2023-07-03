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

use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use crate::macros::cfg_io;

cfg_io! {
    mod tcp;
    pub use tcp::{TcpListener, TcpStream};
    mod udp;
    pub use udp::{UdpSocket, ConnectedUdpSocket};
    use crate::io::{AsyncReadExt, AsyncWriteExt};
    use crate::io::{AsyncRead, ReadBuf, AsyncWrite};
    use std::pin::Pin;
    use std::task::{Context, Poll};
    use std::io::IoSlice;
    use std::net::Shutdown;

    /// Struct of Stream with TcpStream inner
    pub struct Stream {
        inner: TcpStream,
    }

    impl Stream {
        /// Opens a TCP connection to a remote host asynchronously.
        pub async fn connect(addr: SocketAddr) -> io::Result<Self> {
            Ok(Self {
                inner: TcpStream::connect(addr).await?,
            })
        }

        /// Writes data from the buffer into the I/O source.
        pub async fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.inner.write(buf).await
        }

        /// Reads data from the I/O source into the buffer.
        pub async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.inner.read(buf).await
        }

        /// Splits a TcpStream into a read half and a write half,
        /// which can be used to read and write the stream concurrently.
        pub fn into_split(self) -> (SplitReadHalf, SplitWriteHalf) {
            let arc = Arc::new(self);
            let read = SplitReadHalf(Arc::clone(&arc));
            let write = SplitWriteHalf(Arc::clone(&arc));
            (read, write)
        }
    }

    /// Struct of Listener with TcpListener inner
    pub struct Listener {
        inner: TcpListener,
    }

    impl Listener {
        /// A TCP socket server, asynchronously listening for connections.
        pub async fn bind(addr: SocketAddr) -> io::Result<Self> {
            Ok(Self {
                inner: TcpListener::bind(addr).await?
            })
        }

        /// Asynchronously accepts a new incoming connection from this listener.
        pub async fn accept(&self) -> io::Result<(Stream, SocketAddr)> {
            let (inner, addr) = self.inner.accept().await?;
            Ok((Stream { inner }, addr))
        }
    }

    /// Read half of a TcpStream
    pub struct SplitReadHalf(pub(crate) Arc<Stream>);

    /// Write half of a TcpStream
    pub struct SplitWriteHalf(pub(crate) Arc<Stream>);

    impl AsyncRead for SplitReadHalf {
        fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<std::io::Result<()>> {
            self.0.inner.source.poll_read(cx, buf)
        }
    }

    impl AsyncWrite for SplitWriteHalf {
        fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<std::io::Result<usize>> {
            self.0.inner.source.poll_write(cx, buf)
        }

        fn poll_write_vectored(self: Pin<&mut Self>, cx: &mut Context<'_>, bufs: &[IoSlice<'_>]) -> Poll<std::io::Result<usize>> {
            self.0.inner.source.poll_write_vectored(cx, bufs)
        }

        fn is_write_vectored(&self) -> bool {
            self.0.inner.is_write_vectored()
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            self.0.inner.shutdown(Shutdown::Write).into()
        }
    }
}
