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

use std::thread;
use ylong_runtime::io::{AsyncReadExt, AsyncWriteExt};
use ylong_runtime::net::{TcpListener, TcpStream};

fn test_tcp_client() {
    let mut recv_buf = [0_u8; 12];
    let handle = ylong_runtime::spawn(async move {
        loop {
            let addr = "127.0.0.1:8081".parse().unwrap();
            if let Ok(mut client) = TcpStream::connect(addr).await {
                match client.write(b"hello server").await {
                    Ok(n) => {
                        assert_eq!(n, "hello server".len());
                    }
                    Err(e) => {
                        assert_eq!(0, 1, "client send failed {e}");
                    }
                }
                match client.read(&mut recv_buf).await {
                    Ok(n) => {
                        assert_eq!(
                            std::str::from_utf8(&recv_buf).unwrap(),
                            "hello client".to_string()
                        );
                        assert_eq!(n, "hello client".len());
                        break;
                    }
                    Err(e) => {
                        assert_eq!(0, 1, "client recv failed {e}");
                    }
                }
            };
        }
    });
    ylong_runtime::block_on(handle).expect("block_on failed");
}

#[test]
fn sdv_tcp_global_runtime() {
    // Start a thread as client side
    thread::spawn(test_tcp_client);
    let addr = "127.0.0.1:8081".parse().unwrap();
    let handle = ylong_runtime::spawn(async move {
        let listener = TcpListener::bind(addr).await;
        if let Err(e) = listener {
            assert_eq!(0, 1, "Bind Listener Failed {e}");
            return;
        }

        let listener = listener.unwrap();
        let mut socket = match listener.accept().await {
            Ok((socket, _)) => socket,
            Err(e) => {
                assert_eq!(0, 1, "Bind accept Failed {e}");
                return;
            }
        };
        loop {
            let mut buf = [0_u8; 12];
            let _ = match socket.read(&mut buf).await {
                Ok(n) if n == 0 => break,
                Ok(n) => {
                    assert_eq!(
                        std::str::from_utf8(&buf).unwrap(),
                        "hello server".to_string()
                    );
                    assert_eq!(n, "hello server".len());
                    n
                }
                Err(e) => {
                    assert_eq!(0, 1, "recv Failed {e}");
                    break;
                }
            };

            if let Err(e) = socket.write(b"hello client").await {
                assert_eq!(0, 1, "failed to write to socket {e}");
                break;
            }
        }
    });
    ylong_runtime::block_on(handle).expect("block_on failed");
}

#[cfg(feature = "multi_instance_runtime")]
#[test]
fn sdv_tcp_multi_runtime() {
    use ylong_runtime::builder::RuntimeBuilder;
    let runtime = RuntimeBuilder::new_multi_thread().build().unwrap();

    let server = runtime.spawn(async move {
        let addr = "127.0.0.1:8082".parse().unwrap();
        let tcp = TcpListener::bind(addr).await.unwrap();
        let (mut stream, _) = tcp.accept().await.unwrap();
        let mut buf = [0; 100];
        stream.read_exact(&mut buf).await.unwrap();
        assert_eq!(buf, [3; 100]);

        let buf = [2; 100];
        stream.write_all(&buf).await.unwrap();
    });

    let client = runtime.spawn(async move {
        let addr = "127.0.0.1:8082".parse().unwrap();
        let mut tcp = TcpStream::connect(addr).await;
        while tcp.is_err() {
            tcp = TcpStream::connect(addr).await;
        }
        let mut tcp = tcp.unwrap();
        let buf = [3; 100];
        tcp.write_all(&buf).await.unwrap();

        let mut buf = [0; 100];
        tcp.read_exact(&mut buf).await.unwrap();
        assert_eq!(buf, [2; 100]);
    });
    runtime.block_on(server).unwrap();
    runtime.block_on(client).unwrap();
}
