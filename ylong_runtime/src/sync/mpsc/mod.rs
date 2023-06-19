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

//! Message passing style communication

macro_rules! cfg_time {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "time")]
            $item
        )*
    }
}
pub(crate) mod array;
pub mod bounded;
pub(crate) mod channel;
pub(crate) mod queue;
pub mod unbounded;

pub use bounded::{bounded_channel, BoundedReceiver, BoundedSender};
pub use unbounded::{unbounded_channel, UnboundedReceiver, UnboundedSender};

pub(crate) trait Container {
    fn close(&self);

    fn is_close(&self) -> bool;

    fn len(&self) -> usize;
}
