//! The RS1024 checksum: a Reed-Solomon code over GF(1024) that protects the
//! last three words of every SLIP-0039 mnemonic. It is guaranteed to detect any
//! three-word error. The customization string differs for extendable backups,
//! which is why callers pass the `ext` flag.
//!
//! The algorithm is transcribed directly from the Python reference in the spec.

const GENERATOR: [u32; 10] = [
    0x00e0_e040,
    0x01c1_c080,
    0x0383_8100,
    0x0707_0200,
    0x0e0e_0009,
    0x1c0c_2412,
    0x3808_6c24,
    0x3090_fc48,
    0x21b1_f890,
    0x03f3_f120,
];

const CHECKSUM_WORDS: usize = 3;

fn polymod(values: impl Iterator<Item = u32>) -> u32 {
    let mut checksum = 1u32;
    for value in values {
        let top = checksum >> 20;
        checksum = ((checksum & 0xf_ffff) << 10) ^ value;
        for (bit, generator) in GENERATOR.iter().enumerate() {
            if (top >> bit) & 1 != 0 {
                checksum ^= generator;
            }
        }
    }
    checksum
}

fn customization(ext: bool) -> &'static [u8] {
    if ext {
        b"shamir_extendable"
    } else {
        b"shamir"
    }
}

/// The three checksum words for a data part (the mnemonic's words minus the
/// checksum).
pub fn create(ext: bool, data: &[u16]) -> [u16; CHECKSUM_WORDS] {
    let values = customization(ext)
        .iter()
        .map(|&c| c as u32)
        .chain(data.iter().map(|&word| word as u32))
        .chain([0, 0, 0]);
    let polymod = polymod(values) ^ 1;
    [
        ((polymod >> 20) & 1023) as u16,
        ((polymod >> 10) & 1023) as u16,
        (polymod & 1023) as u16,
    ]
}

/// Whether the words (data part *followed by* the three checksum words) carry a
/// valid checksum.
pub fn verify(ext: bool, words: &[u16]) -> bool {
    let values = customization(ext)
        .iter()
        .map(|&c| c as u32)
        .chain(words.iter().map(|&word| word as u32));
    polymod(values) == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_created_checksum_verifies() {
        for ext in [false, true] {
            let data = [1u16, 2, 3, 1000, 512, 7];
            let checksum = create(ext, &data);

            let mut words = data.to_vec();
            words.extend_from_slice(&checksum);
            assert!(verify(ext, &words));
        }
    }

    #[test]
    fn a_single_word_error_is_detected() {
        let data = [1u16, 2, 3, 1000, 512, 7];
        let checksum = create(false, &data);
        let mut words = data.to_vec();
        words.extend_from_slice(&checksum);

        words[2] ^= 1;
        assert!(!verify(false, &words));
    }

    #[test]
    fn the_wrong_customization_string_is_rejected() {
        let data = [9u16, 8, 7];
        let checksum = create(false, &data);
        let mut words = data.to_vec();
        words.extend_from_slice(&checksum);

        assert!(!verify(true, &words));
    }
}
