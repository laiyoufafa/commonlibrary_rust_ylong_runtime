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

//! Bit mask.

/// Mask, representing a segment of consecutive binary bits, e.g.
///
/// When using the mask with a binary number for **and** operations, the value at the mask position of the binary number can be obtained.
///
/// For example: 0000_1111(mask) & 1010_1010(target number) = 0000_1010
///
/// # Example
/// ```rust
/// use ylong_runtime::util::bit::Mask;
///
/// // On a 64-bit machine, you can get the mask 0xf
/// let mask = Mask::new(4, 0);
///
/// ```
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Mask {
    mask: usize,
    shift: u32,
}

impl Mask {
    /// Creates a mask. Generates a mask based on the length of consecutive binary bits + an offset.
    ///
    /// # Parameter
    /// width: Length of consecutive binary bits
    ///
    /// shift: Offset of consecutive binary bits to the **left**
    ///
    /// # Example
    /// When width + shift < Machine word length time
    /// ```rust
    /// use ylong_runtime::util::bit::Mask;
    ///
    /// let width = 4;
    /// let shift = 0;
    /// let mask = Mask::new(width, shift);
    /// // Get mask as 0xf
    ///
    /// let width = 4;
    /// let shift = 4;
    /// let mask = Mask::new(width, shift);
    /// // Get mask as 0xf0
    /// ```
    /// When width >= machine word length, a mask of all 1's is returned regardless of the shift.
    /// ```rust
    /// use ylong_runtime::util::bit::Mask;
    ///
    /// let width = 128;
    /// let shift = 0;
    /// let mask = Mask::new(width, shift);
    /// // On a 64-bit machine, the mask is 0xffff_ffff_ffff_ffff
    /// ```
    /// When width == 0, an all-0 mask is returned, regardless of the shift.
    /// ```rust
    /// use ylong_runtime::util::bit::Mask;
    ///
    /// let width = 0;
    /// let shift = 0;
    /// let mask = Mask::new(width, shift);
    /// // On a 64-bit machine, the mask is 0x0
    /// ```
    /// When width < machine word length and width + shift > machine word length, it will ensure that width remains unchanged and shift becomes machine word length - width.
    /// ```rust
    /// use ylong_runtime::util::bit::Mask;
    ///
    /// let width = 32;
    /// let shift = 64;
    /// let mask = Mask::new(width, shift);
    /// // On a 64-bit machine, the mask is 0xffff_ffff_0000_0000, the offset becomes 32.
    /// ```
    pub const fn new(width: u32, shift: u32) -> Self {
        const USIZE_LEN: u32 = 0usize.wrapping_sub(1).count_ones();
        if width >= USIZE_LEN {
            return Mask {
                mask: 0usize.wrapping_sub(1),
                shift: 0,
            };
        }
        if width == 0 {
            return Mask { mask: 0, shift: 0 };
        }
        let wb = 1usize.wrapping_shl(width) - 1;
        let left = USIZE_LEN - width;
        let sh = if left > shift { shift } else { left };
        Mask {
            mask: wb.wrapping_shl(sh),
            shift: sh,
        }
    }
}

/// Bit is used for some binary processing. A usize type can be converted to a Bit type to get a part of it by concatenating it with a Mask type.
///
/// Uses the get_by_mask method to get the value of the specified bit from the Bit. Uses set_by_mask to add the value of the specified bit to the bit.
///
/// For example, divides a usize type into different meanings according to each bit, uses Mask to get the mask in the corresponding position,
/// and then uses Bit set_by_mask\get_by_mask to modify the specified bit.
///
/// # Example
/// ```not run
/// use ylong_runtime::util::bit::{Mask, Bit};
///
/// // Assign the following meaning to a portion of a usize bit, using a 64-bit machine word length as an example
/// // |---- A ---|---- B ---|---- C ---|---- D ---|
/// // |- 16bits -|- 16bits -|- 16bits -|- 16bits -|
///
/// const A: Mask = Mask::new(16, 48);
/// const B: Mask = Mask::new(16, 32);
/// const C: Mask = Mask::new(16, 16);
/// const D: Mask = Mask::new(16, 0);
///
/// // Create a Bit type instance
/// let base = 0usize;
/// let mut bits = Bit::from_usize(base);
///
/// // Get the value of A
/// let a = bits.get_by_mask(A);
/// assert_eq!(a, 0x0);
///
/// // Modify the value of A
/// let new_a = 0x1234usize;
/// bits.set_by_mask(A, new_a);
/// assert_eq!(bits.as_usize(), 0x1234_0000_0000_0000);
/// ```
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Bit(usize);

