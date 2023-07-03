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

//! This linked list does not have ownership of nodes, and it treats the structure passed
//! in by the user as a node for storage, so the `clear` operation does not release memory,
//! and the `remove` operation needs to ensure that the node is in any linked list held by
//! a caller to ensure the memory validity of pointers within the node. Users need to manage
//! the memory of the instances associated with each node themselves.

use std::ptr::NonNull;

#[derive(Default)]
pub(crate) struct Node<T> {
    prev: Option<NonNull<T>>,
    next: Option<NonNull<T>>,
}

impl<T> Node<T> {
    #[allow(dead_code)]
    pub(crate) fn new() -> Node<T> {
        Node {
            prev: None,
            next: None,
        }
    }
}

impl<T: Link> Node<T> {
    unsafe fn remove_node(node: NonNull<T>) -> Option<NonNull<T>> {
        let prev = T::node(node).as_ref().prev;
        let next = T::node(node).as_ref().next;
        match prev {
            None => return None,
            Some(prev) => T::node(prev).as_mut().next = next,
        }
        match next {
            None => return None,
            Some(next) => T::node(next).as_mut().prev = prev,
        }
        T::node(node).as_mut().prev = None;
        T::node(node).as_mut().next = None;
        Some(node)
    }
}

unsafe impl<T: Send> Send for Node<T> {}
unsafe impl<T: Sync> Sync for Node<T> {}

pub(crate) struct LinkedList<L: Link + Default> {
    head: NonNull<L>,
}

unsafe impl<L: Link + Default + Send> Send for LinkedList<L> {}
unsafe impl<L: Link + Default + Sync> Sync for LinkedList<L> {}

/// Defines the structure of a linked list node.
/// Provides methods for converting between nodes and instances.
///
/// # Safety
///
/// The implementation must ensure that the inserted data does not move in memory.
pub(crate) unsafe trait Link {
    unsafe fn node(ptr: NonNull<Self>) -> NonNull<Node<Self>>
    where
        Self: Sized;
}

impl<L: Link + Default> LinkedList<L> {
    /// Construct a new linked list.
    #[allow(dead_code)]
    pub(crate) fn new() -> LinkedList<L> {
        let head = Box::<L>::default();
        let head_ptr = unsafe { NonNull::new_unchecked(Box::into_raw(head)) };
        let node = unsafe { L::node(head_ptr).as_mut() };
        node.prev = Some(head_ptr);
        node.next = Some(head_ptr);
        LinkedList { head: head_ptr }
    }

    /// Insert an element to the front of the list.
    #[allow(dead_code)]
    pub(crate) fn push_front(&mut self, val: NonNull<L>) {
        unsafe {
            let head = L::node(self.head).as_mut();
            L::node(val).as_mut().next = head.next;
            L::node(val).as_mut().prev = Some(self.head);

            let node = Some(val);
            if let Some(first) = head.next {
                L::node(first).as_mut().prev = node;
            }
            head.next = node;
        }
    }

    /// Pop an element from the back of the list.
    #[allow(dead_code)]
    pub(crate) fn pop_back(&mut self) -> Option<NonNull<L>> {
        unsafe {
            let head = L::node(self.head).as_mut();
            if head.prev != Some(self.head) {
                let node = head.prev.take().unwrap();
                Node::remove_node(node);
                Some(node)
            } else {
                None
            }
        }
    }

    /// Delete an element in list.
    ///
    /// # Safety
    ///
    /// This method can be safely used when the node is in a linked list that the caller has
    /// unique access to or the node is not in any linked list.
    #[allow(dead_code)]
    pub(crate) unsafe fn remove(node: NonNull<L>) -> Option<NonNull<L>> {
        Node::remove_node(node)
    }

    /// Check whether the list is empty.
    #[allow(dead_code)]
    pub(crate) fn is_empty(&self) -> bool {
        unsafe { L::node(self.head).as_ref().next == Some(self.head) }
    }

    /// Empty the list.
    #[allow(dead_code)]
    pub(crate) fn clear(&mut self) {
        let node = unsafe { L::node(self.head).as_mut() };
        node.prev = Some(self.head);
        node.next = Some(self.head);
    }
}

impl<L: Link + Default> Default for LinkedList<L> {
    fn default() -> Self {
        LinkedList::new()
    }
}

impl<L: Link + Default> Drop for LinkedList<L> {
    fn drop(&mut self) {
        let _ = unsafe { Box::from_raw(self.head.as_ptr()) };
    }
}

#[cfg(test)]
mod tests {
    use crate::util::link_list::{Link, LinkedList, Node};
    use std::ptr::{addr_of_mut, NonNull};

    #[derive(Default)]
    struct Entry {
        val: usize,
        node: Node<Entry>,
    }

    impl Entry {
        fn new(val: usize) -> Entry {
            Entry {
                val,
                node: Node::new(),
            }
        }

        fn get_ptr(&self) -> NonNull<Self> {
            NonNull::from(self)
        }
    }

    unsafe fn address_of_node(mut ptr: NonNull<Entry>) -> NonNull<Node<Entry>> {
        let node_ptr = addr_of_mut!(ptr.as_mut().node);
        NonNull::new_unchecked(node_ptr)
    }

