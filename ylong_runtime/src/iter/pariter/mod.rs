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

use std::ops::Add;
use std::{iter::Sum, pin::Pin};

use super::core::core;

mod map;
use map::Map;

mod filter;
use filter::Filter;

mod zip;
use zip::Zip;

mod for_each;

mod sum;
/// A parallel version of the standard iterator trait.
pub trait ParallelIterator: Sized + Send {
    /// The type of the elements the parallel iterator has.
    type Item;

    /// The type of the std iterator that the parallel iterator generate.
    type Iter: Iterator<Item = Self::Item>;

    /// Splits one parallel iterator into two parts,
    fn split(self) -> (Self, Option<Self>);

    /// Returns the number of elements in this parallel iterator.
    fn len(&self) -> usize;

    /// Removes redundant elements, used in zip function, so that the two parts has the same length.
    fn reduce(self, len: usize) -> Self;

    /// Returns the std iterator.
    fn iter(self) -> Self::Iter;

    /// Returns true if the parallel iterator data has a length of 0
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// cache a map_op in the parallel iterator
    fn map<F, B>(self, map_op: F) -> Map<Self, F>
    where
        F: Fn(Self::Item) -> B,
    {
        map::map(self, map_op)
    }

    /// Parallel version of the std iterator filter
    fn filter<F>(self, predicate: F) -> Filter<Self, F>
    where
        F: Fn(&Self::Item) -> bool,
    {
        filter::filter(self, predicate)
    }

    /// Parallel version of the std iterator zip
    fn zip<B>(self, another: B) -> Zip<Self, B>
    where
        B: ParallelIterator,
    {
        zip::zip(self, another)
    }

    /// Execute the OP in parallel on each element produced by the parallel iterator.
    fn for_each<'a, F>(
        self,
        f: F,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), ScheduleError>> + Send + 'a>>
    where
        F: Fn(Self::Item) + Send + Copy + Sync + 'a,
        Self: 'a,
    {
        let fut = async move { for_each::for_each(self, f).await };
        Box::pin(fut)
    }

    /// Parallel version of the std iterator sum.
    fn sum<'a>(
        self,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<Self::Item, ScheduleError>> + Send + 'a>>
    where
        Self: 'a,
        Self::Item: Add<Output = Self::Item> + Sum + Send,
    {
        let fut = async move { sum::sum(self).await };
        Box::pin(fut)
    }

    /// Consumes the parallel iterator.
    fn drive<'a, C>(
        self,
        consumer: C,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<C::Output, ScheduleError>> + Send + 'a>>
    where
        Self: 'a,
        C: Consumer<Self> + Send + Sync + 'a,
    {
        let fut = async move { core(self, consumer).await };
        Box::pin(fut)
    }
}

/// Consumer that comsume a parallel iterator and returns the result.
pub trait Consumer<P: ParallelIterator> {
    /// Type that the consumer return
    type Output: Send;

    /// Consume a parallel iterator
    fn consume(&self, par_iter: P) -> Self::Output;

    /// Returns the result obtained by merging the two split executions
    fn combine(a: Self::Output, b: Self::Output) -> Self::Output;
}
