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

//! Slots container, similar to [`std::collections::LinkedList`]

use std::error::Error;
use std::fmt;
use std::fmt::{Debug, Display, Formatter};
use std::ops;
use std::option::Option::Some;

//Index tag of empty slot, vector will panic if the new capacity exceeds isize::MAX bytes.
const NULL: usize = usize::MAX;

#[derive(Debug, Eq, PartialEq)]
struct Entry<T> {
    data: Option<T>,
    prev: usize,
    next: usize,
}

impl<T> Entry<T> {
    fn new(val: T, prev: usize, next: usize) -> Entry<T> {
        Entry {
            data: Some(val),
            prev,
            next,
        }
    }
}

/// An iterator to traverse through the slots
pub struct SlotsIter<'a, T: 'a> {
    entries: &'a Vec<Entry<T>>,
    len: usize,
    head: usize,
}

/// An entry for the slots.
#[derive(Eq, PartialEq)]
pub struct Slots<T> {
    entries: Vec<Entry<T>>,
    len: usize,
    head: usize,
    tail: usize,
    next: usize,
}

/// The index of slot to remove is invalid.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct SlotsError;

impl Display for SlotsError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "invalid key")
    }
}

impl Error for SlotsError {}

impl<T> Slots<T> {
    /// Construct a new 'Slots' container with initial values of 0.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::util::slots::Slots;
    ///
    /// let slots: Slots<i32> = Slots::new();
    /// ```
    pub fn new() -> Slots<T> {
        Slots::with_capacity(0)
    }

    /// Returns the current number of elements in the container.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::util::slots::Slots;
    ///
    /// let mut slots = Slots::new();
    /// for i in 0..3 {
    ///     slots.push_back(i);
    /// }
    /// assert_eq!(3, slots.len());
    /// ```
    pub fn len(&self) -> usize {
        self.len
    }

    /// Empty the container.
    ///
    /// # Examples
    /// ```
    ///
    /// use ylong_runtime::util::slots::Slots;
    ///
    /// let mut slots = Slots::new();
    /// for i in 0..3 {
    ///     slots.push_back(i);
    /// }
    /// slots.clear();
    /// assert_eq!(slots.len(), 0);
    /// ```
    pub fn clear(&mut self) {
        self.entries.clear();
        self.len = 0;
        self.head = NULL;
        self.tail = NULL;
        self.next = 0;
    }

    /// Insert an element to the end of the list of container.
    ///
    /// Two situations:
    ///
    /// 1.The next slot is exactly the next position of the array. Insert the data into the vector,
    ///   increase the length, and calculate the next insertion position.
    ///
    /// 2.The next slot is the recycled slot. Update the index of `next` and then insert the data.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::util::slots::Slots;
    ///
    /// let mut slots = Slots::new();
    /// let zero = slots.push_back("zero");
    /// let one = slots.push_back("one");
    /// assert_eq!(zero, 0);
    /// assert_eq!(one, 1);
    ///
    /// let temp = slots.remove(zero);
    /// assert_eq!(temp, Ok("zero"));
    ///
    /// let two = slots.push_back("two");
    /// assert_eq!(two, zero);
    /// ```
    pub fn push_back(&mut self, val: T) -> usize {
        let key = self.next;
        let tail = self.tail;

        self.len += 1;

        if key == self.entries.len() {
            // The next slot is exactly the next position of the array.
            // Insert the data into the end of vector.
            self.next = key + 1;
            self.entries.push(Entry::new(val, tail, NULL));
        } else {
            // The next slot is the recycled slot.
            // Update the index of `next` and then insert the data.
            self.next = self.entries[key].next;
            self.entries[key].prev = tail;
            self.entries[key].next = NULL;
            self.entries[key].data = Some(val);
        }

        match self.entries.get_mut(tail) {
            None => {
                self.head = key;
            }
            Some(entry) => {
                entry.next = key;
            }
        }
        self.tail = key;
        key
    }

    /// Pop item from the container head.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::util::slots::Slots;
    ///
    /// let mut slots = Slots::new();
    /// let zero = slots.push_back("zero");
    ///
    /// assert_eq!(slots.pop_front(), Some("zero"));
    /// assert!(!slots.contains(zero));
    /// ```
    pub fn pop_front(&mut self) -> Option<T> {
        let curr = self.head;
        if let Some(entry) = self.entries.get_mut(curr) {
            self.head = entry.next;
            // At the next insertion, update the next insertion position.
            entry.prev = NULL;
            entry.next = self.next;
            let val = entry.data.take();
            match self.entries.get_mut(self.head) {
                None => {
                    self.tail = NULL;
                }
                Some(head) => {
                    head.prev = NULL;
                }
            }
            //Update linked-list information.
            self.len -= 1;
            self.next = curr;
            return val;
        }
        None
    }

