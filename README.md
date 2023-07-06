# ylong_runtime

## Introduction
Rust asynchronous runtime, provides functionalities such as spawning async tasks, async io, async synchronization primitives, parallel calculation.

### Overall structure
![structure](./figures/structure.png)

- `System Service`: System services that use Rust's asynchronous capabilities, including Request, IPC, NetStack, etc.
- `Runtime`: Rust asynchronous runtime provides Rust asynchronous programming interface and task scheduling capabilities for system services. From the functional level, it can be divided into two layers: `ylong_runtime` and `ylong_io`:
   - `ylong_runtime`: The functional body of the asynchronous runtime, which provides a large number of asynchronous version of the Rust std library interfaces, and also provides capabilities such as life cycle management and scheduling of user tasks.
   - `ylong_io`: Relying on the `libc` library, combined with the epoll mechanism of the system, providing the non-blocking TCP and UDP functions, as the base of `ylong_runtime` asynchronous IO.
- `libc`: Rust third party library, providing Rust encapsulation of system libc interface.
- `Kernel`: Capabilities provided by the operating system kernel, such as socket, epoll, etc.

### Crates inner relations
![inner_dependencies](./figures/inner_dependencies.png)

`ylong_runtime` is the main crate of the repository, and users can directly rely on this library when using it. `ylong_runtime` depends on the following three crates:

- `ylong_io`: Provides nonblocking and event-driven TCP/UDP through epoll. Users do not need to directly depend on this library.
- `ylong_ffrt`: Provides a Rust wrapper of the Function Flow Runtime interface, which can be used as the underlying task scheduler of `ylong_runtime`. Users can configure whether to use this scheduler through the feature `ffrt` of `ylong_runtime`, and this scheduler is used by default on OpenHarmony. Users do not need to directly depend on this library.
- `ylong_macros`: The procedural macros required to implement `ylong_runtime`, currently mainly used to provide `select!` procedural macros. Users can configure whether to use this library through the feature `macros` of `ylong_runtime`, which is used by default on OpenHarmony. Users do not need to directly depend on this library.

### ylong_runtime framework
![runtime_framework](./figures/runtime_framework.png)

`ylong_runtime` external API is divided into four modules:

- `Sync`: Asynchronous synchronization primitives, which can be used in an asynchronous context, including asynchronous mutex, read-write lock, semaphore, channel, etc.
- `Async IO`: Asynchronous network IO & file IO, providing IO interfaces that can be used in an asynchronous context, including creation, closing, reading, writing of TCP, UDP, file.
- `Parallel Calculation`: Parallel computing function, which supports automatic splitting of user data into multiple small tasks for parallel processing, and users can asynchronously wait for the processing results of tasks in an asynchronous context.
- `Timer`: asynchronous timer, providing timing functions, including asynchronous sleep, interval, etc.

The feature of the asynchronous interface is that waiting in the asynchronous context will not block the current thread, and the thread will automatically switch to the next executable task, thereby avoiding the waste of thread resources and improving the overall concurrency of the system.

This capability of asynchronous interfaces is achieved through the `Reactor` and `Executor` modules:

- `Reactor`: Monitors IO system events and Timer events, wakes up blocked tasks through the monitored events, and pushes the tasks to the task queue of `Executor`:
   - `IO Driver`: IO event poller, checks whether there are IO events coming;
   - `Timer Driver`: Timer event poller, checks whether a Timer is about to time out;
- `Executor`: The subject of task scheduling and task execution. The task submitted by the user enters the task queue of `Executor` and is executed at the appropriate time. If a task is blocked during execution, `Executor` will select the next executable task to execute according to some strategies. `ylong_runtime` has two schedulers that are interchangeable:
   - `ylong executor`: A task scheduler implemented in Rust.
   - `FFRT executor`: Function Flow Runtime task scheduler, implemented in C++. This implementation is used by default on OpenHarmony.

## Compile

Method 1: Introduce `ylong_runtime` in `Cargo.toml`

```toml
#[dependencies]
ylong_runtime = { git = "https://gitee.com/openharmony-sig/commonlibrary_rust_ylong_runtime.git", features = ["full"]}
```

Method 2: Add dependencies to `BUILD.gn` where appropriate

```
deps += ["//commonlibrary/rust/ylong_runtime/ylong_runtime:lib"]
```

## Directory
```
ylong_runtime
|── docs                            # User guide
|── figures                         # Structure figures in docspo
|── patches                         # Patches for ci
|── ylong_ffrt
|    └── src                        # FFRT rust ffi
|── ylong_io
|    |── exmaples                   # Examples of ylong_io 
|    |── src                        # Source code of ylong_io
|    |    └── sys                   # OS specific implementation
|    |         |── linux            # Epoll driven io
|    |         └── windows          # Iocp driven io
|── ylong_runtime                   
|    |── benches                    # Benchmarks of ylong_runtime
|    |── examples                   # Examples of ylong_runtime
|    |── src                        # Source code of ylong_runtime
|    |    |── builder               # Runtime builder
|    |    |── executor              # Runtime executor
|    |    |── ffrt                  # FFRT adapter
|    |    |── fs                    # Async fs components
|    |    |── io                    # Async io traits and components
|    |    |   └── buffered          # Async BufReader and BufWriter
|    |    |── iter                  # Async parallel iterator
|    |    |   |── parallel          # ParIter implementation for data containers
|    |    |   └── pariter           # Core of pariter
|    |    |── net                   # Async net io and net driver
|    |    |   └── sys               # Async system io
|    |    |       └── tcp           # Async Tcp
|    |    |── sync                  # Runtime synchronization components
|    |    |   └── mpsc              # Mpsc channels
|    |    |── task                  # Async task components
|    |    |── time                  # Timer components
|    |    └── util                  # Utilities
|    |        |── core_affinity     # Vore affinity components
|    |        └── num_cpus          # Num cpus components
|    └── tests                      # Sdv of ylong_runtime
└── ylong_runtime_macros
     |── examples                   # Examples of ylong_macro
     └── src                        # Procedural macro implementation for runtime
```

## User Guide

See [user_guide](./docs/user_guide.md).

## Acknowledgements

Based on the user's habit, the API of this library, after changing the original Rust standard library synchronous interface implementation to asynchronous, retains the original naming style of the standard library, such as ``TcpStream::connect``, ``File::read``, ``File::write`` and so on. We also refer to some of Tokio's general API design ideas, and we would like to express our gratitude to the Rust standard library and Tokio.

