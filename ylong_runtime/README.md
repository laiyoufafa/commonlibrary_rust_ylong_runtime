## 简介
Rust语言不自带异步运行时框架，只提供了``async``/``await``, ``Future``, ``Waker``等异步基础能力。因此，在Rust中使用异步能力需要用户自己实现一个异步调度框架或者使用现有三方库提供的框架。



## 调度框架基本功能及其使用

### 编译
开启"ffrt"或"ffrt_full"feature时，将使用FFRT作为底层Runtime，关闭"ffrt"feature时将使用Rust executor，默认关闭"ffrt"feature。

### `ylong` 调度框架异步线程池使用

```rust
use ylong_runtime::builder::RuntimeBuilder;

// 简单异步任务
async fn test_future(num: usize) -> usize {
    num
}

fn main() {
    // 设置异步线程池最大容量
    let core_pool_size = 4;
    // 设定异步线程池是否进行绑核处理
    let is_affinity = true;

    // 创建运行时环境
    let runtime = RuntimeBuilder::new_multi_thread()
        .is_affinity(is_affinity)
        .core_pool_size(core_pool_size)
        .build()
        .unwrap();
    // 获取任务钩子，通过该钩子等待任务执行结果
    let handle = runtime.spawn(test_future(1));
    // 获取任务执行结果
    let _result = runtime.block_on(handle).unwrap();
}
```

### `ylong` 调度框架阻塞线程池使用

```rust
use std::thread::sleep;
use std::time::Duration;
use ylong_runtime::builder::RuntimeBuilder;

// 阻塞时间较短任务
fn short_wait_time_task() -> i32 {
    sleep(Duration::from_secs(1));
    1
}

fn main() {
    // 阻塞线程池最大容量
    let max_pool_size = 4;
    // 阻塞线程池常驻线程数量
    let permanent_block_pool_size = 2;
    // 最大存活时间
    let keep_alive_time = Duration::from_secs(3);
    // 阻塞线程池是否存在常驻线程
    let is_permanent = true;

    // 创建运行时环境
    let runtime = RuntimeBuilder::new_multi_thread()
        .max_pool_size(max_pool_size)
        .permanent_block_pool_size(permanent_block_pool_size)
        .keep_alive_time(keep_alive_time)
        .is_permanent(is_permanent)
        .build()
        .unwrap();

    // 获取任务钩子
    let handle = runtime
        .spawn_blocking(move || short_wait_time_task())
        .unwrap();
    // 通过任务钩子获取结果
    let _result = runtime.block_on(handle).unwrap();
}
```

### `ylong` 调度框架多实例异步线程池使用

```rust
use ylong_runtime::builder::RuntimeBuilder;

// 简单异步任务
async fn test_future(num: usize) -> usize {
    num
}

fn main() {
    // 创建多实例异步线程池
    // 设置异步线程池最大容量
    let core_pool_size = 4;
    // 设定异步线程池是否进行绑核处理
    let is_affinity = true;
    // 首次创建运行时环境
    let runtime_one = RuntimeBuilder::new_multi_thread()
        .is_affinity(is_affinity)
        .core_pool_size(core_pool_size)
        .build()
        .unwrap();

    // 设置异步线程池最大容量
    let core_pool_size = 4;
    // 设定异步线程池是否进行绑核处理
    let is_affinity = true;
    // 第二次创建运行时环境（只支持异步线程池）
    let runtime_two = RuntimeBuilder::new_multi_thread()
        .is_affinity(is_affinity)
        .core_pool_size(core_pool_size)
        .build()
        .unwrap();

    // 获取各自的任务钩子
    let handle_one = runtime_one.spawn(test_future(1));
    let handle_two = runtime_two.spawn(test_future(2));

    // 通过钩子获取任务执行结果
    let _result_one = runtime_one.block_on(handle_one).unwrap();
    let _result_two = runtime_two.block_on(handle_two).unwrap();
}
```

## ParIter 功能介绍

`ParIter` 及其相关接口定义于模块 `ylong_runtime::iter`，提供相应接口，如 `for_each`，将一些入参为闭包的顺序计算函数，使用异步任务，将其转换为并发计算，从而提升效率。总体的计算任务会被拆分成较小的子任务，待子任务完成后，`ParIter` 会将结果汇总，并返回最终结果。

当前支持功能为：
- for_each
  - 使用默认 feature
    ```rust
    use ylong_runtime::iter::{ParIter, AsParIterMut};
    use ylong_runtime::block_on;
    
    let mut slice = vec![0_u8, 1, 2, 3];
    let handler = slice.par_iter_mut()
                        .for_each(|x| *x += 2);
    block_on(handler).unwrap();
    assert_eq!(
        slice.as_slice(),
        [2, 3, 4, 5]
    );
    ```

  - 打开 `multi_runtime` feature 并使用自定义多线程运行时实例
    ```rust
    use ylong_runtime::iter::{ParIter, ParIterBuilder, AsParIterMut};
    use ylong_runtime::builder::RuntimeBuilder;
    use ylong_runtime::block_on;
    
    let mut slice = vec![0_u8, 1, 2, 3];
    
    let runtime = RuntimeBuilder::new_multi_thread().build().unwrap();
    let builder = ParIterBuilder::new(&runtime);
    let handler = slice.par_iter_mut_with_builder(&runtime)
                        .for_each(|x| *x += 2);
    block_on(handler).unwrap();
    assert_eq!(
        slice.as_slice(),
        [2, 3, 4, 5]
    );
    ```

  - 打开 `current_thread` feature 并使用基于当前线程的运行时实例
    ```rust
    use ylong_runtime::iter::{ParIter, ParIterBuilder, AsParIterMut};
    use ylong_runtime::builder::RuntimeBuilder;
    use ylong_runtime::block_on;
    
    let mut slice = vec![0_u8, 1, 2, 3];
    
    // The runtime passed in can be only set to run with the current thread
    let runtime = RuntimeBuilder::new_current_thread().build().unwrap();
    let builder = ParIterBuilder::new(&runtime);
    let handler = slice.par_iter_mut_with_builder(&builder)
                        .for_each(|x| *x += 2);
    runtime.block_on(handler).unwrap();
    assert_eq!(
        slice.as_slice(),
        [2, 3, 4, 5]
    );
    ```
    
## 致谢

基于用户的使用习惯，本库的API在将原本Rust标准库同步的接口实现改为异步后，保留了标准库原本的命名风格，如``TcpStream::connect``、``File::read``、``File::write``等。同时也参考了Tokio的部分通用API设计思想，我们在此对Rust标准库和Tokio表示感谢