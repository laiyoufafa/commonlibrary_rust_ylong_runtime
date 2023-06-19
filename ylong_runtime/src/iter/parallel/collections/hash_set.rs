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

use super::par_vec_impl;
use std::collections::HashSet;

use crate::iter::parallel::IntoParIter;
use crate::iter::parallel::ParIter;

par_vec_impl!(HashSet<T>, Vec<T>, into_iter, impl <T>);

par_vec_impl!(&'a HashSet<T>, Vec<&'a T>, iter, impl <'a, T>);
