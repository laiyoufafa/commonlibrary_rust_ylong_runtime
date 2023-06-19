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

use std::marker::PhantomData;
use std::ptr::NonNull;

pub(crate) struct Node<T> {
    prev: Option<NonNull<Node<T>>>,
    next: Option<NonNull<Node<T>>>,
    val: T,
}

impl<T> Node<T> {
    fn new(val: T) -> Self {
        Self {
            prev: None,
            next: None,
            val,
        }
    }

    pub(crate) fn get_mut(&mut self) -> &mut T {
        &mut self.val
    }
}

pub(crate) struct LinkedList<T> {
    head: Option<NonNull<Node<T>>>,
    tail: Option<NonNull<Node<T>>>,
    marker: PhantomData<Box<Node<T>>>,
}

impl<T> Default for LinkedList<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> LinkedList<T> {
    pub(crate) fn new() -> Self {
        Self {
            head: None,
            tail: None,
            marker: PhantomData,
        }
    }

    pub(crate) fn add_item(&mut self, val: T) -> NonNull<Node<T>> {
        self.push_back(Box::new(Node::new(val)))
    }

    fn push_back(&mut self, mut node: Box<Node<T>>) -> NonNull<Node<T>> {
        node.next = None;
        node.prev = self.tail;
        let node = Some(Box::leak(node).into());

        match self.tail {
            None => self.head = node,

            Some(tail) => unsafe { (*tail.as_ptr()).next = node },
        }

        self.tail = node;
        node.unwrap()
    }

    pub(crate) fn remove_node(&mut self, mut node: NonNull<Node<T>>) {
        let node = unsafe { node.as_mut() }; // this one is ours now, we can create an &mut.

        match node.prev {
            Some(prev) => unsafe { (*prev.as_ptr()).next = node.next },
            None => self.head = node.next,
        };

        match node.next {
            Some(next) => unsafe { (*next.as_ptr()).prev = node.prev },
            None => self.tail = node.prev,
        };

        unsafe {
            drop(Box::from_raw(node));
        }
    }

    pub(crate) fn for_each_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut T),
    {
        let mut p = self.head;
        while p.is_some() {
            unsafe {
                f(&mut p.unwrap().as_mut().val);
                p = p.unwrap().as_mut().next;
            }
        }
    }
}
