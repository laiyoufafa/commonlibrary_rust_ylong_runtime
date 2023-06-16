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

//! Benchmarks for tcp.
//!
//! Designs of ylong_runtime benchmarks:
//! - Create a client and a server and connect them to each other.
//! - The client sends a message and the server receives the message.
//! - The server sends a message and the client receives the message.
//! - Repeat the preceding operations 1000 times.

use std::thread;
use std::time::Instant;
use tokio::io::{AsyncReadExt as tokioAsyncReadExt, AsyncWriteExt as tokioAsyncWriteExt};
use tokio::net::{TcpListener as tokioTcpListener, TcpStream as tokioTcpStream};
use ylong_runtime::builder::RuntimeBuilder;
use ylong_runtime::io::{AsyncReadExt, AsyncWriteExt};
use ylong_runtime::net::{TcpListener, TcpStream};

const LOOP_NUMS: usize = 1000;

fn ylong_create_client() {
    let mut recv_buf = [0_u8; 12];
    let runtime = RuntimeBuilder::new_multi_thread().build().unwrap();
    let handle = runtime.spawn(async move {
        let addr = "127.0.0.1:9081".parse().unwrap();
        loop {
            if let Ok(mut client) = TcpStream::connect(addr).await {
                match client.write(b"hello server").await {
                    Ok(n) => {
                        assert_eq!(n as usize, "hello server".len());
                    }
                    Err(e) => {
                        panic!("client send failed {}", e);
                    }
                }
                match client.read(&mut recv_buf).await {
                    Ok(n) => {
                        assert_eq!(
                            std::str::from_utf8(&recv_buf).unwrap(),
                            "hello client".to_string()
                        );
                        assert_eq!(n as usize, "hello client".len());
                        break;
                    }
                    Err(e) => {
                        panic!("client recv failed {}", e);
                    }
                }
            };
        }
    });
    runtime.block_on(handle).unwrap();
}

fn tokio_create_client() {
    let mut recv_buf = [0_u8; 12];
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let handle = runtime.spawn(async move {
        loop {
            if let Ok(mut client) = tokioTcpStream::connect("127.0.0.1:9082").await {
                match client.write(b"hello server").await {
                    Ok(n) => {
                        assert_eq!(n as usize, "hello server".len());
                    }
                    Err(e) => {
                        panic!("client send failed {}", e);
                    }
                }
                match client.read(&mut recv_buf).await {
                    Ok(n) => {
                        assert_eq!(
                            std::str::from_utf8(&recv_buf).unwrap(),
                            "hello client".to_string()
                        );
                        assert_eq!(n as usize, "hello client".len());
                        break;
                    }
                    Err(e) => {
                        panic!("client recv failed {}", e);
                    }
                }
            };
        }
    });
    runtime.block_on(handle).unwrap();
}

fn main() {
    println!("ylong tcp read()+write() Loops: {}", LOOP_NUMS);

    let runtime = RuntimeBuilder::new_multi_thread().build().unwrap();
    let addr = "127.0.0.1:9081".parse().unwrap();

    let st = Instant::now();
    for _ in 0..LOOP_NUMS {
        // start client
        thread::spawn(ylong_create_client);
        let handle = runtime.spawn(async move {
            let listener = TcpListener::bind(addr).await.unwrap();

            let (mut socket, _) = listener.accept().await.unwrap();

            loop {
                let mut buf = [0_u8; 12];
                let _ = match socket.read(&mut buf).await {
                    Ok(n) if n == 0 => break,
                    Ok(n) => {
                        assert_eq!(
                            std::str::from_utf8(&buf).unwrap(),
                            "hello server".to_string()
                        );
                        assert_eq!(n as usize, "hello server".len());
                        n
                    }
                    Err(e) => {
                        panic!("recv Failed {}", e);
                    }
                };

                let _ = socket.write(b"hello client").await.unwrap();
            }
        });
        runtime.block_on(handle).unwrap();
    }

    println!(
        "ylong tcp read()+write() cost: {:.6} ms",
        st.elapsed().as_secs_f64() * 1000f64 / 1000.0
    );

    println!("tokio tcp read()+write() Loops: {}", LOOP_NUMS);

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    let st = Instant::now();
    for _ in 0..LOOP_NUMS {
        // start client
        thread::spawn(tokio_create_client);
        let handle = runtime.spawn(async move {
            let listener = tokioTcpListener::bind("127.0.0.1:9082").await.unwrap();

            let (mut socket, _) = listener.accept().await.unwrap();

            loop {
                let mut buf = [0_u8; 12];
                let _ = match socket.read(&mut buf).await {
                    Ok(n) if n == 0 => break,
                    Ok(n) => {
                        assert_eq!(
                            std::str::from_utf8(&buf).unwrap(),
                            "hello server".to_string()
                        );
                        assert_eq!(n as usize, "hello server".len());
                        n
                    }
                    Err(e) => {
                        panic!("recv Failed {}", e);
                    }
                };

                let _ = socket.write(b"hello client").await.unwrap();
            }
        });
        runtime.block_on(handle).unwrap();
    }

    println!(
        "tokio tcp read()+write() cost: {:.6} ms",
        st.elapsed().as_secs_f64() * 1000f64 / 1000.0
    );
}
