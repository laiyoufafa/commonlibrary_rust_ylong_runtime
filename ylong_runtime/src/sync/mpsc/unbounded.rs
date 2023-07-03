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

//! Unbounded channel

use crate::sync::error::{RecvError, SendError};
use crate::sync::mpsc::channel::{channel, Rx, Tx};
use crate::sync::mpsc::queue::Queue;
use crate::sync::mpsc::Container;
cfg_time!(
    use crate::time::timeout;
    use std::time::Duration;
);
/// The sender of unbounded channel.
/// A [`UnboundedSender`] and [`UnboundedReceiver`] handle pair are created by the
/// [`unbounded_channel`] function.
///
/// # Examples
///
/// ```
/// use ylong_runtime::sync::mpsc::unbounded::unbounded_channel;
/// async fn io_func() {
///     let (tx, mut rx) = unbounded_channel();
///     let tx2 = tx.clone();
///     assert!(tx.send(1).is_ok());
///     assert!(!tx.is_closed());
///     assert!(tx.is_same(&tx2));
///     let handle = ylong_runtime::spawn(async move {
///         assert_eq!(rx.recv().await, Ok(1));
///     });
/// }
/// ```
pub struct UnboundedSender<T> {
    channel: Tx<Queue<T>>,
}

impl<T> Clone for UnboundedSender<T> {
    fn clone(&self) -> Self {
        UnboundedSender {
            channel: self.channel.clone(),
        }
    }
}

/// The receiver of unbounded channel.
/// A [`UnboundedSender`] and [`UnboundedReceiver`] handle pair are created by the
/// [`unbounded_channel`] function.
///
/// # Examples
///
/// ```
/// use ylong_runtime::sync::mpsc::unbounded::unbounded_channel;
/// async fn io_func() {
///     let (tx, mut rx) = unbounded_channel();
///     assert!(rx.try_recv().is_err());
///     assert!(tx.send(1).is_ok());
///     let handle = ylong_runtime::spawn(async move {
///         assert_eq!(rx.len(), 1);
///         assert_eq!(rx.recv().await, Ok(1));
///     });
/// }
/// ```
pub struct UnboundedReceiver<T> {
    channel: Rx<Queue<T>>,
}

/// Creates a new mpsc channel and returns a `Sender` and `Receiver` handle pair.
///
/// # Examples
///
/// ```
/// use ylong_runtime::sync::mpsc::unbounded::unbounded_channel;
/// async fn io_func() {
///     let (tx, mut rx) = unbounded_channel();
///     let handle = ylong_runtime::spawn(async move {
///         assert_eq!(rx.recv().await, Ok(1));
///     });
///     assert!(tx.send(1).is_ok());
/// }
/// ```
pub fn unbounded_channel<T>() -> (UnboundedSender<T>, UnboundedReceiver<T>) {
    let queue = Queue::new();
    let (tx, rx) = channel(queue);
    (UnboundedSender::new(tx), UnboundedReceiver::new(rx))
}

impl<T> UnboundedSender<T> {
    fn new(channel: Tx<Queue<T>>) -> UnboundedSender<T> {
        UnboundedSender { channel }
    }

    /// Sends values to the associated receiver.
    ///
    /// An error containing the sent value would be returned if the receiver is closed or dropped.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::sync::mpsc::unbounded::unbounded_channel;
    /// let (tx, mut rx) = unbounded_channel();
    /// assert!(tx.send(1).is_ok());
    /// assert_eq!(rx.try_recv().unwrap(), 1);
    /// ```
    pub fn send(&self, value: T) -> Result<(), SendError<T>> {
        self.channel.send(value)
    }

    /// Checks whether the channel is closed. If so, the sender could not
    /// send values anymore. It returns true if the [`UnboundedReceiver`] is dropped
    /// or calls the [`close`] method.
    ///
    /// [`close`]: UnboundedReceiver::close
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::sync::mpsc::unbounded::unbounded_channel;
    /// let (tx, rx) = unbounded_channel::<isize>();
    /// assert!(!tx.is_closed());
    /// drop(rx);
    /// assert!(tx.is_closed());
    /// ```
    pub fn is_closed(&self) -> bool {
        self.channel.is_close()
    }

    /// Checks whether the sender and another sender belong to the same channel
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::sync::mpsc::unbounded::unbounded_channel;
    /// let (tx, rx) = unbounded_channel::<isize>();
    /// let tx2 = tx.clone();
    /// assert!(tx.is_same(&tx2));
    /// ```
    pub fn is_same(&self, other: &Self) -> bool {
        self.channel.is_same(&other.channel)
    }

    /// Gets the number of values in the channel.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::sync::mpsc::unbounded::unbounded_channel;
    /// let (tx, rx) = unbounded_channel();
    /// assert_eq!(tx.len(), 0);
    /// tx.send(1).unwrap();
    /// assert_eq!(tx.len(), 1);
    /// ```
    pub fn len(&self) -> usize {
        self.channel.len()
    }

