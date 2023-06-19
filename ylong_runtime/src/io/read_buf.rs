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

use std::mem::MaybeUninit;

/// This buf comes from std::io::ReadBuf, an unstable std lib. This buffer is a wrapper around byte
/// buffer and it allows users to read data into an uninitialized memory. It tracks three regions in
/// the buffer: a region at the beginning of the buffer that has been logically filled with data,
/// a region that has been initialized at some point but not yet logically filled, and a region at
/// the end that is fully uninitialized. The filled region is guaranteed to be a subset of the
/// initialized region.
///
/// In summary, the contents of the buffer can be visualized as:
/// ```not_rust
/// [             capacity              ]
/// [ filled |         unfilled         ]
/// [    initialized    | uninitialized ]
/// ```
pub struct ReadBuf<'a> {
    pub(crate) buf: &'a mut [MaybeUninit<u8>],
    filled: usize,
    initialized: usize,
}

impl<'a> ReadBuf<'a> {
    /// Creates a `ReadBuf` from a fully initialized byte buffer.
    #[inline]
    pub fn new(buf: &'a mut [u8]) -> ReadBuf<'a> {
        ReadBuf {
            buf: unsafe { &mut *(buf as *mut [u8] as *mut [MaybeUninit<u8>]) },
            filled: 0,
            initialized: buf.len(),
        }
    }

    /// Creates a `ReadBuf` from an uninitialized byte buffer.
    #[inline]
    pub fn create(
        buf: &'a mut [MaybeUninit<u8>],
        filled: usize,
        initialized: usize,
    ) -> ReadBuf<'a> {
        ReadBuf {
            buf,
            filled,
            initialized,
        }
    }

    /// Creates a `ReadBuf` from a fully uninitialized byte buffer.
    #[inline]
    pub fn uninit(buf: &mut [MaybeUninit<u8>]) -> ReadBuf<'_> {
        ReadBuf {
            buf,
            filled: 0,
            initialized: 0,
        }
    }

    /// Returns the total buffer capacity.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.buf.len()
    }

    /// Returns the size of the filled portion of the buffer
    #[inline]
    pub fn filled_len(&self) -> usize {
        self.filled
    }

    /// Returns the length of the initialized portion of the buffer.
    #[inline]
    pub fn initialized_len(&self) -> usize {
        self.initialized
    }

    /// Returns a new ReadBuf that uses the first `n` unfilled bytes of the buffer.
    #[inline]
    pub fn take(&mut self, n: usize) -> ReadBuf<'_> {
        let rsize = std::cmp::min(n, self.remaining());
        ReadBuf::uninit(&mut self.unfilled_mut()[..rsize])
    }

    /// Returns a reference to the filled portion of the `ReadBuf`.
    #[inline]
    pub fn filled(&self) -> &[u8] {
        unsafe { &*(&self.buf[..self.filled] as *const [MaybeUninit<u8>] as *const [u8]) }
    }

    /// Returns a mutable reference to the filled portion of the `ReadBuf`.
    #[inline]
    pub fn filled_mut(&mut self) -> &mut [u8] {
        unsafe { &mut *(&mut self.buf[..self.filled] as *mut [MaybeUninit<u8>] as *mut [u8]) }
    }

    /// Returns a mutable reference to the unfilled portion of the `ReadBuf`.
    #[inline]
    pub fn unfilled_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        &mut self.buf[self.filled..]
    }

    /// Returns a reference to the initialized portion of the `ReadBuf`.
    #[inline]
    pub fn initialized(&self) -> &[u8] {
        unsafe { &*(&self.buf[..self.initialized] as *const [MaybeUninit<u8>] as *const [u8]) }
    }

    /// Returns a mutable reference to the initialized portion of the `ReadBuf`.
    #[inline]
    pub fn initialized_mut(&mut self) -> &mut [u8] {
        unsafe { &mut *(&mut self.buf[..self.initialized] as *mut [MaybeUninit<u8>] as *mut [u8]) }
    }

    /// Returns a mutable reference to the entire buffer, including the initialized and uninitialized
    /// portion. If the buffer is partially initialized, the caller must call [`ReadBuf::assume_init`] with
    /// the number of bytes initialized.
    #[inline]
    pub fn inner_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        self.buf
    }

    /// Returns the remaining unfilled space of the `ReadBuf`.
    #[inline]
    pub fn remaining(&self) -> usize {
        self.buf.len() - self.filled
    }

    /// Returns a mutable reference to the first n bytes of the unfilled portion of the `ReadBuf`.
    /// This method guarantees the returned buffer is fully initialized.
    ///
    /// # Panics
    /// Panics if n is bigger than the remaining capacity of the buf.
    #[inline]
    pub fn initialize_unfilled_to(&mut self, n: usize) -> &mut [u8] {
        if n > self.remaining() {
            panic!("overflowed: try to initialize more bytes than the buffer's capacity")
        }
        let end = self.filled + n;

        if self.initialized < end {
            unsafe {
                self.buf[self.initialized..self.filled + n]
                    .as_mut_ptr()
                    .write_bytes(0, end - self.initialized);
            }
            self.initialized = end;
        }
        unsafe { &mut *(&mut self.buf[self.filled..end] as *mut [MaybeUninit<u8>] as *mut [u8]) }
    }

    /// Returns a mutable reference to the unfilled portion of the `ReadBuf`. This method guarantees
    /// the the buffer is fully initialized.
    #[inline]
    pub fn initialize_unfilled(&mut self) -> &mut [u8] {
        self.initialize_unfilled_to(self.remaining())
    }

    /// Clears the `ReadBuf`. The filled size turns to zero while the initialized size is unchanged.
    #[inline]
    pub fn clear(&mut self) {
        self.filled = 0;
    }

    /// Sets the filled size of the buffer.
    ///
    /// # Panics
    /// Panics if the filled portion is bigger than the initialized portion of the buffer.
    #[inline]
    pub fn set_filled(&mut self, n: usize) {
        if n > self.initialized {
            panic!("buf's filled size becomes larger than the initialized size")
        }
        self.filled = n;
    }

    /// Advances the filled portion of the buffer by n bytes.
    ///
    /// # Panics
    /// 1. Panics if the filled size is overflowed after adding the advance size.
    /// 2. Panics if the filled portion becomes larger than the initialized portion of the buffer.
    #[inline]
    pub fn advance(&mut self, n: usize) {
        let filled = self
            .filled
            .checked_add(n)
            .expect("buf filled size overflow");
        self.set_filled(filled);
    }

    /// Makes the n bytes after the filled portion of the buffer become initialized. If adding
    /// n bytes exceeds the capacity, the initialized size will be set to the capacity.
    #[inline]
    pub fn assume_init(&mut self, n: usize) {
        let end = std::cmp::min(self.filled + n, self.capacity());
        if end > self.initialized {
            self.initialized = end;
        }
    }

    /// Appends the input data into the `BufRead`. Advances the filled size and initialized size
    /// accordingly.
    ///
    /// # Panics
    /// Panics if the size of the appending buffer is greater than the remaining size of the `ReadBuf`
    #[inline]
    pub fn append(&mut self, buf: &[u8]) {
        if buf.len() > self.remaining() {
            panic!("slice size is larger than the buf's remaining size");
        }
        let end = self.filled + buf.len();
        unsafe {
            self.buf[self.filled..end]
                .as_mut_ptr()
                .cast::<u8>()
                .copy_from_nonoverlapping(buf.as_ptr(), buf.len());
        }

        if self.initialized < end {
            self.initialized = end;
        }
        self.filled = end;
    }
}
