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

//! Tcp server usage in ylong_runtime.

use std::io;
use std::time::Instant;
use ylong_runtime::builder::RuntimeBuilder;
use ylong_runtime::io::{AsyncReadExt, AsyncWriteExt};
use ylong_runtime::net::TcpListener;

fn main() -> io::Result<()> {
    let runtime = RuntimeBuilder::new_multi_thread().build().unwrap();
    let handle = runtime.spawn(async move {
        let addr = "127.0.0.1:8080".parse().unwrap();
        let server = match TcpListener::bind(addr).await {
            Ok(server) => server,
            Err(_) => return,
        };
        let mut recv_buf = [0_u8; 1024];
        loop {
            let (mut stream, _) = match server.accept().await {
                Ok((stream, addr)) => (stream, addr),
                Err(_e) => continue,
            };
            println!("Accept connection!");
            loop {
                let start = Instant::now();
                match stream.read(&mut recv_buf).await {
                    Ok(n) if n == 0 => break,
                    Ok(n) => {
                        println!("Receive len: {n}.");
                    }
                    Err(_) => {
                        println!("Receive Failed.");
                        break;
                    }
                }
                match stream.write(&recv_buf).await {
                    Ok(n) if n == 0 => {
                        println!("Server now break");
                        break;
                    }
                    Ok(n) => {
                        let end = Instant::now();
                        println!("Server CostTime: {:?}", end - start);
                        println!("send Succeeded: {n}",);
                    }
                    Err(_) => {
                        println!("Send Failed.");
                        break;
                    }
                };
            }
        }
    });
    let _ = runtime.block_on(handle);
    Ok(())
}
