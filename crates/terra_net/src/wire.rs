//! The wire (D636): a bit packer and the [`Wire`] trait every part, intent and operation is
//! written and read by. Ported from Glenn Fiedler's bit packer ("Reading and Writing
//! Packets", gafferongames.com; his yojimbo's `serialize`, BSD-3-Clause, Copyright (c) 2015 -
//! 2024, Mas Bandwidth LLC): values are packed least significant bit first, with no padding
//! but before a run of bytes.
//!
//! What arrives is a stranger's (D51), so the [`Reader`] is total: any bytes give a value or a
//! [`Malformed`], never a panic, and a length is refused past [`MOST_RUN`] before anything is
//! taken for it.

/// Why bytes could not be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Malformed {
    /// The bytes ended before the value did.
    Short,
    /// A value outside what its field allows.
    Range,
    /// A length past the cap, or a varint past ten bytes.
    Long,
}

impl std::fmt::Display for Malformed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Malformed::Short => "the bytes end before the value does",
            Malformed::Range => "a value outside its field's range",
            Malformed::Long => "a length past the wire's cap",
        })
    }
}

impl std::error::Error for Malformed {}

/// The longest run of bytes a length may promise: no message is larger.
pub const MOST_RUN: usize = 64 * 1024;

/// Something written to and read from the wire.
pub trait Wire: Sized {
    fn write(&self, to: &mut Writer);
    fn read(from: &mut Reader) -> Result<Self, Malformed>;

    /// It as bytes of its own.
    fn encoded(&self) -> Vec<u8> {
        let mut writer = Writer::default();
        self.write(&mut writer);
        writer.finish()
    }

    /// It from bytes holding nothing else.
    fn decoded(bytes: &[u8]) -> Result<Self, Malformed> {
        Self::read(&mut Reader::new(bytes))
    }
}

/// How many bits hold a number up to `most`.
pub const fn bits_for(most: u32) -> u32 {
    32 - most.leading_zeros()
}

/// Packs values into bytes.
#[derive(Default)]
pub struct Writer {
    bytes: Vec<u8>,
    /// Bits not yet a whole byte, the low `held` of it.
    scratch: u64,
    held: u32,
}

impl Writer {
    /// A writer into a buffer kept from before, emptied.
    pub fn reusing(mut bytes: Vec<u8>) -> Writer {
        bytes.clear();
        Writer {
            bytes,
            scratch: 0,
            held: 0,
        }
    }

    /// The low `count` bits of a value, at most 64.
    pub fn bits(&mut self, value: u64, count: u32) {
        debug_assert!(count <= 64);
        if count > 32 {
            self.put(value & 0xFFFF_FFFF, 32);
            self.put((value >> 32) & ((1 << (count - 32)) - 1), count - 32);
        } else if count > 0 {
            self.put(value & ((1 << count) - 1), count);
        }
    }

    /// At most 32 bits, none set above `count`: with under a byte held they fit the scratch.
    fn put(&mut self, value: u64, count: u32) {
        self.scratch |= value << self.held;
        self.held += count;
        while self.held >= 8 {
            self.bytes.push(self.scratch as u8);
            self.scratch >>= 8;
            self.held -= 8;
        }
    }

    pub fn bool(&mut self, value: bool) {
        self.put(u64::from(value), 1);
    }

    /// A number from 0 to `most`, in as few bits as hold `most`.
    pub fn ranged(&mut self, value: u32, most: u32) {
        debug_assert!(value <= most);
        self.bits(u64::from(value.min(most)), bits_for(most));
    }

    /// A number in seven bits a byte, small ones short (LEB128).
    pub fn varint(&mut self, mut value: u64) {
        loop {
            let low = value & 0x7F;
            value >>= 7;
            self.put(low | (u64::from(value != 0) << 7), 8);
            if value == 0 {
                break;
            }
        }
    }

    /// A value between two bounds to a number of bits: its nearest step, the bounds exact.
    pub fn quantised(&mut self, value: f32, min: f32, max: f32, bits: u32) {
        debug_assert!(bits <= 32 && max > min);
        let steps = ((1u64 << bits) - 1) as f64;
        let share = (f64::from(value) - f64::from(min)) / (f64::from(max) - f64::from(min));
        self.bits((share.clamp(0.0, 1.0) * steps + 0.5) as u64, bits);
    }

    /// Fills the byte under way with zeros.
    pub fn align(&mut self) {
        if self.held > 0 {
            self.bytes.push(self.scratch as u8);
            self.scratch = 0;
            self.held = 0;
        }
    }

