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
use crate::sync::error::{RecvError, SendError};
use crate::sync::mpsc::Container;
use std::cell::RefCell;
use std::mem::MaybeUninit;
use std::ptr;
use std::ptr::NonNull;
use std::sync::atomic::Ordering::{AcqRel, Acquire, Release};
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize};
use std::task::Poll::{Pending, Ready};
use std::task::{Context, Poll};

/// The capacity of a block.
const CAPACITY: usize = 32;
/// The offset of the index.
const INDEX_SHIFT: usize = 1;
/// The flag marks that Array is closed.
const CLOSED: usize = 0b01;

pub(crate) struct Node<T> {
    has_value: AtomicBool,
    value: RefCell<MaybeUninit<T>>,
}

struct Block<T> {
    data: [Node<T>; CAPACITY],
    next: AtomicPtr<Block<T>>,
}

impl<T> Block<T> {
    fn new() -> Block<T> {
        Block {
            data: unsafe { MaybeUninit::zeroed().assume_init() },
            next: AtomicPtr::new(ptr::null_mut()),
        }
    }

    fn reclaim(&self) {
        self.next.store(ptr::null_mut(), Release);
    }

    fn try_insert(&self, ptr: *mut Block<T>) -> Result<(), *mut Block<T>> {
        match self
            .next
            .compare_exchange_weak(ptr::null_mut(), ptr, AcqRel, Acquire)
        {
            Ok(_) => Ok(()),
            Err(new_ptr) => Err(new_ptr),
        }
    }

    fn insert(&self, ptr: *mut Block<T>) {
        let mut curr = self;
        // The number of cycles is limited. Recycling blocks is to avoid frequent creation and
        // destruction, but trying too many times may consume more resources. Every block should
        // stop trying after failing to insert for a certain times.
        for _ in 0..5 {
            match curr.try_insert(ptr) {
                Ok(_) => return,
                Err(next) => {
                    curr = unsafe { next.as_ref().unwrap() };
                }
            }
        }
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

struct Head<T> {
    block: NonNull<Block<T>>,
    index: usize,
}

struct Tail<T> {
    block: AtomicPtr<Block<T>>,
    index: AtomicUsize,
}

/// Unbounded lockless queue.
pub(crate) struct Queue<T> {
    head: RefCell<Head<T>>,
    tail: Tail<T>,
    rx_waker: AtomicWaker,
}

unsafe impl<T: Send> Send for Queue<T> {}
unsafe impl<T: Send> Sync for Queue<T> {}

impl<T> Queue<T> {
    pub(crate) fn new() -> Queue<T> {
        let block = Box::new(Block::new());
        let block_ptr = Box::into_raw(block);
        Queue {
            head: RefCell::new(Head {
                block: NonNull::new(block_ptr).unwrap(),
                index: 0,
            }),
            tail: Tail {
                block: AtomicPtr::new(block_ptr),
                index: AtomicUsize::new(0),
            },
            rx_waker: AtomicWaker::new(),
        }
    }

    pub(crate) fn send(&self, value: T) -> Result<(), SendError<T>> {
        let mut tail = self.tail.index.load(Acquire);
        let mut block_ptr = self.tail.block.load(Acquire);
        loop {
            if tail & CLOSED == CLOSED {
                return Err(SendError::Closed(value));
            }
            let index = (tail >> INDEX_SHIFT) % (CAPACITY + 1);
            if index == CAPACITY {
                tail = self.tail.index.load(Acquire);
                block_ptr = self.tail.block.load(Acquire);
                continue;
            }
            let block = unsafe { &*block_ptr };
            if index + 1 == CAPACITY && block.next.load(Acquire).is_null() {
                let new_block = Box::new(Block::<T>::new());
                let new_block_ptr = Box::into_raw(new_block);
                block.insert(new_block_ptr);
            }
            match self.tail.index.compare_exchange_weak(
                tail,
                tail + (1 << INDEX_SHIFT),
                AcqRel,
                Acquire,
            ) {
                Ok(_) => {
                    if index + 1 == CAPACITY {
                        let next_block = block.next.load(Acquire);
                        self.tail.block.store(next_block, Release);
                        self.tail.index.fetch_add(1 << INDEX_SHIFT, Release);
                    }
                    let node = block.data.get(index).unwrap();
                    unsafe {
                        node.value.as_ptr().write(MaybeUninit::new(value));
                    }
                    node.has_value.store(true, Release);
                    self.rx_waker.wake();
                    return Ok(());
                }
                Err(_) => {
                    tail = self.tail.index.load(Acquire);
                    block_ptr = self.tail.block.load(Acquire);
                }
            }
        }
    }

    pub(crate) fn try_recv(&self) -> Result<T, RecvError> {
        let mut head = self.head.borrow_mut();
        let head_index = head.index;
        let block_ptr = head.block.as_ptr();
        let block = unsafe { &*block_ptr };
        let index = head_index % (CAPACITY + 1);
        let node = block.data.get(index).unwrap();
        // Check whether the node is ready to read.
        if node.has_value.swap(false, Acquire) {
            let value = unsafe { node.value.as_ptr().read().assume_init() };
            if index + 1 == CAPACITY {
                head.block = NonNull::new(block.next.load(Acquire)).unwrap();
                block.reclaim();
                unsafe { (*self.tail.block.load(Acquire)).insert(block_ptr) };
                // When the nodes in a block are full, the last index is reserved as a buffer
                // for `Send` to synchronize two atomic operations.
                head.index = head_index.wrapping_add(2);
            } else {
                head.index = head_index.wrapping_add(1);
            }
            Ok(value)
        } else {
            let tail_index = self.tail.index.load(Acquire);
            if tail_index & CLOSED == CLOSED {
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
}

impl<T> Container for Queue<T> {
    fn close(&self) {
        self.tail.index.fetch_or(CLOSED, Release);
        self.rx_waker.wake();
    }

    fn is_close(&self) -> bool {
        self.tail.index.load(Acquire) & CLOSED == CLOSED
    }

    fn len(&self) -> usize {
        let head = self.head.borrow().index;
        let mut tail = self.tail.index.load(Acquire) >> INDEX_SHIFT;
        if tail % (CAPACITY + 1) == CAPACITY {
            tail = tail.wrapping_add(1);
        }
        let head_redundant = head / (CAPACITY + 1);
        let tail_redundant = tail / (CAPACITY + 1);
        tail - head - (tail_redundant - head_redundant)
    }
}

impl<T> Drop for Queue<T> {
    fn drop(&mut self) {
        let head = self.head.borrow_mut();
        let mut head_index = head.index;
        let tail_index = self.tail.index.load(Acquire) >> INDEX_SHIFT;
        let mut block_ptr = head.block.as_ptr();
        while head_index < tail_index {
            let index = head_index % (CAPACITY + 1);
            unsafe {
                if index == CAPACITY {
                    let next_node_ptr = (*block_ptr).next.load(Acquire);
                    drop(Box::from_raw(block_ptr));
                    block_ptr = next_node_ptr;
                } else {
                    let node = (*block_ptr).data.get_mut(index).unwrap();
                    node.value.get_mut().as_mut_ptr().drop_in_place();
                }
            }
            head_index = head_index.wrapping_add(1);
        }
        while !block_ptr.is_null() {
            unsafe {
                let next_node_ptr = (*block_ptr).next.load(Acquire);
                drop(Box::from_raw(block_ptr));
                block_ptr = next_node_ptr;
            }
        }
    }
}
