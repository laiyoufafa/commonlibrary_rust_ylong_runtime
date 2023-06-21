# ylong_runtime

## 简介
Rust语言在提供了``async``/``await``, ``Future``, ``Waker``等异步基础组件的同时，并不提供具体的异步运行时实现。而具体实现则需要用户自己构建，或者使用社区已构建好的异步运行时。这样的好处是用户可以在自己的使用场景做定制化的优化。

## 编译构建

1. 在Cargo.toml中引入ylong_runtime

```toml
#[dependence]
ylong_runtime = { git = "https://gitee.com/openharmony-sig/commonlibrary_rust_ylong_runtime.git", version = "1.9.0", features = ["full"]}
```

2. 在 BUILD.gn 合适的地方添加依赖

```
deps += ["//commonlibrary/rust/ylong_runtime/ylong_runtime:ylong_runtime"]
```

## 使用说明

### `ylong` 全局线程池

```rust
use std::net::{Ipv4Addr, SocketAddrV4};
use ylong_runtime::io::*;
use ylong_runtime::net::TcpListener;
fn main() -> std::io::Result<()> {
    ylong_runtime::block_on(async {
        let ip = Ipv4Addr::new(127, 0, 0, 1);
        let addr = SocketAddrV4::new(ip, 8080);
        let listener = TcpListener::bind(addr.into()).await?;
        loop {
            let (mut stream, _) = listener.accept().await?;
            stream.write_all("hello ylong".as_bytes());
        }
    })
}

```

#### 线程池设置

可以链式设置runtime的具体配置。必须在`block_on`，`spawn`之前设置，否则runtime会使用默认配置。

```rust
fn main() {
    let _ = ylong_runtime::builder::RuntimeBuilder::new_multi_thread()
        .worker_stack_size(10)
    	.keep_alive_time(std::time::Duration::from_secs(10))
        .build_global();

    let fut = async {

    };
    let _ = ylong_runtime::block_on(fut);
}
```



### `ylong` 调度框架非异步线程池(spawn_blocking)使用

```rust
fn main() {
    let fut = async {
        // 这里可以是闭包也可以是函数。
        let join_handle = ylong_runtime::spawn_blocking(|| {});
        // 等待任务执行完成
        let _result = join_handle.await;
    };
    let _ = ylong_runtime::block_on(fut);
}

```



### ParIter 功能介绍

`ParIter` 及其相关接口定义于模块 `ylong_runtime::iter`，ParIter支持数据在线程中做并行迭代，一组数据会在线程中被分割，分割后的数据会在线程中并行执行迭代器中的操作。

```rust
use ylong_runtime::iter::prelude::*;

fn main() {
    ylong_runtime::block_on(fut());
}

async fn fut() {
    let v = (1..30).into_iter().collect::<Vec<usize>>();
    let sum = v.par_iter().map(|x| fibbo(*x)).sum().await.unwrap();
    println!("{}", sum);
}

fn fibbo(i: usize) -> usize {
    match i {
        0 => 1,
        1 => 1,
        n => fibbo(n - 1) + fibbo(n - 2),
    }
}
```



## 致谢

基于用户的使用习惯，本库的API在将原本Rust标准库同步的接口实现改为异步后，保留了标准库原本的命名风格，如``TcpStream::connect``、``File::read``、``File::write``等。同时也参考了Tokio的部分通用API设计思想，在此对Rust标准库和Tokio表示感谢