    fn get_val(ptr: NonNull<Entry>) -> usize {
        unsafe { ptr.as_ref().val }
    }

    unsafe impl Link for Entry {
        unsafe fn node(ptr: NonNull<Self>) -> NonNull<Node<Self>> {
            address_of_node(ptr)
        }
    }

    /// UT test for `is_empty()` and `clear()`.
    ///
    /// # Title
    /// ut_link_list_is_empty_and_clear
    ///
    /// # Brief
    /// 1.Create a linked list.
    /// 2.Check if the list is empty before and after pushing nodes into the list.
    /// 3.Check if the list is empty before and after clear the list.
    #[test]
    fn ut_link_list_is_empty_and_clear() {
        let mut list = LinkedList::<Entry>::new();
        assert!(list.is_empty());
        let node1 = Entry::new(1);
        let node1 = node1.get_ptr();
        let node2 = Entry::new(2);
        let node2 = node2.get_ptr();
        let node3 = Entry::new(3);
        let node3 = node3.get_ptr();
        list.push_front(node1);
        assert!(!list.is_empty());
        list.push_front(node2);
        list.push_front(node3);
        list.clear();
        assert!(list.is_empty());
    }

    /// UT test for `push_front()` and `pop_back()`.
    ///
    /// # Title
    /// ut_link_list_push_and_pop
    ///
    /// # Brief
    /// 1.Create a linked list.
    /// 2.Push nodes into the list.
    /// 3.Pop nodes from the list and check the value.
    #[test]
    fn ut_link_list_push_and_pop() {
        let mut list = LinkedList::<Entry>::new();
        assert!(list.is_empty());
        let node1 = Entry::new(1);
        let node1 = node1.get_ptr();
        let node2 = Entry::new(2);
        let node2 = node2.get_ptr();
        let node3 = Entry::new(3);
        let node3 = node3.get_ptr();
        list.push_front(node1);
        assert!(!list.is_empty());
        list.push_front(node2);
        list.push_front(node3);
        assert_eq!(1, get_val(list.pop_back().unwrap()));
        assert_eq!(2, get_val(list.pop_back().unwrap()));
        assert_eq!(3, get_val(list.pop_back().unwrap()));
        assert!(list.pop_back().is_none());
        assert!(list.is_empty());
    }

    /// UT test for `push_front()` and `remove()`.
    ///
    /// # Title
    /// ut_link_list_remove
    ///
    /// # Brief
    /// 1.Create a linked list.
    /// 2.Push nodes into the list.
    /// 3.Remove the first node from the list and check the list.
    /// 4.Remove the second node from the list and check the list.
    /// 5.Remove the third node from the list and check the list.
    #[test]
    fn ut_link_list_remove() {
        let mut list = LinkedList::<Entry>::new();
        assert!(list.is_empty());
        let node1 = Entry::new(1);
        let node1_ptr = node1.get_ptr();
        let node2 = Entry::new(2);
        let node2_ptr = node2.get_ptr();
        let node3 = Entry::new(3);
        let node3_ptr = node3.get_ptr();
        list.push_front(node1_ptr);
        assert!(!list.is_empty());
        list.push_front(node2_ptr);
        list.push_front(node3_ptr);
        unsafe {
            assert!(LinkedList::remove(node1_ptr).is_some());
            assert!(LinkedList::remove(node1_ptr).is_none());
            assert_eq!(Some(node2_ptr), node3.node.next);
            assert_eq!(Some(node3_ptr), node2.node.prev);
            assert!(LinkedList::remove(node2_ptr).is_some());
            assert!(LinkedList::remove(node2_ptr).is_none());
            assert!(LinkedList::remove(node3_ptr).is_some());
            assert!(LinkedList::remove(node3_ptr).is_none());
        }

        list.push_front(node1_ptr);
        list.push_front(node2_ptr);
        list.push_front(node3_ptr);
        unsafe {
            assert!(LinkedList::remove(node2_ptr).is_some());
            assert!(LinkedList::remove(node2_ptr).is_none());
            assert_eq!(Some(node1_ptr), node3.node.next);
            assert_eq!(Some(node3_ptr), node1.node.prev);
            assert!(LinkedList::remove(node1_ptr).is_some());
            assert!(LinkedList::remove(node1_ptr).is_none());
            assert!(LinkedList::remove(node3_ptr).is_some());
            assert!(LinkedList::remove(node3_ptr).is_none());
        }

        list.push_front(node1_ptr);
        list.push_front(node2_ptr);
        list.push_front(node3_ptr);
        unsafe {
            assert!(LinkedList::remove(node3_ptr).is_some());
            assert!(LinkedList::remove(node3_ptr).is_none());
            assert_eq!(Some(node1_ptr), node2.node.next);
            assert_eq!(Some(node2_ptr), node1.node.prev);
            assert!(LinkedList::remove(node1_ptr).is_some());
            assert!(LinkedList::remove(node1_ptr).is_none());
            assert!(LinkedList::remove(node2_ptr).is_some());
            assert!(LinkedList::remove(node2_ptr).is_none());
        }
        assert!(list.is_empty());
    }
}
