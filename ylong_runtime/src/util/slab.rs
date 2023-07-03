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

//! ## `slab` can allocate storage space for the same data type
//!
//! `slab` will pre-allocate space for the stored data.
//! When the amount of stored data exceeds the pre-allocated space,
//! `slab` has a growth strategy similar to the [`vec`][vec] module,
//! and when new space is needed, the `slab` grows to **twice**.
//!
//! ### Page
//!
//! The primary storage space in `slab` is a two-dimensional array
//! that holds ['vec'][vec] containers on each page, which grows as
//! `page` grows, with `page` initially being 32 in length and each
//! new `page` added thereafter requiring a length of 2x. The total
//! number of pages in `page` is 19.
//!
//! ### Release
//!
//! When a piece of data in `slab` is no longer in use and is freed,
//! the space where the current data store is located should be reused,
//! and this operation will be used in conjunction with the allocation operation.
//!
//! ### Allocate
//!
//! There are two cases of space allocation for `slab`. One case is
//! that the current space has never been allocated before, then normal
//! space allocation is done for the current container and the parameters
//! are updated. In the other case, it is used in conjunction with the
//! function release. i.e., when the allocation is done again, the space
//! where the previously freed data is located will be used.
//!
//! ### Compact
//!
//! is used to clean up the resources in the `slab` container after a
//! specific number of loops, which is one of the most important uses
//! of this container, to clean up the space that has been allocated
//! but has not yet been used.
//!
//! [vec]: https://doc.rust-lang.org/std/vec/index.html

use std::cell::UnsafeCell;
use std::ops::Deref;
use std::sync::atomic::Ordering::{Relaxed, SeqCst};
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::{Arc, Mutex};

/// The maximum number of `pages` that `Slab` can hold
const NUM_PAGES: usize = 19;

/// The minimum number of `slots` that `page` can hold
const PAGE_INITIAL_SIZE: usize = 32;
const PAGE_INDEX_SHIFT: u32 = PAGE_INITIAL_SIZE.trailing_zeros() + 1;

/// trait bounds mechanism, so that the binder must implement the `Entry` and `Default` trait methods
pub trait Entry: Default {
    /// Resets the entry.
    fn reset(&self);
}
// #################################################################################################
/// Reference to data stored in `slab`
pub struct Ref<T> {
    value: *const Value<T>,
}

/// Release operation of data stored in `slab` for reuse in the next allocated space
impl<T> Drop for Ref<T> {
    fn drop(&mut self) {
        unsafe {
            let _ = (*self.value).release();
        }
    }
}

/// Provide unquote operation for user-friendly operation
impl<T> Deref for Ref<T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &(*self.value).value }
    }
}

unsafe impl<T: Sync> Sync for Ref<T> {}
unsafe impl<T: Sync> Send for Ref<T> {}
// #################################################################################################

/// The Address of the stored data.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Address(usize);

/// Gets the bit size of a pointer.
pub const fn pointer_width() -> u32 {
    std::mem::size_of::<usize>() as u32 * 8
}

impl Address {
    /// Get the number of `page` pages at the current address
    pub fn page(&self) -> usize {
        let slot_shifted = (self.0 + PAGE_INITIAL_SIZE) >> PAGE_INDEX_SHIFT;
        (pointer_width() - slot_shifted.leading_zeros()) as usize
    }

    /// Convert `Address` to `usize`
    pub const fn as_usize(self) -> usize {
        self.0
    }

    /// Convert `usize` to `Address`
    pub fn from_usize(src: usize) -> Address {
        Address(src)
    }
}
// #################################################################################################

/// Amortized allocation for homogeneous data types.
pub struct Slab<T> {
    /// Essentially a two-dimensional array, the constituent units in the container
    pages: [Arc<Page<T>>; NUM_PAGES],
}

impl<T: Entry> Default for Slab<T> {
    fn default() -> Slab<T> {
        Slab::new()
    }
}

impl<T: Entry> Slab<T> {
    /// Set up the initialization parameters
    pub fn new() -> Slab<T> {
        let mut slab = Slab {
            pages: Default::default(),
        };

        // The minimum number of `slots` that can fit in a `page` at initialization, where the default value is 32
        let mut len = PAGE_INITIAL_SIZE;
        // The sum of the lengths of all `pages` before this `page`, i.e. the sum of `len`
        let mut prev_len: usize = 0;

        for page in &mut slab.pages {
            let page = Arc::get_mut(page).unwrap();
            page.len = len;
            page.prev_len = prev_len;
            // The `len` of each `page` will be doubled from the previous one
            len *= 2;
            prev_len += page.len;
        }

        slab
    }

