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

//! One-shot channel is used to send a single message from a single sender to a single receiver.
//! The [`channel`] function returns a [`Sender`] and [`Receiver`] handle pair that controls channel.
//!
//! The `Sender` handle is used by the producer to send a message.
//! The `Receiver` handle is used by the consumer to receive the message. It has implemented the
//! `Future` trait
//!
//! The `send` method is not async. It can be called from non-async context.
//!
//! # Examples
//!
//! ```
//! use ylong_runtime::sync::oneshot;
//! async fn io_func() {
//!     let (tx, rx) = oneshot::channel();
//!     ylong_runtime::spawn(async move {
//!         if let Err(_) = tx.send(6) {
//!             println!("Receiver dropped");
//!         }
//!     });
//!
//!     match rx.await {
//!         Ok(v) => println!("received : {:?}", v),
//!         Err(_) => println!("Sender dropped")
//!     }
//! }
//! ```
use super::atomic_waker::AtomicWaker;
use super::error::RecvError;
use std::cell::RefCell;
use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::{AcqRel, Acquire, Release, SeqCst};
use std::sync::Arc;
use std::task::Poll::{Pending, Ready};
use std::task::{Context, Poll};

/// Initial state.
const INIT: usize = 0b00;
/// Sender has sent the value.
const SENT: usize = 0b01;
/// Channel is closed.
const CLOSED: usize = 0b10;

/// Creates a new one-shot channel with a `Sender` and `Receiver` handle pair.
///
/// The `Sender` can send a single value to the `Receiver`.
///
/// # Examples
///
/// ```
/// use ylong_runtime::sync::oneshot;
/// async fn io_func() {
///     let (tx, rx) = oneshot::channel();
///     ylong_runtime::spawn(async move {
///         if let Err(_) = tx.send(6) {
///             println!("Receiver dropped");
///         }
///     });
///
///     match rx.await {
///         Ok(v) => println!("received : {:?}", v),
///         Err(_) => println!("Sender dropped")
///     }
/// }
/// ```
pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let channel = Arc::new(Channel::new());
    let tx = Sender {
        channel: channel.clone(),
    };
    let rx = Receiver { channel };
    (tx, rx)
}

/// Sends a single value to the associated [`Receiver`].
/// A [`Sender`] and [`Receiver`] handle pair is created by the [`channel`] function.
///
/// # Examples
///
/// ```
/// use ylong_runtime::sync::oneshot;
/// async fn io_func() {
///     let (tx, rx) = oneshot::channel();
///     ylong_runtime::spawn(async move {
///         if let Err(_) = tx.send(6) {
///             println!("Receiver dropped");
///         }
///     });
///
///     match rx.await {
///         Ok(v) => println!("received : {:?}", v),
///         Err(_) => println!("Sender dropped")
///     }
/// }
/// ```
///
/// The receiver will fail with a [`RecvError`] if the sender is dropped without sending a value.
///
/// # Examples
///
/// ```
/// use ylong_runtime::sync::oneshot;
/// async fn io_func() {
///     let (tx, rx) = oneshot::channel::<()>();
///     ylong_runtime::spawn(async move {
///         drop(tx);
///     });
///
///     match rx.await {
///         Ok(v) => panic!("This won't happen"),
///         Err(_) => println!("Sender dropped")
///     }
/// }
/// ```
#[derive(Debug)]
pub struct Sender<T> {
    channel: Arc<Channel<T>>,
}

impl<T> Sender<T> {
    /// Sends a single value to the associated [`Receiver`], returns the value back
    /// if it fails to send.
    ///
    /// The sender will consume itself when calling this method. It can send a single value in
    /// synchronous code as it doesn't need waiting.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::sync::oneshot;
    /// async fn io_func() {
    ///     let (tx, rx) = oneshot::channel();
    ///     ylong_runtime::spawn(async move {
    ///         if let Err(_) = tx.send(6) {
    ///             println!("Receiver dropped");
    ///         }
    ///     });
    ///
    ///     match rx.await {
    ///         Ok(v) => println!("received : {:?}", v),
    ///         Err(_) => println!("Sender dropped")
    ///     }
    /// }
    /// ```
    pub fn send(self, value: T) -> Result<(), T> {
        self.channel.value.borrow_mut().replace(value);

        loop {
            match self.channel.state.load(Acquire) {
                INIT => {
                    if self
                        .channel
                        .state
                        .compare_exchange_weak(INIT, SENT, AcqRel, Acquire)
                        .is_ok()
                    {
                        self.channel.waker.wake();
                        return Ok(());
                    }
                }
                CLOSED => {
                    // value is stored in this function before.
                    return Err(self.channel.take_value().unwrap());
                }
                _ => unreachable!(),
            }
        }
    }

