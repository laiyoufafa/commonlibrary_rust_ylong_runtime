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

//! Tcp async benchmark

use std::net::SocketAddr;
use std::time::Instant;
use ylong_runtime::builder::RuntimeBuilder;
use ylong_runtime::io::{AsyncReadExt, AsyncWriteExt};
use ylong_runtime::net::{TcpListener, TcpStream};

async fn run_client(addr: &str) {
    let socket_addr: SocketAddr = addr.parse().unwrap();
    let mut client_stream = TcpStream::connect(socket_addr).await;
    while client_stream.is_err() {
        client_stream = TcpStream::connect(socket_addr).await
    }
    let mut client = client_stream.unwrap();
    for _ in 0..100 {
        let mut buf = [1; 16384];
        let n = client.write(&buf).await.unwrap();
        assert_eq!(n, 16384);

        let n = client.read(&mut buf).await.unwrap();
        assert_eq!(n, 16384)
    }
}

async fn run_server(addr: &str) {
    let socket_addr: SocketAddr = addr.parse().unwrap();
    let socket = TcpListener::bind(socket_addr).await.unwrap();
    let mut server = socket.accept().await.unwrap().0;

    for _ in 0..100 {
        let mut buf = [0; 16384];
        let n = server.read(&mut buf).await.unwrap();
        assert_eq!(n, 16384);

        let buf = [2; 16384];
        let n = server.write(&buf).await.unwrap();
        assert_eq!(n, 16384);
    }
}

fn main() {
    let runtime = RuntimeBuilder::new_multi_thread()
        .max_blocking_pool_size(5)
        .build()
        .unwrap();

    let st = Instant::now();
    for _ in 0..100 {
        let handle1 = runtime.spawn(async move {
            run_client("127.0.0.1:2000").await;
        });

        let handle2 = runtime.spawn(async move {
            run_server("127.0.0.1:2000").await;
        });

        let handle3 = runtime.spawn(async move {
            run_client("127.0.0.1:2001").await;
        });

        let handle4 = runtime.spawn(async move {
            run_server("127.0.0.1:2001").await;
        });

        let handle5 = runtime.spawn(async move {
            run_client("127.0.0.1:2002").await;
        });

        let handle6 = runtime.spawn(async move {
            run_server("127.0.0.1:2002").await;
        });

        let handle7 = runtime.spawn(async move {
            run_client("127.0.0.1:2003").await;
        });

        let handle8 = runtime.spawn(async move {
            run_server("127.0.0.1:2003").await;
        });

        let handle9 = runtime.spawn(async move {
            run_client("127.0.0.1:2004").await;
        });

        let handle10 = runtime.spawn(async move {
            run_server("127.0.0.1:2004").await;
        });

        let _ = runtime.block_on(handle1);
        let _ = runtime.block_on(handle2);
        let _ = runtime.block_on(handle3);
        let _ = runtime.block_on(handle4);
        let _ = runtime.block_on(handle5);
        let _ = runtime.block_on(handle6);
        let _ = runtime.block_on(handle7);
        let _ = runtime.block_on(handle8);
        let _ = runtime.block_on(handle9);
        let _ = runtime.block_on(handle10);
    }
    let time = st.elapsed().as_secs_f64();
    println!("time: {time:?}");
}
