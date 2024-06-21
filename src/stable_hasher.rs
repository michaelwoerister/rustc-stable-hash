//! Stable hasher adapted for cross-platform independent hash.

use crate::sip128::SipHasher128;

use std::fmt::Debug;
use std::hash::Hasher;

#[cfg(test)]
mod tests;

/// Trait for retrieving the result of the stable hashing operation.
pub trait StableHasherResult<const W: usize>: Sized {
    fn finish(hash: [u64; W]) -> Self;
}

pub trait HashImpl<const W: usize>: Hasher + Debug + Default {
    fn short_write<const LEN: usize>(&mut self, bytes: [u8; LEN]);
    fn finish(self) -> [u64; W];
}

/// When hashing something that ends up affecting properties like symbol names,
/// we want these symbol names to be calculated independently of other factors
/// like what architecture you're compiling *from*.
///
/// To that end we always convert integers to little-endian format before
/// hashing and the architecture dependent `isize` and `usize` types are
/// extended to 64 bits if needed.
#[derive(Debug)]
pub struct StableHasher<const W: usize, H: HashImpl<W>> {
    state: H,
}

impl HashImpl<2> for SipHasher128 {
    fn finish(self) -> [u64; 2] {
        self.finish128()
    }

    fn short_write<const LEN: usize>(&mut self, bytes: [u8; LEN]) {
        self.short_write(bytes);
    }
}

impl<const W: usize, H: HashImpl<W>> StableHasher<W, H> {
    #[inline]
    pub fn new() -> Self {
        StableHasher {
            state: Default::default(),
        }
    }

    #[inline]
    pub fn finish<R: StableHasherResult<W>>(self) -> R {
        R::finish(self.state.finish())
    }
}

impl<const W: usize, H: HashImpl<W>> Hasher for StableHasher<W, H> {
    fn finish(&self) -> u64 {
        panic!("use StableHasher::finalize instead");
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        self.state.write(bytes);
    }

    #[cfg(feature = "nightly")]
    #[inline]
    fn write_str(&mut self, s: &str) {
        self.state.write_str(s);
    }

    #[cfg(feature = "nightly")]
    #[inline]
    fn write_length_prefix(&mut self, len: usize) {
        // Our impl for `usize` will extend it if needed.
        self.write_usize(len);
    }

    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.state.write_u8(i);
    }

    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.state.short_write(i.to_le_bytes());
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.state.short_write(i.to_le_bytes());
    }

    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.state.short_write(i.to_le_bytes());
    }

    #[inline]
    fn write_u128(&mut self, i: u128) {
        self.write_u64(i as u64);
        self.write_u64((i >> 64) as u64);
    }

    #[inline]
    fn write_usize(&mut self, i: usize) {
        // Always treat usize as u64 so we get the same results on 32 and 64 bit
        // platforms. This is important for symbol hashes when cross compiling,
        // for example.
        self.state.short_write((i as u64).to_le_bytes());
    }

    #[inline]
    fn write_i8(&mut self, i: i8) {
        self.state.write_i8(i);
    }

    #[inline]
    fn write_i16(&mut self, i: i16) {
        self.state.short_write((i as u16).to_le_bytes());
    }

    #[inline]
    fn write_i32(&mut self, i: i32) {
        self.state.short_write((i as u32).to_le_bytes());
    }

    #[inline]
    fn write_i64(&mut self, i: i64) {
        self.state.short_write((i as u64).to_le_bytes());
    }

    #[inline]
    fn write_i128(&mut self, i: i128) {
        self.state.write(&(i as u128).to_le_bytes());
    }

    #[inline]
    fn write_isize(&mut self, i: isize) {
        // Always treat isize as a 64-bit number so we get the same results on 32 and 64 bit
        // platforms. This is important for symbol hashes when cross compiling,
        // for example. Sign extending here is preferable as it means that the
        // same negative number hashes the same on both 32 and 64 bit platforms.
        let value = i as u64;

        // Cold path
        #[cold]
        #[inline(never)]
        fn hash_value<const W: usize, H: HashImpl<W>>(state: &mut H, value: u64) {
            state.write_u8(0xFF);
            state.short_write(value.to_le_bytes());
        }

        // `isize` values often seem to have a small (positive) numeric value in practice.
        // To exploit this, if the value is small, we will hash a smaller amount of bytes.
        // However, we cannot just skip the leading zero bytes, as that would produce the same hash
        // e.g. if you hash two values that have the same bit pattern when they are swapped.
        // See https://github.com/rust-lang/rust/pull/93014 for context.
        //
        // Therefore, we employ the following strategy:
        // 1) When we encounter a value that fits within a single byte (the most common case), we
        // hash just that byte. This is the most common case that is being optimized. However, we do
        // not do this for the value 0xFF, as that is a reserved prefix (a bit like in UTF-8).
        // 2) When we encounter a larger value, we hash a "marker" 0xFF and then the corresponding
        // 8 bytes. Since this prefix cannot occur when we hash a single byte, when we hash two
        // `isize`s that fit within a different amount of bytes, they should always produce a different
        // byte stream for the hasher.
        if value < 0xFF {
            self.state.write_u8(value as u8);
        } else {
            hash_value(&mut self.state, value);
        }
    }
}
