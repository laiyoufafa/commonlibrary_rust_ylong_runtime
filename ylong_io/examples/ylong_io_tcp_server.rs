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

//! cargo build --example ylong_io_tcp_server --no-default-features --features="ylong_tcp"
//! Uses with ylong_io_tcp_server, start ylong_io_tcp_server first, then start ylong_io_tcp_client

use std::collections::HashMap;
use std::io;
use std::io::{Read, Write};
use std::str::from_utf8;
use ylong_io::{EventTrait, Events, Interest, Poll, TcpListener, Token};

const SERVER: Token = Token(0);

fn main() -> io::Result<()> {
    let poll = Poll::new()?;
    let addr = "127.0.0.1:1234".parse().unwrap();
    let mut server = TcpListener::bind(addr)?;
    println!("server socket {server:?}\n");

    poll.register(&mut server, SERVER, Interest::READABLE)?;
    let mut events = Events::with_capacity(128);
    // Map of `Token` -> `TcpListener`.
    let mut connections = HashMap::new();
    let mut unique_token = Token(SERVER.0 + 1);
    loop {
        poll.poll(&mut events, None)?;

        for event in events.iter() {
            if SERVER == event.token() {
                let ret = server.accept()?;
                let (mut stream, addr) = ret;
                println!("Accept connection addr: {stream:?} {addr:?}");
                let token = Token(unique_token.0 + 1);
                unique_token = Token(unique_token.0 + 1);
                poll.register(
                    &mut stream,
                    token,
                    Interest::READABLE.add(Interest::WRITABLE),
                )?;
                connections.insert(token, stream);
            } else {
                match connections.get_mut(&event.token()) {
                    Some(connection) => {
                        if event.is_writable() {
                            //println!("server writable\n");
                            match connection.write(b"Hello client_from writable") {
                                Err(err) => {
                                    println!("1.Send failed {err}");
                                    poll.deregister(connection)?;
                                    poll.register(connection, event.token(), Interest::READABLE)?;
                                    break;
                                }
                                Ok(n) => {
                                    println!("1.send len: {n}\n");
                                    poll.deregister(connection)?;
                                    poll.register(connection, event.token(), Interest::READABLE)?;
                                    break;
                                }
                            }
                        } else if event.is_readable() {
                            println!("server readable\n");
                            let mut msg_buf = [0_u8; 100];
                            match connection.read(&mut msg_buf) {
                                Ok(0) => {
                                    poll.deregister(connection)?;
                                }
                                Ok(n) => {
                                    if let Ok(str_buf) = from_utf8(&msg_buf[0..n]) {
                                        println!("recv msg : {str_buf:?}, len : {n}");
                                    } else {
                                        println!("Received (none UTF-8) data: {:?}", &msg_buf);
                                    }
                                }
                                Err(_n) => {
                                    poll.deregister(connection)?;
                                    break;
                                }
                            }
                        }
                    }
                    None => break,
                }
            }
        }
    }
}
