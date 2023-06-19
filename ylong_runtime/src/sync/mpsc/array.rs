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

use crate::futures::poll_fn;
use crate::sync::atomic_waker::AtomicWaker;
use crate::sync::error::SendError::{Closed, Full};
use crate::sync::error::{RecvError, SendError};
use crate::sync::mpsc::Container;
use crate::sync::wake_list::WakerList;
use std::cell::RefCell;
use std::future::Future;
use std::mem::MaybeUninit;
use std::pin::Pin;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::{AcqRel, Acquire, Release};
use std::task::Poll::{Pending, Ready};
use std::task::{Context, Poll};

/// The offset of the index.
const INDEX_SHIFT: usize = 1;
/// The flag marks that Array is closed.
const CLOSED: usize = 0b01;

pub(crate) struct Node<T> {
    index: AtomicUsize,
    value: RefCell<MaybeUninit<T>>,
}

/// Bounded lockless queue.
pub(crate) struct Array<T> {
    head: RefCell<usize>,
    tail: AtomicUsize,
    capacity: usize,
    rx_waker: AtomicWaker,
    waiters: WakerList,
    data: Box<[Node<T>]>,
}

unsafe impl<T: Send> Send for Array<T> {}
unsafe impl<T: Send> Sync for Array<T> {}

pub(crate) enum SendPosition {
    Pos(usize),
    Full,
    Closed,
}

impl<T> Array<T> {
    pub(crate) fn new(capacity: usize) -> Array<T> {
        assert!(capacity > 0, "Capacity cannot be zero.");
        let data = (0..capacity)
            .map(|i| Node {
                index: AtomicUsize::new(i),
                value: RefCell::new(MaybeUninit::uninit()),
            })
            .collect();
        Array {
            head: RefCell::new(0),
            tail: AtomicUsize::new(0),
            capacity,
            rx_waker: AtomicWaker::new(),
            waiters: WakerList::new(),
            data,
        }
    }

    fn prepare_send(&self) -> SendPosition {
        let mut tail = self.tail.load(Acquire);
        loop {
            if tail & CLOSED == CLOSED {
                return SendPosition::Closed;
            }
            let index = (tail >> INDEX_SHIFT) % self.capacity;
            let node = self.data.get(index).unwrap();
            let node_index = node.index.load(Acquire);

            // Compare the index of the node with the tail to avoid senders in different cycles
            // writing data to the same point at the same time.
            if (tail >> INDEX_SHIFT) == node_index {
                match self.tail.compare_exchange_weak(
                    tail,
                    tail.wrapping_add(1 << INDEX_SHIFT),
                    AcqRel,
                    Acquire,
                ) {
                    Ok(_) => {
                        return SendPosition::Pos(index);
                    }
                    Err(actual) => {
                        tail = actual;
                    }
                }
            } else {
                return SendPosition::Full;
            }
        }
    }

    pub(crate) fn write(&self, index: usize, value: T) {
        let node = self.data.get(index).unwrap();
        node.value.borrow_mut().write(value);

        // Mark that the node has data.
        node.index.fetch_sub(1, Release);
        self.rx_waker.wake();
    }

    pub(crate) async fn get_position(&self) -> SendPosition {
        Position { array: self }.await
    }

    pub(crate) fn try_send(&self, value: T) -> Result<(), SendError<T>> {
        match self.prepare_send() {
            SendPosition::Pos(index) => {
                self.write(index, value);
                Ok(())
            }
            SendPosition::Full => Err(Full(value)),
            SendPosition::Closed => Err(Closed(value)),
        }
    }

    pub(crate) async fn send(&self, value: T) -> Result<(), SendError<T>> {
        match self.get_position().await {
            SendPosition::Pos(index) => {
                self.write(index, value);
                Ok(())
            }
            SendPosition::Closed => Err(Closed(value)),
            // If the array is full, the task will wait until it's available.
            SendPosition::Full => unreachable!(),
        }
    }

    pub(crate) fn try_recv(&self) -> Result<T, RecvError> {
        let head = *self.head.borrow();
        let index = head % self.capacity;
        let node = self.data.get(index).unwrap();
        let node_index = node.index.load(Acquire);

        // Check whether the node has data.
        if head == node_index.wrapping_add(1) {
            let value = unsafe { node.value.as_ptr().read().assume_init() };
            // Adding one indicates that this point is empty, Adding <capacity> enables the
            // corresponding tail node to write in.
            node.index.fetch_add(self.capacity + 1, Release);
            self.waiters.notify_one();
            self.head.replace(head + 1);
            Ok(value)
        } else {
            let tail = self.tail.load(Acquire);
            if tail & CLOSED == CLOSED {
                Err(RecvError::Closed)
            } else {
                Err(RecvError::Empty)
            }
        }
    }

    fn poll_recv(&self, cx: &mut Context<'_>) -> Poll<Result<T, RecvError>> {
        match self.try_recv() {
            Ok(val) => return Ready(Ok(val)),
            Err(RecvError::Closed) => return Ready(Err(RecvError::Closed)),
            _ => {}
        }

        self.rx_waker.register_by_ref(cx.waker());

        match self.try_recv() {
            Ok(val) => Ready(Ok(val)),
            Err(RecvError::Closed) => Ready(Err(RecvError::Closed)),
            Err(RecvError::Empty) => Pending,
            _ => unreachable!(),
        }
    }

    pub(crate) async fn recv(&self) -> Result<T, RecvError> {
        poll_fn(|cx| self.poll_recv(cx)).await
    }

    pub(crate) fn capacity(&self) -> usize {
        self.capacity
    }
}

impl<T> Container for Array<T> {
    fn close(&self) {
        self.tail.fetch_or(CLOSED, Release);
        self.waiters.notify_all();
        self.rx_waker.wake();
    }

    fn is_close(&self) -> bool {
        self.tail.load(Acquire) & CLOSED == CLOSED
    }

    fn len(&self) -> usize {
        let head = *self.head.borrow();
        let tail = self.tail.load(Acquire) >> INDEX_SHIFT;
        tail - head
    }
}

impl<T> Drop for Array<T> {
    fn drop(&mut self) {
        let len = self.len();
        if len == 0 {
            return;
        }
        let head = *self.head.borrow();
        for i in 0..len {
            let mut index = head + i;
            if index > self.capacity {
                index -= self.capacity;
            }
            let node = self.data.get_mut(index).unwrap();
            unsafe {
                node.value.borrow_mut().as_mut_ptr().drop_in_place();
            }
        }
    }
}

struct Position<'a, T> {
    array: &'a Array<T>,
}

impl<T> Future for Position<'_, T> {
    type Output = SendPosition;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.array.prepare_send() {
            SendPosition::Pos(index) => return Ready(SendPosition::Pos(index)),
            SendPosition::Closed => return Ready(SendPosition::Closed),
            SendPosition::Full => {}
        }

        self.array.waiters.insert(cx.waker().clone());

        let tail = self.array.tail.load(Acquire);
        let index = (tail >> INDEX_SHIFT) % self.array.capacity;
        let node = self.array.data.get(index).unwrap();
        let node_index = node.index.load(Acquire);
        if (tail >> INDEX_SHIFT) == node_index || tail & CLOSED == CLOSED {
            self.array.waiters.notify_one();
        }
        Pending
    }
}
