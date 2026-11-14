use std::mem;

use wide::{u8x16, u16x8, u32x4, u64x2};

// TODO: relying on auto-vectorization for swapping bits is significantly faster
// than this code, we should figure out a way to make that work dynamically and
// then delete all this.

mod private {
    pub trait Sealed {}

    impl Sealed for u16 {}
    impl Sealed for u32 {}
    impl Sealed for u64 {}
    impl Sealed for i16 {}
    impl Sealed for i32 {}
    impl Sealed for i64 {}
    impl Sealed for f32 {}
    impl Sealed for f64 {}
}
pub trait SwappableNumber: private::Sealed {}
impl<T: private::Sealed> SwappableNumber for T {}

/// Swap the bytes of every 16-bit value in the given slice.
///
/// The values are swapped a whole vector at a time, with the leftover values at
/// the end of the slice being swapped one at a time.
#[inline]
fn swap_endianness_16bit(bytes: &mut [u8]) {
    let (chunks, remainder) = bytes.as_chunks_mut::<16>();
    for chunk in chunks {
        let values: u16x8 = bytemuck::cast(u8x16::new(*chunk));
        // swap the two bytes of every value
        let swapped: u16x8 = (values << 8u32) | (values >> 8u32);
        let swapped: u8x16 = bytemuck::cast(swapped);
        chunk.copy_from_slice(&swapped.to_array());
    }

    for value in remainder.as_chunks_mut::<2>().0 {
        value.reverse();
    }
}

/// Swap the bytes of every 32-bit value in the given slice.
#[inline]
fn swap_endianness_32bit(bytes: &mut [u8]) {
    let (chunks, remainder) = bytes.as_chunks_mut::<16>();
    for chunk in chunks {
        let values: u32x4 = bytemuck::cast(u8x16::new(*chunk));
        // reverse the four bytes of every value
        let swapped: u32x4 = ((values << 24u32) & 0xff00_0000)
            | ((values << 8u32) & 0x00ff_0000)
            | ((values >> 8u32) & 0x0000_ff00)
            | ((values >> 24u32) & 0x0000_00ff);
        let swapped: u8x16 = bytemuck::cast(swapped);
        chunk.copy_from_slice(&swapped.to_array());
    }

    for value in remainder.as_chunks_mut::<4>().0 {
        value.reverse();
    }
}

/// Swap the bytes of every 64-bit value in the given slice.
#[inline]
fn swap_endianness_64bit(bytes: &mut [u8]) {
    let (chunks, remainder) = bytes.as_chunks_mut::<16>();
    for chunk in chunks {
        let values: u64x2 = bytemuck::cast(u8x16::new(*chunk));
        // reverse the eight bytes of every value
        let swapped: u64x2 = ((values << 56u32) & 0xff00_0000_0000_0000)
            | ((values << 40u32) & 0x00ff_0000_0000_0000)
            | ((values << 24u32) & 0x0000_ff00_0000_0000)
            | ((values << 8u32) & 0x0000_00ff_0000_0000)
            | ((values >> 8u32) & 0x0000_0000_ff00_0000)
            | ((values >> 24u32) & 0x0000_0000_00ff_0000)
            | ((values >> 40u32) & 0x0000_0000_0000_ff00)
            | ((values >> 56u32) & 0x0000_0000_0000_00ff);
        let swapped: u8x16 = bytemuck::cast(swapped);
        chunk.copy_from_slice(&swapped.to_array());
    }

    for value in remainder.as_chunks_mut::<8>().0 {
        value.reverse();
    }
}

/// Swap the endianness of the given array (unless we're on a big-endian system)
/// in-place depending on the width of the given type.
fn swap_endianness_from_type<T: SwappableNumber>(items: &mut [u8]) {
    let item_width = mem::size_of::<T>();

    if cfg!(target_endian = "little") {
        match item_width {
            2 => swap_endianness_16bit(items),
            4 => swap_endianness_32bit(items),
            8 => swap_endianness_64bit(items),
            _ => panic!("unsupported size of type"),
        }
    }
}