    /// Easy to call for allocation
    pub fn handle(&self) -> Slab<T> {
        Slab {
            pages: self.pages.clone(),
        }
    }

    /// Space allocation for containers
    ///
    /// # Safety
    /// 1. The essence of space allocation to the container is actually to allocate each page of the container for the operation
    /// 2. Before allocating each page of the container, we will try to get lock permission to prevent multiple threads from having permission to modify the state
    ///
    /// Using pointers
    pub unsafe fn allocate(&self) -> Option<(Address, Ref<T>)> {
        // Find the first available `slot`
        for page in &self.pages[..] {
            if let Some((addr, val)) = Page::allocate(page) {
                return Some((addr, val));
            }
        }

        None
    }

    /// Iterating over the data in the container
    pub fn for_each(&mut self, mut f: impl FnMut(&T)) {
        for page_idx in 0..self.pages.len() {
            let slots = self.pages[page_idx].slots.lock().unwrap();

            for slot_idx in 0..slots.slots.len() {
                unsafe {
                    let slot = slots.slots.as_ptr().add(slot_idx);
                    let value = slot as *const Value<T>;

                    f(&(*value).value);
                }
            }
        }
    }

    /// Used to get the reference stored at the given address
    pub fn get(&mut self, addr: Address) -> Option<&T> {
        let page_idx = addr.page();
        let slot_idx = self.pages[page_idx].slot(addr);

        if !self.pages[page_idx].allocated.load(SeqCst) {
            return None;
        }

        unsafe {
            // Fetch by pointer, usage is similar to `C`
            let slot = self.pages[page_idx]
                .slots
                .lock()
                .unwrap()
                .slots
                .as_ptr()
                .add(slot_idx);

            let value = slot as *const Value<T>;

            Some(&(*value).value)
        }
    }

    /// Used to clean up the resources in the `Slab` container after a specific number of loops, which is one of the most important uses of this container
    ///
    /// # Safety
    /// Releasing resources here does not release resources that are being used or have not yet been allocated
    /// 1. The release of each page will initially determine if the resources on the current page are being used or if the current page has not been allocated
    /// 2. Next, it will determine whether the `slots` of the current page are owned by other threads to prevent its resources from changing to the used state
    /// 3. Finally, the checks are performed again, with the same checks as in the first step, to prevent state changes and ensure that no errors or invalid releases are made
    ///
    /// Using atomic variables
    pub unsafe fn compact(&mut self) {
        for (_, page) in (self.pages[1..]).iter().enumerate() {
            // The `slots` of the current `page` are being used, or the current `page` is not allocated and not cleaned up.
            if page.used.load(Relaxed) != 0 || !page.allocated.load(Relaxed) {
                continue;
            }

            // The current `slots` are being owned by other threads and are not cleaned up.
            let mut slots = match page.slots.try_lock() {
                Ok(slots) => slots,
                _ => continue,
            };

            // Check again, if the `slots` of the current `page` are being used, or if the current `page` is not allocated, do not clean up.
            if slots.used > 0 || slots.slots.capacity() == 0 {
                continue;
            }

            page.allocated.store(false, Relaxed);

            let vec = std::mem::take(&mut slots.slots);
            slots.head = 0;

            drop(slots);
            drop(vec);
        }
    }
}
// #################################################################################################
struct Page<T> {
    // Number of `slots` currently being used
    pub used: AtomicUsize,
    // Whether the current `page` is allocated space
    pub allocated: AtomicBool,
    // The number of `slots` that `page` can hold
    pub len: usize,
    // The sum of the lengths of all `pages` before the `page`, i.e. the sum of the number of `slots`
    pub prev_len: usize,
    // `Slots`
    pub slots: Mutex<Slots<T>>,
}

unsafe impl<T: Sync> Sync for Page<T> {}
unsafe impl<T: Sync> Send for Page<T> {}

impl<T> Page<T> {
    // Get the location of the `slot` in the current `page` based on the current `Address`.
    fn slot(&self, addr: Address) -> usize {
        addr.0 - self.prev_len
    }

    // Get the current `Address` based on the `slot` location in the current `page`
    fn addr(&self, slot: usize) -> Address {
        Address(slot + self.prev_len)
    }

