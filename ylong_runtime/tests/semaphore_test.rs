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

#![cfg(feature = "sync")]
use std::sync::Arc;
use ylong_runtime::sync::{AutoRelSemaphore, Semaphore};
use ylong_runtime::task::JoinHandle;

/// SDV test for `AutoRelSemaphore::acquire()`.
///
/// # Title
/// auto_release_sem_acquire_test
///
/// # Brief
/// 1.Create a counting auto-release-semaphore with an initial capacity.
/// 2.Acquire an auto-release-permit.
/// 3.Asynchronously acquires a permit.
/// 4.Check the number of permits in every stage.
#[test]
fn auto_release_sem_acquire_test() {
    let sem = Arc::new(AutoRelSemaphore::new(1).unwrap());
    let sem2 = sem.clone();
    let handle = ylong_runtime::spawn(async move {
        let _permit2 = sem2.acquire().await.unwrap();
        assert_eq!(sem2.current_permits(), 0);
    });
    ylong_runtime::block_on(handle).expect("block_on failed");
    assert_eq!(sem.current_permits(), 1);
}

/// SDV test for `AutoRelSemaphore::try_acquire()`.
///
/// # Title
/// auto_release_sem_try_acquire_test
///
/// # Brief
/// 1.Create a counting auto-release-semaphore with an initial capacity.
/// 2.Acquire an auto-release-permit.
/// 3.Fail to acquire an auto-release-permit.
/// 4.Acquire an auto-release-permit successfully after the last one is recycled.
#[test]
fn auto_release_sem_try_acquire_test() {
    let sem = AutoRelSemaphore::new(1).unwrap();
    let permit = sem.try_acquire();
    assert!(permit.is_ok());
    let permit2 = sem.try_acquire();
    assert!(permit2.is_err());
    drop(permit);
    let permit3 = sem.try_acquire();
    assert!(permit3.is_ok());
}

/// SDV test for `Semaphore::release()`.
///
/// # Title
/// release_test
///
/// # Brief
/// 1.Create a counting semaphore with an initial capacity.
/// 2.Call `Semaphore::release()` to add a permit to the semaphore.
/// 3.Check the number of permits before and after releasing.
#[test]
fn release_test() {
    let sem = Semaphore::new(2).unwrap();
    assert_eq!(sem.current_permits(), 2);
    sem.release();
    assert_eq!(sem.current_permits(), 3);
}

/// SDV test for `AutoRelSemaphore::close()`.
///
/// # Title
/// auto_release_sem_close_test
///
/// # Brief
/// 1.Create a counting auto-release-semaphore with an initial capacity.
/// 2.Close the semaphore.
/// 3.Fail to acquire an auto-release-permit.
#[test]
fn auto_release_sem_close_test() {
    let sem = Arc::new(AutoRelSemaphore::new(2).unwrap());
    let sem2 = sem.clone();
    assert!(!sem.is_closed());
    sem.close();
    assert!(sem.is_closed());
    let permit = sem.try_acquire();
    assert!(permit.is_err());
    let handle = ylong_runtime::spawn(async move {
        let permit2 = sem2.acquire().await;
        assert!(permit2.is_err());
    });
    ylong_runtime::block_on(handle).expect("block_on failed");
}

/// Stress test for `AutoRelSemaphore::acquire()`.
///
/// # Title
/// auto_release_sem_stress_test
///
/// # Brief
/// 1.Create a counting auto-release-semaphore with an initial capacity.
/// 2.Repeating acquiring an auto-release-permit for a huge number of times.
/// 3.Check the correctness of function of semaphore.
#[test]
fn auto_release_sem_stress_test() {
    let sem = Arc::new(AutoRelSemaphore::new(5).unwrap());
    let mut tasks: Vec<JoinHandle<()>> = Vec::new();

    for _ in 0..1000 {
        let sem2 = sem.clone();
        tasks.push(ylong_runtime::spawn(async move {
            let _permit = sem2.acquire().await;
        }));
    }
    for t in tasks {
        let _ = ylong_runtime::block_on(t);
    }
    let permit1 = sem.try_acquire();
    assert!(permit1.is_ok());
    let permit2 = sem.try_acquire();
    assert!(permit2.is_ok());
    let permit3 = sem.try_acquire();
    assert!(permit3.is_ok());
    let permit4 = sem.try_acquire();
    assert!(permit4.is_ok());
    let permit5 = sem.try_acquire();
    assert!(permit5.is_ok());
    assert!(sem.try_acquire().is_err());
}

/// Stress test for `AutoRelSemaphore::acquire()` and `AutoRelSemaphore::drop()`.
///
/// # Title
/// async_stress_test
///
/// # Brief
/// 1.Create a counting auto-release-semaphore with an initial capacity.
/// 2.Repeating acquiring a pair of auto-release-permit for a huge number of times.
/// 3.Check the correctness of the future of `Permit`.
#[test]
fn async_stress_test() {
    let mut tasks: Vec<JoinHandle<()>> = Vec::new();

    for _ in 0..50000 {
        let sem = Arc::new(AutoRelSemaphore::new(1).unwrap());
        let sem2 = sem.clone();
        tasks.push(ylong_runtime::spawn(async move {
            let _permit = sem.acquire().await;
        }));
        tasks.push(ylong_runtime::spawn(async move {
            let _permit = sem2.acquire().await;
        }));
    }
    for t in tasks {
        let _ = ylong_runtime::block_on(t);
    }
}

/// SDV test for `Semaphore::try_acquire()`.
///
/// # Title
/// try_acquire_test
///
/// # Brief
/// 1.Create a counting semaphore with an initial capacity.
/// 2.Acquire permits successfully.
/// 3.Fail to acquire a permit when all permits are consumed.
#[test]
fn try_acquire_test() {
    let sem = Semaphore::new(2).unwrap();
    let permit = sem.try_acquire();
    assert!(permit.is_ok());
    drop(permit);
    assert_eq!(sem.current_permits(), 1);
    let permit2 = sem.try_acquire();
    assert!(permit2.is_ok());
    drop(permit2);
    assert_eq!(sem.current_permits(), 0);
    let permit3 = sem.try_acquire();
    assert!(permit3.is_err());
}

/// SDV test for `Semaphore::acquire()`.
///
/// # Title
/// acquire_test
///
/// # Brief
/// 1.Create a counting semaphore with an initial capacity.
/// 2.Acquire a permit.
/// 3.Asynchronously acquires a permit.
/// 4.Check the number of permits in every stage.
#[test]
fn acquire_test() {
    let sem = Arc::new(Semaphore::new(0).unwrap());
    let sem2 = sem.clone();
    let handle = ylong_runtime::spawn(async move {
        let _permit2 = sem2.acquire().await.unwrap();
        assert_eq!(sem2.current_permits(), 0);
    });
    sem.release();
    ylong_runtime::block_on(handle).expect("block_on failed");
    assert_eq!(sem.current_permits(), 0);
}