    /// Returns `true` if the channel contains no elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::sync::mpsc::unbounded::unbounded_channel;
    /// let (tx, rx) = unbounded_channel();
    /// assert!(tx.is_empty());
    /// tx.send(1).unwrap();
    /// assert!(!tx.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T> Drop for UnboundedSender<T> {
    fn drop(&mut self) {
        self.channel.close();
    }
}

impl<T> UnboundedReceiver<T> {
    fn new(channel: Rx<Queue<T>>) -> UnboundedReceiver<T> {
        UnboundedReceiver { channel }
    }

    /// Gets the number of values in the channel.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::sync::mpsc::unbounded::unbounded_channel;
    /// let (tx, rx) = unbounded_channel();
    /// tx.send(1).unwrap();
    /// tx.send(2).unwrap();
    /// assert_eq!(rx.len(), 2);
    /// ```
    pub fn len(&self) -> usize {
        self.channel.len()
    }

    /// Returns `true` if the channel contains no elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::sync::mpsc::unbounded::unbounded_channel;
    /// let (tx, rx) = unbounded_channel();
    /// assert!(rx.is_empty());
    /// tx.send(1).unwrap();
    /// assert!(!rx.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Attempts to receive a value from the associated [`UnboundedSender`].
    ///
    /// # Return value
    /// * `Ok(T)` if receiving a value successfully.
    /// * `Err(RecvError::Empty)` if no value has been sent yet.
    /// * `Err(RecvError::Closed)` if all senders have been dropped.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::sync::error::RecvError;
    /// use ylong_runtime::sync::mpsc::unbounded::unbounded_channel;
    /// let (tx, mut rx) = unbounded_channel();
    /// match rx.try_recv() {
    ///     Err(RecvError::Empty) => {}
    ///     _ => panic!("This won't happen"),
    /// }
    /// tx.send(1).unwrap();
    /// match rx.try_recv() {
    ///     Ok(_) => {}
    ///     _ => panic!("This won't happen"),
    /// }
    /// drop(tx);
    /// match rx.try_recv() {
    ///     Err(RecvError::Closed) => {}
    ///     _ => panic!("This won't happen"),
    /// }
    /// ```
    pub fn try_recv(&mut self) -> Result<T, RecvError> {
        self.channel.try_recv()
    }

    /// Receives a value from the associated [`UnboundedSender`].
    ///
    /// The `receiver` can still receive all sent messages in the channel after the channel is closed.
    ///
    /// # Return value
    /// * `Ok(T)` if receiving a value successfully.
    /// * `Err(RecvError::Closed)` if all senders have been dropped.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::sync::mpsc::unbounded::unbounded_channel;
    /// async fn io_func() {
    ///     let (tx, mut rx) = unbounded_channel();
    ///     let handle = ylong_runtime::spawn(async move {
    ///         assert_eq!(rx.recv().await, Ok(1));
    ///     });
    ///     assert!(tx.send(1).is_ok());
    /// }
    /// ```
    pub async fn recv(&mut self) -> Result<T, RecvError> {
        self.channel.recv().await
    }

    /// Attempts to receive a value from the associated [`UnboundedSender`] in a limited
    /// amount of time.
    ///
    /// The `receiver` can still receive all sent messages in the channel after the channel
    /// is closed.
    ///
    /// # Return value
    /// * `Ok(T)` if receiving a value successfully.
    /// * `Err(RecvError::Closed)` if all senders have been dropped.
    /// * `Err(RecvError::TimeOut)` if receiving timeout has elapsed.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    /// use ylong_runtime::sync::mpsc::unbounded::unbounded_channel;
    /// async fn io_func() {
    ///     let (tx, mut rx) = unbounded_channel();
    ///     let handle = ylong_runtime::spawn(async move {
    ///         tx.send(1).unwrap();
    ///         assert_eq!(rx.recv_timeout(Duration::from_millis(10)).await, Ok(1));
    ///     });
    /// }
    /// ```
    #[cfg(feature = "time")]
    pub async fn recv_timeout(&mut self, time: Duration) -> Result<T, RecvError> {
        match timeout(time, self.channel.recv()).await {
            Ok(res) => res,
            Err(_) => Err(RecvError::TimeOut),
        }
    }

    /// Closes the channel, prevents the `Sender` from sending more values.
    ///
    /// The `Sender` will fail to call [`send`] after the `Receiver` called
    /// `close`. It will do nothing if the channel is already closed.
    ///
    /// [`send`]: UnboundedSender::send
    ///
    /// # Examples
    /// ```
    /// use ylong_runtime::sync::mpsc::unbounded::unbounded_channel;
    /// async fn io_func() {
    ///     let (tx, mut rx) = unbounded_channel();
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
    /// use ylong_runtime::sync::mpsc::unbounded::unbounded_channel;
    /// async fn io_func() {
    ///     let (tx, mut rx) = unbounded_channel();
    ///     assert!(tx.send("Hello").is_ok());
    ///
    ///     rx.close();
    ///
    ///     let msg = rx.try_recv().unwrap();
    ///     assert_eq!(msg, "Hello");
    /// }
    /// ```
    pub fn close(&mut self) {
        self.channel.close();
    }
}

impl<T> Drop for UnboundedReceiver<T> {
    fn drop(&mut self) {
        self.channel.close();
    }
}
