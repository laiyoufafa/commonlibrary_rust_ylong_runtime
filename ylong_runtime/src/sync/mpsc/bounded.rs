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

//! Bounded channel

use crate::sync::error::{RecvError, SendError};
use crate::sync::mpsc::array::Array;
use crate::sync::mpsc::channel::{channel, Rx, Tx};
use crate::sync::mpsc::Container;
cfg_time!(
    use crate::sync::mpsc::array::SendPosition;
    use crate::time::timeout;
    use std::time::Duration;
);

/// The sender of bounded channel.
/// A [`BoundedSender`] and [`BoundedReceiver`] handle pair are created by the
/// [`bounded_channel`] function.
///
/// # Examples
///
/// ```
/// use ylong_runtime::sync::mpsc::bounded::bounded_channel;
/// async fn io_func() {
///     let (tx, mut rx) = bounded_channel(1);
///     let tx2 = tx.clone();
///     let handle = ylong_runtime::spawn(async move {
///         assert!(tx.send(1).await.is_ok());
///         assert!(!tx.is_closed());
///         assert!(tx.is_same(&tx2));
///     });
///     let handle2 = ylong_runtime::spawn(async move {
///         assert_eq!(rx.recv().await, Ok(1));
///     });
///     let _ = ylong_runtime::block_on(handle);
///     let _ = ylong_runtime::block_on(handle2);
/// }
/// ```
pub struct BoundedSender<T> {
    channel: Tx<Array<T>>,
}

impl<T> Clone for BoundedSender<T> {
    fn clone(&self) -> Self {
        BoundedSender {
            channel: self.channel.clone(),
        }
    }
}

/// The receiver of bounded channel.
/// A [`BoundedSender`] and [`BoundedReceiver`] handle pair are created by the
/// [`bounded_channel`] function.
///
/// # Examples
///
/// ```
/// use ylong_runtime::sync::mpsc::bounded::bounded_channel;
/// async fn io_func() {
///     let (tx, mut rx) = bounded_channel(1);
///     assert!(rx.try_recv().is_err());
///     let handle = ylong_runtime::spawn(async move {
///         assert!(tx.send(1).await.is_ok());
///     });
///     let handle2 = ylong_runtime::spawn(async move {
///         assert_eq!(rx.len(), 1);
///         assert_eq!(rx.recv().await, Ok(1));
///     });
///     let _ = ylong_runtime::block_on(handle);
///     let _ = ylong_runtime::block_on(handle2);
/// }
/// ```
pub struct BoundedReceiver<T> {
    channel: Rx<Array<T>>,
}

/// Creates a new mpsc channel, and returns the `Sender` and `Receiver` handle pair.
///
/// The channel is bounded with the passed in capacity.
///
/// # Panics
///
/// Panics if the new capacity is initialized to zero.
///
/// # Examples
///
/// ```
/// use ylong_runtime::sync::mpsc::bounded::bounded_channel;
/// async fn io_func() {
///     let (tx, mut rx) = bounded_channel(1);
///     let handle = ylong_runtime::spawn(async move {
///         assert_eq!(rx.recv().await, Ok(1));
///     });
///     let handle2 = ylong_runtime::spawn(async move {
///         assert!(tx.send(1).await.is_ok());
///     });
///     let _ = ylong_runtime::block_on(handle);
///     let _ = ylong_runtime::block_on(handle2);
/// }
/// ```
pub fn bounded_channel<T>(number: usize) -> (BoundedSender<T>, BoundedReceiver<T>) {
    let array = Array::new(number);
    let (tx, rx) = channel(array);
    (BoundedSender::new(tx), BoundedReceiver::new(rx))
}

impl<T> BoundedSender<T> {
    fn new(channel: Tx<Array<T>>) -> BoundedSender<T> {
        BoundedSender { channel }
    }

    /// Attempts to send a value to the associated [`BoundedReceiver`].
    ///
    /// If the receiver has been closed or the channel is full, this method will return an error
    /// containing sent value.
    ///
    /// # Return value
    /// * `Ok(T)` if receiving a value successfully.
    /// * `Err(SendError::Full(T))` if the buffer of channel is full.
    /// * `Err(SendError::Closed(T))` if all senders have been dropped.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::sync::error::RecvError;
    /// use ylong_runtime::sync::mpsc::bounded::bounded_channel;
    /// let (tx, mut rx) = bounded_channel(1);
    /// match rx.try_recv() {
    ///     Err(RecvError::Empty) => {}
    ///     _ => panic!("This won't happen"),
    /// }
    /// tx.try_send(1).unwrap();
    /// match rx.try_recv() {
    ///     Ok(_) => {}
    ///     _ => panic!("This won't happen"),
    /// }
    /// ```
    pub fn try_send(&self, value: T) -> Result<(), SendError<T>> {
        self.channel.try_send(value)
    }

