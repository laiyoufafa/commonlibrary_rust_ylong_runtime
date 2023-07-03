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

use std::sync::Mutex;

pub(super) struct Sleepers {
    workers: Mutex<Vec<usize>>,
    num_workers: usize,
}

impl Sleepers {
    pub(super) fn new(num_workers: usize) -> Sleepers {
        Sleepers {
            workers: Mutex::new(Vec::with_capacity(num_workers)),
            num_workers,
        }
    }

    #[inline]
    pub(super) fn is_parked(&self, worker_id: usize) -> bool {
        let workers = self.workers.lock().unwrap();
        workers.contains(&worker_id)
    }

    pub(super) fn pop_specific(&self, worker_id: usize) -> bool {
        let mut workers = self.workers.lock().unwrap();

        for idx in 0..workers.len() {
            if workers[idx] == worker_id {
                workers.swap_remove(idx);
                return true;
            }
        }
        false
    }

    pub(super) fn pop(&self) -> Option<usize> {
        let mut workers = self.workers.lock().unwrap();
        workers.pop()
    }

    pub(super) fn push(&self, worker_id: usize) -> bool {
        let mut workers = self.workers.lock().unwrap();
        workers.push(worker_id);
        let len = workers.len();
        drop(workers);
        len == self.num_workers
    }
}
