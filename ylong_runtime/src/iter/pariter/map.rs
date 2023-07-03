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

use std::iter;

use super::ParallelIterator;

pub fn map<P, F>(par_iter: P, map_op: F) -> Map<P, F> {
    Map {
        base: par_iter,
        map_op,
    }
}

#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct Map<P, F> {
    base: P,
    map_op: F,
}

impl<P, F, B> ParallelIterator for Map<P, F>
where
    P: ParallelIterator,
    F: Fn(P::Item) -> B + Copy + Send,
{
    type Item = B;
    type Iter = iter::Map<P::Iter, F>;

    fn len(&self) -> usize {
        self.base.len()
    }

    fn reduce(self, len: usize) -> Self {
        Self {
            base: self.base.reduce(len),
            map_op: self.map_op,
        }
    }

    fn split(self) -> (Self, Option<Self>) {
        let map_op = self.map_op;
        let (left, right) = self.base.split();
        (
            Map { base: left, map_op },
            right.map(|right| Map {
                base: right,
                map_op,
            }),
        )
    }

    fn iter(self) -> Self::Iter {
        self.base.iter().map(self.map_op)
    }
}
