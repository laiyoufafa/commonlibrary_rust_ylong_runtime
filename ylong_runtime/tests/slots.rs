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

use ylong_runtime::util::slots::Slots;

/// SDV test for Slots
/// # Title
/// sdv_huge_data_push_back
/// # Brief
/// 1.Push a large amount of data into the initialized container.
/// 2.Check the correctness of inserted data iteratively.
#[test]
fn sdv_slots_huge_data_push_back() {
    let mut slots = Slots::new();
    let mut keys = Vec::new();

    for data in 0..10000 {
        let key = slots.push_back(data);
        keys.push(key);
    }

    for (index, key) in keys.iter().enumerate() {
        assert_eq!(slots[*key], index);
    }
}

/// SDV test for Slots
/// # Title
/// sdv_huge_data_remove
/// # Brief
/// 1.Push a large amount of data into the initialized container.
/// 2.Remove the first half of the container.
/// 3.Push new data into the container again.
/// 4.Check the correctness of data sequence and values.
#[test]
fn sdv_slots_huge_data_remove() {
    let mut slots = Slots::new();
    let mut keys = Vec::new();

    for data in 0..10000 {
        let key = slots.push_back(data);
        keys.push(key);
    }

    for remove_index in 0..5000 {
        let res = slots.remove(remove_index);
        assert!(res.is_ok());
    }

    for data in 10000..15000 {
        slots.push_back(data);
    }

    let mut cnt = 14999;
    for key in 0..5000 {
        assert_eq!(slots[key], cnt);
        cnt -= 1;
    }
}

/// SDV test for Slots
/// # Title
/// sdv_remove_and_pop
/// # Brief
/// 1.Push data into the initialized container.
/// 2.Remove slots that have been popped.
/// 3.Remove slots at wrong index.
/// 4.Pop the remaining data.
#[test]
fn sdv_slots_remove_and_pop() {
    let mut slots = Slots::new();

    for data in 0..100 {
        slots.push_back(data);
    }

    for index in 0..10 {
        slots.pop_front();
        let res = slots.remove(index);
        assert!(res.is_err());
    }

    for remove_index in 100..150 {
        let res = slots.remove(remove_index);
        assert!(res.is_err());
    }

    for remove_index in 10..20 {
        let res = slots.remove(remove_index);
        assert!(res.is_ok());
    }

    for index in 20..100 {
        assert_eq!(slots.pop_front(), Some(index));
    }
    assert!(slots.pop_front().is_none());
}

/// SDV test for Slots
/// # Title
/// sdv_huge_data_find
/// # Brief
/// 1.Push a large amount of data into the initialized container.
/// 2.Find data through key-value pairs.
#[test]
fn sdv_slots_huge_data_find() {
    let mut slots = Slots::new();
    let mut keys = Vec::new();

    for data in 0..10000 {
        let key = slots.push_back(data);
        keys.push(key);
    }

    for key in keys {
        assert_eq!(slots[key], key);
    }
}

/// SDV test for Slots
/// # Title
/// sdv_huge_data_pop_front
/// # Brief
/// 1.Push a large amount of data into the initialized container.
/// 2.Pop the first half of the container.
/// 3.Push new data into the container again.
/// 4.Pop all of the data and check correctness of data sequence and values.
#[test]
fn sdv_slots_huge_data_pop_front() {
    let mut slots = Slots::new();

    for data in 0..10000 {
        slots.push_back(data);
    }

    for _ in 0..5000 {
        slots.pop_front();
    }

    for data in 10000..15000 {
        slots.push_back(data);
    }

    let mut cnt = 14999;
    for key in 0..5000 {
        assert_eq!(slots[key], cnt);
        cnt -= 1;
    }
}
