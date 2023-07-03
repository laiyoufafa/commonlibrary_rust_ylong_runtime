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

use crate::io::AsyncSeek;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

/// A future for seeking the io.
///
/// Returned by [`crate::io::AsyncSeekExt::seek`]
pub struct SeekTask<'a, T: ?Sized> {
    seek: &'a mut T,
    pos: Option<io::SeekFrom>,
}

impl<'a, T: ?Sized> SeekTask<'a, T> {
    pub(crate) fn new(seek: &'a mut T, pos: io::SeekFrom) -> SeekTask<'a, T> {
        SeekTask {
            seek,
            pos: Some(pos),
        }
    }
}

impl<T> Future for SeekTask<'_, T>
where
    T: AsyncSeek + Unpin + ?Sized,
{
    type Output = io::Result<u64>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let pos = self.pos.take().unwrap();
        let ret = Pin::new(&mut self.seek).poll_seek(cx, pos);
        self.pos = Some(pos);
        ret
    }
}
