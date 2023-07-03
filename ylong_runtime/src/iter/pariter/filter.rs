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

use super::ParallelIterator;
use std::iter;

pub fn filter<P, F>(par_iter: P, predicate: F) -> Filter<P, F> {
    Filter {
        base: par_iter,
        predicate,
    }
}

impl<P, F> ParallelIterator for Filter<P, F>
where
    P: ParallelIterator,
    F: Fn(&P::Item) -> bool + Copy + Send,
{
    type Item = P::Item;
    type Iter = iter::Filter<P::Iter, F>;

    fn len(&self) -> usize {
        self.base.len()
    }

    fn reduce(self, len: usize) -> Self {
        Self {
            base: self.base.reduce(len),
            predicate: self.predicate,
        }
    }

    fn split(self) -> (Self, Option<Self>) {
        let predicate = self.predicate;
        let (left, right) = self.base.split();
        (
            Filter {
                base: left,
                predicate,
            },
            right.map(|right| Filter {
                base: right,
                predicate,
            }),
        )
    }

    fn iter(self) -> Self::Iter {
        self.base.iter().filter(self.predicate)
    }
}

pub struct Filter<P, F> {
    base: P,
    predicate: F,
}
