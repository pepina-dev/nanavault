//! The SLIP-0039 word list: 1024 words, embedded in the binary, mapping each
//! 10-bit value to a word and back. The list is alphabetically sorted, so the
//! reverse lookup is a binary search.

use std::sync::OnceLock;

const RAW: &str = include_str!("wordlist.txt");

/// Number of words a 10-bit index can address.
pub const WORD_COUNT: usize = 1024;

fn words() -> &'static [&'static str] {
    static WORDS: OnceLock<Vec<&'static str>> = OnceLock::new();
    WORDS.get_or_init(|| {
        let list: Vec<&'static str> = RAW
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        assert_eq!(
            list.len(),
            WORD_COUNT,
            "the SLIP-0039 word list must contain exactly 1024 words"
        );
        list
    })
}

/// The word for a 10-bit index, or `None` if the index is out of range.
pub fn word(index: u16) -> Option<&'static str> {
    words().get(index as usize).copied()
}

/// The 10-bit index of a word, or `None` if the word is not in the list.
pub fn index(word: &str) -> Option<u16> {
    words().binary_search(&word).ok().map(|i| i as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_is_sorted_and_complete() {
        assert_eq!(words().len(), WORD_COUNT);
        assert!(words().windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn lookups_are_inverses() {
        for i in 0..WORD_COUNT as u16 {
            let w = word(i).unwrap();
            assert_eq!(index(w), Some(i));
        }
        assert_eq!(word(1024), None);
        assert_eq!(index("definitelynotaword"), None);
    }
}
