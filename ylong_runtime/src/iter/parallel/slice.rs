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

use super::ParSplit;

impl<T> ParSplit for &[T] {
    fn len(&self) -> usize {
        <[T]>::len(self)
    }

    fn reduce(self, len: usize) -> Self {
        self.split_at(len).0
    }
    fn split(self) -> (Self, Option<Self>) {
        let idx = self.len() >> 1;
        let (left, right) = self.split_at(idx);
        (left, Some(right))
    }
}

impl<'a, T> ParSplit for &'a mut [T] {
    fn len(&self) -> usize {
        <[T]>::len(self)
    }

    fn reduce(self, len: usize) -> Self {
        self.split_at_mut(len).0
    }

    fn split(self) -> (Self, Option<Self>) {
        let idx = self.len() >> 1;
        let (left, right) = self.split_at_mut(idx);
        (left, Some(right))
    }
}
