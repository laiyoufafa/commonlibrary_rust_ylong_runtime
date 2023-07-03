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

use crate::error::ScheduleError;
use core::ops::Add;
use std::iter::Sum;

use super::{Consumer, ParallelIterator};

pub async fn sum<P>(par_iter: P) -> Result<P::Item, ScheduleError>
where
    P: ParallelIterator + Send,
    P::Item: Add<Output = P::Item> + Sum + Send,
{
    let consumer = SumConsumer;
    par_iter.drive(consumer).await
}

struct SumConsumer;

impl<P> Consumer<P> for SumConsumer
where
    P: ParallelIterator,
    P::Item: Add<Output = P::Item> + Sum + Send,
{
    type Output = P::Item;
    fn consume(&self, par_iter: P) -> Self::Output {
        par_iter.iter().sum()
    }
    fn combine(a: Self::Output, b: Self::Output) -> Self::Output {
        a.add(b)
    }
}
