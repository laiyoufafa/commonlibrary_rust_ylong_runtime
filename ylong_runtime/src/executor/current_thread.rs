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

use crate::cfg_io;
use crate::executor::Schedule;
use crate::task::{JoinHandle, Task, TaskBuilder, VirtualTableType};
use std::collections::VecDeque;
use std::future::Future;
use std::mem;
use std::pin::Pin;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::{Acquire, Release};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
cfg_io!(
    use std::time::Duration;
    use crate::net::Driver;
    use crate::net::Handle;
);

pub(crate) struct CurrentThreadSpawner {
    pub(crate) scheduler: Arc<CurrentThreadScheduler>,
    parker: Arc<Parker>,
    #[cfg(feature = "net")]
    pub(crate) handle: Arc<Handle>,
}

#[derive(Default)]
pub(crate) struct CurrentThreadScheduler {
    pub(crate) inner: Mutex<VecDeque<Task>>,
}

unsafe impl Sync for CurrentThreadScheduler {}

impl Schedule for CurrentThreadScheduler {
    #[inline]
    fn schedule(&self, task: Task, _lifo: bool) {
        let mut queue = self.inner.lock().unwrap();
        queue.push_back(task);
    }
}

impl CurrentThreadScheduler {
    fn pop(&self) -> Option<Task> {
        let mut queue = self.inner.lock().unwrap();
        queue.pop_front()
    }
}

pub(crate) struct Parker {
    is_wake: AtomicBool,
    #[cfg(feature = "net")]
    driver: Arc<Mutex<Driver>>,
}

impl Parker {
    fn new(#[cfg(feature = "net")] driver: Arc<Mutex<Driver>>) -> Parker {
        Parker {
            is_wake: AtomicBool::new(false),
            #[cfg(feature = "net")]
            driver,
        }
    }

    fn wake(&self) {
        self.is_wake.store(true, Release);
    }

    fn get_wake_up(&self) -> bool {
        self.is_wake
            .compare_exchange(true, false, Release, Acquire)
            .is_ok()
    }
}

static CURRENT_THREAD_RAW_WAKER_VIRTUAL_TABLE: RawWakerVTable =
    RawWakerVTable::new(clone, wake, wake_by_ref, drop);

fn clone(ptr: *const ()) -> RawWaker {
    let parker = unsafe { Arc::from_raw(ptr as *const Parker) };

    // increment the ref count
    mem::forget(parker.clone());

    let data = Arc::into_raw(parker) as *const ();
    RawWaker::new(data, &CURRENT_THREAD_RAW_WAKER_VIRTUAL_TABLE)
}

fn wake(ptr: *const ()) {
    let parker = unsafe { Arc::from_raw(ptr as *const Parker) };
    parker.wake();
}

fn wake_by_ref(ptr: *const ()) {
    let parker = unsafe { Arc::from_raw(ptr as *const Parker) };
    parker.wake();
    mem::forget(parker);
}

fn drop(ptr: *const ()) {
    unsafe { mem::drop(Arc::from_raw(ptr as *const Parker)) };
}

impl CurrentThreadSpawner {
    #[cfg(not(feature = "net"))]
    pub(crate) fn new() -> Self {
        Self {
            scheduler: Default::default(),
            parker: Arc::new(Parker::new()),
        }
    }

    #[cfg(feature = "net")]
    pub(crate) fn new() -> Self {
        let (handle, driver) = crate::net::Driver::initialize();
        Self {
            scheduler: Default::default(),
            parker: Arc::new(Parker::new(driver)),
            handle,
        }
    }

    fn waker(&self) -> Waker {
        let data = Arc::into_raw(self.parker.clone()) as *const ();
        unsafe { Waker::from_raw(RawWaker::new(data, &CURRENT_THREAD_RAW_WAKER_VIRTUAL_TABLE)) }
    }

    pub(crate) fn spawn<T>(&self, builder: &TaskBuilder, task: T) -> JoinHandle<T::Output>
    where
        T: Future + Send + 'static,
        T::Output: Send + 'static,
    {
        let scheduler = Arc::downgrade(&self.scheduler);
        let (task, handle) = Task::create_task(builder, scheduler, task, VirtualTableType::Ylong);

        let mut queue = self.scheduler.inner.lock().unwrap();
        queue.push_back(task);

        handle
    }

    pub(crate) fn block_on<T>(&self, future: T) -> T::Output
    where
        T: Future,
    {
        let waker = self.waker();
        let mut cx = Context::from_waker(&waker);

        let mut future = future;
        let mut future = unsafe { Pin::new_unchecked(&mut future) };

        if let Poll::Ready(res) = future.as_mut().poll(&mut cx) {
            return res;
        }

        loop {
            if self.parker.get_wake_up() {
                if let Poll::Ready(res) = future.as_mut().poll(&mut cx) {
                    return res;
                }
            }
            if let Some(task) = self.scheduler.pop() {
                task.run();
            } else {
                #[cfg(feature = "net")]
                if let Ok(mut driver) = self.parker.driver.try_lock() {
                    let _ = driver
                        .drive(Some(Duration::from_millis(0)))
                        .expect("io driver failed");
                }
            }
        }
    }
}

