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

use super::IntoParIter;
use super::ParIter;
use super::ParSplit;

impl<T> ParSplit for Vec<T> {
    fn len(&self) -> usize {
        <Vec<T>>::len(self)
    }

    fn reduce(mut self, len: usize) -> Self {
        self.truncate(len);
        self
    }

    fn split(mut self) -> (Self, Option<Self>) {
        let idx = self.len() >> 1;
        let right = self.split_off(idx);
        if right.is_empty() {
            (self, None)
        } else {
            (self, Some(right))
        }
    }
}

impl<'a, T> IntoParIter for &'a Vec<T> {
    type Data = &'a [T];
    fn into_par_iter(self) -> ParIter<&'a [T]> {
        <&[T]>::into_par_iter(self)
    }
}

impl<'a, T> IntoParIter for &'a mut Vec<T> {
    type Data = &'a mut [T];
    fn into_par_iter(self) -> ParIter<&'a mut [T]> {
        <&mut [T]>::into_par_iter(self)
    }
}
