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

use super::pariter::Consumer;
use super::pariter::ParallelIterator;

use crate::error::ScheduleError;
use crate::executor::{global_default_async, AsyncHandle};
use crate::task::{JoinHandle, TaskBuilder};
use crate::{cfg_ffrt, cfg_not_ffrt};
cfg_not_ffrt! {
    use crate::executor::{async_pool::AsyncPoolSpawner};
}

cfg_ffrt! {
    use crate::executor::PlaceholderScheduler;
    use crate::ffrt::spawner::ffrt_submit;
    use crate::task::{Task, VirtualTableType};
    use crate::util::num_cpus::get_cpu_num;
    use std::sync::Weak;
    use ylong_ffrt::FfrtTaskAttr;
}

pub(crate) async fn core<P, C>(par_iter: P, consumer: C) -> Result<C::Output, ScheduleError>
where
    P: ParallelIterator + Send,
    C: Consumer<P> + Send + Sync,
{
    let task_builder = TaskBuilder::new();
    let runtime = global_default_async();

    match runtime.async_spawner {
        #[cfg(feature = "current_thread_runtime")]
        AsyncHandle::CurrentThread(_) => Ok(consumer.consume(par_iter)),
        #[cfg(not(feature = "ffrt"))]
        AsyncHandle::MultiThread(ref runtime) => {
            const MIN_SPLIT_LEN: usize = 1;
            let split_time = runtime.exe_mng_info.num_workers;
            recur(
                runtime,
                &task_builder,
                par_iter,
                &consumer,
                MIN_SPLIT_LEN,
                split_time,
            )
            .await
        }
        #[cfg(feature = "ffrt")]
        AsyncHandle::FfrtMultiThread => {
            const MIN_SPLIT_LEN: usize = 1;
            let split_time = get_cpu_num() as usize;
            recur_ffrt(
                &task_builder,
                par_iter,
                &consumer,
                MIN_SPLIT_LEN,
                split_time,
            )
            .await
        }
    }
}

#[cfg(not(feature = "ffrt"))]
async fn recur<P, C>(
    runtime: &AsyncPoolSpawner,
    task_builder: &TaskBuilder,
    par_iter: P,
    consumer: &C,
    min_split_len: usize,
    mut split_time: usize,
) -> Result<C::Output, ScheduleError>
where
    P: ParallelIterator + Send,
    C: Consumer<P> + Send + Sync,
{
    if (par_iter.len() >> 1) <= min_split_len || split_time == 0 {
        return Ok(consumer.consume(par_iter));
    }
    let (left, right) = par_iter.split();
    let right = match right {
        Some(a) => a,
        None => {
            return Ok(consumer.consume(left));
        }
    };
    split_time >>= 1;
    unsafe {
        let left = spawn_task(
            runtime,
            task_builder,
            left,
            consumer,
            min_split_len,
            split_time,
        );
        let right = spawn_task(
            runtime,
            task_builder,
            right,
            consumer,
            min_split_len,
            split_time,
        );
        let left = left.await??;
        let right = right.await??;
        Ok(C::combine(left, right))
    }
}

// Safety
// No restriction on lifetime to static, so it must be ensured that the data pointed to is always valid until the execution is completed,
// in other word .await the join handle after it is created.
#[cfg(not(feature = "ffrt"))]
#[inline]
unsafe fn spawn_task<P, C>(
    runtime: &AsyncPoolSpawner,
    task_builder: &TaskBuilder,
    par_iter: P,
    consumer: &C,
    min_split_len: usize,
    split_time: usize,
) -> JoinHandle<Result<C::Output, ScheduleError>>
where
    P: ParallelIterator + Send,
    C: Consumer<P> + Send + Sync,
{
    runtime.spawn_with_ref(
        task_builder,
        recur(
            runtime,
            task_builder,
            par_iter,
            consumer,
            min_split_len,
            split_time,
        ),
    )
}

#[cfg(feature = "ffrt")]
async fn recur_ffrt<P, C>(
    task_builder: &TaskBuilder,
    par_iter: P,
    consumer: &C,
    min_split_len: usize,
    mut split_time: usize,
) -> Result<C::Output, ScheduleError>
where
    P: ParallelIterator + Send,
    C: Consumer<P> + Send + Sync,
{
    if (par_iter.len() >> 1) <= min_split_len || split_time <= 0 {
        return Ok(consumer.consume(par_iter));
    }
    let (left, right) = par_iter.split();
    let right = match right {
        Some(a) => a,
        None => {
            return Ok(consumer.consume(left));
        }
    };
    split_time = split_time >> 1;
    unsafe {
        let left = spawn_task_ffrt(task_builder, left, consumer, min_split_len, split_time);
        let right = spawn_task_ffrt(task_builder, right, consumer, min_split_len, split_time);
        let left = left.await??;
        let right = right.await??;
        Ok(C::combine(left, right))
    }
}

// Safety
// No restriction on lifetime to static, so it must be ensured that the data pointed to is always valid until the execution is completed,
// in other word .await the join handle after it is created.
#[cfg(feature = "ffrt")]
#[inline]
unsafe fn spawn_task_ffrt<P, C>(
    task_builder: &TaskBuilder,
    par_iter: P,
    consumer: &C,
    min_split_len: usize,
    split_time: usize,
) -> JoinHandle<Result<C::Output, ScheduleError>>
where
    P: ParallelIterator + Send,
    C: Consumer<P> + Send + Sync,
{
    let scheduler: Weak<PlaceholderScheduler> = Weak::new();
    let raw_task = Task::create_raw_task(
        task_builder,
        scheduler,
        recur_ffrt(task_builder, par_iter, consumer, min_split_len, split_time),
        VirtualTableType::Ffrt,
    );
    let mut join_handle = JoinHandle::new(raw_task);
    let task = Task(raw_task);
    let attr = FfrtTaskAttr::new();
    let handle = ffrt_submit(task, &attr);
    join_handle.set_task_handle(handle);
    join_handle
}
