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

use super::ParallelIterator;
use std::iter;

pub fn zip<A, B>(mut a: A, mut b: B) -> Zip<A, B>
where
    A: ParallelIterator,
    B: ParallelIterator,
{
    let len_a = a.len();
    let len_b = b.len();
    if len_a > len_b {
        a = a.reduce(len_b);
    } else {
        b = b.reduce(len_a);
    }
    Zip { a, b }
}

pub struct Zip<A, B> {
    a: A,
    b: B,
}

impl<A, B> ParallelIterator for Zip<A, B>
where
    A: ParallelIterator,
    B: ParallelIterator,
{
    type Item = (A::Item, B::Item);
    type Iter = iter::Zip<A::Iter, B::Iter>;

    fn len(&self) -> usize {
        self.a.len()
    }

    fn reduce(self, len: usize) -> Self {
        Self {
            a: self.a.reduce(len),
            b: self.b.reduce(len),
        }
    }

    fn split(self) -> (Self, Option<Self>) {
        let (left_a, right_a) = self.a.split();
        let (left_b, right_b) = self.b.split();
        let left = Zip {
            a: left_a,
            b: left_b,
        };
        let right = match (right_a, right_b) {
            (Some(a), Some(b)) => Some(Zip { a, b }),
            _ => None,
        };
        (left, right)
    }

    fn iter(self) -> Self::Iter {
        self.a.iter().zip(self.b.iter())
    }
}
