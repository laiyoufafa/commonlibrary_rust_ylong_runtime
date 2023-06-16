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

//! Examples for task scheduling
use std::hint::black_box;
use ylong_runtime::task::yield_now;

fn recur_fibbo(a: u64) -> u64 {
    if a == 0 || a == 1 {
        return a;
    }
    recur_fibbo(a - 1) + recur_fibbo(a - 2)
}

fn fibbo() -> u64 {
    let mut ret = 0;
    for j in 1..15 {
        ret += black_box(recur_fibbo(j as u64));
    }
    black_box(ret)
}

fn ylong_schedule_task(num: usize) {
    let mut handlers = Vec::with_capacity(num);
    for _ in 0..num {
        handlers.push(ylong_runtime::spawn(async move {
            yield_now().await;
        }));
    }

    for handler in handlers {
        ylong_runtime::block_on(handler).unwrap();
    }
}

fn tokio_schedule_task(num: usize) {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let mut handlers = Vec::with_capacity(num);
    for _ in 0..num {
        handlers.push(runtime.spawn(async move {
            yield_now().await;
        }));
    }

    for handler in handlers {
        runtime.block_on(handler).unwrap();
    }
}

fn ylong_schedule_fibbo_task(num: usize) {
    let mut handlers = Vec::with_capacity(num);
    for _ in 0..num {
        handlers.push(ylong_runtime::spawn(async move {
            fibbo();
            yield_now().await;
        }));
    }

    for handler in handlers {
        ylong_runtime::block_on(handler).unwrap();
    }
}

fn tokio_schedule_fibbo_task(num: usize) {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let mut handlers = Vec::with_capacity(num);
    for _ in 0..num {
        handlers.push(runtime.spawn(async move {
            fibbo();
            yield_now().await;
        }));
    }

    for handler in handlers {
        runtime.block_on(handler).unwrap();
    }
}

fn main() {
    ylong_schedule_task(10000);
    ylong_schedule_fibbo_task(10000);
    tokio_schedule_task(10000);
    tokio_schedule_fibbo_task(10000);
}