    /// Checks whether channel is closed. if so, the sender could not
    /// send any value anymore. It returns true if the [`Receiver`] is dropped
    /// or calls the [`close`] method.
    ///
    /// [`close`]: Receiver::close
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::sync::oneshot;
    /// async fn io_func() {
    ///     let (tx, rx) = oneshot::channel();
    ///     assert!(!tx.is_closed());
    ///
    ///     drop(rx);
    ///
    ///     assert!(tx.is_closed());
    ///     assert!(tx.send("no receive").is_err());
    /// }
    /// ```
    pub fn is_closed(&self) -> bool {
        self.channel.state.load(Acquire) == CLOSED
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        if self.channel.state.swap(SENT, SeqCst) == INIT {
            self.channel.waker.wake();
        }
    }
}

/// Receives a single value from the associated [`Sender`].
/// A [`Sender`] and [`Receiver`] handle pair is created by the [`channel`] function.
///
/// There is no `recv` method to receive the message because the receiver itself implements the
/// [`Future`] trait. To receive a value, `.await` the `Receiver` object directly.
///
/// # Examples
///
/// ```
/// use ylong_runtime::sync::oneshot;
/// async fn io_func() {
///     let (tx, rx) = oneshot::channel();
///     ylong_runtime::spawn(async move {
///         if let Err(_) = tx.send(6) {
///             println!("Receiver dropped");
///         }
///     });
///
///     match rx.await {
///         Ok(v) => println!("received : {:?}", v),
///         Err(_) => println!("Sender dropped")
///     }
/// }
/// ```
///
/// The receiver will fail with [`RecvError`], if the sender is dropped without sending a value.
///
/// # Examples
///
/// ```
/// use ylong_runtime::sync::oneshot;
/// async fn io_func() {
///     let (tx, rx) = oneshot::channel::<u32>();
///     ylong_runtime::spawn(async move {
///         drop(tx);
///     });
///
///     match rx.await {
///         Ok(v) => panic!("This won't happen"),
///         Err(_) => println!("Sender dropped")
///     }
/// }
/// ```
#[derive(Debug)]
pub struct Receiver<T> {
    channel: Arc<Channel<T>>,
}

impl<T> Receiver<T> {
    /// Attempts to receive a value from the associated [`Sender`].
    ///
    /// The method will still receive the result if the `Sender` gets dropped after
    /// sending the message.
    ///
    /// # Return value
    /// The function returns:
    ///  * `Ok(T)` if receiving a value successfully.
    ///  * `Err(RecvError::Empty)` if no value has been sent yet.
    ///  * `Err(RecvError::Closed)` if the sender has dropped without sending
    ///   a value, or if the message has already been received.
    ///
    /// # Examples
    ///
    /// `try_recv` before a value is sent, then after.
    ///
    /// ```
    /// use ylong_runtime::sync::error::RecvError;
    /// use ylong_runtime::sync::oneshot;
    /// async fn io_func() {
    ///     let (tx, mut rx) = oneshot::channel();
    ///     match rx.try_recv() {
    ///         Err(RecvError::Empty) => {}
    ///         _ => panic!("This won't happen"),
    ///     }
    ///
    ///     // Send a value
    ///     tx.send("Hello").unwrap();
    ///
    ///     match rx.try_recv() {
    ///         Ok(value) => assert_eq!(value, "Hello"),
    ///         _ => panic!("This won't happen"),
    ///     }
    /// }
    /// ```
    ///
    /// `try_recv` when the sender dropped before sending a value
    ///
    /// ```
    /// use ylong_runtime::sync::error::RecvError;
    /// use ylong_runtime::sync::oneshot;
    /// async fn io_func() {
    ///     let (tx, mut rx) = oneshot::channel::<()>();
    ///     drop(tx);
    ///
    ///     match rx.try_recv() {
    ///         Err(RecvError::Closed) => {}
    ///         _ => panic!("This won't happen"),
    ///     }
    /// }
    /// ```
    pub fn try_recv(&mut self) -> Result<T, RecvError> {
        match self.channel.state.load(Acquire) {
            INIT => Err(RecvError::Empty),
            SENT => self.channel.take_value_sent(),
            CLOSED => Err(RecvError::Closed),
            _ => unreachable!(),
        }
    }

    /// Closes the channel, prevents the `Sender` from sending a value.
    ///
    /// The `Sender` will fail to call [`send`] after the `Receiver` called
    /// `close`. It will do nothing if the channel is already closed or the message
    /// has been already received.
    ///
    /// [`send`]: Sender::send
    /// [`try_recv`]: Receiver::try_recv
    ///
    /// # Examples
    /// ```
    /// use ylong_runtime::sync::oneshot;
    /// async fn io_func() {
    ///     let (tx, mut rx) = oneshot::channel();
    ///     assert!(!tx.is_closed());
    ///
    ///     rx.close();
    ///
    ///     assert!(tx.is_closed());
    ///     assert!(tx.send("no receive").is_err());
    /// }
    /// ```
    ///
    /// Receive a value sent **before** calling `close`
    ///
    /// ```
    /// use ylong_runtime::sync::oneshot;
    /// async fn io_func() {
    ///     let (tx, mut rx) = oneshot::channel();
    ///     assert!(tx.send("Hello").is_ok());
    ///
    ///     rx.close();
    ///
    ///     let msg = rx.try_recv().unwrap();
    ///     assert_eq!(msg, "Hello");
    /// }
    /// ```
    pub fn close(&mut self) {
        let _ = self
            .channel
            .state
            .compare_exchange(INIT, CLOSED, AcqRel, Acquire);
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        self.close();
    }
}