    /// Delete an element in container.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::util::slots::Slots;
    ///
    /// let mut slots = Slots::new();
    /// let zero = slots.push_back("zero");
    ///
    /// assert_eq!(slots.remove(zero), Ok("zero"));
    /// assert!(!slots.contains(zero));
    /// ```
    pub fn remove(&mut self, key: usize) -> Result<T, SlotsError> {
        if let Some(entry) = self.entries.get_mut(key) {
            if let Some(val) = entry.data.take() {
                let prev = entry.prev;
                let next = entry.next;
                // At the next insertion, update the next insertion position
                entry.prev = NULL;
                entry.next = self.next;
                //If this node is the header node, update the header node; otherwise, update the `next` of the previous node.
                match self.entries.get_mut(prev) {
                    None => {
                        self.head = next;
                    }
                    Some(slot) => {
                        slot.next = next;
                    }
                }
                //If this node is the tail node, update the tail node; otherwise, update the `prev` of the next node.
                match self.entries.get_mut(next) {
                    None => {
                        self.tail = prev;
                    }
                    Some(slot) => {
                        slot.prev = prev;
                    }
                }
                //Update linked-list information.
                self.len -= 1;
                self.next = key;
                return Ok(val);
            }
        }
        Err(SlotsError)
    }

    /// Get the reference of the element in the container according to the index.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::util::slots::Slots;
    ///
    /// let mut slots = Slots::new();
    /// let key = slots.push_back("key");
    ///
    /// assert_eq!(slots.get(key), Some(&"key"));
    /// assert_eq!(slots.get(123), None);
    /// ```
    pub fn get(&self, key: usize) -> Option<&T> {
        match self.entries.get(key) {
            Some(entry) => match entry.data.as_ref() {
                Some(val) => Some(val),
                None => None,
            },
            None => None,
        }
    }

    /// Get the mutable reference of the element in the container according to the index.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::util::slots::Slots;
    ///
    /// let mut slots = Slots::new();
    /// let key = slots.push_back("key");
    ///
    /// *slots.get_mut(key).unwrap() = "change_key";
    /// assert_eq!(*slots.get(key).unwrap(), "change_key");
    /// assert_eq!(slots.get(123), None);
    /// ```
    pub fn get_mut(&mut self, key: usize) -> Option<&mut T> {
        match self.entries.get_mut(key) {
            Some(entry) => match entry.data.as_mut() {
                Some(val) => Some(val),
                None => None,
            },
            None => None,
        }
    }

    /// Check whether the index is valid and whether there is an element at the index in the container.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::util::slots::Slots;
    ///
    /// let mut slots = Slots::new();
    /// let key = slots.push_back("key");
    ///
    /// assert!(slots.contains(key));
    /// assert!(!slots.contains(123));
    /// ```
    pub fn contains(&self, key: usize) -> bool {
        match self.entries.get(key) {
            Some(entry) => entry.data.is_some(),
            None => false,
        }
    }

    /// Create an immutable iterator for the container to poll all elements int the order of insertion.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::util::slots::Slots;
    ///
    /// let mut slots = Slots::new();
    /// for i in 0..3 {
    ///     slots.push_back(i);
    /// }
    ///
    /// for (key, element) in slots.iter() {
    ///     assert_eq!(slots.get(key).unwrap(), element);
    /// }
    ///
    /// ```
    pub fn iter(&self) -> SlotsIter<T> {
        SlotsIter {
            entries: &self.entries,
            len: self.len,
            head: self.head,
        }
    }

    /// Check whether the container is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::util::slots::Slots;
    ///
    /// let mut slots = Slots::new();
    /// assert!(slots.is_empty());
    ///
    /// slots.push_back(1);
    /// assert!(!slots.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Construct a new 'Slots' container with a capacity.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::util::slots::Slots;
    ///
    /// let slots: Slots<i32> = Slots::with_capacity(0);
    /// ```
    pub fn with_capacity(capacity: usize) -> Slots<T> {
        Slots {
            entries: Vec::with_capacity(capacity),
            head: NULL,
            tail: NULL,
            next: 0,
            len: 0,
        }
    }

    /// Get the capacity of the container.
    ///
    /// # Examples
    ///
    /// ```
    /// use ylong_runtime::util::slots::Slots;
    ///
    /// let mut slots: Slots<i32> = Slots::new();
    /// assert_eq!(slots.capacity(), 0);
    /// ```
    pub fn capacity(&self) -> usize {
        self.entries.capacity()
    }
}