impl Bit {
    /// Converts a usize type to a Bit type
    ///
    /// # Example
    /// ```rust
    /// use ylong_runtime::util::bit::Bit;
    ///
    /// let base = 0usize;
    /// let bits = Bit::from_usize(base);
    /// ```
    pub fn from_usize(val: usize) -> Self {
        Bit(val)
    }

    /// Converts a Bit type to a usize type
    ///
    /// # Example
    /// ```rust
    /// use ylong_runtime::util::bit::Bit;
    ///
    /// let base = 0usize;
    /// let bits = Bit::from_usize(base);
    /// let val = bits.as_usize();
    /// assert_eq!(val, 0);
    /// ```
    pub fn as_usize(&self) -> usize {
        self.0
    }

    /// Overwrites a usize data to the specified mask position of the bit.
    ///
    /// If the binary length of the given usize data is greater than the binary bit length of the mask, it will be truncated.
    ///
    /// # Example
    /// When Mask binary length >= val binary length (excluding leading zeros)
    /// ```rust
    /// use ylong_runtime::util::bit::{Mask, Bit};
    ///
    /// // mask's length 16
    /// const MASK: Mask = Mask::new(16, 0);
    /// let mut bits = Bit::from_usize(0);
    ///
    /// // val's length 16（dividing the leading 0）
    /// let val = 0x1234usize;
    /// bits.set_by_mask(MASK, val);
    /// assert_eq!(bits, Bit::from_usize(0x1234));
    ///
    /// ```
    /// When Mask binary length < val binary length (excluding leading zeros), val is truncated and only the low bit is retained.
    /// ```rust
    /// use ylong_runtime::util::bit::{Mask, Bit};
    ///
    /// // mask's length 16
    /// const MASK: Mask = Mask::new(16, 0);
    /// let mut bits = Bit::from_usize(0);
    ///
    /// // val's length 32（dividing the leading 0）
    /// let val = 0x1234_5678usize;
    /// bits.set_by_mask(MASK, val);
    /// // val is truncated, leaving only the lower 16 bits.
    /// assert_eq!(bits, Bit::from_usize(0x5678));
    /// ```
    pub fn set_by_mask(&mut self, mask: Mask, val: usize) {
        self.0 = (self.0 & !mask.mask) | ((val << mask.shift) & mask.mask);
    }

    /// Gets the binary value at the specified mask position from the bit.
    ///
    /// # Example
    /// ```not run
    /// use ylong_runtime::util::bit::{Mask, Bit};
    ///
    /// const MASK: Mask = Mask::new(16, 0);
    ///
    /// let base = 0x0123_4567_89ab_cdefusize;
    /// let bits = Bit::from_usize(base);
    ///
    /// let val = bits.get_by_mask(MASK);
    /// assert_eq!(val, 0xcdef);
    /// ```
    pub fn get_by_mask(&self, mask: Mask) -> usize {
        (self.0 & mask.mask) >> mask.shift
    }

    /// Sets bit to a new usize value.
    ///
    /// # Example
    /// ```not run
    /// use ylong_runtime::util::bit::{Mask, Bit};
    ///
    /// let base = 0x0usize;
    /// let mut bits = Bit::from_usize(base);
    /// bits.set(0xffff_ffff_ffff_ffff);
    /// assert_eq!(bits.as_usize(), 0xffff_ffff_ffff_ffff);
    /// ```
    pub fn set(&mut self, val: usize) {
        self.0 = val;
    }

    /// Clear bit to 0usize
    ///
    /// # Example
    /// ```rust
    /// use ylong_runtime::util::bit::{Mask, Bit};
    ///
    /// let base = 0xffffusize;
    /// let mut bits = Bit::from_usize(base);
    /// bits.clear();
    /// assert_eq!(bits.as_usize(), 0x0);
    /// ```
    pub fn clear(&mut self) {
        self.0 = 0;
    }
}