#[cfg(all(test))]
mod test {
    macro_rules! cfg_sync {
        ($($item:item)*) => {
            $(
                #[cfg(feature = "sync")]
                $item
            )*
        }
    }
    macro_rules! cfg_net {
        ($($item:item)*) => {
            $(
                #[cfg(feature = "net")]
                $item
            )*
        }
    }
    use crate::executor::current_thread::CurrentThreadSpawner;
    use crate::task::{yield_now, TaskBuilder};
    use std::future::Future;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    cfg_sync! {
        use crate::sync::Waiter;
        use std::sync::atomic::AtomicUsize;
        use std::sync::atomic::Ordering::{Acquire, Release};
        use std::sync::{Condvar, Mutex};
        use std::sync::Arc;

        pub(crate) struct Parker {
            mutex: Mutex<bool>,
            condvar: Condvar,
        }

        impl Parker {
            fn new() -> Parker {
                Parker {
                    mutex: Mutex::new(false),
                    condvar: Condvar::new(),
                }
            }

            fn notified(&self) {
                let mut guard = self.mutex.lock().unwrap();

                while !*guard {
                    guard = self.condvar.wait(guard).unwrap();
                }
                *guard = false;
            }

            fn notify_one(&self) {
                let mut guard = self.mutex.lock().unwrap();
                *guard = true;
                drop(guard);
                self.condvar.notify_one();
            }
        }
    }

    cfg_net! {
        use std::net::SocketAddr;
        use crate::net::{TcpListener, TcpStream};
        use crate::io::{AsyncReadExt, AsyncWriteExt};

        pub async fn ylong_tcp_server(addr: SocketAddr) {
            let tcp = TcpListener::bind(addr).await.unwrap();
            let (mut stream, _) = tcp.accept().await.unwrap();
            for _ in 0..3 {
                let mut buf = [0; 100];
                stream.read_exact(&mut buf).await.unwrap();
                assert_eq!(buf, [3; 100]);

                let buf = [2; 100];
                stream.write_all(&buf).await.unwrap();
            }
        }

        pub async fn ylong_tcp_client(addr: SocketAddr) {
            let mut tcp = TcpStream::connect(addr).await;
            while tcp.is_err() {
                tcp = TcpStream::connect(addr).await;
            }
            let mut tcp = tcp.unwrap();
            for _ in 0..3 {
                let buf = [3; 100];
                tcp.write_all(&buf).await.unwrap();

                let mut buf = [0; 100];
                tcp.read_exact(&mut buf).await.unwrap();
                assert_eq!(buf, [2; 100]);
            }
        }
    }

    struct YieldTask {
        cnt: u8,
    }

