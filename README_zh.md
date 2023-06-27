# ylong_runtime

## 简介
Rust异步运行时库，用于生成并执行异步任务。主要分为Time, Sync, Parallel calculation, IO四个模块。

### 图一 整体架构图
![structure](./figure/structure.png)

## 目录
```
ylong_runtime
|── ylong_ffrt
|    └── src                        # FFRT ffi封装
|── ylong_io
|    |── exmaples                   # ylong_io 代码示例
|    |── src                        # ylong_io 源码
|    |    └── sys                   # 操作系统相关io实现
|    |         |── linux            # Linux 事件驱动IO实现
|    |         └── windows          # Windows 事件驱动IO实现
|── ylong_runtime                   
|    |── benches                    # ylong_runtime 性能用例
|    |── examples                   # ylong_runtime 代码示例
|    |── src                        # ylong_runtime 源码
|    |    |── builder               # Runtime builder实现
|    |    |── executor              # Runtime executor实现
|    |    |── ffrt                  # FFRT 适配
|    |    |── fs                    # 异步文件IO实现
|    |    |── io                    # 异步IO接口以及对外API
|    |    |   └── buffered          # 异步缓存读写实现
|    |    |── iter                  # 异步并行迭代器实现
|    |    |   |── parallel          # 数据容器适配
|    |    |   └── pariter           # 并行迭代核心业务实现
|    |    |── net                   # 异步网络IO/Driver实现
|    |    |   └── sys               # 系统IO异步实现
|    |    |       └── tcp           # 异步TCP实现
|    |    |── sync                  # 异步同步原语
|    |    |   └── mpsc              # 单生产者多消费者通道实现
|    |    |── task                  # 异步任务实现
|    |    |── time                  # 定时器实现
|    |    └── util                  # 公共组件
|    |        |── core_affinity     # 绑核实现
|    |        └── num_cpus          # 获取核数实现
|    └── tests                      # ylong_runtime 测试用例
└── ylong_runtime_macros            # ylong_runtime 宏实现
```

## 编译构建

方法一：在Cargo.toml中引入ylong_runtime

```toml
#[dependence]
ylong_runtime = { git = "https://gitee.com/openharmony-sig/commonlibrary_rust_ylong_runtime.git", version = "1.9.0", features = ["full"]}
```

如果需要编译ffrt版本，将ylong_ffrt目录下的``build_ffrt.rs``文件重命名为``build.rs``, 并设置`LD_LIBRARY_PATH`

方法二：在 BUILD.gn 合适的地方添加依赖

```
deps += ["//commonlibrary/rust/ylong_runtime/ylong_runtime:lib"]
```

## 用户指南

详情内容请见[用户指南](./docs/user_guide.md)

## 致谢

基于用户的使用习惯，本库的API在将原本Rust标准库同步的接口实现改为异步后，保留了标准库原本的命名风格，如``TcpStream::connect``、``File::read``、``File::write``等。同时也参考了Tokio的部分通用API设计思想，在此对Rust标准库和Tokio表示感谢