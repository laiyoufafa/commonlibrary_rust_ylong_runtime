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

//! Benchmarks for the udp.

#![feature(test)]

extern crate core;

pub mod task_helpers;

#[cfg(test)]
mod udp_bench {
    extern crate test;

    use crate::task_helpers::tokio_runtime;
    use test::Bencher;

    use ylong_runtime::net::UdpSocket;

    use tokio::net::UdpSocket as tokioUdpSocket;

    /// benchmark test for ylong udp connect.
    ///
    /// # Title
    /// ylong_udp_connect
    ///
    /// # Brief
    /// 1.Create sender.
    /// 2.Bind and Connect.
    #[bench]
    fn ylong_udp_connect(b: &mut Bencher) {
        let sender_addr = "127.0.0.1:8093".parse().unwrap();
        let receiver_addr = "127.0.0.1:8094".parse().unwrap();
        b.iter(|| {
            let handle = ylong_runtime::spawn(async move {
                let sender = UdpSocket::bind(sender_addr).await.unwrap();

                let _connected_sender = sender.connect(receiver_addr).await.unwrap();
            });
            ylong_runtime::block_on(handle).unwrap();
        });
    }

    /// benchmark test for tokio udp connect.
    ///
    /// # Title
    /// tokio_udp_connect
    ///
    /// # Brief
    /// 1.Create sender.
    /// 2.Bind and Connect.
    #[bench]
    fn tokio_udp_connect(b: &mut Bencher) {
        let sender_addr = "127.0.0.1:8095";
        let receiver_addr = "127.0.0.1:8096";
        let runtime = tokio_runtime();
        b.iter(|| {
            let handle = runtime.spawn(async move {
                let sender = tokioUdpSocket::bind(sender_addr).await.unwrap();

                sender.connect(receiver_addr).await.unwrap();
            });
            runtime.block_on(handle).unwrap();
        });
    }

    /// benchmark test for ylong udp function send() and recv().
    ///
    /// # Title
    /// ylong_udp_send_recv
    ///
    /// # Brief
    /// 1.Create sender and receiver, bind their new UdpSockets and connect to each other.
    /// 2.Sender use send() to send message.
    /// 3.Receiver use recv() to receives message.
    /// 4.Check if the test results are correct.
    #[bench]
    fn ylong_udp_send_recv(b: &mut Bencher) {
        let basic_addr = "127.0.0.1:";
        let port = 8081;
        b.iter(|| {
            let mut handlers = Vec::new();
            for i in 0..10 {
                let sender_addr = (basic_addr.to_owned() + &*(port + 2 * i).to_string())
                    .parse()
                    .unwrap();
                let receiver_addr = (basic_addr.to_owned() + &*(port + 2 * i + 1).to_string())
                    .parse()
                    .unwrap();
                handlers.push(ylong_runtime::spawn(async move {
                    let sender = UdpSocket::bind(sender_addr).await.unwrap();
                    let receiver = UdpSocket::bind(receiver_addr).await.unwrap();

                    let connected_sender = sender.connect(receiver_addr).await.unwrap();
                    let connected_receiver = receiver.connect(sender_addr).await.unwrap();

                    for _ in 0..1000 {
                        connected_sender.send(b"Hello").await.unwrap();

                        let mut recv_buf = [0_u8; 12];
                        let len = connected_receiver.recv(&mut recv_buf[..]).await.unwrap();

                        assert_eq!(&recv_buf[..len], b"Hello");
                    }
                }));
            }
            for handler in handlers {
                ylong_runtime::block_on(handler).unwrap();
            }
        });
    }

