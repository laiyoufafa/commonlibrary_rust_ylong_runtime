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

use super::pariter::ParallelIterator;

/// Parallel iterator implementation for array
pub mod array;
/// Parallel iterator implementation for std collections
pub mod collections;

/// Parallel iterator implementation for slice
pub mod slice;

/// Parallel iterator implementation for vec
pub mod vec;

/// Splits data into two parts of the same type
pub trait ParSplit: Sized + IntoIterator {
    /// Returns the number of the elements in the data
    fn len(&self) -> usize;

    /// Reduces the number of elements in the data
    fn reduce(self, len: usize) -> Self;

    /// Splits data into two parts, if it can no longer be divided, returns None.
    fn split(self) -> (Self, Option<Self>);

    /// Returns true if the parsplit has a length of 0
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Trait that defines interfaces for a struct to convert itself into `ParIter`.
pub trait IntoParIter {
    /// Type that can be splitted
    type Data: ParSplit;

    /// Crates a parallel iterator from a value.
    fn into_par_iter(self) -> ParIter<Self::Data>;
}

/// Parallel iterator
pub struct ParIter<T: ParSplit> {
    data: T,
}

impl<T: ParSplit> IntoParIter for T {
    type Data = T;
    fn into_par_iter(self) -> ParIter<Self::Data> {
        ParIter { data: self }
    }
}

/// Trait that defines interfaces for a struct to convert itself into `ParIter`.
pub trait AsParIter<'a> {
    /// Type of data that can be splitted
    type Data: ParSplit + 'a;

    /// Crates a immutable parallel iterator for a immutable reference
    fn par_iter(&'a self) -> ParIter<Self::Data>;
}

impl<'a, T: 'a> AsParIter<'a> for T
where
    &'a T: IntoParIter,
{
    type Data = <&'a T as IntoParIter>::Data;
    fn par_iter(&'a self) -> ParIter<Self::Data> {
        self.into_par_iter()
    }
}

/// Trait that defines interfaces for a struct to convert itself into `ParIter`.
pub trait AsParIterMut<'a> {
    /// Type of data that can be splitted.
    type Data: ParSplit + 'a;

    /// Crates a mutable parallel iterator for a mutable reference
    fn par_iter_mut(&'a mut self) -> ParIter<Self::Data>;
}

impl<'a, T: 'a> AsParIterMut<'a> for T
where
    &'a mut T: IntoParIter,
{
    type Data = <&'a mut T as IntoParIter>::Data;

    fn par_iter_mut(&'a mut self) -> ParIter<Self::Data> {
        self.into_par_iter()
    }
}

impl<T: ParSplit + Send> ParallelIterator for ParIter<T> {
    type Item = T::Item;
    type Iter = T::IntoIter;

    fn len(&self) -> usize {
        self.data.len()
    }

    fn reduce(self, len: usize) -> Self {
        Self {
            data: self.data.reduce(len),
        }
    }

    fn iter(self) -> Self::Iter {
        self.data.into_iter()
    }

    fn split(self) -> (Self, Option<Self>) {
        let (left, right) = self.data.split();
        let left = ParIter { data: left };
        let right = right.map(|data| ParIter { data });
        (left, right)
    }
}
