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

//! Benchmarks for the tcp.

#![feature(test)]

extern crate core;

mod task_helpers;

#[macro_export]
macro_rules! tokio_tcp_task {
    ($runtime: expr, $bench: ident, $server: ident, $client: ident, $port: literal, $task_num: literal, $loop_num: literal, $buf_size: literal) => {
        pub async fn $server(addr: String) {
            let tcp = tokioTcpListener::bind(addr).await.unwrap();
            let (mut stream, _) = tcp.accept().await.unwrap();
            for _ in 0..$loop_num {
                let mut buf = [0; $buf_size];
                stream.read_exact(&mut buf).await.unwrap();
                assert_eq!(buf, [3; $buf_size]);

                let buf = [2; $buf_size];
                stream.write_all(&buf).await.unwrap();
            }
        }

        pub async fn $client(addr: String) {
            let mut tcp = tokioTcpStream::connect(addr.clone()).await;
            while tcp.is_err() {
                tcp = tokioTcpStream::connect(addr.clone()).await;
            }
            let mut tcp = tcp.unwrap();
            for _ in 0..$loop_num {
                let buf = [3; $buf_size];
                tcp.write_all(&buf).await.unwrap();

                let mut buf = [0; $buf_size];
                tcp.read_exact(&mut buf).await.unwrap();
                assert_eq!(buf, [2; $buf_size]);
            }
        }

        #[bench]
        fn $bench(b: &mut Bencher) {
            let runtime = $runtime;
            let basic_addr = "127.0.0.1:";
            let port = $port;
            b.iter(black_box(|| {
                let mut handlers = Vec::new();
                for i in 0..$task_num {
                    let addr = basic_addr.to_owned() + &*(port + i).to_string();
                    handlers.push(runtime.spawn($server(addr.clone())));
                    handlers.push(runtime.spawn($client(addr.clone())));
                }
                for handler in handlers {
                    runtime.block_on(handler).unwrap();
                }
            }));
        }
    };
}

#[macro_export]
macro_rules! ylong_tcp_task {
    ($bench: ident, $server: ident, $client: ident, $port: literal, $task_num: literal, $loop_num: literal, $buf_size: literal) => {
        pub async fn $server(addr: SocketAddr) {
            let tcp = TcpListener::bind(addr).await.unwrap();
            let (mut stream, _) = tcp.accept().await.unwrap();
            for _ in 0..$loop_num {
                let mut buf = [0; $buf_size];
                stream.read_exact(&mut buf).await.unwrap();
                assert_eq!(buf, [3; $buf_size]);

                let buf = [2; $buf_size];
                stream.write_all(&buf).await.unwrap();
            }
        }

        pub async fn $client(addr: SocketAddr) {
            let mut tcp = TcpStream::connect(addr).await;
            while tcp.is_err() {
                tcp = TcpStream::connect(addr).await;
            }
            let mut tcp = tcp.unwrap();
            for _ in 0..$loop_num {
                let buf = [3; $buf_size];
                tcp.write_all(&buf).await.unwrap();

                let mut buf = [0; $buf_size];
                tcp.read_exact(&mut buf).await.unwrap();
                assert_eq!(buf, [2; $buf_size]);
            }
        }

        #[bench]
        fn $bench(b: &mut Bencher) {
            let basic_addr = "127.0.0.1:";
            let port = $port;
            b.iter(black_box(|| {
                let mut handlers = Vec::new();
                for i in 0..$task_num {
                    let addr = (basic_addr.to_owned() + &*(port + i).to_string())
                        .parse()
                        .unwrap();
                    handlers.push(ylong_runtime::spawn($server(addr)));
                    handlers.push(ylong_runtime::spawn($client(addr)));
                }
                for handler in handlers {
                    ylong_runtime::block_on(handler).unwrap();
                }
            }));
        }
    };
}

#[cfg(test)]
mod tcp_bench {
    extern crate test;

    pub use crate::task_helpers::tokio_runtime;
    use std::hint::black_box;
    use std::net::SocketAddr;
    use test::Bencher;
    use tokio::io::AsyncReadExt as tokioAsyncReadExt;
    use tokio::io::AsyncWriteExt as tokioAsyncWriteExt;
    use tokio::net::TcpListener as tokioTcpListener;
    use tokio::net::TcpStream as tokioTcpStream;
    use ylong_runtime::io::{AsyncReadExt, AsyncWriteExt};
    use ylong_runtime::net::{TcpListener, TcpStream};

    ylong_tcp_task!(
        ylong_tcp_10_1000_10000,
        ylong_server1,
        ylong_client1,
        8181,
        10,
        1000,
        20000
    );
    ylong_tcp_task!(
        ylong_tcp_10_10_100,
        ylong_server2,
        ylong_client2,
        8191,
        10,
        1000,
        100
    );
    ylong_tcp_task!(
        ylong_tcp_120_1000_10000,
        ylong_server3,
        ylong_client3,
        8201,
        120,
        1000,
        20000
    );
    ylong_tcp_task!(
        ylong_tcp_120_10_100,
        ylong_server4,
        ylong_client4,
        8321,
        120,
        1000,
        100
    );
    tokio_tcp_task!(
        tokio_runtime(),
        tokio_tcp_10_1000_10000,
        tokio_server1,
        tokio_client1,
        8441,
        10,
        1000,
        20000
    );
    tokio_tcp_task!(
        tokio_runtime(),
        tokio_tcp_10_10_100,
        tokio_server2,
        tokio_client2,
        8451,
        10,
        1000,
        100
    );
    tokio_tcp_task!(
        tokio_runtime(),
        tokio_tcp_120_1000_10000,
        tokio_server3,
        tokio_client3,
        8461,
        120,
        1000,
        20000
    );
    tokio_tcp_task!(
        tokio_runtime(),
        tokio_tcp_120_10_100,
        tokio_server4,
        tokio_client4,
        8581,
        120,
        1000,
        100
    );
}
