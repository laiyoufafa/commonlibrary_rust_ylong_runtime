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

//! An example for `tcp`
use std::net::SocketAddr;
use ylong_runtime::io::{AsyncReadExt, AsyncWriteExt};
use ylong_runtime::net::{TcpListener, TcpStream};

const BUF_SIZE: usize = 10000;
const LOOP_NUM: usize = 1000;
const TASK_NUM: usize = 120;

async fn ylong_tcp_server(addr: SocketAddr) {
    let tcp = TcpListener::bind(addr).await.unwrap();
    let (mut stream, _) = tcp.accept().await.unwrap();
    for _ in 0..LOOP_NUM {
        let mut buf = [0; BUF_SIZE];
        stream.read_exact(&mut buf).await.unwrap();
        assert_eq!(buf, [3; BUF_SIZE]);

        let buf = [2; BUF_SIZE];
        stream.write_all(&buf).await.unwrap();
    }
}

async fn ylong_tcp_client(addr: SocketAddr) {
    let mut tcp = TcpStream::connect(addr).await;
    while tcp.is_err() {
        tcp = TcpStream::connect(addr).await;
    }
    let mut tcp = tcp.unwrap();
    for _ in 0..LOOP_NUM {
        let buf = [3; BUF_SIZE];
        tcp.write_all(&buf).await.unwrap();

        let mut buf = [0; BUF_SIZE];
        tcp.read_exact(&mut buf).await.unwrap();
        assert_eq!(buf, [2; BUF_SIZE]);
    }
}

fn ylong_tcp_send_recv() {
    let basic_addr = "127.0.0.1:";
    let port = 8181;
    let mut handlers = Vec::new();
    for i in 0..TASK_NUM {
        let addr = (basic_addr.to_owned() + &*(port + i).to_string())
            .parse()
            .unwrap();
        handlers.push(ylong_runtime::spawn(ylong_tcp_server(addr)));
        handlers.push(ylong_runtime::spawn(ylong_tcp_client(addr)));
    }
    for handler in handlers {
        ylong_runtime::block_on(handler).unwrap();
    }
}

fn main() {
    ylong_tcp_send_recv();
}
