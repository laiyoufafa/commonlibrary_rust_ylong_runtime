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

use std::{io, thread};
use ylong_runtime::net::UdpSocket;

/// SDV test for `send()` and `recv()`.
///
/// # Title
/// test_send_recv
///
/// # Brief
/// 1.Create UdpSocket and connect to the remote address.
/// 2.Sender sends message first.
/// 3.Receiver receives message.
/// 4.Check if the test results are correct.
#[test]
fn sdv_udp_send_recv() {
    let sender_addr = "127.0.0.1:8081".parse().unwrap();
    let receiver_addr = "127.0.0.1:8082".parse().unwrap();
    let handle = ylong_runtime::spawn(async move {
        let sender = match UdpSocket::bind(sender_addr).await {
            Ok(socket) => socket,
            Err(e) => {
                panic!("Bind Socket Failed {}", e);
            }
        };

        let receiver = match UdpSocket::bind(receiver_addr).await {
            Ok(socket) => socket,
            Err(e) => {
                panic!("Bind Socket Failed {}", e);
            }
        };

        let connected_sender = match sender.connect(receiver_addr).await {
            Ok(socket) => socket,
            Err(e) => {
                panic!("Connect Socket Failed {}", e);
            }
        };
        let connected_receiver = match receiver.connect(sender_addr).await {
            Ok(socket) => socket,
            Err(e) => {
                panic!("Connect Socket Failed {}", e);
            }
        };

        match connected_sender.send(b"Hello").await {
            Ok(n) => {
                assert_eq!(n, "Hello".len());
            }
            Err(e) => {
                panic!("Sender Send Failed {}", e);
            }
        }

        let mut recv_buf = [0_u8; 12];
        let len = connected_receiver.recv(&mut recv_buf[..]).await.unwrap();

        assert_eq!(&recv_buf[..len], b"Hello");
    });
    ylong_runtime::block_on(handle).expect("block_on failed");
}

/// SDV test for `send_to()` and `recv_from()`.
///
/// # Title
/// test_send_to_recv_from
///
/// # Brief
/// 1.Create UdpSocket.
/// 2.Sender sends message to the specified address.
/// 3.Receiver receives message and return the address the message from.
/// 4.Check if the test results are correct.
#[test]
fn sdv_udp_send_to_recv_from() {
    let sender_addr = "127.0.0.1:8085".parse().unwrap();
    let receiver_addr = "127.0.0.1:8086".parse().unwrap();
    let handle = ylong_runtime::spawn(async move {
        let sender = match UdpSocket::bind(sender_addr).await {
            Ok(socket) => socket,
            Err(e) => {
                panic!("Bind Socket Failed {}", e);
            }
        };

        let receiver = match UdpSocket::bind(receiver_addr).await {
            Ok(socket) => socket,
            Err(e) => {
                panic!("Bind Socket Failed {}", e);
            }
        };

        match sender.send_to(b"Hello", receiver_addr).await {
            Ok(n) => {
                assert_eq!(n, "Hello".len());
            }
            Err(e) => {
                panic!("Sender Send Failed {}", e);
            }
        }

        let mut recv_buf = [0_u8; 12];
        let (len, addr) = receiver.recv_from(&mut recv_buf[..]).await.unwrap();
        assert_eq!(&recv_buf[..len], b"Hello");
        assert_eq!(addr, sender_addr);
    });
    ylong_runtime::block_on(handle).expect("block_on failed");
}

fn sdv_udp_send() {
    let sender_addr = "127.0.0.1:8089".parse().unwrap();
    let receiver_addr = "127.0.0.1:8090".parse().unwrap();
    let handle = ylong_runtime::spawn(async move {
        let sender = match UdpSocket::bind(sender_addr).await {
            Ok(socket) => socket,
            Err(e) => {
                panic!("Bind Socket Failed {}", e);
            }
        };

        let connected_sender = match sender.connect(receiver_addr).await {
            Ok(socket) => socket,
            Err(e) => {
                panic!("Connect Socket Failed {}", e);
            }
        };

        match connected_sender.send(b"Hello").await {
            Ok(n) => {
                assert_eq!(n, "Hello".len());
            }
            Err(e) => {
                panic!("Sender Send Failed {}", e);
            }
        }
    });
    ylong_runtime::block_on(handle).expect("block_on failed");
}

