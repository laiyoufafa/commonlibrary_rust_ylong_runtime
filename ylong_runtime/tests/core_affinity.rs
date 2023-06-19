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

#![cfg(target_os = "linux")]
use std::thread;
use ylong_runtime::util::core_affinity::linux::{get_current_affinity, set_current_affinity};
use ylong_runtime::util::num_cpus::get_cpu_num;

#[test]
fn sdv_affinity_set() {
    let cpus = get_current_affinity();
    assert!(!cpus.is_empty());

    let ret = set_current_affinity(0).is_ok();
    assert!(ret);

    let cpus = get_current_affinity();
    assert_eq!(cpus.len(), 1);
}

#[test]
fn sdv_affinity_core() {
    let cpus = get_cpu_num() as usize;
    let handle = thread::spawn(move || {
        set_current_affinity(0 % cpus).unwrap();
        let core_ids = get_current_affinity();
        for coreid in core_ids.iter() {
            assert_eq!(*coreid, 0_usize);
        }
    });
    let _ = handle.join();
    thread::spawn(move || {
        set_current_affinity(1 % cpus).unwrap();
        let core_ids = get_current_affinity();
        for coreid in core_ids.iter() {
            assert_eq!(*coreid, 1_usize);
        }
    });
}