    /// A run of bytes with its length, on a byte's edge so it is copied whole.
    pub fn run(&mut self, bytes: &[u8]) {
        debug_assert!(bytes.len() <= MOST_RUN);
        self.varint(bytes.len() as u64);
        self.align();
        self.bytes.extend_from_slice(bytes);
    }

    /// How many bytes it would be if finished now.
    pub fn len(&self) -> usize {
        self.bytes.len() + usize::from(self.held > 0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn finish(mut self) -> Vec<u8> {
        self.align();
        self.bytes
    }
}

/// Reads what a [`Writer`] packed, trusting nothing.
pub struct Reader<'a> {
    bytes: &'a [u8],
    /// The next bit.
    at: usize,
}

impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Reader<'a> {
        Reader { bytes, at: 0 }
    }

    /// The next `count` bits, at most 64.
    pub fn bits(&mut self, count: u32) -> Result<u64, Malformed> {
        debug_assert!(count <= 64);
        if self.at + count as usize > self.bytes.len() * 8 {
            return Err(Malformed::Short);
        }
        let (mut value, mut got) = (0u64, 0u32);
        while got < count {
            let skip = (self.at & 7) as u32;
            let take = (8 - skip).min(count - got);
            let byte = u64::from(self.bytes[self.at >> 3]);
            value |= ((byte >> skip) & ((1 << take) - 1)) << got;
            got += take;
            self.at += take as usize;
        }
        Ok(value)
    }

    pub fn bool(&mut self) -> Result<bool, Malformed> {
        Ok(self.bits(1)? == 1)
    }

    /// A number from 0 to `most`; one past it is refused.
    pub fn ranged(&mut self, most: u32) -> Result<u32, Malformed> {
        let value = self.bits(bits_for(most))? as u32;
        if value > most {
            return Err(Malformed::Range);
        }
        Ok(value)
    }

    pub fn varint(&mut self) -> Result<u64, Malformed> {
        let mut value = 0u64;
        for group in 0..10 {
            let byte = self.bits(8)?;
            // The tenth byte holds the 64th bit alone.
            if group == 9 && byte > 1 {
                return Err(Malformed::Long);
            }
            value |= (byte & 0x7F) << (7 * group);
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(Malformed::Long)
    }

    pub fn quantised(&mut self, min: f32, max: f32, bits: u32) -> Result<f32, Malformed> {
        let steps = ((1u64 << bits) - 1) as f64;
        let share = self.bits(bits)? as f64 / steps;
        Ok((f64::from(min) + share * (f64::from(max) - f64::from(min))) as f32)
    }

    pub fn align(&mut self) {
        self.at = (self.at + 7) & !7;
    }

    /// A run of bytes with its length, refused past [`MOST_RUN`] or the bytes left.
    pub fn run(&mut self) -> Result<&'a [u8], Malformed> {
        let length = self.varint()?;
        if length > MOST_RUN as u64 {
            return Err(Malformed::Long);
        }
        self.align();
        let start = self.at >> 3;
        let end = start + length as usize;
        if end > self.bytes.len() {
            return Err(Malformed::Short);
        }
        self.at = end * 8;
        Ok(&self.bytes[start..end])
    }

    /// Whether a whole byte or more is still to read: padding is under a byte.
    pub fn more(&self) -> bool {
        self.at + 8 <= self.bytes.len() * 8
    }
}

macro_rules! wire_bits {
    ($($kind:ty => $bits:expr),*) => {$(
        impl Wire for $kind {
            fn write(&self, to: &mut Writer) {
                to.bits(u64::from(*self), $bits);
            }
            fn read(from: &mut Reader) -> Result<Self, Malformed> {
                Ok(from.bits($bits)? as $kind)
            }
        }
    )*};
}
wire_bits!(u8 => 8, u16 => 16, u32 => 32, u64 => 64);

impl Wire for bool {
    fn write(&self, to: &mut Writer) {
        to.bool(*self);
    }
    fn read(from: &mut Reader) -> Result<Self, Malformed> {
        from.bool()
    }
}

impl Wire for i32 {
    fn write(&self, to: &mut Writer) {
        to.bits(u64::from(*self as u32), 32);
    }
    fn read(from: &mut Reader) -> Result<Self, Malformed> {
        Ok(from.bits(32)? as u32 as i32)
    }
}

impl Wire for f32 {
    fn write(&self, to: &mut Writer) {
        to.bits(u64::from(self.to_bits()), 32);
    }
    fn read(from: &mut Reader) -> Result<Self, Malformed> {
        Ok(f32::from_bits(from.bits(32)? as u32))
    }
}