/*
* @title  mask new function ut test
* @design Conditions of use override
* @precon None
* @brief  1. Get the current machine word length
*         2. Call the new function according to the machine word length, pass in the parameters, and create the Mask
*         3. Check return value
* @expect 1. On a 64-bit machine, width >= 64, get width = 64, shift = 0
*         2. On a 64-bit machine, width == 0, get width = 0, shift = 0
*         3. On a 64-bit machine, 0 < width < 64, width + shift >= 64, get width = input width,  shift = 64 - input width
*         4. On a 64-bit machine, 0 < width < 64, width + shift < 64, call width = input width,  shift = input shift
*         5. On a 32-bit machine, width >= 32, call new, get width = 32, shift = 0
*         6. On a 32-bit machine, width == 0, call new, get width = 0, shift = 0
*         7. On a 32-bit machine, 0 < width < 32, width + shift >= 32, get width = input width, shift = 32 - input width
*         8. On a 32-bit machine, 0 < width < 32, width + shift < 32, get width = input width, shift = input width
* @auto   Yes
*/
#[test]
fn ut_mask_new() {
    const USIZE_LEN: u32 = 0usize.wrapping_sub(1).count_ones();

    match USIZE_LEN {
        32 => ut_mask_new_bit32(),
        64 => ut_mask_new_bit64(),
        _ => {}
    }

    fn ut_mask_new_bit32() {
        assert_eq!(
            Mask::new(0, 0),
            Mask {
                mask: 0x0,
                shift: 0
            }
        );
        assert_eq!(
            Mask::new(0, 16),
            Mask {
                mask: 0x0,
                shift: 0
            }
        );
        assert_eq!(
            Mask::new(0, 32),
            Mask {
                mask: 0x0,
                shift: 0
            }
        );
        assert_eq!(
            Mask::new(0, 64),
            Mask {
                mask: 0x0,
                shift: 0
            }
        );
        assert_eq!(
            Mask::new(1, 0),
            Mask {
                mask: 0x1,
                shift: 0
            }
        );
        assert_eq!(
            Mask::new(1, 16),
            Mask {
                mask: 0x1_0000,
                shift: 16
            }
        );
        assert_eq!(
            Mask::new(1, 31),
            Mask {
                mask: 0x8000_0000,
                shift: 31
            }
        );
        assert_eq!(
            Mask::new(1, 32),
            Mask {
                mask: 0x8000_0000,
                shift: 31
            }
        );
        assert_eq!(
            Mask::new(1, 128),
            Mask {
                mask: 0x8000_0000,
                shift: 31
            }
        );
        assert_eq!(
            Mask::new(4, 0),
            Mask {
                mask: 0xf,
                shift: 0
            }
        );
        assert_eq!(
            Mask::new(4, 16),
            Mask {
                mask: 0xf_0000,
                shift: 16
            }
        );
        assert_eq!(
            Mask::new(4, 28),
            Mask {
                mask: 0xf000_0000,
                shift: 28
            }
        );
        assert_eq!(
            Mask::new(4, 32),
            Mask {
                mask: 0xf000_0000,
                shift: 28
            }
        );
        assert_eq!(
            Mask::new(4, 64),
            Mask {
                mask: 0xf000_0000,
                shift: 28
            }
        );
        assert_eq!(
            Mask::new(16, 0),
            Mask {
                mask: 0xffff,
                shift: 0
            }
        );
        assert_eq!(
            Mask::new(16, 16),
            Mask {
                mask: 0xffff_0000,
                shift: 16
            }
        );
        assert_eq!(
            Mask::new(16, 32),
            Mask {
                mask: 0xffff_0000,
                shift: 16
            }
        );
        assert_eq!(
            Mask::new(16, 64),
            Mask {
                mask: 0xffff_0000,
                shift: 16
            }
        );
        assert_eq!(
            Mask::new(32, 0),
            Mask {
                mask: 0xffff_ffff,
                shift: 0
            }
        );
        assert_eq!(
            Mask::new(32, 16),
            Mask {
                mask: 0xffff_ffff,
                shift: 0
            }
        );
        assert_eq!(
            Mask::new(32, 32),
            Mask {
                mask: 0xffff_ffff,
                shift: 0
            }
        );
        assert_eq!(
            Mask::new(32, 64),
            Mask {
                mask: 0xffff_ffff,
                shift: 0
            }
        );
        assert_eq!(
            Mask::new(64, 0),
            Mask {
                mask: 0xffff_ffff,
                shift: 0
            }
        );
        assert_eq!(
            Mask::new(64, 16),
            Mask {
                mask: 0xffff_ffff,
                shift: 0
            }
        );
        assert_eq!(
            Mask::new(64, 32),
            Mask {
                mask: 0xffff_ffff,
                shift: 0
            }
        );
        assert_eq!(
            Mask::new(64, 64),
            Mask {
                mask: 0xffff_ffff,
                shift: 0
            }
        );
    }

    fn ut_mask_new_bit64() {
        assert_eq!(
            Mask::new(0, 0),
            Mask {
                mask: 0x0,
                shift: 0
            }
        );
        assert_eq!(
            Mask::new(0, 32),
            Mask {
                mask: 0x0,
                shift: 0
            }
        );
        assert_eq!(
            Mask::new(0, 64),
            Mask {
                mask: 0x0,
                shift: 0
            }
        );
        assert_eq!(
            Mask::new(0, 128),
            Mask {
                mask: 0x0,
                shift: 0
            }
        );

        assert_eq!(
            Mask::new(1, 0),
            Mask {
                mask: 0x1,
                shift: 0
            }
        );
        assert_eq!(
            Mask::new(1, 32),
            Mask {
                mask: 0x1_0000_0000,
                shift: 32
            }
        );
        assert_eq!(
            Mask::new(1, 63),
            Mask {
                mask: 0x8000_0000_0000_0000,
                shift: 63
            }
        );
        assert_eq!(
            Mask::new(1, 64),
            Mask {
                mask: 0x8000_0000_0000_0000,
                shift: 63
            }
        );
        assert_eq!(
            Mask::new(1, 128),
            Mask {
                mask: 0x8000_0000_0000_0000,
                shift: 63
            }
        );

        assert_eq!(
            Mask::new(4, 0),
            Mask {
                mask: 0xf,
                shift: 0
            }
        );
        assert_eq!(
            Mask::new(4, 32),
            Mask {
                mask: 0xf_0000_0000,
                shift: 32
            }
        );
        assert_eq!(
            Mask::new(4, 60),
            Mask {
                mask: 0xf000_0000_0000_0000,
                shift: 60
            }
        );
        assert_eq!(
            Mask::new(4, 64),
            Mask {
                mask: 0xf000_0000_0000_0000,
                shift: 60
            }
        );
        assert_eq!(
            Mask::new(4, 128),
            Mask {
                mask: 0xf000_0000_0000_0000,
                shift: 60
            }
        );

        assert_eq!(
            Mask::new(32, 0),
            Mask {
                mask: 0xffff_ffff,
                shift: 0
            }
        );
        assert_eq!(
            Mask::new(32, 32),
            Mask {
                mask: 0xffff_ffff_0000_0000,
                shift: 32
            }
        );
        assert_eq!(
            Mask::new(32, 64),
            Mask {
                mask: 0xffff_ffff_0000_0000,
                shift: 32
            }
        );
        assert_eq!(
            Mask::new(32, 128),
            Mask {
                mask: 0xffff_ffff_0000_0000,
                shift: 32
            }
        );

        assert_eq!(
            Mask::new(64, 0),
            Mask {
                mask: 0xffff_ffff_ffff_ffff,
                shift: 0
            }
        );
        assert_eq!(
            Mask::new(64, 32),
            Mask {
                mask: 0xffff_ffff_ffff_ffff,
                shift: 0
            }
        );
        assert_eq!(
            Mask::new(64, 64),
            Mask {
                mask: 0xffff_ffff_ffff_ffff,
                shift: 0
            }
        );
        assert_eq!(
            Mask::new(64, 128),
            Mask {
                mask: 0xffff_ffff_ffff_ffff,
                shift: 0
            }
        );

        assert_eq!(
            Mask::new(128, 0),
            Mask {
                mask: 0xffff_ffff_ffff_ffff,
                shift: 0
            }
        );
        assert_eq!(
            Mask::new(128, 32),
            Mask {
                mask: 0xffff_ffff_ffff_ffff,
                shift: 0
            }
        );
        assert_eq!(
            Mask::new(128, 64),
            Mask {
                mask: 0xffff_ffff_ffff_ffff,
                shift: 0
            }
        );
        assert_eq!(
            Mask::new(128, 128),
            Mask {
                mask: 0xffff_ffff_ffff_ffff,
                shift: 0
            }
        );
    }
}

