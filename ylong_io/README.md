## ylong_io 介绍
提供通信 io 功能，底层实现使用 epoll。

### 功能介绍
- 支持 tcp 协议:

  支持使用 tcp 协议进行通信，并使用 epoll 进行异步 io。

### 使用实例
TCP 通信实例

```
use std::collections::HashMap;
use std::io;
use std::io::{Read, Write};
use ylong_io::{EventTrait, Events, Interest, Poll, TcpListener, Token};

// 唯一标识 SEVER 的 Token
const SERVER: Token = Token(0);

fn main() -> io::Result<()> {
    // 创建 Poll 实例，用于 epoll 查找事件。
    let poll = Poll::new()?;  
    
    // 设置本地地址，并 bind 本地端口，得到 fd。
    let addr = "127.0.0.1:1234".parse().unwrap();
    let mut server = TcpListener::bind(addr)?;
    
    // 将 server 的 fd 注册到 epoll 中进行监听，监听可读事件。
    poll.register(&mut server, SERVER, Interest::READABLE)?;
    let mut events = Events::with_capacity(128);
    
    // 保存远端传来的链接。
    let mut connections = HashMap::new();
    // 远端传来的 socket fd 用从 1 开始的计数标号。
    let mut unique_token = Token(SERVER.0 + 1);
    
    // 事件循环
    loop {
        // 检查是否有事件到来。
        poll.poll(&mut events, None)?;

        for event in events.iter() {
            // server 有事件到来，说明远端有新连接到来。
            if SERVER == event.token() {
                let (mut stream, addr) = server.accept()?;
                
                // 设置Token。
                let token = Token(unique_token.0 + 1);
                unique_token = Token(unique_token.0 + 1);
                // 注册到 epoll 中监听。
                poll.register(
                    &mut stream,
                    token,
                    Interest::READABLE.add(Interest::WRITABLE),
                )?;
                // 添加到数据结构中进行保存。
                connections.insert(token, stream);
            } else {
                // 通过唯一标识查找连接。
                match connections.get_mut(&event.token()) {
                         //... 执行通信相关的逻辑
                    }
                    None => break,
                }
            }
        }
    }
}

  ```

### 如何使用
在 Cargo.toml 文件中添加 dependencies 即可
```
[dependencies]
ylong_io = "0.1.0"
```