/// Swaps the endianness of the given data and return it as a `Vec<u8>`.
#[inline]
pub fn swap_endianness_as_u8<T: SwappableNumber>(data: &[u8]) -> Vec<u8> {
    let mut items = data.to_vec();
    swap_endianness_from_type::<T>(&mut items);

    items
}

#[inline]
pub fn swap_endianness<T: SwappableNumber>(data: &[u8]) -> Vec<T> {
    let width_of_t = mem::size_of::<T>();
    let length_of_vec_t = data.len() / width_of_t;

    // the data must be a multiple of the item width, otherwise it's UB
    assert_eq!(data.len() % width_of_t, 0);

    // have the vec be of T initially so it's aligned
    let mut vec_t = Vec::<T>::with_capacity(length_of_vec_t);
    let mut vec_u8: Vec<u8> = {
        let ptr = vec_t.as_mut_ptr() as *mut u8;
        mem::forget(vec_t);
        // SAFETY: the new capacity is correct since we checked that data.len() is a
        // multiple of width_of_t
        unsafe { Vec::from_raw_parts(ptr, 0, data.len()) }
    };
    vec_u8.extend_from_slice(data);

    swap_endianness_from_type::<T>(&mut vec_u8);

    // now convert our Vec<u8> back to Vec<T>

    let ptr = vec_u8.as_mut_ptr() as *mut T;
    mem::forget(vec_u8);
    // SAFETY: The length won't be greater than the length of the original data
    unsafe { Vec::from_raw_parts(ptr, length_of_vec_t, length_of_vec_t) }
}

#[cfg(test)]
// swap_endianness_as_u8 only does anything on LE systems, so otherwise it'll
// error
#[cfg(target_endian = "little")]
mod tests {
    use super::*;

    #[test]
    fn test_swap_endianness_u16() {
        assert_eq!(
            swap_endianness_as_u8::<u16>(&[1, 2, 3, 4, 5, 6, 7, 8]),
            [2, 1, 4, 3, 6, 5, 8, 7]
        );
    }
    #[test]
    fn test_swap_endianness_u32() {
        assert_eq!(
            swap_endianness_as_u8::<u32>(&[1, 2, 3, 4, 5, 6, 7, 8]),
            [4, 3, 2, 1, 8, 7, 6, 5]
        );
    }
    #[test]
    fn test_swap_endianness_u64() {
        assert_eq!(
            swap_endianness_as_u8::<u64>(&[1, 2, 3, 4, 5, 6, 7, 8]),
            [8, 7, 6, 5, 4, 3, 2, 1]
        );
    }

    #[test]
    fn test_swap_endianness_u64_vec() {
        assert_eq!(
            swap_endianness::<u64>(&[1, 2, 3, 4, 5, 6, 7, 8, 8, 7, 6, 5, 4, 3, 2, 1]),
            vec![
                u64::from_le_bytes([8, 7, 6, 5, 4, 3, 2, 1]),
                u64::from_le_bytes([1, 2, 3, 4, 5, 6, 7, 8])
            ]
        )
    }

    #[test]
    fn test_swap_endianness_long() {
        // longer than a single vector, so it uses both the vector and the
        // leftover code paths
        let data: Vec<u8> = (0..37).collect();

        let swapped = swap_endianness_as_u8::<u16>(&data);
        for (i, value) in swapped.as_chunks::<2>().0.iter().enumerate() {
            assert_eq!(value, &[data[i * 2 + 1], data[i * 2]]);
        }

        let swapped = swap_endianness_as_u8::<u32>(&data);
        for (i, value) in swapped.as_chunks::<4>().0.iter().enumerate() {
            assert_eq!(
                value,
                &[
                    data[i * 4 + 3],
                    data[i * 4 + 2],
                    data[i * 4 + 1],
                    data[i * 4],
                ]
            );
        }

        let swapped = swap_endianness_as_u8::<u64>(&data);
        for (i, value) in swapped.as_chunks::<8>().0.iter().enumerate() {
            let expected: Vec<u8> = (0..8).rev().map(|j| data[i * 8 + j]).collect();
            assert_eq!(value, &expected[..]);
        }
    }
}
