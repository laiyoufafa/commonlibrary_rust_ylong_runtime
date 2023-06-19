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

use crate::time::Clock;
use std::ptr::NonNull;

// Pointer structure to Timer to circumvent lifecycle issues caused by references.
#[derive(Debug, Ord, PartialOrd, Eq, PartialEq)]
pub(crate) struct TimerHandle {
    pub(crate) inner: NonNull<Clock>,
}

impl TimerHandle {
    // Return inner,
    // which is a non_null pointer to Timer.
    pub(crate) fn inner(&self) -> NonNull<Clock> {
        self.inner
    }
}