    fn release(&self, value: *const Value<T>) {
        let mut locked = self.slots.lock().unwrap();

        // Get the current `slot` based on the `value` value
        let idx = locked.index_for(value);
        locked.slots[idx].next = locked.head as u32;
        locked.head = idx;
        locked.used -= 1;

        self.used.store(locked.used, Relaxed);
    }
}

impl<T: Entry> Page<T> {
    unsafe fn allocate(me: &Arc<Page<T>>) -> Option<(Address, Ref<T>)> {
        if me.used.load(Relaxed) == me.len {
            return None;
        }

        let mut locked = me.slots.lock().unwrap();

        if locked.head < locked.slots.len() {
            let locked = &mut *locked;

            let idx = locked.head;
            let slot = &locked.slots[idx];

            locked.head = slot.next as usize;

            locked.used += 1;
            me.used.store(locked.used, Relaxed);

            (*slot.value.get()).value.reset();

            Some((me.addr(idx), slot.gen_ref(me)))
        } else if me.len == locked.slots.len() {
            None
        } else {
            let idx = locked.slots.len();

            if idx == 0 {
                locked.slots.reserve_exact(me.len);
            }

            locked.slots.push(Slot {
                value: UnsafeCell::new(Value {
                    value: Default::default(),
                    page: &**me as *const _,
                }),
                next: 0,
            });

            locked.head += 1;
            locked.used += 1;
            me.used.store(locked.used, Relaxed);
            me.allocated.store(true, Relaxed);

            Some((me.addr(idx), locked.slots[idx].gen_ref(me)))
        }
    }
}

impl<T> Default for Page<T> {
    fn default() -> Page<T> {
        Page {
            used: AtomicUsize::new(0),
            allocated: AtomicBool::new(false),
            len: 0,
            prev_len: 0,
            slots: Mutex::new(Slots::new()),
        }
    }
}
// #################################################################################################
struct Slots<T> {
    pub slots: Vec<Slot<T>>,
    pub head: usize,
    pub used: usize,
}

impl<T> Slots<T> {
    fn new() -> Slots<T> {
        Slots {
            slots: Vec::new(),
            head: 0,
            used: 0,
        }
    }

    fn index_for(&self, slot: *const Value<T>) -> usize {
        use std::mem;

        // Get the first address of the current `page`
        let base = &self.slots[0] as *const _ as usize;

        // The case where the first address is 0 and `page` is unallocated
        // if base == 0 {
        //     logerr!("`page` unallocated");
        // }

        // Get the current `slot` address
        let slot = slot as usize;
        // Get `Vec` internal element size
        let width = mem::size_of::<Slot<T>>();

        // if slot < base {
        //     logerr!("wrong address");
        // }

        // Get the current `idx`
        (slot - base) / width
        // if idx >= self.slots.len() as usize {
        //     logerr!("idx out of range");
        // }
    }
}
// #################################################################################################
#[derive(Debug)]
struct Slot<T> {
    pub value: UnsafeCell<Value<T>>,
    pub next: u32,
}

impl<T> Slot<T> {
    fn gen_ref(&self, page: &Arc<Page<T>>) -> Ref<T> {
        std::mem::forget(page.clone());
        let slot = self as *const Slot<T>;
        let value = slot as *const Value<T>;

        Ref { value }
    }
}
// #################################################################################################
#[derive(Debug)]
struct Value<T> {
    pub value: T,
    pub page: *const Page<T>,
}

impl<T> Value<T> {
    unsafe fn release(&self) -> Arc<Page<T>> {
        let page = Arc::from_raw(self.page);
        page.release(self as *const _);
        page
    }
}

#[cfg(all(test))]
mod test {
    use crate::util::slab::{Address, Entry, Slab, NUM_PAGES, PAGE_INITIAL_SIZE};
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering::SeqCst;

    struct Foo {
        cnt: AtomicUsize,
        id: AtomicUsize,
    }

    impl Default for Foo {
        fn default() -> Foo {
            Foo {
                cnt: AtomicUsize::new(0),
                id: AtomicUsize::new(0),
            }
        }
    }

    impl Entry for Foo {
        fn reset(&self) {
            self.cnt.fetch_add(1, SeqCst);
        }
    }

