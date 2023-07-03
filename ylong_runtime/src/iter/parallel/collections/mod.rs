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

mod binary_heap;
mod btree_map;
mod btree_set;
mod hash_map;
mod hash_set;
mod linked_list;
mod vec_deque;

macro_rules! par_vec_impl {
    ($t: ty, $iter: ty, $f: ident, impl $($args: tt)*) => {
        impl $($args)* IntoParIter for $t {
            type Data = $iter;

            fn into_par_iter(self) -> ParIter<Self::Data> {
                <Self::Data>::into_par_iter(self.$f().collect())
            }
        }
    }
}
pub(crate) use par_vec_impl;
