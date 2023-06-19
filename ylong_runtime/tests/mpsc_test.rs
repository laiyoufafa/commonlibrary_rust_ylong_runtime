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

use std::time::Duration;
use ylong_runtime::sync::mpsc::bounded_channel;
use ylong_runtime::sync::mpsc::unbounded_channel;
use ylong_runtime::sync::{RecvError, SendError};
use ylong_runtime::task::JoinHandle;

/// SDV test for `UnboundedSender`.
///
/// # Title
/// sdv_unbounded_send_recv_test
///
/// # Brief
/// 1.Create a unbounded mpsc channel.
/// 2.Send two values to the receiver then drop.
/// 3.Receive two values successfully and then receive error.
#[test]
fn sdv_unbounded_send_recv_test() {
    let (tx, mut rx) = unbounded_channel();
    let handle = ylong_runtime::spawn(async move {
        assert_eq!(rx.recv().await, Ok(1));
        assert_eq!(rx.recv().await, Ok(2));
        assert_eq!(rx.recv().await, Err(RecvError::Closed));
    });
    assert!(tx.send(1).is_ok());
    assert!(tx.send(2).is_ok());
    drop(tx);
    let _ = ylong_runtime::block_on(handle);
}

/// SDV test for `UnboundedSender`.
///
/// # Title
/// sdv_unbounded_send_try_recv_test
///
/// # Brief
/// 1.Create a unbounded mpsc channel.
/// 2.Try receiving before and after sender sends a value.
/// 3.Try receiving after sender has been dropped.
#[test]
fn sdv_unbounded_send_try_recv_test() {
    let (tx, mut rx) = unbounded_channel();
    assert_eq!(rx.try_recv(), Err(RecvError::Empty));
    assert!(tx.send(1).is_ok());
    assert_eq!(rx.try_recv(), Ok(1));
    drop(tx);
    assert_eq!(rx.try_recv(), Err(RecvError::Closed));
}

/// SDV test for `UnboundedSender`.
///
/// # Title
/// sdv_unbounded_send_recv_timeout_test
///
/// # Brief
/// 1.Create a unbounded mpsc channel.
/// 2.Send a value to the receiver.
/// 3.Receive the value in the limited time twice.
#[test]
fn sdv_unbounded_send_recv_timeout_test() {
    let (tx, mut rx) = unbounded_channel();
    let handle = ylong_runtime::spawn(async move {
        assert!(tx.send(1).is_ok());
        assert_eq!(rx.recv_timeout(Duration::from_millis(10)).await, Ok(1));
        assert_eq!(
            rx.recv_timeout(Duration::from_millis(10)).await,
            Err(RecvError::TimeOut)
        );
    });
    let _ = ylong_runtime::block_on(handle);
}

/// SDV test for `BoundedSender`.
///
/// # Title
/// sdv_bounded_send_recv_test
///
/// # Brief
/// 1.Create a bounded mpsc channel with capacity.
/// 2.Send two value to the receiver.
/// 3.Receive two values successfully and then receive error.
#[test]
fn sdv_bounded_send_recv_test() {
    let (tx, mut rx) = bounded_channel::<i32>(1);
    let handle = ylong_runtime::spawn(async move {
        assert_eq!(rx.recv().await, Ok(1));
        assert_eq!(rx.recv().await, Ok(2));
        assert_eq!(rx.recv().await, Err(RecvError::Closed));
    });

    ylong_runtime::spawn(async move {
        assert!(tx.send(1).await.is_ok());
        assert!(tx.send(2).await.is_ok());
    });
    let _ = ylong_runtime::block_on(handle);
}

/// SDV test for `BoundedSender`.
///
/// # Title
/// sdv_bounded_try_send_try_recv_test
///
/// # Brief
/// 1.Create a bounded mpsc channel with capacity.
/// 2.Try receiving and fails.
/// 3.Try sending two values and one succeeds and one fails.
/// 4.Try receiving and succeeds.
/// 5.Drop the sender and then receiver fails to receive.
#[test]
fn sdv_bounded_try_send_try_recv_test() {
    let (tx, mut rx) = bounded_channel::<i32>(1);
    assert_eq!(rx.try_recv(), Err(RecvError::Empty));
    assert!(tx.try_send(1).is_ok());
    assert_eq!(tx.try_send(2), Err(SendError::Full(2)));
    assert_eq!(rx.try_recv(), Ok(1));
    drop(tx);
    assert_eq!(rx.try_recv(), Err(RecvError::Closed));
}

/// SDV test for `BoundedSender`.
///
/// # Title
/// sdv_bounded_send_timeout_recv_timeout_test
///
/// # Brief
/// 1.Create a bounded mpsc channel with capacity.
/// 2.Send two values to the receiver in the limited time and one succeeds and one fails.
/// 3.Receive two values from the sender in the limited time and one succeeds and one fails.
#[test]
fn sdv_bounded_send_timeout_recv_timeout_test() {
    let (tx, mut rx) = bounded_channel(1);
    let handle = ylong_runtime::spawn(async move {
        assert!(tx.send_timeout(1, Duration::from_millis(10)).await.is_ok());
        assert!(tx.send_timeout(2, Duration::from_millis(10)).await.is_err());
        assert_eq!(rx.recv_timeout(Duration::from_millis(10)).await, Ok(1));
        assert_eq!(
            rx.recv_timeout(Duration::from_millis(10)).await,
            Err(RecvError::TimeOut)
        );
    });
    let _ = ylong_runtime::block_on(handle);
}