    /*
     * @title  Slab::new() ut test
     * @design The function has no input, no exception branch, direct check function, return value
     * @precon After calling Slab::new() to initialize the container, get its object
     * @brief  Describe test case execution
     *         1、Check the parameters for completion of initialization, such as the number of pages to be checked, the length of each page
     * @expect 1、The initialization of the completed parameters should be the same as the expected value
     * @auto   true
     */
    #[test]
    fn ut_slab_new() {
        let slab = Slab::<Foo>::new();
        assert_eq!(slab.pages.len(), NUM_PAGES);

        for (index, page) in slab.pages.iter().enumerate() {
            assert_eq!(page.len, PAGE_INITIAL_SIZE * 2_usize.pow(index as u32));
        }
    }

    /*
     * @title  Slab::for_each() ut test
     * @design The function has no invalid input, no exception branch, direct check function, return value
     * @precon After calling Slab::new() to initialize the container, get its object
     * @brief  Describe test case execution
     *         1、To deposit data into the container, call this function to verify that the data is correctly deposited
     * @expect 1、The data is correctly stored and can be matched one by one
     * @auto   true
     */
    #[test]
    fn ut_slab_for_each() {
        let mut slab = Slab::<Foo>::new();
        let alloc = slab.handle();

        unsafe {
            // Find the first available `slot` and return its `addr` and `Ref`
            let (_, foo1) = alloc.allocate().unwrap();
            // Modify the current `id`
            foo1.id.store(1, SeqCst);

            // Find the second available `slot` and return its `addr` and `Ref`
            let (_, foo2) = alloc.allocate().unwrap();
            foo2.id.store(2, SeqCst);

            // Find the second available `slot` and return its `addr` and `Ref`
            let (_, foo3) = alloc.allocate().unwrap();
            foo3.id.store(3, SeqCst);
        }

        let mut temp = vec![3, 2, 1];
        slab.for_each(|value| {
            assert_eq!(temp.pop().unwrap(), value.id.load(SeqCst));
        });
    }

    /*
     * @title  Slab::get() ut test
     * @design The function has no invalid input, there is an exception branch, direct check function, return value
     *         1、No space has been allocated for the current address
     *         2、Space is allocated at the current address
     * @precon After calling Slab::new() to initialize the container, get its object
     * @brief  Describe test case execution
     *         1、Allocate container space and deposit data, get the data address, and see if the data can be fetched
     *         2、Create invalid data address to see if data can be obtained
     * @expect 1、Valid data addresses can correctly acquire data
     *         2、Invalid data address does not acquire data correctly
     * @auto   true
     */
    #[test]
    fn ut_slab_get() {
        let mut slab = Slab::<Foo>::new();

        unsafe {
            let (addr, _) = slab.allocate().unwrap();
            assert!(slab.get(addr).is_some());

            let un_addr = Address::from_usize(10000);
            assert!(slab.get(un_addr).is_none());
        }
    }

    /*
     * @title  Slab::compact() ut test
     * @design The function has no invalid input, there is an exception branch, direct check function, return value
     *         1、No space has been allocated for the current address
     *         2、Space is allocated at the current address
     * @precon After calling Slab::new() to initialize the container, get its object
     * @brief  Describe test case execution
     *         1、Pages with allocated space on the first page are not set to unallocated even if they are not used.
     *         2、Pages other than the first page, once assigned and unused, will be set to unassigned status
     * @expect Whether it is set to unassigned or not is related to the page assignment status and usage status
     * @auto   true
     */
    #[test]
    fn ut_slab_compact() {
        let mut slab = Slab::<Foo>::new();
        let mut address = Vec::new();

        unsafe {
            for data in 0..33 {
                let (addr, foo) = slab.allocate().unwrap();
                foo.id.store(data, SeqCst);
                address.push((addr, foo));
            }
            slab.compact();
            assert_eq!(slab.get(address[32].0).unwrap().id.load(SeqCst), 32);
        }

        let mut slab = Slab::<Foo>::new();
        let mut address = Vec::new();

        unsafe {
            assert!(!slab.pages[1].allocated.load(SeqCst));

            for _ in 0..33 {
                let (addr, foo) = slab.allocate().unwrap();
                address.push((addr, foo));
            }
            assert!(slab.pages[1].allocated.load(SeqCst));
            assert_eq!(slab.pages[1].used.load(SeqCst), 1);
            drop(address.pop().unwrap().1);
            assert!(slab.pages[1].allocated.load(SeqCst));
            assert_eq!(slab.pages[1].used.load(SeqCst), 0);
            slab.compact();
            assert!(!slab.pages[1].allocated.load(SeqCst));
        }
    }
}