    impl Future for YieldTask {
        type Output = ();

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            if self.cnt > 0 {
                self.cnt -= 1;
                cx.waker().wake_by_ref();
                Poll::Pending
            } else {
                Poll::Ready(())
            }
        }
    }

    /// UT test for `spawn()`.
    ///
    /// # Title
    /// ut_current_thread_block_on
    ///
    /// # Brief
    /// 1.Check the running status of tasks in the queue when the yield task is awakened once.
    /// 2.Check the running status of tasks in the queue when the yield task is awakened twice.
    /// 3.Check the running status of tasks in the queue when the yield task is awakened three times.
    #[test]
    fn ut_current_thread_spawn() {
        let spawner = CurrentThreadSpawner::new();
        spawner.spawn(&TaskBuilder::default(), async move { yield_now().await });
        spawner.spawn(&TaskBuilder::default(), async move { yield_now().await });
        spawner.block_on(YieldTask { cnt: 1 });
        assert_eq!(spawner.scheduler.inner.lock().unwrap().len(), 2);

        let spawner = CurrentThreadSpawner::new();
        spawner.spawn(&TaskBuilder::default(), async move { yield_now().await });
        spawner.spawn(&TaskBuilder::default(), async move { yield_now().await });
        spawner.block_on(YieldTask { cnt: 2 });
        assert_eq!(spawner.scheduler.inner.lock().unwrap().len(), 2);

        let spawner = CurrentThreadSpawner::new();
        spawner.spawn(&TaskBuilder::default(), async move { yield_now().await });
        spawner.spawn(&TaskBuilder::default(), async move { yield_now().await });
        spawner.block_on(YieldTask { cnt: 3 });
        assert_eq!(spawner.scheduler.inner.lock().unwrap().len(), 2);
    }

    /// UT test for `block_on()`.
    ///
    /// # Title
    /// ut_current_thread_block_on
    ///
    /// # Brief
    /// 1.Check the running status of tasks in the queue when the yield task is awakened once.
    /// 2.Check the running status of tasks in the queue when the yield task is awakened twice.
    /// 3.Check the running status of tasks in the queue when the yield task is awakened three times.
    #[test]
    fn ut_current_thread_block_on() {
        let spawner = CurrentThreadSpawner::new();
        spawner.spawn(&TaskBuilder::default(), async move { 1 });
        spawner.spawn(&TaskBuilder::default(), async move { 1 });
        spawner.block_on(YieldTask { cnt: 1 });
        assert_eq!(spawner.scheduler.inner.lock().unwrap().len(), 2);

        let spawner = CurrentThreadSpawner::new();
        spawner.spawn(&TaskBuilder::default(), async move { 1 });
        spawner.spawn(&TaskBuilder::default(), async move { 1 });
        spawner.block_on(YieldTask { cnt: 2 });
        assert_eq!(spawner.scheduler.inner.lock().unwrap().len(), 1);

        let spawner = CurrentThreadSpawner::new();
        spawner.spawn(&TaskBuilder::default(), async move { 1 });
        spawner.spawn(&TaskBuilder::default(), async move { 1 });
        spawner.block_on(YieldTask { cnt: 3 });
        assert_eq!(spawner.scheduler.inner.lock().unwrap().len(), 0);
    }

    /// UT test for `spawn()` and `block_on()`.
    ///
    /// # Title
    /// ut_current_thread_run_queue
    ///
    /// # Brief
    /// 1.Spawn two tasks before the blocked task running and check the status of two tasks.
    /// 2.Spawn two tasks after the blocked task running and check the status of two tasks.
    #[test]
    #[cfg(feature = "sync")]
    fn ut_current_thread_run_queue() {
        use crate::builder::RuntimeBuilder;
        let spawner = Arc::new(RuntimeBuilder::new_current_thread().build().unwrap());

        let finished = Arc::new(AtomicUsize::new(0));

        let finished_clone = finished.clone();
        let notify1 = Arc::new(Parker::new());
        let notify1_clone = notify1.clone();
        spawner.spawn(async move {
            finished_clone.fetch_add(1, Release);
            notify1_clone.notify_one();
        });

        let finished_clone = finished.clone();
        let notify2 = Arc::new(Parker::new());
        let notify2_clone = notify2.clone();
        spawner.spawn(async move {
            finished_clone.fetch_add(1, Release);
            notify2_clone.notify_one();
        });

        let waiter = Arc::new(Waiter::new());
        let waiter_clone = waiter.clone();
        let spawner_clone = spawner.clone();
        let join = std::thread::spawn(move || {
            spawner_clone.block_on(async move { waiter_clone.wait().await })
        });

        notify1.notified();
        notify2.notified();
        assert_eq!(finished.load(Acquire), 2);

        let finished_clone = finished.clone();
        let notify1 = Arc::new(Parker::new());
        let notify1_clone = notify1.clone();
        spawner.spawn(async move {
            finished_clone.fetch_add(1, Release);
            notify1_clone.notify_one();
        });

        let finished_clone = finished.clone();
        let notify2 = Arc::new(Parker::new());
        let notify2_clone = notify2.clone();
        spawner.spawn(async move {
            finished_clone.fetch_add(1, Release);
            notify2_clone.notify_one();
        });

        notify1.notified();
        notify2.notified();
        assert_eq!(finished.load(Acquire), 4);

        waiter.wake_one();
        join.join().unwrap();

        #[cfg(feature = "net")]
        crate::executor::worker::CURRENT_WORKER.with(|ctx| {
            ctx.set(std::ptr::null());
        });
    }

    /// UT test for io tasks.
    ///
    /// # Title
    /// ut_current_thread_io
    ///
    /// # Brief
    /// 1.Spawns a tcp server to read and write data for three times.
    /// 2.Spawns a tcp client to read and write data for three times.
    #[test]
    #[cfg(feature = "net")]
    fn ut_current_thread_io() {
        use crate::builder::RuntimeBuilder;

        let spawner = RuntimeBuilder::new_current_thread().build().unwrap();
        let addr = "127.0.0.1:8701".parse().unwrap();
        spawner.spawn(ylong_tcp_server(addr));
        spawner.block_on(ylong_tcp_client(addr));

        let spawner = RuntimeBuilder::new_current_thread().build().unwrap();
        let addr = "127.0.0.1:8702".parse().unwrap();
        spawner.spawn(ylong_tcp_client(addr));
        spawner.block_on(ylong_tcp_server(addr));
    }
}