/*
* @title  bit from_usize function ut test
* @design Conditions of use override
* @precon None
* @brief  None
* @expect 1. Pass in any usize, call from_usize, and check the return value
* @auto   Yes
*/
#[test]
fn ut_bit_from_usize() {
    const USIZE_MAX: usize = 0usize.wrapping_sub(1);
    const USIZE_MAX_HALF: usize = 0usize.wrapping_sub(1) / 2;

    assert_eq!(Bit::from_usize(0), Bit(0));
    assert_eq!(Bit::from_usize(USIZE_MAX_HALF), Bit(USIZE_MAX_HALF));
    assert_eq!(Bit::from_usize(USIZE_MAX), Bit(USIZE_MAX));
}

/*
* @title  bit as_usize function ut test
* @design Conditions of use override
* @precon None
* @brief  1. Creating a Bit Instance
*         2. Call as_usize
*         3. Check return value
* @expect 1. Get the bit internal usize
* @auto   Yes
*/
#[test]
fn ut_bit_as_usize() {
    const USIZE_MAX: usize = 0usize.wrapping_sub(1);
    const USIZE_MAX_HALF: usize = 0usize.wrapping_sub(1) / 2;

    assert_eq!(Bit::from_usize(0).as_usize(), 0);
    assert_eq!(Bit::from_usize(USIZE_MAX_HALF).as_usize(), USIZE_MAX_HALF);
    assert_eq!(Bit::from_usize(USIZE_MAX).as_usize(), USIZE_MAX);
}

