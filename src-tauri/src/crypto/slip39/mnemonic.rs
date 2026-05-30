//! The share mnemonic: the bit-level packing of a [`Share`] into 10-bit words,
//! the RS1024 checksum, and the word-list encoding — and the exact inverse.
//!
//! Field layout (big-endian throughout), per `slip-0039.md`:
//!
//! ```text
//! id(15) ext(1) e(4) group_index(4) group_threshold-1(4) group_count-1(4)
//! member_index(4) member_threshold-1(4) [padding] share_value(8·n) checksum(30)
//! ```
//!
//! The share value is left-padded with zero bits so the whole data part is a
//! multiple of 10 bits.

use super::{rs1024, wordlist, Error, Result};

/// The shortest valid mnemonic (a 128-bit secret): 20 words.
const MIN_WORDS: usize = 20;
/// The checksum occupies the final three words.
const CHECKSUM_WORDS: usize = 3;

/// A single decoded SLIP-0039 share. Thresholds and counts are stored as their
/// natural values (e.g. a 2-of-3 share has `member_threshold == 2`); the
/// minus-one encoding lives only in the bit packing.
pub struct Share {
    pub id: u16,
    pub ext: bool,
    pub e: u8,
    pub group_index: u8,
    pub group_threshold: u8,
    pub group_count: u8,
    pub member_index: u8,
    pub member_threshold: u8,
    pub share_value: Vec<u8>,
}

/// Encode a share as its space-separated mnemonic.
pub fn encode(share: &Share) -> String {
    let mut writer = BitWriter::new();
    writer.write(share.id as u32, 15);
    writer.write(share.ext as u32, 1);
    writer.write(share.e as u32, 4);
    writer.write(share.group_index as u32, 4);
    writer.write((share.group_threshold - 1) as u32, 4);
    writer.write((share.group_count - 1) as u32, 4);
    writer.write(share.member_index as u32, 4);
    writer.write((share.member_threshold - 1) as u32, 4);

    let value_bits = share.share_value.len() * 8;
    let padding = (10 - value_bits % 10) % 10;
    writer.write(0, padding as u32);
    for &byte in &share.share_value {
        writer.write(byte as u32, 8);
    }

    let data_words = writer.into_words();
    let checksum = rs1024::create(share.ext, &data_words);

    data_words
        .iter()
        .chain(checksum.iter())
        .map(|&w| wordlist::word(w).expect("a 10-bit index always maps to a word"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Decode and checksum-validate a mnemonic into a [`Share`].
pub fn decode(mnemonic: &str) -> Result<Share> {
    let words: Vec<&str> = mnemonic.split_whitespace().collect();
    if words.len() < MIN_WORDS {
        return Err(Error::InvalidLength);
    }

    let indices = words
        .iter()
        .map(|w| wordlist::index(w).ok_or_else(|| Error::UnknownWord((*w).to_string())))
        .collect::<Result<Vec<u16>>>()?;

    let data_words = &indices[..indices.len() - CHECKSUM_WORDS];
    let mut reader = BitReader::from_words(data_words);

    let id = reader.read(15)? as u16;
    let ext = reader.read(1)? == 1;

    // The customization string depends on `ext`, so the flag must be read
    // before the checksum can be checked.
    if !rs1024::verify(ext, &indices) {
        return Err(Error::InvalidChecksum);
    }

    let e = reader.read(4)? as u8;
    let group_index = reader.read(4)? as u8;
    let group_threshold = reader.read(4)? as u8 + 1;
    let group_count = reader.read(4)? as u8 + 1;
    let member_index = reader.read(4)? as u8;
    let member_threshold = reader.read(4)? as u8 + 1;

    let remaining = reader.remaining();
    let padding = remaining % 16;
    if padding > 8 {
        return Err(Error::InvalidPadding);
    }
    if reader.read(padding as u32)? != 0 {
        return Err(Error::InvalidPadding);
    }

    let value_bits = remaining - padding;
    if value_bits < 128 || !value_bits.is_multiple_of(8) {
        return Err(Error::InvalidLength);
    }
    let mut share_value = Vec::with_capacity(value_bits / 8);
    for _ in 0..value_bits / 8 {
        share_value.push(reader.read(8)? as u8);
    }

    Ok(Share {
        id,
        ext,
        e,
        group_index,
        group_threshold,
        group_count,
        member_index,
        member_threshold,
        share_value,
    })
}

/// Appends values bit-by-bit, most-significant bit first.
struct BitWriter {
    bits: Vec<bool>,
}

impl BitWriter {
    fn new() -> Self {
        Self { bits: Vec::new() }
    }

    fn write(&mut self, value: u32, width: u32) {
        for shift in (0..width).rev() {
            self.bits.push((value >> shift) & 1 == 1);
        }
    }

    fn into_words(self) -> Vec<u16> {
        debug_assert_eq!(
            self.bits.len() % 10,
            0,
            "data part must be a whole number of words"
        );
        self.bits
            .chunks(10)
            .map(|chunk| {
                chunk
                    .iter()
                    .fold(0u16, |word, &bit| (word << 1) | bit as u16)
            })
            .collect()
    }
}

/// Reads values bit-by-bit, most-significant bit first.
struct BitReader {
    bits: Vec<bool>,
    position: usize,
}

impl BitReader {
    fn from_words(words: &[u16]) -> Self {
        let mut bits = Vec::with_capacity(words.len() * 10);
        for &word in words {
            for shift in (0..10).rev() {
                bits.push((word >> shift) & 1 == 1);
            }
        }
        Self { bits, position: 0 }
    }

    fn read(&mut self, width: u32) -> Result<u32> {
        if self.position + width as usize > self.bits.len() {
            return Err(Error::InvalidLength);
        }
        let mut value = 0u32;
        for _ in 0..width {
            value = (value << 1) | self.bits[self.position] as u32;
            self.position += 1;
        }
        Ok(value)
    }

    fn remaining(&self) -> usize {
        self.bits.len() - self.position
    }
}