/// SDV test for `BoundedSender` and `UnboundedSender`.
///
/// # Title
/// sdv_is_closed
///
/// # Brief
/// 1.Create a unbounded mpsc channel.
/// 2.Check the close state of unbounded channel before and after the receiver is dropped.
/// 3.Create a bounded mpsc channel with capacity.
/// 3.Check the close state of bounded channel before and after the receiver is dropped.
#[test]
fn sdv_mpsc_is_closed() {
    let (tx, rx) = unbounded_channel::<i32>();
    assert!(!tx.is_closed());
    drop(rx);
    assert!(tx.is_closed());
    assert!(tx.send(1).is_err());

    let (tx, rx) = bounded_channel::<i32>(1);
    assert!(!tx.is_closed());
    drop(rx);
    assert!(tx.is_closed());
    assert!(tx.try_send(1).is_err());
}

/// SDV test for `BoundedSender` and `UnboundedSender`.
///
/// # Title
/// sdv_is_same
///
/// # Brief
/// 1.Create two unbounded mpsc channels.
/// 2.Check whether senders have the same source.
/// 3.Create two bounded mpsc channels with capacity.
/// 2.Check whether senders have the same source.
#[test]
fn sdv_mpsc_is_same() {
    let (tx, _) = unbounded_channel::<i32>();
    let (tx2, _) = unbounded_channel::<i32>();

    assert!(!tx.is_same(&tx2));
    assert!(tx.is_same(&tx));

    let (tx, _) = bounded_channel::<i32>(1);
    let (tx2, _) = bounded_channel::<i32>(1);

    assert!(!tx.is_same(&tx2));
    assert!(tx.is_same(&tx));
}

/// SDV test for `BoundedSender` and `UnboundedSender`.
///
/// # Title
/// sdv_len
///
/// # Brief
/// 1.Create two unbounded mpsc channels.
/// 2.Check the correctness of length of channel.
/// 3.Create two bounded mpsc channels with capacity.
/// 2.Check the correctness of length of channel.
#[test]
fn sdv_mpsc_len() {
    let (tx, mut rx) = unbounded_channel();
    for i in 0..35 {
        let _ = tx.send(i);
    }
    assert_eq!(rx.len(), 35);
    for _ in 0..3 {
        let _ = rx.try_recv();
    }
    assert_eq!(rx.len(), 32);
    for _ in 0..33 {
        let _ = rx.try_recv();
    }
    assert_eq!(rx.len(), 0);

    let (tx, mut rx) = bounded_channel(10);
    for i in 0..12 {
        let _ = tx.try_send(i);
    }
    assert_eq!(rx.len(), 10);
    for _ in 0..2 {
        let _ = rx.try_recv();
    }
    assert_eq!(rx.len(), 8);
    for _ in 0..9 {
        let _ = rx.try_recv();
    }
    assert_eq!(rx.len(), 0);
}

/// SDV test for `BoundedSender` and `UnboundedSender`.
///
/// # Title
/// sdv_multi_send_recv_test
///
/// # Brief
/// 1.Create a unbounded mpsc channel.
/// 2.Send and receive for many times.
/// 3.Create a bounded mpsc channel with capacity.
/// 2.Send and receive for many times.
#[test]
fn sdv_multi_send_recv_test() {
    let (tx, mut rx) = unbounded_channel();
    let mut tasks: Vec<JoinHandle<()>> = Vec::new();
    for i in 0..1000 {
        let tx2 = tx.clone();
        tasks.push(ylong_runtime::spawn(async move {
            assert!(tx2.send(i).is_ok());
        }));
    }

    let handle = ylong_runtime::spawn(async move {
        for _ in 0..1000 {
            assert!(rx.recv().await.is_ok());
        }
        assert!(rx.try_recv().is_err());
    });
    for t in tasks {
        let _ = ylong_runtime::block_on(t);
    }
    let _ = ylong_runtime::block_on(handle);

    let (tx, mut rx) = bounded_channel(10);
    let mut tasks: Vec<JoinHandle<()>> = Vec::new();
    for i in 0..1000 {
        let tx2 = tx.clone();
        tasks.push(ylong_runtime::spawn(async move {
            assert_eq!(tx2.send(i).await, Ok(()));
        }));
    }

    let handle = ylong_runtime::spawn(async move {
        for _ in 0..1000 {
            assert!(rx.recv().await.is_ok());
        }
        assert!(rx.try_recv().is_err());
    });
    for t in tasks {
        let _ = ylong_runtime::block_on(t);
    }
    let _ = ylong_runtime::block_on(handle);
}
