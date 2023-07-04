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
use std::io::SeekFrom;
use ylong_runtime::fs::File;
use ylong_runtime::io::{
    AsyncBufReadExt, AsyncBufReader, AsyncReadExt, AsyncSeekExt, AsyncWriteExt,
};
use ylong_runtime::net::{TcpListener, TcpStream};

/// SDV test for AsyncBufReader `read_util`
///
/// # Brief
/// 1. Establish an asynchronous tcp connection.
/// 2. The client sends some data to the server. The message contains a `:` character.
/// 3. The server wraps the TcpStream inside a AsyncBufReader
/// 4. The server calls `read_until` with a delimiter ':'.
/// 5. Check the read buf.
/// 6. The server calls `read`.
/// 7. Check the read buf.
#[test]
fn sdv_buf_reader_read_until() {
    let server = ylong_runtime::spawn(async move {
        let addr = "127.0.0.1:8180".parse().unwrap();
        let tcp = TcpListener::bind(addr).await.unwrap();
        let (stream, _) = tcp.accept().await.unwrap();
        let mut buf_reader = AsyncBufReader::new(stream);
        let mut res = vec![];
        let ret = buf_reader.read_until(b':', &mut res).await.unwrap();
        assert_eq!(ret, 4);
        assert_eq!(res.as_slice(), &[1, 2, 3, b':']);

        let mut buf = [0; 4];
        let ret = buf_reader.read(&mut buf).await.unwrap();
        assert_eq!(ret, 3);
        assert_eq!(buf, [4, 5, 6, 0]);
    });

    let client = ylong_runtime::spawn(async move {
        let addr = "127.0.0.1:8180".parse().unwrap();
        let mut tcp = TcpStream::connect(addr).await;
        while tcp.is_err() {
            tcp = TcpStream::connect(addr).await;
        }
        let mut tcp = tcp.unwrap();
        let buf = [1, 2, 3, b':', 4, 5, 6];
        tcp.write_all(&buf).await.unwrap();
    });

    ylong_runtime::block_on(server).unwrap();
    ylong_runtime::block_on(client).unwrap();
}

/// SDV test for AsyncBufReader `read_line`
///
/// # Brief
/// 1. Establish an asynchronous tcp connection.
/// 2. The client sends some data to the server. The message contains the `\n` byte.
/// 3. The server wraps the TcpStream inside a AsyncBufReader
/// 4. The server calls `read_line` with a delimiter '\n'.
/// 5. Check the read buf.
/// 6. The server calls `read_line` again.
/// 7. Check the read buf.
#[test]
fn sdv_buf_reader_read_line() {
    let server = ylong_runtime::spawn(async move {
        let addr = "127.0.0.1:8181".parse().unwrap();
        let tcp = TcpListener::bind(addr).await.unwrap();
        let (stream, _) = tcp.accept().await.unwrap();
        let mut buf_reader = AsyncBufReader::new(stream);
        let mut res = String::from("Hello");
        let ret = buf_reader.read_line(&mut res).await.unwrap();
        assert_eq!(ret, 6);
        assert_eq!(res.as_str(), "HelloWorld\n");
        let ret = buf_reader.read_line(&mut res).await.unwrap();
        assert_eq!(ret, 4);
        assert_eq!(res.as_str(), "HelloWorld\nline");
    });

    let client = ylong_runtime::spawn(async move {
        let addr = "127.0.0.1:8181".parse().unwrap();
        let mut tcp = TcpStream::connect(addr).await;
        while tcp.is_err() {
            tcp = TcpStream::connect(addr).await;
        }
        let mut tcp = tcp.unwrap();
        let buf = "World\nline".as_bytes();
        tcp.write_all(buf).await.unwrap();
    });

    ylong_runtime::block_on(server).unwrap();
    ylong_runtime::block_on(client).unwrap();
}

/// SDV test for AsyncBufReader `split`
///
/// # Brief
/// 1. Establish an asynchronous tcp connection.
/// 2. The client sends some data to the server. The message contains a `-` character.
/// 3. The server wraps the TcpStream inside a AsyncBufReader
/// 4. The server calls `split` to get segments and calls `next` with a delimiter
///    '-' for several times.
/// 5. Check the read buf.
#[test]
fn sdv_buf_reader_split() {
    let server = ylong_runtime::spawn(async move {
        let addr = "127.0.0.1:8182".parse().unwrap();
        let tcp = TcpListener::bind(addr).await.unwrap();
        let (stream, _) = tcp.accept().await.unwrap();
        let buf_reader = AsyncBufReader::new(stream);
        let mut segments = buf_reader.split(b'-');
        assert_eq!(segments.next().await.unwrap(), Some(b"lorem".to_vec()));
        assert_eq!(segments.next().await.unwrap(), Some(b"ipsum".to_vec()));
        assert_eq!(segments.next().await.unwrap(), Some(b"dolor".to_vec()));
        assert_eq!(segments.next().await.unwrap(), None);
    });

    let client = ylong_runtime::spawn(async move {
        let addr = "127.0.0.1:8182".parse().unwrap();
        let mut tcp = TcpStream::connect(addr).await;
        while tcp.is_err() {
            tcp = TcpStream::connect(addr).await;
        }
        let mut tcp = tcp.unwrap();
        let buf = "lorem-ipsum-dolor".as_bytes();
        tcp.write_all(buf).await.unwrap();
    });

    ylong_runtime::block_on(server).unwrap();
    ylong_runtime::block_on(client).unwrap();
}