    /// Sends a value to the associated receiver
    ///
    /// If the receiver has been closed, this method will return an error containing the sent
    /// value.
    ///
    /// # Return value
    /// * `Ok()` if sending a value successfully.
    /// * `Err(SendError::Closed(T))` if receiver has been dropped or closed.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::sync::mpsc::bounded::bounded_channel;
    /// async fn io_func() {
    ///     let (tx, mut rx) = bounded_channel(1);
    ///     let handle = ylong_runtime::spawn(async move {
    ///         assert_eq!(rx.recv().await, Ok(1));
    ///     });
    ///     let handle2 = ylong_runtime::spawn(async move {
    ///         assert!(tx.send(1).await.is_ok());
    ///     });
    ///     let _ = ylong_runtime::block_on(handle);
    ///     let _ = ylong_runtime::block_on(handle2);
    /// }
    /// ```
    pub async fn send(&self, value: T) -> Result<(), SendError<T>> {
        self.channel.send(value).await
    }

    /// Attempts to send a value to the associated receiver in a limited amount of time.
    ///
    /// If the receiver has been closed or the time limit has been passed, this
    /// method will return an error containing the sent value.
    ///
    /// # Return value
    /// * `Ok()` if sending a value successfully.
    /// * `Err(SendError::Closed(T))` if receiver has been dropped or closed.
    /// * `Err(SendError::TimeOut(T))` if time limit has been passed.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    /// use ylong_runtime::sync::mpsc::bounded::bounded_channel;
    /// async fn io_func() {
    ///     let (tx, mut rx) = bounded_channel(1);
    ///     let handle = ylong_runtime::spawn(async move {
    ///         assert_eq!(rx.recv().await, Ok(1));
    ///     });
    ///     let handle2 = ylong_runtime::spawn(async move {
    ///         assert!(tx.send_timeout(1, Duration::from_millis(10)).await.is_ok());
    ///     });
    ///     let _ = ylong_runtime::block_on(handle);
    ///     let _ = ylong_runtime::block_on(handle2);
    /// }
    /// ```
    #[cfg(feature = "time")]
    pub async fn send_timeout(&self, value: T, time: Duration) -> Result<(), SendError<T>> {
        match timeout(time, self.channel.get_position()).await {
            Ok(res) => match res {
                SendPosition::Pos(index) => {
                    self.channel.write(index, value);
                    Ok(())
                }
                SendPosition::Closed => Err(SendError::Closed(value)),
                SendPosition::Full => unreachable!(),
            },
            Err(_) => Err(SendError::TimeOut(value)),
        }
    }

    /// Checks whether the channel is closed. If so, the sender could not
    /// send values anymore. It returns true after the [`BoundedReceiver`] is dropped
    /// or the [`close`] method gets called.
    ///
    /// [`close`]: BoundedReceiver::close
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::sync::mpsc::bounded::bounded_channel;
    /// let (tx, rx) = bounded_channel::<isize>(1);
    /// assert!(!tx.is_closed());
    /// drop(rx);
    /// assert!(tx.is_closed());
    /// ```
    pub fn is_closed(&self) -> bool {
        self.channel.is_close()
    }

    /// Checks whether the sender and another sender belong to the same channel.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::sync::mpsc::bounded::bounded_channel;
    /// let (tx, rx) = bounded_channel::<isize>(1);
    /// let tx2 = tx.clone();
    /// assert!(tx.is_same(&tx2));
    /// ```
    pub fn is_same(&self, other: &Self) -> bool {
        self.channel.is_same(&other.channel)
    }

    /// Gets the capacity of the channel.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::sync::mpsc::bounded::bounded_channel;
    /// let (tx, rx) = bounded_channel::<isize>(5);
    /// assert_eq!(tx.capacity(), 5);
    /// ```
    pub fn capacity(&self) -> usize {
        self.channel.capacity()
    }