impl Wire for f64 {
    fn write(&self, to: &mut Writer) {
        to.bits(self.to_bits(), 64);
    }
    fn read(from: &mut Reader) -> Result<Self, Malformed> {
        Ok(f64::from_bits(from.bits(64)?))
    }
}

impl<T: Wire, const N: usize> Wire for [T; N] {
    fn write(&self, to: &mut Writer) {
        for item in self {
            item.write(to);
        }
    }
    fn read(from: &mut Reader) -> Result<Self, Malformed> {
        let mut items = Vec::with_capacity(N);
        for _ in 0..N {
            items.push(T::read(from)?);
        }
        items.try_into().map_err(|_| Malformed::Short)
    }
}

impl Wire for Vec<u8> {
    fn write(&self, to: &mut Writer) {
        to.run(self);
    }
    fn read(from: &mut Reader) -> Result<Self, Malformed> {
        Ok(from.run()?.to_vec())
    }
}

impl Wire for String {
    fn write(&self, to: &mut Writer) {
        to.run(self.as_bytes());
    }
    fn read(from: &mut Reader) -> Result<Self, Malformed> {
        String::from_utf8(from.run()?.to_vec()).map_err(|_| Malformed::Range)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_gen::hash::splitmix64;

    #[test]
    fn what_is_written_is_read_back() {
        let mut writer = Writer::default();
        writer.bool(true);
        writer.ranged(5, 6);
        writer.varint(300);
        writer.varint(u64::MAX);
        writer.bits(0x1_2345_6789_ABCD, 49);
        writer.quantised(0.25, -1.0, 1.0, 12);
        writer.run(b"terra");
        (-7i32).write(&mut writer);
        [1.5f64, -2.5, 1.0e9].write(&mut writer);
        "troll".to_string().write(&mut writer);
        let bytes = writer.finish();
        let mut reader = Reader::new(&bytes);
        assert!(reader.bool().unwrap());
        assert_eq!(reader.ranged(6).unwrap(), 5);
        assert_eq!(reader.varint().unwrap(), 300);
        assert_eq!(reader.varint().unwrap(), u64::MAX);
        assert_eq!(reader.bits(49).unwrap(), 0x1_2345_6789_ABCD);
        // A step of 12 bits over 2 is 0.0005: within half of it.
        assert!((reader.quantised(-1.0, 1.0, 12).unwrap() - 0.25).abs() < 0.00025);
        assert_eq!(reader.run().unwrap(), b"terra");
        assert_eq!(i32::read(&mut reader).unwrap(), -7);
        assert_eq!(<[f64; 3]>::read(&mut reader).unwrap(), [1.5, -2.5, 1.0e9]);
        assert_eq!(String::read(&mut reader).unwrap(), "troll");
        assert!(!reader.more());
    }

    #[test]
    fn a_reader_refuses_rather_than_trusts() {
        assert_eq!(Reader::new(&[]).bits(1), Err(Malformed::Short));
        // 7 of 0..=6, in three bits.
        assert_eq!(Reader::new(&[0b111]).ranged(6), Err(Malformed::Range));
        // A varint that never ends, and one whose tenth byte overflows 64 bits.
        assert_eq!(Reader::new(&[0xFF; 16]).varint(), Err(Malformed::Long));
        // A run promising more than the cap, then more than is left.
        let mut long = Writer::default();
        long.varint(MOST_RUN as u64 + 1);
        assert_eq!(
            Reader::new(&long.finish()).run().err(),
            Some(Malformed::Long)
        );
        assert_eq!(
            Reader::new(&[200, 1, 2]).run().err(),
            Some(Malformed::Short)
        );
        assert_eq!(String::decoded(&[2, 0xFF, 0xFE]), Err(Malformed::Range));
    }

    /// Every read of random bytes ends in a value or an error: the test is that none panics.
    #[test]
    fn random_bytes_never_panic() {
        for seed in 0..2_000u64 {
            let length = (splitmix64(seed) % 40) as usize;
            let bytes: Vec<u8> = (0..length)
                .map(|i| splitmix64(seed * 131 + i as u64) as u8)
                .collect();
            let mut reader = Reader::new(&bytes);
            for step in 0..64u64 {
                let _ = match splitmix64(seed ^ (step << 32)) % 6 {
                    0 => reader.bool().map(|_| ()),
                    1 => reader.ranged(1000).map(|_| ()),
                    2 => reader.varint().map(|_| ()),
                    3 => reader.run().map(|_| ()),
                    4 => String::read(&mut reader).map(|_| ()),
                    _ => reader.bits(64).map(|_| ()),
                };
            }
        }
    }
}