/*
* @title  bit set function ut test
* @design Conditions of use override
* @precon None
* @brief  1. Creating a Bit Instance
*         2. Call the set function and pass in a new usize
*         3. Check return value
* @expect 1. Bit Internal value becomes the new usize passed in
* @auto   Yes
*/
#[test]
fn ut_bit_set() {
    const USIZE_MAX: usize = 0usize.wrapping_sub(1);
    const USIZE_MAX_HALF: usize = 0usize.wrapping_sub(1) / 2;

    let mut b = Bit::from_usize(0);
    b.set(0xf0f0);
    assert_eq!(b.as_usize(), 0xf0f0);

    let mut b = Bit::from_usize(USIZE_MAX_HALF);
    b.set(0xf0f0);
    assert_eq!(b.as_usize(), 0xf0f0);

    let mut b = Bit::from_usize(USIZE_MAX);
    b.set(0xf0f0);
    assert_eq!(b.as_usize(), 0xf0f0);
}

/*
* @title  bit clear function ut test
* @design Conditions of use override
* @precon None
* @brief  1. Creating a Bit Instance
*         2. Call clear()
*         3. Calibrate the instance
* @expect 1. The internal value of this Bit instance is cleared to zero
* @auto   Yes
*/
#[test]
fn ut_bit_clear() {
    const USIZE_MAX: usize = 0usize.wrapping_sub(1);
    const USIZE_MAX_HALF: usize = 0usize.wrapping_sub(1) / 2;

    let mut b = Bit::from_usize(0);
    b.clear();
    assert_eq!(b.as_usize(), 0);

    let mut b = Bit::from_usize(USIZE_MAX_HALF);
    b.clear();
    assert_eq!(b.as_usize(), 0);

    let mut b = Bit::from_usize(USIZE_MAX);
    b.clear();
    assert_eq!(b.as_usize(), 0);
}