impl<T> Default for Slots<T> {
    fn default() -> Slots<T> {
        Slots::new()
    }
}

impl<T> ops::Index<usize> for Slots<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        match self.entries.get(index) {
            Some(entry) => match entry.data {
                Some(ref val) => val,
                None => panic!("invalid index"),
            },
            None => panic!("invalid index"),
        }
    }
}

impl<T> ops::IndexMut<usize> for Slots<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        match self.entries.get_mut(index) {
            Some(entry) => match entry.data {
                Some(ref mut val) => val,
                None => panic!("invalid index"),
            },
            None => panic!("invalid index"),
        }
    }
}

impl<T> Debug for Slots<T>
where
    T: Debug,
{
    fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
        write!(
            fmt,
            "Slab {{ len: {}, head: {}, tail: {}, next: {}, cap: {} }}",
            self.len,
            self.head,
            self.tail,
            self.next,
            self.entries.capacity(),
        )
    }
}

impl<'a, T> Iterator for SlotsIter<'a, T> {
    type Item = (usize, &'a T);

    fn next(&mut self) -> Option<(usize, &'a T)> {
        if self.len == 0 {
            return None;
        }
        let head = self.head;
        if let Some(entry) = self.entries.get(head) {
            self.len -= 1;
            self.head = entry.next;
            if let Some(val) = entry.data.as_ref() {
                return Some((head, val));
            }
        }
        None
    }
}

#[cfg(all(test))]
mod test {
    use crate::util::slots::Slots;

    #[derive(Debug, Eq, PartialEq)]
    struct Data {
        inner: i32,
    }

    impl Data {
        fn new(inner: i32) -> Self {
            Data { inner }
        }
    }

    /// UT test for Slots::new().
    /// # Title
    /// ut_slots_new
    /// # Brief
    /// 1.Verify the parameters after initialization, which should be consistent with the initial value.
    #[test]
    fn ut_slots_new() {
        let slots: Slots<i32> = Slots::new();
        assert_eq!(slots.len, 0);
        assert_eq!(slots.next, 0);
        assert_eq!(slots.entries, Vec::with_capacity(0));
    }

    /// UT test for Slots::with_capacity().
    /// # Title
    /// ut_slots_with_capacity
    /// # Brief
    /// 1.Verify the parameters after initialization, which should be consistent with the initial value.
    #[test]
    fn ut_slots_with_capacity() {
        let slots_new: Slots<i32> = Slots::new();
        let slots_with_capacity: Slots<i32> = Slots::with_capacity(0);
        assert_eq!(slots_new, slots_with_capacity);
    }

    /// UT test for Slots::len().
    /// # Title
    /// ut_slots_len
    /// # Brief
    /// 1.Inserting a certain number of data into the container.
    #[test]
    fn ut_slots_len() {
        let mut slots = Slots::new();
        assert_eq!(slots.len(), 0_usize);

        for i in 0..1000 {
            slots.push_back(i);
        }
        assert_eq!(slots.len(), 1000);
    }

    /// UT test for Slots::clear().
    /// # Title
    /// ut_slots_clear
    /// # Brief
    /// 1.Empty the container to make it exactly the same as the initialized container.
    #[test]
    fn ut_slots_clear() {
        let mut slots = Slots::new();
        for i in 0..1000 {
            slots.push_back(i);
        }
        assert_ne!(slots, Slots::new());

        slots.clear();
        assert_eq!(slots, Slots::new());
    }

    /// UT test for Slots::insert().
    /// # Title
    /// ut_slots_push_back
    /// # Brief
    /// 1.The next slot is exactly the next position of the array. Insert the data into the vector,
    /// increase the length, and calculate the next insertion position.
    /// 2.The next slot is the recycled slot. Update the index of `next` and then insert the data.
    #[test]
    fn ut_slots_push_back() {
        let mut slots = Slots::new();
        let mut keys = Vec::new();

        for data in 0..100 {
            let key = slots.push_back(data);
            keys.push(key);
        }
        for (data, key) in keys.iter().enumerate() {
            assert_eq!(*slots.get(*key).unwrap(), data);
        }

        let mut slots = Slots::new();
        let mut keys = Vec::new();

        for data in 0..100 {
            let key = slots.push_back(data);
            keys.push(key);
        }
        for index in 0..50 {
            let res = slots.remove(index);
            assert!(res.is_ok());
        }
        for data in 100..150 {
            slots.push_back(data);
        }
        let mut cnt = 149;
        for index in 0..50 {
            assert_eq!(*slots.get(index).unwrap(), cnt);
            cnt -= 1;
        }

        let mut slots = Slots::new();
        let mut keys = Vec::new();

        for data in 0..100 {
            let key = slots.push_back(Data::new(data));
            keys.push(key);
        }
        for index in 0..50 {
            let res = slots.remove(index);
            assert!(res.is_ok());
        }
        for data in 100..150 {
            slots.push_back(Data::new(data));
        }
        let mut cnt = 149;
        for index in 0..50 {
            assert_eq!(*slots.get(index).unwrap(), Data::new(cnt));
            cnt -= 1;
        }
    }

