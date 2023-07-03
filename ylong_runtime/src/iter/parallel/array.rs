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

impl<'a, T, const N: usize> IntoParIter for &'a [T; N] {
    type Data = &'a [T];
    fn into_par_iter(self) -> super::ParIter<&'a [T]> {
        <&[T]>::into_par_iter(self)
    }
}

impl<'a, T, const N: usize> IntoParIter for &'a mut [T; N] {
    type Data = &'a mut [T];
    fn into_par_iter(self) -> ParIter<&'a mut [T]> {
        <&mut [T]>::into_par_iter(self)
    }
}
