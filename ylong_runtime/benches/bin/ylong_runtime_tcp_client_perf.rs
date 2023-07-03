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

//! Tcp client usage in ylong_runtime.

use std::io;
use std::time::Instant;
use ylong_runtime::io::{AsyncReadExt, AsyncWriteExt};
use ylong_runtime::net::TcpStream;

fn main() -> io::Result<()> {
    let handle = ylong_runtime::spawn(async move {
        let addr = "127.0.0.1:8080".parse().unwrap();
        let mut stream = match TcpStream::connect(addr).await {
            Ok(stream) => stream,
            Err(err) => return Err(err),
        };
        println!("client connect Ok");
        let mut recv_buf = [1_u8; 1024];
        let start = Instant::now();
        match stream.write(&recv_buf).await {
            Ok(len) => {
                println!("Send {len} bytes");
            }
            Err(_) => {
                println!("Send Failed.");
            }
        };

        match stream.read(&mut recv_buf).await {
            Ok(n) => {
                let end = Instant::now();
                println!("async_tcp_client CostTime: {:?}", end - start);
                println!("async tcp receive {n} bytes");
            }
            Err(_) => {
                println!("Receive Failed.");
            }
        };
        Ok(())
    });
    let _ = ylong_runtime::block_on(handle);
    Ok(())
}
