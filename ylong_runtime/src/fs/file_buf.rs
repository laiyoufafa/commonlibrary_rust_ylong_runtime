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

use crate::io::ReadBuf;
use std::io;
use std::io::{Read, Write};

pub(crate) struct FileBuf {
    pub(crate) buf: Vec<u8>,
    idx: usize,
}

const MAX_BUF_LEN: usize = 16384;

impl FileBuf {
    #[inline]
    pub(crate) fn with_capacity(n: usize) -> FileBuf {
        FileBuf {
            buf: Vec::with_capacity(n),
            idx: 0,
        }
    }

    pub(crate) fn remaining(&self) -> usize {
        self.buf.len() - self.idx
    }

    pub(crate) fn append_to(&mut self, buf: &mut ReadBuf<'_>) -> usize {
        let n = std::cmp::min(self.remaining(), buf.remaining());
        let r_idx = n + self.idx;
        buf.append(&self.buf[self.idx..r_idx]);

        if r_idx == self.buf.len() {
            self.idx = 0;
            self.buf.truncate(0);
        } else {
            self.idx = r_idx;
        }
        n
    }

    #[inline]
    pub(crate) fn append(&mut self, other: &[u8]) -> usize {
        let n = std::cmp::min(other.len(), MAX_BUF_LEN);
        self.buf.extend_from_slice(&other[..n]);
        n
    }

    #[allow(clippy::uninit_vec)]
    #[inline]
    pub(crate) fn reserve(&mut self, size: usize) {
        let len = std::cmp::min(size, MAX_BUF_LEN);
        self.buf.reserve(len.saturating_sub(self.buf.len()));
        // need to set length in order to read
        unsafe { self.buf.set_len(len) }
    }

    pub(crate) fn read<R: Read>(&mut self, io: &mut R) -> io::Result<usize> {
        let ret = loop {
            match io.read(&mut self.buf) {
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {}
                res => break res,
            }
        };
        match ret {
            Ok(n) => self.buf.truncate(n),
            _ => self.buf.truncate(0),
        }
        ret
    }

    #[inline]
    pub(crate) fn write<W: Write>(&mut self, io: &mut W) -> io::Result<()> {
        let res = io.write_all(&self.buf);
        self.buf.clear();
        res
    }

    // Returns the number of bytes that get dropped in the file buf
    #[inline]
    pub(crate) fn drop_unread(&mut self) -> i64 {
        let ret = self.buf.len();
        self.clear();
        ret as i64
    }

    #[inline]
    pub(crate) fn clear(&mut self) {
        self.idx = 0;
        self.buf.clear();
    }
}
