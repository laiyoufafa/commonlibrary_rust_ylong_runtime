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

//! An example for `par_iter`

use ylong_runtime::iter::prelude::*;

fn main() {
    let fut = async {
        let sum = (1..30)
            .collect::<Vec<usize>>()
            .into_par_iter()
            .map(fibbo)
            .sum()
            .await
            .unwrap();
        println!("{sum}");
    };
    ylong_runtime::block_on(fut);
}

fn fibbo(n: usize) -> usize {
    match n {
        0 => 1,
        1 => 1,
        n => fibbo(n - 1) + fibbo(n - 2),
    }
}