    /// benchmark test for tokio udp function send() and recv().
    ///
    /// # Title
    /// tokio_udp_send_recv
    ///
    /// # Brief
    /// 1.Create sender and receiver, bind their new UdpSockets and connect to each other.
    /// 2.Sender use send() to send message.
    /// 3.Receiver use recv() to receives message.
    /// 4.Check if the test results are correct.
    #[bench]
    fn tokio_udp_send_recv(b: &mut Bencher) {
        let basic_addr = "127.0.0.1:";
        let port = 8121;
        let runtime = tokio_runtime();
        b.iter(|| {
            let mut handlers = Vec::new();
            for i in 0..10 {
                let sender_addr = basic_addr.to_owned() + &*(port + 2 * i).to_string();
                let receiver_addr = basic_addr.to_owned() + &*(port + 2 * i + 1).to_string();
                handlers.push(runtime.spawn(async move {
                    let sender = tokioUdpSocket::bind(sender_addr.clone()).await.unwrap();
                    let receiver = tokioUdpSocket::bind(receiver_addr.clone()).await.unwrap();

                    sender.connect(receiver_addr).await.unwrap();
                    receiver.connect(sender_addr).await.unwrap();

                    for _ in 0..1000 {
                        sender.send(b"Hello").await.unwrap();

                        let mut recv_buf = [0_u8; 12];
                        let len = receiver.recv(&mut recv_buf[..]).await.unwrap();

                        assert_eq!(&recv_buf[..len], b"Hello");
                    }
                }));
            }
            for handler in handlers {
                runtime.block_on(handler).unwrap();
            }
        });
    }

    /// benchmark test for ylong udp function send_to() and recv_from().
    ///
    /// # Title
    /// ylong_udp_send_to_recv_from
    ///
    /// # Brief
    /// 1.Create sender and receiver.
    /// 2.Sender use send_to() to send message.
    /// 3.Receiver use recv_from() to receives message.
    /// 4.Check if the test results are correct.
    #[bench]
    fn ylong_udp_send_to_recv_from(b: &mut Bencher) {
        let basic_addr = "127.0.0.1:";
        let port = 8141;
        b.iter(|| {
            let mut handlers = Vec::new();
            for i in 0..10 {
                let sender_addr = (basic_addr.to_owned() + &*(port + 2 * i).to_string())
                    .parse()
                    .unwrap();
                let receiver_addr = (basic_addr.to_owned() + &*(port + 2 * i + 1).to_string())
                    .parse()
                    .unwrap();
                handlers.push(ylong_runtime::spawn(async move {
                    let sender = UdpSocket::bind(sender_addr).await.unwrap();
                    let receiver = UdpSocket::bind(receiver_addr).await.unwrap();

                    for _ in 0..1000 {
                        sender.send_to(b"Hello", receiver_addr).await.unwrap();

                        let mut recv_buf = [0_u8; 12];
                        let (len, addr) = receiver.recv_from(&mut recv_buf[..]).await.unwrap();
                        assert_eq!(&recv_buf[..len], b"Hello");
                        assert_eq!(addr, sender_addr);
                    }
                }));
            }
            for handler in handlers {
                ylong_runtime::block_on(handler).unwrap();
            }
        });
    }

    /// benchmark test for tokio udp function send_to() and recv_from().
    ///
    /// # Title
    /// tokio_udp_send_to_recv_from
    ///
    /// # Brief
    /// 1.Create sender and receiver.
    /// 2.Sender use send_to() to send message.
    /// 3.Receiver use recv_from() to receives message.
    /// 4.Check if the test results are correct.
    #[bench]
    fn tokio_udp_send_to_recv_from(b: &mut Bencher) {
        let basic_addr = "127.0.0.1:";
        let port = 8161;
        let runtime = tokio_runtime();
        b.iter(|| {
            let mut handlers = Vec::new();
            for i in 0..10 {
                let sender_addr = basic_addr.to_owned() + &*(port + 2 * i).to_string();
                let receiver_addr = basic_addr.to_owned() + &*(port + 2 * i + 1).to_string();
                handlers.push(runtime.spawn(async move {
                    let sender = tokioUdpSocket::bind(sender_addr.clone()).await.unwrap();
                    let receiver = tokioUdpSocket::bind(receiver_addr.clone()).await.unwrap();

                    for _ in 0..1000 {
                        sender
                            .send_to(b"Hello", receiver_addr.clone())
                            .await
                            .unwrap();

                        let mut recv_buf = [0_u8; 12];
                        let (len, addr) = receiver.recv_from(&mut recv_buf[..]).await.unwrap();

                        assert_eq!(&recv_buf[..len], b"Hello");
                        assert_eq!(addr.to_string(), sender_addr.clone());
                    }
                }));
            }
            for handler in handlers {
                runtime.block_on(handler).unwrap();
            }
        });
    }
}