/// SDV test for functions in a multithreaded environment.
///
/// # Title
/// test_thread_func
///
/// # Brief
/// 1.Create sender and receiver threads, bind their new UdpSockets and connect to each other.
/// 2.Sender send message in sender thread.
/// 3.Receiver receives message in receiver thread.
/// 4.Check if the test results are correct.
#[test]
fn sdv_udp_recv() {
    let sender_addr = "127.0.0.1:8089".parse().unwrap();
    let receiver_addr = "127.0.0.1:8090".parse().unwrap();
    let handle = ylong_runtime::spawn(async move {
        let receiver = match UdpSocket::bind(receiver_addr).await {
            Ok(socket) => socket,
            Err(e) => {
                panic!("Bind Socket Failed {}", e);
            }
        };

        let connected_receiver = match receiver.connect(sender_addr).await {
            Ok(socket) => socket,
            Err(e) => {
                panic!("Connect Socket Failed {}", e);
            }
        };
        thread::spawn(sdv_udp_send);
        let mut recv_buf = [0_u8; 12];
        let len = connected_receiver.recv(&mut recv_buf[..]).await.unwrap();

        assert_eq!(&recv_buf[..len], b"Hello");
    });
    ylong_runtime::block_on(handle).expect("block_on failed");
}

/// SDV test for `try_send_to()` and `try_recv_from()`.
///
/// # Brief
/// 1.Create UdpSocket.
/// 2.Sender tries to send message to the specified address.
/// 3.Receiver tries to receive message and return the address the message from.
/// 4.Check if the test results are correct.
#[test]
fn sdv_udp_try_recv_from() {
    let sender_addr = "127.0.0.1:8091".parse().unwrap();
    let receiver_addr = "127.0.0.1:8092".parse().unwrap();
    let handle = ylong_runtime::spawn(async move {
        let sender = match UdpSocket::bind(sender_addr).await {
            Ok(socket) => socket,
            Err(e) => {
                panic!("Bind Socket Failed {}", e);
            }
        };

        let receiver = match UdpSocket::bind(receiver_addr).await {
            Ok(socket) => socket,
            Err(e) => {
                panic!("Bind Socket Failed {}", e);
            }
        };

        sender.writable().await.unwrap();
        let mut ret = sender.try_send_to(b"Hello", receiver_addr);
        while let Err(ref e) = ret {
            if e.kind() == io::ErrorKind::WouldBlock {
                ret = sender.try_send_to(b"Hello", receiver_addr);
            } else {
                panic!("try_send_to failed: {}", e);
            }
        }

        assert_eq!(ret.unwrap(), 5);

        let mut recv_buf = [0_u8; 12];
        receiver.readable().await.unwrap();
        let mut ret = receiver.try_recv_from(&mut recv_buf[..]);
        while let Err(ref e) = ret {
            if e.kind() == io::ErrorKind::WouldBlock {
                ret = receiver.try_recv_from(&mut recv_buf[..]);
            } else {
                panic!("try_send_to failed: {}", e);
            }
        }
        let (len, peer_addr) = ret.unwrap();
        assert_eq!(&recv_buf[..len], b"Hello");
        assert_eq!(peer_addr, sender_addr);
    });
    ylong_runtime::block_on(handle).expect("block_on failed");
}

fn sdv_udp_try_send() {
    let sender_addr = "127.0.0.1:8093".parse().unwrap();
    let receiver_addr = "127.0.0.1:8094".parse().unwrap();
    let handle = ylong_runtime::spawn(async move {
        let sender = match UdpSocket::bind(sender_addr).await {
            Ok(socket) => socket,
            Err(e) => {
                panic!("Bind Socket Failed {}", e);
            }
        };

        let connected_sender = match sender.connect(receiver_addr).await {
            Ok(socket) => socket,
            Err(e) => {
                panic!("Connect Socket Failed {}", e);
            }
        };

        connected_sender.writable().await.unwrap();
        match connected_sender.try_send(b"Hello") {
            Ok(n) => {
                assert_eq!(n, "Hello".len());
            }
            Err(e) => {
                panic!("Sender Send Failed {}", e);
            }
        }
    });
    ylong_runtime::block_on(handle).expect("block_on failed");
}

/// SDV test for try_send and try_recv
///
/// # Brief
/// 1.Create sender and receiver threads, bind their new UdpSockets and connect to each other.
/// 2.Sender waits for writable events and attempts to send message in sender thread.
/// 3.Receiver waits for readable events and attempts to receive message in receiver thread.
/// 4.Check if the test results are correct.
#[test]
fn sdv_udp_try_recv() {
    let sender_addr = "127.0.0.1:8093".parse().unwrap();
    let receiver_addr = "127.0.0.1:8094".parse().unwrap();
    let handle = ylong_runtime::spawn(async move {
        let receiver = match UdpSocket::bind(receiver_addr).await {
            Ok(socket) => socket,
            Err(e) => {
                panic!("Bind Socket Failed {}", e);
            }
        };

        let connected_receiver = match receiver.connect(sender_addr).await {
            Ok(socket) => socket,
            Err(e) => {
                panic!("Connect Socket Failed {}", e);
            }
        };
        thread::spawn(sdv_udp_try_send);
        connected_receiver.readable().await.unwrap();
        let mut recv_buf = [0_u8; 12];
        let len = connected_receiver.try_recv(&mut recv_buf[..]).unwrap();

        assert_eq!(&recv_buf[..len], b"Hello");
    });
    ylong_runtime::block_on(handle).expect("block_on failed");
}
