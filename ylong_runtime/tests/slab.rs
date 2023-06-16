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

use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::{Arc, Mutex};
use std::thread;
use ylong_runtime::util::slab::{Entry, Slab};

struct TestEntry {
    cnt: AtomicUsize,
    id: AtomicUsize,
}

impl Default for TestEntry {
    fn default() -> TestEntry {
        TestEntry {
            cnt: AtomicUsize::new(0),
            id: AtomicUsize::new(0),
        }
    }
}

impl Entry for TestEntry {
    fn reset(&self) {
        self.cnt.fetch_add(1, SeqCst);
    }
}

/*
 * @title  Slab SDV test
 * @design The entire Slab as a container, the use of scenarios are mainly add, delete, change and check
 * @precon After calling Slab::new() to initialize the container, get its object
 * @brief  Describe test case execution
 *         1、Inserting large amounts of data into a container
 *         2、Make changes to these inserted data
 *         3、Modified data by address verification
 *         4、Multiplexing mechanism after calibration is released
 * @expect Data validation successful
 * @auto   true
// */
#[test]
fn sdv_slab_insert_move() {
    let mut slab = Slab::<TestEntry>::new();
    let alloc = slab.handle();

    unsafe {
        // Find the first available `slot` and return its `addr` and `Ref`
        let (addr1, test_entry1) = alloc.allocate().unwrap();
        // Modify the current `id`
        slab.get(addr1).unwrap().id.store(1, SeqCst);
        // The `reset` function has not been called yet, so `cnt` remains unchanged
        assert_eq!(0, slab.get(addr1).unwrap().cnt.load(SeqCst));

        // Find the second available `slot` and return its `addr` and `Ref`
        let (addr2, test_entry2) = alloc.allocate().unwrap();
        slab.get(addr2).unwrap().id.store(2, SeqCst);
        assert_eq!(0, slab.get(addr2).unwrap().cnt.load(SeqCst));

        // This verifies that the function of finding data based on `addr` is working
        assert_eq!(1, slab.get(addr1).unwrap().id.load(SeqCst));
        assert_eq!(2, slab.get(addr2).unwrap().id.load(SeqCst));

        // Active destruct, the `slot` will be reused
        drop(test_entry1);

        assert_eq!(1, slab.get(addr1).unwrap().id.load(SeqCst));

        // Allocate again, but then the allocated `slot` should use the previously destructured `slot`
        let (addr3, test_entry3) = alloc.allocate().unwrap();
        // Comparison, equal is successful
        assert_eq!(addr3, addr1);
        assert_eq!(1, slab.get(addr3).unwrap().cnt.load(SeqCst));
        slab.get(addr3).unwrap().id.store(3, SeqCst);
        assert_eq!(3, slab.get(addr3).unwrap().id.load(SeqCst));

        drop(test_entry2);
        drop(test_entry3);

        // Cleaned regularly, but the first `page` is never cleaned
        slab.compact();
        assert!(slab.get(addr1).is_some());
        assert!(slab.get(addr2).is_some());
        assert!(slab.get(addr3).is_some());
    }
}

/*
 * @title  Slab SDV test
 * @design The entire Slab as a container, the use of scenarios are mainly add, delete, change and check
 * @precon After calling Slab::new() to initialize the container, get its object
 * @brief  Describe test case execution
 *         1、Inserting large amounts of data into a container
 *         2、Verify by address that the data is in the correct location
 * @expect Data validation successful
 * @auto   true
 */
#[test]
fn sdv_slab_insert_many() {
    unsafe {
        // Verify that `page` is being allocated properly in the case of a large number of inserts.
        let mut slab = Slab::<TestEntry>::new();
        let alloc = slab.handle();
        let mut entries = vec![];

        for i in 0..10_000 {
            let (addr, val) = alloc.allocate().unwrap();
            val.id.store(i, SeqCst);
            entries.push((addr, val));
        }

        for (i, (addr, v)) in entries.iter().enumerate() {
            assert_eq!(i, v.id.load(SeqCst));
            assert_eq!(i, slab.get(*addr).unwrap().id.load(SeqCst));
        }

        entries.clear();

        for i in 0..10_000 {
            let (addr, val) = alloc.allocate().unwrap();
            val.id.store(10_000 - i, SeqCst);
            entries.push((addr, val));
        }

        for (i, (addr, v)) in entries.iter().enumerate() {
            assert_eq!(10_000 - i, v.id.load(SeqCst));
            assert_eq!(10_000 - i, slab.get(*addr).unwrap().id.load(SeqCst));
        }
    }
}

/*
 * @title  Slab SDV test
 * @design The entire Slab as a container, the use of scenarios are mainly add, delete, change and check
 * @precon After calling Slab::new() to initialize the container, get its object
 * @brief  Describe test case execution
 *         1、Inserting large amounts of data into a container
 *         2、Verify by address that the data is in the correct location
 * @expect Data validation successful
 * @auto   true
 */