/*
* @title  bit set_by_mask function ut test
* @design Conditions of use override
* @precon None
* @brief  1. Create a Bit instance, create a Mask instance
*         2. Call set_by_mask()
*         3. Verify the Bit instance
* @expect 1. Mask length < val valid value length, the truncated part of the valid value is overwritten into the Bit instance
*         2. Mask length >= val valid value length, the complete part of the valid value is overwritten into the Bit instance
* @auto   Yes
*/
#[test]
fn ut_bit_set_by_mask() {
    const USIZE_LEN: u32 = 0usize.wrapping_sub(1).count_ones();

    match USIZE_LEN {
        32 => ut_bit_set_by_mask_bit32(),
        64 => ut_bit_set_by_mask_bit64(),
        _ => {}
    }

    fn ut_bit_set_by_mask_bit32() {
        let val = 0x0usize;
        let mut bit = Bit::from_usize(val);
        bit.set_by_mask(Mask::new(0, 0), 0x0);
        assert_eq!(bit, Bit::from_usize(0x0));

        bit.clear();
        bit.set_by_mask(Mask::new(4, 0), 0xf);
        assert_eq!(bit, Bit::from_usize(0xf));

        bit.clear();
        bit.set_by_mask(Mask::new(8, 0), 0xff);
        assert_eq!(bit, Bit::from_usize(0xff));

        bit.clear();
        bit.set_by_mask(Mask::new(4, 4), 0xf);
        assert_eq!(bit, Bit::from_usize(0xf0));

        bit.clear();
        bit.set_by_mask(Mask::new(4, 28), 0xf);
        assert_eq!(bit, Bit::from_usize(0xf000_0000));

        bit.clear();
        bit.set_by_mask(Mask::new(0, 0), 0xffff_ffff);
        assert_eq!(bit, Bit::from_usize(0x0));

        bit.clear();
        bit.set_by_mask(Mask::new(4, 0), 0xffff_ffff);
        assert_eq!(bit, Bit::from_usize(0xf));

        bit.clear();
        bit.set_by_mask(Mask::new(4, 4), 0xffff_ffff);
        assert_eq!(bit, Bit::from_usize(0xf0));

        let val = 0xffff_ffff;
        let mut bit = Bit::from_usize(val);
        bit.set_by_mask(Mask::new(0, 0), 0x0);
        assert_eq!(bit, Bit::from_usize(0xffff_ffff));

        bit.set(val);
        bit.set_by_mask(Mask::new(4, 0), 0x0);
        assert_eq!(bit, Bit::from_usize(0xffff_fff0));

        bit.set(val);
        bit.set_by_mask(Mask::new(4, 4), 0x0);
        assert_eq!(bit, Bit::from_usize(0xffff_ff0f));

        bit.set(val);
        bit.set_by_mask(Mask::new(4, 8), 0x0);
        assert_eq!(bit, Bit::from_usize(0xffff_f0ff));

        bit.set(val);
        bit.set_by_mask(Mask::new(4, 0), 0xabcd);
        assert_eq!(bit, Bit::from_usize(0xffff_fffd));

        bit.set(val);
        bit.set_by_mask(Mask::new(8, 0), 0xabcd);
        assert_eq!(bit, Bit::from_usize(0xffff_ffcd));

        bit.set(val);
        bit.set_by_mask(Mask::new(16, 0), 0xabcd);
        assert_eq!(bit, Bit::from_usize(0xffff_abcd));

        bit.set(val);
        bit.set_by_mask(Mask::new(16, 16), 0xabcd);
        assert_eq!(bit, Bit::from_usize(0xabcd_ffff));

        bit.set(val);
        bit.set_by_mask(Mask::new(32, 0), 0xabcd);
        assert_eq!(bit, Bit::from_usize(0x0000_abcd));
    }

    fn ut_bit_set_by_mask_bit64() {
        let val = 0x0usize;
        let mut bit = Bit::from_usize(val);
        bit.set_by_mask(Mask::new(0, 0), 0x0);
        assert_eq!(bit, Bit::from_usize(0x0));

        bit.clear();
        bit.set_by_mask(Mask::new(4, 0), 0xf);
        assert_eq!(bit, Bit::from_usize(0xf));

        bit.clear();
        bit.set_by_mask(Mask::new(8, 0), 0xff);
        assert_eq!(bit, Bit::from_usize(0xff));

        bit.clear();
        bit.set_by_mask(Mask::new(4, 4), 0xf);
        assert_eq!(bit, Bit::from_usize(0xf0));

        bit.clear();
        bit.set_by_mask(Mask::new(4, 32), 0xf);
        assert_eq!(bit, Bit::from_usize(0xf_0000_0000));

        bit.clear();
        bit.set_by_mask(Mask::new(4, 0), 0xffff_ffff_ffff_ffff);
        assert_eq!(bit, Bit::from_usize(0xf));

        bit.clear();
        bit.set_by_mask(Mask::new(4, 4), 0xffff_ffff_ffff_ffff);
        assert_eq!(bit, Bit::from_usize(0xf0));

        bit.clear();
        bit.set_by_mask(Mask::new(0, 0), 0xffff_ffff_ffff_ffff);
        assert_eq!(bit, Bit::from_usize(0));

        let val = 0xffff_ffff_ffff_ffffusize;
        let mut bit = Bit::from_usize(val);
        bit.set_by_mask(Mask::new(0, 0), 0x0);
        assert_eq!(bit, Bit::from_usize(0xffff_ffff_ffff_ffff));

        bit.set(0xffff_ffff_ffff_ffff);
        bit.set_by_mask(Mask::new(4, 0), 0x0);
        assert_eq!(bit, Bit::from_usize(0xffff_ffff_ffff_fff0));

        bit.set(0xffff_ffff_ffff_ffff);
        bit.set_by_mask(Mask::new(4, 4), 0x0);
        assert_eq!(bit, Bit::from_usize(0xffff_ffff_ffff_ff0f));

        bit.set(0xffff_ffff_ffff_ffff);
        bit.set_by_mask(Mask::new(4, 32), 0x0);
        assert_eq!(bit, Bit::from_usize(0xffff_fff0_ffff_ffff));

        bit.set(0xffff_ffff_ffff_ffff);
        bit.set_by_mask(Mask::new(4, 60), 0x0);
        assert_eq!(bit, Bit::from_usize(0x0fff_ffff_ffff_ffff));

        bit.set(0xffff_ffff_ffff_ffff);
        bit.set_by_mask(Mask::new(16, 0), 0xabcd);
        assert_eq!(bit, Bit::from_usize(0xffff_ffff_ffff_abcd));

        bit.set(0xffff_ffff_ffff_ffff);
        bit.set_by_mask(Mask::new(32, 0), 0xabcd);
        assert_eq!(bit, Bit::from_usize(0xffff_ffff_0000_abcd));

        bit.set(0xffff_ffff_ffff_ffff);
        bit.set_by_mask(Mask::new(12, 0), 0xabcd);
        assert_eq!(bit, Bit::from_usize(0xffff_ffff_ffff_fbcd));

        bit.set(0xffff_ffff_ffff_ffff);
        bit.set_by_mask(Mask::new(16, 8), 0xabcd);
        assert_eq!(bit, Bit::from_usize(0xffff_ffff_ffab_cdff));
    }
}

