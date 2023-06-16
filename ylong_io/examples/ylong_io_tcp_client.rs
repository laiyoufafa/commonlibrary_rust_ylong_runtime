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

//! cargo build --example ylong_io_tcp_client --no-default-features --features="ylong_tcp"
//! Uses with ylong_io_tcp_server, start ylong_io_tcp_server first, then start ylong_io_tcp_client

use std::io::{Read, Write};
use std::thread::sleep;
use std::time::Duration;
use ylong_io::TcpStream;

fn main() {
    let addr = "127.0.0.1:1234".parse().unwrap();
    let mut buffer = [0_u8; 1024];
    let mut stream = match TcpStream::connect(addr) {
        Err(err) => {
            println!("TcpStream::connect err {err}");
            return;
        }
        Ok(addr) => addr,
    };
    loop {
        sleep(Duration::from_micros(300));
        match stream.read(&mut buffer) {
            Ok(_) => {
                println!("1.Receive msg: {}\n", String::from_utf8_lossy(&buffer));
                break;
            }
            Err(_) => continue,
        }
    }

    println!("client socket: {stream:?}");

    match stream.write(b"Hello World") {
        Ok(n) => println!("send len: {n}"),
        _ => println!("send failed"),
    }

    if stream.read(&mut buffer).is_ok() {
        println!("2.Receive msg: {}\n", String::from_utf8_lossy(&buffer));
    }
}