#[test]
fn sdv_slab_insert_drop_reverse() {
    unsafe {
        let mut slab = Slab::<TestEntry>::new();
        let alloc = slab.handle();
        let mut entries = vec![];

        for i in 0..10_000 {
            let (addr, val) = alloc.allocate().unwrap();
            val.id.store(i, SeqCst);
            entries.push((addr, val));
        }

        for _ in 0..10 {
            for _ in 0..1_000 {
                entries.pop();
            }

            for (i, (addr, v)) in entries.iter().enumerate() {
                assert_eq!(i, v.id.load(SeqCst));
                assert_eq!(i, slab.get(*addr).unwrap().id.load(SeqCst));
            }
        }
    }
}

/*
 * @title  Slab SDV test
 * @design The entire Slab as a container, the use of scenarios are mainly add, delete, change and check
 * @precon After calling Slab::new() to initialize the container, get its object
 * @brief  Describe test case execution
 *         1、Multi-threaded allocation of container space, inserting data into it, and verifying that the function is correct
 * @expect Data validation successful
 * @auto   true
 */
#[test]
fn sdv_slab_multi_allocate() {
    // Multi-threaded either allocating space, or modifying values.
    // finally comparing the values given by the acquired address to be equal.
    let mut slab = Slab::<TestEntry>::new();
    let thread_one_alloc = slab.handle();
    let thread_two_alloc = slab.handle();
    let thread_three_alloc = slab.handle();

    let capacity = 3001;
    let free_queue = Arc::new(Mutex::new(Vec::with_capacity(capacity)));
    let free_queue_2 = free_queue.clone();
    let free_queue_3 = free_queue.clone();
    let free_queue_4 = free_queue.clone();

    unsafe {
        let thread_one = thread::spawn(move || {
            for i in 0..10_00 {
                let (addr, test_entry) = thread_one_alloc.allocate().unwrap();
                test_entry.id.store(i, SeqCst);
                free_queue.lock().unwrap().push((addr, test_entry, i));
            }
        });

        let thread_two = thread::spawn(move || {
            for i in 10_00..20_00 {
                let (addr, test_entry) = thread_two_alloc.allocate().unwrap();
                test_entry.id.store(i, SeqCst);
                free_queue_2.lock().unwrap().push((addr, test_entry, i));
            }
        });

        let thread_three = thread::spawn(move || {
            for i in 20_00..30_00 {
                let (addr, test_entry) = thread_three_alloc.allocate().unwrap();
                test_entry.id.store(i, SeqCst);
                free_queue_3.lock().unwrap().push((addr, test_entry, i));
            }
        });

        thread_one
            .join()
            .expect("Couldn't join on the associated thread");
        thread_two
            .join()
            .expect("Couldn't join on the associated thread");
        thread_three
            .join()
            .expect("Couldn't join on the associated thread");
    }

    for _ in 0..30_00 {
        let temp = free_queue_4.clone().lock().unwrap().pop().unwrap();
        assert_eq!(slab.get(temp.0).unwrap().id.load(SeqCst), temp.2);
    }
}

/*
 * @title  Slab SDV test
 * @design The entire Slab as a container, the use of scenarios are mainly add, delete, change and check
 * @precon After calling Slab::new() to initialize the container, get its object
 * @brief  Describe test case execution
 *         1、Multi-threaded allocation of container space, inserting data into it, and verifying that the function is correct
 *         2、Free up some of the data space and check if the data is reused in the multi-threaded case
 * @expect Data validation successful
 * @auto   true
 */
#[test]
fn sdv_slab_multi_allocate_drop() {
    // allocate space and free the used `slot` in the multi-threaded case.
    // retaining the address of the freed `slot` and allocating it again.
    // the address after reallocation is the same as the address of the previously freed `slot`.
    let slab = Slab::<TestEntry>::new();
    let thread_one_alloc = slab.handle();
    let thread_two_alloc = slab.handle();

    let capacity = 2001;
    let free_queue_one = Arc::new(Mutex::new(Vec::with_capacity(capacity)));
    let free_queue_one_2 = free_queue_one.clone();

    let free_queue_two = Arc::new(Mutex::new(Vec::with_capacity(capacity)));
    let free_queue_two_2 = free_queue_two.clone();

    unsafe {
        let thread_one = thread::spawn(move || {
            for i in 0..1000 {
                let (addr, test_entry) = thread_one_alloc.allocate().unwrap();
                test_entry.id.store(i, SeqCst);
                drop(test_entry);

                free_queue_one.lock().unwrap().push(addr);
            }
        });

        thread_one
            .join()
            .expect("Couldn't join on the associated thread");

        let thread_two = thread::spawn(move || {
            thread::park();
            for i in 0..1000 {
                let (addr, test_entry) = thread_two_alloc.allocate().unwrap();
                test_entry.id.store(i, SeqCst);

                free_queue_two.lock().unwrap().push(addr);
            }
        });

        thread_two.thread().unpark();
        thread_two
            .join()
            .expect("Couldn't join on the associated thread");

        for _ in 0..1000 {
            assert_eq!(
                free_queue_one_2.clone().lock().unwrap().pop().unwrap(),
                free_queue_two_2.lock().unwrap().pop().unwrap()
            );
        }
    }
}
