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

extern crate core;

use ylong_runtime::io::{AsyncBufReader, AsyncReadExt, AsyncWriteExt};
use ylong_runtime::net::{TcpListener, TcpStream};

/// SDV for &[u8]::poll_read.
///
/// # Brief
/// 1. use global runtime to spawn a task
/// 2. construct a buf and a slice, buf's length is greater than the slice
/// 3. call AsyncReadExt::read on them, check the returns are correct
/// 4. construct another buf and slice, buf's length is smaller than the slice
/// 5. call AsyncReadExt::read on them, check the returns are correct
#[test]
fn sdv_async_read_slice() {
    ylong_runtime::block_on(async move {
        let mut buf = [0; 5];

        let slice = vec![1u8, 2, 3];
        let res = AsyncReadExt::read(&mut slice.as_slice(), &mut buf)
            .await
            .unwrap();
        assert_eq!(res, 3);
        assert_eq!(buf, [1, 2, 3, 0, 0]);
        assert_eq!(slice, vec![1, 2, 3]);

        let mut buf = [0; 2];
        let slice = vec![1u8, 2, 3, 4, 5];
        let res = AsyncReadExt::read(&mut slice.as_slice(), &mut buf)
            .await
            .unwrap();
        assert_eq!(res, 2);
        assert_eq!(buf, [1, 2]);
        assert_eq!(slice, vec![1, 2, 3, 4, 5]);
    });
}

/// SDV for AsyncBufReader::read_line.
///
/// # Brief
/// 1. Establish an asynchronous tcp connection.
/// 2. The client sends some data to the server. The message is valid UTF-8.
/// 3. The server wraps the TcpStream inside a AsyncBufReader
/// 4. The server calls `read_to_string` on the string "Hello".
/// 5. Check the read buf.
#[test]
fn sdv_buf_reader_read_to_string() {
    let server = ylong_runtime::spawn(async move {
        let addr = "127.0.0.1:8186".parse().unwrap();
        let tcp = TcpListener::bind(addr).await.unwrap();
        let (stream, _) = tcp.accept().await.unwrap();
        let mut buf_reader = AsyncBufReader::new(stream);
        let mut res = String::from("Hello");
        let ret = buf_reader.read_to_string(&mut res).await.unwrap();
        assert_eq!(ret, 5);
        assert_eq!(res.as_str(), "HelloWorld");
    });

    let client = ylong_runtime::spawn(async move {
        let addr = "127.0.0.1:8186".parse().unwrap();
        let mut tcp = TcpStream::connect(addr).await;
        while tcp.is_err() {
            tcp = TcpStream::connect(addr).await;
        }
        let mut tcp = tcp.unwrap();
        let buf = [87, 111, 114, 108, 100];
        tcp.write_all(&buf).await.unwrap();
    });

    ylong_runtime::block_on(server).unwrap();
    ylong_runtime::block_on(client).unwrap();
}
