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

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use ylong_runtime::time::sleep;

type AppId = usize;

struct Manager {
    map: HashMap<AppId, Arc<Worker>>,
}

impl Manager {
    fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }
}

struct Task {}

impl Task {
    async fn download(self) {
        const TOTAL_SIZE: usize = 100 * 1024;
        const RECV_SIZE: usize = 1024;

        let mut left = TOTAL_SIZE;
        loop {
            let recv = RECV_SIZE;
            left -= recv;
            if left == 0 {
                break;
            }
            sleep(Duration::from_millis(50)).await;
        }
    }
}

struct Worker {}

impl Worker {
    fn new() -> Self {
        Self {}
    }

    async fn execute(&self, task: Task) {
        task.download().await;
    }
}

async fn simulate() {
    const APPS_NUM: usize = 10;
    const TASKS_NUM: usize = 5;

    let mut manager = Manager::new();

    let mut handles = Vec::new();

    for i in 0..APPS_NUM {
        manager
            .map
            .entry(i)
            .or_insert_with(|| Arc::new(Worker::new()));
        let worker = manager.map.get(&i).unwrap();

        for _ in 0..TASKS_NUM {
            let task = Task {};
            let worker = worker.clone();
            handles.push(ylong_runtime::spawn(async move {
                worker.execute(task).await;
            }));
        }
    }

    for handle in handles {
        let _ = handle.await;
    }
}

/// SDV test for multi time create.
///
/// # Title
/// test_multi_timer
///
/// # Brief
/// 1.Creates multi threads and multi timers.
/// 2.Checks if the test results are correct.
#[test]
fn test_multi_timer() {
    ylong_runtime::block_on(simulate());
}