impl<T> Future for Receiver<T> {
    type Output = Result<T, RecvError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.channel.state.load(Acquire) {
            INIT => {
                self.channel.waker.register_by_ref(cx.waker());
                if self.channel.state.load(Acquire) == SENT {
                    Ready(self.channel.take_value_sent())
                } else {
                    Pending
                }
            }
            SENT => Ready(self.channel.take_value_sent()),
            CLOSED => Ready(Err(RecvError::Closed)),
            _ => unreachable!(),
        }
    }
}

struct Channel<T> {
    /// The state of the channel.
    state: AtomicUsize,

    /// The value passed by channel, it is set by `Sender` and read by `Receiver`.
    value: RefCell<Option<T>>,

    /// The waker to notify the sender task or the receiver task.
    waker: AtomicWaker,
}

impl<T> Channel<T> {
    fn new() -> Channel<T> {
        Channel {
            state: AtomicUsize::new(INIT),
            value: RefCell::new(None),
            waker: AtomicWaker::new(),
        }
    }

    fn take_value_sent(&self) -> Result<T, RecvError> {
        match self.take_value() {
            Some(val) => {
                self.state.store(CLOSED, Release);
                Ok(val)
            }
            None => Err(RecvError::Closed),
        }
    }

    fn take_value(&self) -> Option<T> {
        self.value.borrow_mut().take()
    }
}

unsafe impl<T: Send> Send for Channel<T> {}
unsafe impl<T: Send> Sync for Channel<T> {}

impl<T> Drop for Channel<T> {
    fn drop(&mut self) {
        self.waker.take_waker();
    }
}

impl<T: Debug> Debug for Channel<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Channel")
            .field("state", &self.state.load(Acquire))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use crate::spawn;
    use crate::sync::error::RecvError;
    use crate::sync::oneshot;

    /// UT test for `send()` and `try_recv()`.
    ///
    /// # Title
    /// send_try_recv
    ///
    /// # Brief
    /// 1.Call channel to create a sender and a receiver handle pair.
    /// 2.Receiver tries receiving a message before the sender sends one.
    /// 3.Receiver tries receiving a message after the sender sends one.
    /// 4.Check if the test results are correct.
    #[test]
    fn send_try_recv() {
        let (tx, mut rx) = oneshot::channel();
        match rx.try_recv() {
            Err(RecvError::Empty) => {}
            _ => unreachable!(),
        }
        tx.send("hello").unwrap();

        match rx.try_recv() {
            Ok(value) => assert_eq!(value, "hello"),
            _ => unreachable!(),
        }

        match rx.try_recv() {
            Err(RecvError::Closed) => {}
            _ => unreachable!(),
        }
    }

    /// UT test for `send()` and async receive.
    ///
    /// # Title
    /// send_recv_await
    ///
    /// # Brief
    /// 1.Call channel to create a sender and a receiver handle pair.
    /// 2.Sender sends message in ont thread.
    /// 3.Receiver receives message in another thread.
    /// 4.Check if the test results are correct.
    #[test]
    fn send_recv_await() {
        let (tx, rx) = oneshot::channel();
        if tx.send(6).is_err() {
            panic!("Receiver dropped");
        }
        spawn(async move {
            match rx.await {
                Ok(v) => assert_eq!(v, 6),
                Err(_) => panic!("Sender dropped"),
            }
        });
    }

    /// UT test for `is_closed()` and `close`.
    ///
    /// # Title
    /// close_rx
    ///
    /// # Brief
    /// 1.Call channel to create a sender and a receiver handle pair.
    /// 2.Check whether the sender is closed.
    /// 3.Close the receiver.
    /// 4.Check whether the receiver will receive the message sent before it closed.
    /// 5.Check if the test results are correct.
    #[test]
    fn close_rx() {
        let (tx, mut rx) = oneshot::channel();
        assert!(!tx.is_closed());
        rx.close();

        assert!(tx.is_closed());
        assert!(tx.send("never received").is_err());

        let (tx, mut rx) = oneshot::channel();
        assert!(tx.send("will receive").is_ok());

        rx.close();

        let msg = rx.try_recv().unwrap();
        assert_eq!(msg, "will receive");
    }
}
