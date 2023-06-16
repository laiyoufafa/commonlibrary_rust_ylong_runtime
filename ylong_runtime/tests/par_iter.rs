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

use std::collections::{HashMap, HashSet};
/// SDV test for par_iter
/// # Title
/// sdv_par_iter
/// # Brief
/// 1.Creates a parallel iterator and adds elements together..
/// 2.Checks the correctness of the answer.
use ylong_runtime::iter::prelude::*;
#[test]
fn sdv_par_iter_test() {
    let fut = async {
        let a = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let b = vec![2, 3, 4, 5];

        let sum = a
            .par_iter()
            .map(|x| *x + 1)
            .filter(|x| *x < 5)
            .zip(b.par_iter())
            .map(|x| x.0 * (*x.1))
            .sum()
            .await
            .unwrap();
        assert_eq!(sum, 29);

        let s = a.iter().copied().collect::<HashSet<i32>>();
        let sum = s.into_par_iter().sum().await.unwrap();
        assert_eq!(sum, 55);

        let m = a
            .iter()
            .zip(b.iter())
            .map(|x| (*x.0, *x.1))
            .collect::<HashMap<i32, i32>>();
        let sum = m.into_par_iter().map(|x| x.0 * x.1).sum().await.unwrap();
        assert_eq!(sum, 40);

        let sum = a
            .into_par_iter()
            .map(|x| x + 1)
            .filter(|x| *x < 5)
            .zip(b.into_par_iter())
            .map(|x| x.0 * x.1)
            .sum()
            .await
            .unwrap();
        assert_eq!(sum, 29);
    };
    ylong_runtime::block_on(fut);
}