/*
* @title  bit get_by_mask function ut test
* @design Conditions of use override
* @precon None
* @brief  1. Create a Bit instance, create a Mask instance
*         2. Call get_by_mask()
*         3. Check return value
* @expect 1. Gets the value of the Bit instance on the corresponding Mask bit
* @auto   Yes
*/
#[test]
fn ut_bit_get_by_mask() {
    const USIZE_LEN: u32 = 0usize.wrapping_sub(1).count_ones();

    match USIZE_LEN {
        32 => ut_bit_get_by_mask_bit32(),
        64 => ut_bit_get_by_mask_bit64(),
        _ => {}
    }

    fn ut_bit_get_by_mask_bit32() {
        let val = 0x0usize;
        let bit = Bit::from_usize(val);
        assert_eq!(bit.get_by_mask(Mask::new(0, 0)), 0x0);
        assert_eq!(bit.get_by_mask(Mask::new(1, 0)), 0x0);
        assert_eq!(bit.get_by_mask(Mask::new(16, 0)), 0x0);
        assert_eq!(bit.get_by_mask(Mask::new(32, 0)), 0x0);

        let val = 0xffff_ffffusize;
        let bit = Bit::from_usize(val);
        assert_eq!(bit.get_by_mask(Mask::new(0, 0)), 0x0);
        assert_eq!(bit.get_by_mask(Mask::new(1, 0)), 0x1);
        assert_eq!(bit.get_by_mask(Mask::new(16, 0)), 0xffff);
        assert_eq!(bit.get_by_mask(Mask::new(32, 0)), 0xffff_ffff);

        let val = 0x1234_cdefusize;
        let bit = Bit::from_usize(val);
        assert_eq!(bit.get_by_mask(Mask::new(0, 0)), 0x0);
        assert_eq!(bit.get_by_mask(Mask::new(4, 0)), 0xf);
        assert_eq!(bit.get_by_mask(Mask::new(8, 0)), 0xef);
        assert_eq!(bit.get_by_mask(Mask::new(12, 0)), 0xdef);
        assert_eq!(bit.get_by_mask(Mask::new(16, 0)), 0xcdef);
        assert_eq!(bit.get_by_mask(Mask::new(32, 0)), 0x1234_cdef);
        assert_eq!(bit.get_by_mask(Mask::new(4, 4)), 0xe);
        assert_eq!(bit.get_by_mask(Mask::new(8, 4)), 0xde);
        assert_eq!(bit.get_by_mask(Mask::new(12, 4)), 0xcde);
        assert_eq!(bit.get_by_mask(Mask::new(4, 16)), 0x4);
        assert_eq!(bit.get_by_mask(Mask::new(8, 16)), 0x34);
        assert_eq!(bit.get_by_mask(Mask::new(12, 16)), 0x234);
        assert_eq!(bit.get_by_mask(Mask::new(3, 0)), 0x7);
        assert_eq!(bit.get_by_mask(Mask::new(3, 4)), 0x6);
        assert_eq!(bit.get_by_mask(Mask::new(3, 8)), 0x5);
        assert_eq!(bit.get_by_mask(Mask::new(3, 16)), 0x4);
        assert_eq!(bit.get_by_mask(Mask::new(3, 20)), 0x3);
        assert_eq!(bit.get_by_mask(Mask::new(3, 24)), 0x2);
        assert_eq!(bit.get_by_mask(Mask::new(3, 1)), 0x7);
        assert_eq!(bit.get_by_mask(Mask::new(3, 5)), 0x7);
        assert_eq!(bit.get_by_mask(Mask::new(3, 9)), 0x6);
    }

    fn ut_bit_get_by_mask_bit64() {
        let val = 0x0usize;
        let bit = Bit::from_usize(val);
        assert_eq!(bit.get_by_mask(Mask::new(0, 0)), 0x0);
        assert_eq!(bit.get_by_mask(Mask::new(1, 0)), 0x0);
        assert_eq!(bit.get_by_mask(Mask::new(32, 0)), 0x0);
        assert_eq!(bit.get_by_mask(Mask::new(64, 0)), 0x0);

        let val = 0xffff_ffff_ffff_ffffusize;
        let bit = Bit::from_usize(val);
        assert_eq!(bit.get_by_mask(Mask::new(0, 0)), 0x0);
        assert_eq!(bit.get_by_mask(Mask::new(1, 0)), 0x1);
        assert_eq!(bit.get_by_mask(Mask::new(32, 0)), 0xffff_ffff);
        assert_eq!(bit.get_by_mask(Mask::new(64, 0)), 0xffff_ffff_ffff_ffff);

        let val = 0x0123_4567_89ab_cdefusize;
        let bit = Bit::from_usize(val);
        assert_eq!(bit.get_by_mask(Mask::new(0, 0)), 0x0);
        assert_eq!(bit.get_by_mask(Mask::new(4, 0)), 0xf);
        assert_eq!(bit.get_by_mask(Mask::new(8, 0)), 0xef);
        assert_eq!(bit.get_by_mask(Mask::new(32, 0)), 0x89ab_cdef);
        assert_eq!(bit.get_by_mask(Mask::new(64, 0)), 0x0123_4567_89ab_cdef);
        assert_eq!(bit.get_by_mask(Mask::new(4, 4)), 0xe);
        assert_eq!(bit.get_by_mask(Mask::new(8, 4)), 0xde);
        assert_eq!(bit.get_by_mask(Mask::new(12, 4)), 0xcde);
        assert_eq!(bit.get_by_mask(Mask::new(60, 4)), 0x0012_3456_789a_bcde);
        assert_eq!(bit.get_by_mask(Mask::new(4, 32)), 0x7);
        assert_eq!(bit.get_by_mask(Mask::new(8, 32)), 0x67);
        assert_eq!(bit.get_by_mask(Mask::new(12, 32)), 0x567);
        assert_eq!(bit.get_by_mask(Mask::new(3, 0)), 0x7);
        assert_eq!(bit.get_by_mask(Mask::new(3, 4)), 0x6);
        assert_eq!(bit.get_by_mask(Mask::new(3, 8)), 0x5);
        assert_eq!(bit.get_by_mask(Mask::new(3, 1)), 0x7);
        assert_eq!(bit.get_by_mask(Mask::new(3, 5)), 0x7);
        assert_eq!(bit.get_by_mask(Mask::new(3, 9)), 0x6);
    }
}
