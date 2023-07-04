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

use std::fs;
use std::io::{IoSlice, SeekFrom};
use ylong_runtime::fs::File;
use ylong_runtime::io::{AsyncBufWriter, AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use ylong_runtime::net::{TcpListener, TcpStream};

/// SDV test for AsyncBufWriter `write`
///
/// # Brief
/// 1. Establish an asynchronous tcp connection.
/// 2. The client wraps the TcpStream inside a AsyncBufWriter and calls `write` to send some data.
/// 4. The server receives data.
/// 5. Check the read buf.
#[test]
fn sdv_buf_writer_write() {
    let server = ylong_runtime::spawn(async move {
        let addr = "127.0.0.1:8184".parse().unwrap();
        let tcp = TcpListener::bind(addr).await.unwrap();
        let (mut stream, _) = tcp.accept().await.unwrap();

        let mut buf = [0; 6];
        let ret = stream.read(&mut buf).await.unwrap();
        assert_eq!(ret, 6);
        assert_eq!(buf, [1, 2, 3, 4, 5, 6]);
    });

    let client = ylong_runtime::spawn(async move {
        let addr = "127.0.0.1:8184".parse().unwrap();
        let mut tcp = TcpStream::connect(addr).await;
        while tcp.is_err() {
            tcp = TcpStream::connect(addr).await;
        }
        let tcp = tcp.unwrap();
        let buf = [1, 2, 3, 4, 5, 6];

        let mut buf_writer = AsyncBufWriter::with_capacity(10, tcp);
        buf_writer.write(&buf).await.unwrap();
        buf_writer.flush().await.unwrap();
    });

    ylong_runtime::block_on(server).unwrap();
    ylong_runtime::block_on(client).unwrap();
}

/// SDV test for AsyncBufWriter `write_vectored`
///
/// # Brief
/// 1. Establish an asynchronous tcp connection.
/// 2. The client wraps the TcpStream inside a AsyncBufWriter and calls `write_vectored`
///    to send segmented data.
/// 4. The server receives data.
/// 5. Check the read buf.
#[test]
fn sdv_buf_writer_write_vectored() {
    let server = ylong_runtime::spawn(async move {
        let addr = "127.0.0.1:8185".parse().unwrap();
        let tcp = TcpListener::bind(addr).await.unwrap();
        let (mut stream, _) = tcp.accept().await.unwrap();

        let mut buf = [0; 17];
        let ret = stream.read(&mut buf).await.unwrap();
        assert_eq!(ret, 17);
        assert_eq!(buf, "lorem-ipsum-dolor".as_bytes());
    });

    let client = ylong_runtime::spawn(async move {
        let addr = "127.0.0.1:8185".parse().unwrap();
        let mut tcp = TcpStream::connect(addr).await;
        while tcp.is_err() {
            tcp = TcpStream::connect(addr).await;
        }
        let tcp = tcp.unwrap();
        let buf1 = "lorem-".as_bytes();
        let buf2 = "ipsum-".as_bytes();
        let buf3 = "dolor".as_bytes();
        let bufs = &mut [IoSlice::new(buf1), IoSlice::new(buf2), IoSlice::new(buf3)][..];

        let mut buf_writer = AsyncBufWriter::with_capacity(3, tcp);
        buf_writer.write_vectored(bufs).await.unwrap();
        buf_writer.flush().await.unwrap();
    });

    ylong_runtime::block_on(server).unwrap();
    ylong_runtime::block_on(client).unwrap();
}

/// SDV test for AsyncBufWriter `seek`
///
/// # Brief
/// 1. Create a file and write data.
/// 2. Open the file, seek to three different positions in the file and read data.
/// 5. Check the read buf.
#[test]
fn sdv_buf_writer_seek() {
    let handle = ylong_runtime::spawn(async move {
        let mut file = File::create("./tests/buf_writer_seek_file").await.unwrap();
        let buf = "lorem-ipsum-dolor".as_bytes();
        let res = file.write(buf).await.unwrap();
        assert_eq!(res, 17);
    });
    ylong_runtime::block_on(handle).unwrap();

    let handle1 = ylong_runtime::spawn(async move {
        let file = File::open("./tests/buf_writer_seek_file").await.unwrap();
        let mut buf_writer = AsyncBufWriter::new(file);

        let seek = buf_writer.seek(SeekFrom::Start(5)).await.unwrap();
        let mut buf = [0; 12];
        let ret = buf_writer.read(&mut buf).await.unwrap();
        assert_eq!(seek, 5);
        assert_eq!(ret, 12);
        assert_eq!(buf, "-ipsum-dolor".as_bytes());

        buf_writer.seek(SeekFrom::Start(5)).await.unwrap();
        let seek = buf_writer.seek(SeekFrom::Current(7)).await.unwrap();
        let mut buf = [0; 5];
        let ret = buf_writer.read(&mut buf).await.unwrap();
        assert_eq!(seek, 12);
        assert_eq!(ret, 5);
        assert_eq!(buf, "dolor".as_bytes());

        let seek = buf_writer.seek(SeekFrom::End(-5)).await.unwrap();
        let mut buf = [0; 5];
        let ret = buf_writer.read(&mut buf).await.unwrap();
        assert_eq!(seek, 12);
        assert_eq!(ret, 5);
        assert_eq!(buf, "dolor".as_bytes());
    });
    ylong_runtime::block_on(handle1).unwrap();
    assert!(fs::remove_file("./tests/buf_writer_seek_file").is_ok());
}
