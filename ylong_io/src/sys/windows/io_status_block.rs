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

use std::fmt;
use std::ops::{Deref, DerefMut};
use windows_sys::Win32::System::WindowsProgramming::{IO_STATUS_BLOCK, IO_STATUS_BLOCK_0};

pub(crate) struct IoStatusBlock(IO_STATUS_BLOCK);

unsafe impl Send for IoStatusBlock {}

impl IoStatusBlock {
    pub(crate) fn zeroed() -> Self {
        Self(IO_STATUS_BLOCK {
            Anonymous: IO_STATUS_BLOCK_0 { Status: 0 },
            Information: 0,
        })
    }
}

impl Deref for IoStatusBlock {
    type Target = IO_STATUS_BLOCK;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for IoStatusBlock {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl fmt::Debug for IoStatusBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IoStatusBlock").finish()
    }
}
