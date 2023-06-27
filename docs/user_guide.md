# ylong_runtime 用户指南

ylong_runtime提供了异步任务生成以及调度的能力，同时异步化了各类系统IO，并提供了同步原语，定时器功能。

ylong_runtime 整体分为4个库：

- ylong_ffrt: Function Flow Runtime的FFI层
- ylong_io: 事件驱动型网络IO库
- ylong_runtime: 异步调度框架库
- ylong_runtime_macros：调度框架宏库

其中 ylong_runtime 承载了异步调度框架的主体功能，并且依赖其余三个库提供的底层能力。

如果需要查看详细的接口说明请查看对应接口的 docs，可以使用 `cargo doc --open` 生成并查看 docs。

## 配置异步调度框架

可以链式设置runtime的具体配置。必须在`block_on`，`spawn`之前设置，否则`runtime`会使用默认配置。 相关接口位于`ylong_runtime::builder`.

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

## 生成异步任务

通过``spawn``生成异步任务

```rust
fn main() {
    let handle = ylong_runtime::spawn(async move {
        // 嵌套生成异步任务
        let handle = ylong_runtime::spawn(async move {
            1
        });
        
        // 在异步上下文使用await异步等待任务完成
        let res = handle.await.unwrap();
        assert_eq!(res, 1);
        1
    });
    // 在同步上下文使用block_on阻塞等待异步任务完成
    let res = ylong_runtime::block_on(handle).unwrap();
    assert_eq!(res, 1);
}

```

## 生成非异步任务

通过``spawn_blocking``生成任务

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

## 取消异步任务

通过``join_handle``的``cancel``接口取消任务。如果任务成功取消，等待该任务将立即返回一个错误。

```rust
use std::time::Duration;
use ylong_runtime::error::ErrorKind;
use ylong_runtime::time::sleep;

fn main() {
    let handle = ylong_runtime::spawn(async move {
        let task = ylong_runtime::spawn(async move {
            sleep(Duration::from_secs(100)).await;
        });
        task.cancel();
        let res = task.await.err().unwrap();
        assert_eq!(res.kind(), ErrorKind::TaskCanceled);
    });
    ylong_runtime::block_on(handle).unwrap();
}
```

## 使用网络异步IO

使用 ylong_runtime 中封装的异步IO(TCP/UDP)进行网络连接，相关接口定义位于模块 `ylong_runtime::net`.

```rust
use ylong_runtime::net::{TcpListener, TcpStream};
use ylong_runtime::io::AsyncWrite;

fn main() {
    let addr = "127.0.0.1:8081".parse().unwrap();
    let handle = ylong_runtime::spawn(async move {
        // 异步TCP服务端，监听一个端口
        let listener = TcpListener::bind(addr).await;
        if let Err(e) = listener {
            assert_eq!(0, 1, "Bind Listener Failed {e}");
        }

        let listener = listener.unwrap();
        // 接受一个TCP连接请求
        let mut socket = match listener.accept().await {
            Ok((socket, _)) => socket,
            Err(e) => {
                assert_eq!(0, 1, "Bind accept Failed {e}");
            }
        };

        // 发送一条信息给客户端
        if let Err(e) = socket.write(b"hello client").await {
            assert_eq!(0, 1, "failed to write to socket {e}");
        }
    });
}

```

## 使用定时器
定时器相关接口位于模块`ylong_runtime::time`. 里面包括了任务睡眠，任务超时，以及周期性定时器的功能。

```rust
use std::time::Duration;
use ylong_runtime::time::{timeout, sleep};

fn main() {
    let handle = ylong_runtime::spawn(async move {
        // 这个任务将在100微秒后超时，任务在这个时间内完成将返回结果。
        let result = timeout(Duration::from_millis(100), async { 1 }).await;
        assert_eq!(result.unwrap(), 1);
        
        // 如果任务超时，将返回一个超时错误。
        let result = timeout(Duration::from_millis(100), sleep(Duration::from_secs(1)));
        assert!(result.is_err());
    });
}


```

## 使用ParIter

`ParIter` 及其相关接口定义于模块 `ylong_runtime::iter`，`ParIter`支持数据在线程中做并行迭代，一组数据会在线程中被分割，分割后的数据会在线程中并行执行迭代器中的操作。

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