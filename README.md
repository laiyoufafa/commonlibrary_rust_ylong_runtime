# ylong_runtime

## Introduction
Rust language doesn't provide an asynchronous runtime. Instead, it provides basic primitives and functionalities such as ``async``, ``await``, ``Future``, and ``Waker``. Therefore, it's users' responsibilities to implement the runtime, or choose from an existing third party's runtime.

## Compile Build

1. Introduce ylong_runtime in Cargo.toml

```toml
#[dependence]
ylong_runtime = { git = "https://gitee.com/openharmony-sig/commonlibrary_rust_ylong_runtime.git", version = "1.9.0", features = ["full"]}
```

2. Add dependencies to BUILD.gn where appropriate

```
deps += ["//commonlibrary/rust/ylong_runtime/ylong_runtime:ylong_runtime"]
```



## Usage

### `ylong` global thread pool

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


#### Thread pool settings

The specific settings of runtime can be chained, Must be set before `block_on` and `spawn`, otherwise `runtime` will use the default configuration.

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

### `ylong` scheduling framework non-asynchronous thread pool (spawn_blocking)

```rust
fn main() {
    let fut = async {
        // It could be a closure or function.
        let join_handle = runtime.spawn_blocking(|| {});
        // Waits the task until finished.
        let _result = join_handle.await;
    };
    let _ = ylong_runtime::block_on(fut);
}
```


### ParIter Introduction

ParIter and its related interfaces are defined in the module `ylong_runtime::iter`. ParIter supports parallel iterations over threads, where a set of data is split and the split data performs the operations in the iterator in parallel over the threads.

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

## Acknowledgements

Based on the user's habit, the API of this library, after changing the original Rust standard library synchronous interface implementation to asynchronous, retains the original naming style of the standard library, such as``TcpStream::connect``, ``File::read``, ``File::write`` and so on. We also refer to some of Tokio's general API design ideas, and we would like to express our gratitude to the Rust standard library and Tokio