    /// UT test for Slots::pop_front()
    /// # Title
    /// ut_slots_pop_front
    /// # Brief
    /// 1.Pop the slot from the head of container.
    #[test]
    fn ut_slots_pop_front() {
        let mut slots = Slots::new();
        for data in 0..100 {
            slots.push_back(data);
        }
        for index in 0..50 {
            assert_eq!(slots.pop_front(), Some(index));
            assert_eq!(slots.get(index), None);
        }
        assert_eq!(slots.len(), 50);

        for data in 100..150 {
            slots.push_back(data);
        }
        for target in 50..150 {
            assert_eq!(slots.pop_front(), Some(target));
        }
        assert_eq!(slots.pop_front(), None);
        assert_eq!(slots.len(), 0);
    }

    /// UT test for Slots::remove().
    /// # Title
    /// ut_slots_remove
    /// # Brief
    /// 1.Get the invalid data location.
    /// 2.Get the valid data location, and it stores data.
    /// 3.Get the valid data location, and it doesn't store data.
    #[test]
    fn ut_slots_remove() {
        let mut slots = Slots::new();
        let mut keys = Vec::new();

        for data in 0..100 {
            let key = slots.push_back(data);
            keys.push(key);
        }
        assert_eq!(slots.remove(0), Ok(0));
    }

    /// UT test for Slots::get().
    /// # Title
    /// ut_slots_get
    /// # Brief
    /// 1.Enter the location that stores data.
    /// 2.Enter the location that doesn't store data.
    #[test]
    fn ut_slots_get() {
        let mut slots = Slots::new();
        let key = slots.push_back("key");

        assert_eq!(slots.get(key), Some(&"key"));
        assert_eq!(slots.get(123), None);
    }

    /// UT test for Slots::get_mut().
    /// # Title
    /// ut_slots_get_mut
    /// # Brief
    /// 1.Enter the location that stores data.
    /// 2.Enter the location that doesn't store data.
    #[test]
    fn ut_slots_get_mut() {
        let mut slots = Slots::new();
        let key = slots.push_back("key");

        *slots.get_mut(key).unwrap() = "change_key";
        assert_eq!(*slots.get(key).unwrap(), "change_key");
        assert_eq!(slots.get(123), None);
    }

    /// UT test for Slots::contains().
    /// # Title
    /// ut_slots_contains
    /// # Brief
    /// 1.The container exists the location and there are data at that location.
    /// 2.The container exists the location and there are no data at that location.
    /// 3.The container doesn't exist the location.
    #[test]
    fn ut_slots_contains() {
        let mut slots = Slots::new();
        let key = slots.push_back("key");

        assert!(slots.contains(key));
        assert!(!slots.contains(123));

        let res = slots.remove(key);
        assert!(res.is_ok());
        assert!(!slots.contains(key));
    }

    /// UT test for Slots::iter().
    /// # Title
    /// ut_slots_iter
    /// # Brief
    /// 1.Validate elements through iterators.
    #[test]
    fn ut_slots_iter() {
        let mut slots = Slots::new();

        for i in 0..3 {
            slots.push_back(i);
        }

        for (key, element) in slots.iter() {
            assert_eq!(slots.get(key).unwrap(), element);
        }
    }

    /// UT test for Slots::is_empty().
    /// # Title
    /// ut_slots_is_empty
    /// # Brief
    /// 1.Verify empty container, the result is true.
    /// 2.Verify non-empty container, the result is false.
    #[test]
    fn ut_slots_is_empty() {
        let mut slots = Slots::new();
        assert!(slots.is_empty());

        slots.push_back(1);
        assert!(!slots.is_empty());
    }

    /// UT test for Slots::capacity().
    /// # Title
    /// ut_slots_capacity
    /// # Brief
    /// 1.Verify the container that is initialized.
    /// 2.Verify the container that is initialized with specific capacity.
    #[test]
    fn ut_slots_capacity() {
        let slots: Slots<i32> = Slots::new();
        assert_eq!(slots.capacity(), 0);

        let slots: Slots<i32> = Slots::with_capacity(10);
        assert_eq!(slots.capacity(), 10);
    }
}