    /// Gets the number of values in the channel.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::sync::mpsc::bounded::bounded_channel;
    /// let (tx, rx) = bounded_channel(5);
    /// assert_eq!(tx.len(), 0);
    /// tx.try_send(1).unwrap();
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
    /// use ylong_runtime::sync::mpsc::bounded::bounded_channel;
    /// let (tx, rx) = bounded_channel(5);
    /// assert!(tx.is_empty());
    /// tx.try_send(1).unwrap();
    /// assert!(!tx.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T> Drop for BoundedSender<T> {
    fn drop(&mut self) {
        self.channel.close();
    }
}

impl<T> BoundedReceiver<T> {
    fn new(channel: Rx<Array<T>>) -> BoundedReceiver<T> {
        BoundedReceiver { channel }
    }

    /// Get the number of values in the channel.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::sync::mpsc::bounded::bounded_channel;
    /// let (tx, mut rx) = bounded_channel(3);
    /// tx.try_send(1).unwrap();
    /// tx.try_send(2).unwrap();
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
    /// use ylong_runtime::sync::mpsc::bounded::bounded_channel;
    /// let (tx, rx) = bounded_channel(5);
    /// assert!(rx.is_empty());
    /// tx.try_send(1).unwrap();
    /// assert!(!rx.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Attempts to receive a value from the associated [`BoundedSender`].
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
    /// use ylong_runtime::sync::mpsc::bounded::bounded_channel;
    /// let (tx, mut rx) = bounded_channel(1);
    /// match rx.try_recv() {
    ///     Err(RecvError::Empty) => {}
    ///     _ => panic!("This won't happen"),
    /// }
    /// tx.try_send(1).unwrap();
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

    /// Receives a value from the associated [`BoundedSender`].
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
    /// use ylong_runtime::sync::mpsc::bounded::bounded_channel;
    /// async fn io_func() {
    ///     let (tx, mut rx) = bounded_channel(1);
    ///     let handle = ylong_runtime::spawn(async move {
    ///         assert_eq!(rx.recv().await, Ok(1));
    ///     });
    ///     tx.try_send(1).unwrap();
    ///     let _ = ylong_runtime::block_on(handle);
    /// }
    /// ```
    pub async fn recv(&mut self) -> Result<T, RecvError> {
        self.channel.recv().await
    }

    /// Attempts to receive a value from the associated [`BoundedSender`] in a limited amount of time.
    ///
    /// The `receiver` can still receive all sent messages in the channel after the channel is closed.
    ///
    /// # Return value
    /// * `Ok(T)` if receiving a value successfully.
    /// * `Err(RecvError::Closed)` if all senders have been dropped.
    /// * `Err(RecvError::TimeOut)` if time limit has been passed.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    /// use ylong_runtime::sync::mpsc::bounded::bounded_channel;
    /// async fn io_func() {
    ///     let (tx, mut rx) = bounded_channel(1);
    ///     let handle = ylong_runtime::spawn(async move {
    ///         tx.try_send(1).unwrap();
    ///         assert_eq!(rx.recv_timeout(Duration::from_millis(10)).await, Ok(1));
    ///     });
    ///     let _ = ylong_runtime::block_on(handle);
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
    /// The `Sender` will fail to call [`send`] or [`try_send`] after the `Receiver` called
    /// `close`. It will do nothing if the channel is already closed.
    ///
    /// [`send`]: BoundedSender::send
    /// [`try_send`]: BoundedSender::try_send
    ///
    /// # Examples
    /// ```
    /// use ylong_runtime::sync::mpsc::bounded::bounded_channel;
    /// async fn io_func() {
    ///     let (tx, mut rx) = bounded_channel(1);
    ///     assert!(!tx.is_closed());
    ///
    ///     rx.close();
    ///
    ///     assert!(tx.is_closed());
    ///     assert!(tx.try_send("no receive").is_err());
    /// }
    /// ```
    ///
    /// Receives a value sent **before** calling `close`
    ///
    /// ```
    /// use ylong_runtime::sync::mpsc::bounded::bounded_channel;
    /// async fn io_func() {
    ///     let (tx, mut rx) = bounded_channel(1);
    ///     assert!(tx.try_send("Hello").is_ok());
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

impl<T> Drop for BoundedReceiver<T> {
    fn drop(&mut self) {
        self.channel.close();
    }
}