/// SDV test for AsyncBufReader `lines`
///
/// # Brief
/// 1. Establish an asynchronous tcp connection.
/// 2. The client sends some data to the server. The message contains the `\n` byte.
/// 3. The server wraps the TcpStream inside a AsyncBufReader
/// 4. The server calls `lines` to get segments and calls `next_line` with a delimiter
///    '\n' for several times.
/// 5. Check the read buf.
#[test]
fn sdv_buf_reader_lines() {
    let server = ylong_runtime::spawn(async move {
        let addr = "127.0.0.1:8183".parse().unwrap();
        let tcp = TcpListener::bind(addr).await.unwrap();
        let (stream, _) = tcp.accept().await.unwrap();
        let buf_reader = AsyncBufReader::new(stream);
        let mut segments = buf_reader.lines();
        assert_eq!(
            segments.next_line().await.unwrap(),
            Some(String::from("lorem"))
        );
        assert_eq!(
            segments.next_line().await.unwrap(),
            Some(String::from("ipsum"))
        );
        assert_eq!(
            segments.next_line().await.unwrap(),
            Some(String::from("dolor"))
        );
        assert_eq!(segments.next_line().await.unwrap(), None);
    });

    let client = ylong_runtime::spawn(async move {
        let addr = "127.0.0.1:8183".parse().unwrap();
        let mut tcp = TcpStream::connect(addr).await;
        while tcp.is_err() {
            tcp = TcpStream::connect(addr).await;
        }
        let mut tcp = tcp.unwrap();
        let buf = "lorem\r\nipsum\ndolor".as_bytes();
        tcp.write_all(buf).await.unwrap();
    });

    ylong_runtime::block_on(server).unwrap();
    ylong_runtime::block_on(client).unwrap();
}

/// SDV test for AsyncBufReader `seek`
///
/// # Brief
/// 1. Create a file and write data.
/// 2. Open the file and call `read_until` with a delimiter '-'.
/// 3. Seek to three different positions in the file and read data.
/// 5. Check the read buf.
#[test]
fn sdv_buf_reader_seek() {
    let handle = ylong_runtime::spawn(async move {
        let mut file = File::create("./tests/buf_reader_seek_file").await.unwrap();
        let buf = "lorem-ipsum-dolor".as_bytes();
        let res = file.write(buf).await.unwrap();
        assert_eq!(res, 17);
    });
    ylong_runtime::block_on(handle).unwrap();

    let handle1 = ylong_runtime::spawn(async move {
        let file = File::open("./tests/buf_reader_seek_file").await.unwrap();
        let mut buf_reader = AsyncBufReader::new(file);
        let mut res = vec![];
        let ret = buf_reader.read_until(b'-', &mut res).await.unwrap();
        assert_eq!(ret, 6);
        assert_eq!(res, "lorem-".as_bytes());

        let seek = buf_reader.seek(SeekFrom::Current(5)).await.unwrap();
        let mut buf = [0; 6];
        let ret = buf_reader.read(&mut buf).await.unwrap();
        assert_eq!(seek, 11);
        assert_eq!(ret, 6);
        assert_eq!(buf, "-dolor".as_bytes());

        let seek = buf_reader.seek(SeekFrom::Start(5)).await.unwrap();
        let mut buf = [0; 12];
        let ret = buf_reader.read(&mut buf).await.unwrap();
        assert_eq!(seek, 5);
        assert_eq!(ret, 12);
        assert_eq!(buf, "-ipsum-dolor".as_bytes());

        let seek = buf_reader.seek(SeekFrom::End(-5)).await.unwrap();
        let mut buf = [0; 5];
        let ret = buf_reader.read(&mut buf).await.unwrap();
        assert_eq!(seek, 12);
        assert_eq!(ret, 5);
        assert_eq!(buf, "dolor".as_bytes());
    });
    ylong_runtime::block_on(handle1).unwrap();
    assert!(fs::remove_file("./tests/buf_reader_seek_file").is_ok());
}
