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

//! Error of sync

use std::error::Error;
use std::fmt::{Debug, Display, Formatter};

/// Error returned by `Sender`.
#[derive(Debug, Eq, PartialEq)]
pub enum SendError<T> {
    /// The channel is full now.
    Full(T),
    /// The receiver of channel was closed or dropped.
    Closed(T),
    /// Sending timeout.
    TimeOut(T),
}

impl<T> Display for SendError<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SendError::Full(_) => write!(f, "channel is full"),
            SendError::Closed(_) => write!(f, "channel is closed"),
            SendError::TimeOut(_) => write!(f, "channel sending timeout"),
        }
    }
}

impl<T: Debug> Error for SendError<T> {}

/// Error returned by `Receiver`.
#[derive(Debug, Eq, PartialEq, Clone)]
pub enum RecvError {
    /// sender has not sent a value yet.
    Empty,
    /// sender was dropped.
    Closed,
    /// Receiving timeout.
    TimeOut,
}

impl Display for RecvError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RecvError::Empty => write!(f, "channel is empty"),
            RecvError::Closed => write!(f, "channel is closed"),
            RecvError::TimeOut => write!(f, "channel receiving timeout"),
        }
    }
}

impl Error for RecvError {}
