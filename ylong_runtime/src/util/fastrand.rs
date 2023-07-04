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

//! A simple fast pseudorandom implementation, ranges from 0 to usize::MAX
//! Reference: xorshift* <https://dl.acm.org/doi/10.1145/2845077>

use std::cell::Cell;
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::num::Wrapping;

/// Generates a fast random ranging from 0 to usize::MAX
///
/// # Examples
/// ```rust
/// use ylong_runtime::util::fastrand::fast_random;
/// let rand = fast_random();
/// assert!(rand <= u64::MAX);
/// ```
pub fn fast_random() -> u64 {
    thread_local! {
        static RNG: Cell<Wrapping<u64>> = Cell::new(Wrapping(seed()));
    }

    RNG.with(|rng| {
        let mut s = rng.get();
        s ^= s >> 12;
        s ^= s << 25;
        s ^= s >> 27;
        rng.set(s);
        s.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    })
}

fn seed() -> u64 {
    let seed = RandomState::new();

    let mut out = 0;
    let mut count = 0;
    while out == 0 {
        count += 1;
        let mut hasher = seed.build_hasher();
        hasher.write_usize(count);
        out = hasher.finish();
    }
    out
}
