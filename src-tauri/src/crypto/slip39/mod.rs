//! SLIP-0039 Shamir secret sharing.
//!
//! [The standard] splits a master secret into mnemonic word lists, any
//! `threshold` of which reconstruct it. NanaVault uses the simplest case: a
//! single `threshold`-of-`count` group, the extendable backup flag set, and an
//! **empty passphrase** — the share-recovery path must never require a
//! remembered secret, since the whole point of it is that the user forgot the
//! password.
//!
//! There is no production-grade SLIP-0039 crate for Rust, so this is a
//! from-scratch implementation. Its correctness does not rest on review alone:
//! [`combine`] is checked against the complete official test-vector suite (see
//! the tests at the bottom of this file).
//!
//! [The standard]: https://github.com/satoshilabs/slips/blob/master/slip-0039.md

mod feistel;
mod gf256;
mod mnemonic;
mod rs1024;
mod sss;
mod wordlist;

use zeroize::Zeroizing;

use mnemonic::Share;

/// The fixed generation parameters: a single 1-of-1 group wrapping the
/// `threshold`-of-`count` member split, and the fastest iteration exponent.
/// The derived key is already memory-hard (Argon2); the share layer only needs
/// to bind the shares together and carry the secret.
const ITERATION_EXPONENT: u8 = 0;
const GROUP_INDEX: u8 = 0;
const GROUP_THRESHOLD: u8 = 1;
const GROUP_COUNT: u8 = 1;
const APP_PASSPHRASE: &[u8] = b"";

pub type Result<T> = core::result::Result<T, Error>;

/// Why splitting or combining shares failed.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid sharing parameters: {0}")]
    InvalidParameters(String),
    #[error("a share contains a word that is not in the SLIP-0039 list: {0}")]
    UnknownWord(String),
    #[error("a share has an invalid length")]
    InvalidLength,
    #[error("a share has an invalid checksum")]
    InvalidChecksum,
    #[error("a share has invalid padding")]
    InvalidPadding,
    #[error("the shares do not belong to the same backup")]
    Mismatch,
    #[error("not the right number of shares to recover the secret")]
    InsufficientShares,
    #[error("two shares share the same index")]
    DuplicateIndex,
    #[error("the recovered secret failed its integrity digest; shares may be corrupt or forged")]
    DigestMismatch,
}

/// Split `secret` into `share_count` mnemonics, any `threshold` of which
/// reconstruct it.
pub fn split(secret: &[u8], threshold: u8, share_count: u8) -> Result<Vec<String>> {
    generate(secret, threshold, share_count, APP_PASSPHRASE)
}

/// Reconstruct a secret from a quorum of mnemonics.
pub fn combine(mnemonics: &[String]) -> Result<Zeroizing<Vec<u8>>> {
    recover(mnemonics, APP_PASSPHRASE)
}

fn generate(
    secret: &[u8],
    threshold: u8,
    share_count: u8,
    passphrase: &[u8],
) -> Result<Vec<String>> {
    if threshold == 0 || threshold > share_count || share_count > 16 {
        return Err(Error::InvalidParameters(format!(
            "need 1 ≤ threshold ≤ shares ≤ 16, got {threshold}-of-{share_count}"
        )));
    }

    let id = random_identifier()?;
    let encrypted = feistel::encrypt(secret, passphrase, ITERATION_EXPONENT, id, true);

    let members = sss::split_secret(threshold, share_count, &encrypted)?;
    Ok(members
        .into_iter()
        .map(|(member_index, share_value)| {
            mnemonic::encode(&Share {
                id,
                ext: true,
                e: ITERATION_EXPONENT,
                group_index: GROUP_INDEX,
                group_threshold: GROUP_THRESHOLD,
                group_count: GROUP_COUNT,
                member_index,
                member_threshold: threshold,
                share_value,
            })
        })
        .collect())
}

fn recover(mnemonics: &[String], passphrase: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
    if mnemonics.is_empty() {
        return Err(Error::InsufficientShares);
    }

    let shares: Vec<Share> = mnemonics
        .iter()
        .map(|m| mnemonic::decode(m))
        .collect::<Result<_>>()?;

    let first = &shares[0];
    let consistent = shares.iter().all(|s| {
        s.id == first.id
            && s.ext == first.ext
            && s.e == first.e
            && s.group_threshold == first.group_threshold
            && s.group_count == first.group_count
            && s.share_value.len() == first.share_value.len()
    });
    if !consistent || first.group_count < first.group_threshold {
        return Err(Error::Mismatch);
    }

    let encrypted = recover_encrypted_master_secret(&shares)?;
    let master = feistel::decrypt(&encrypted, passphrase, first.e, first.id, first.ext);
    Ok(Zeroizing::new(master))
}

/// Two-level recovery: rebuild each group share from its member shares, then
/// rebuild the encrypted master secret from the group shares.
fn recover_encrypted_master_secret(shares: &[Share]) -> Result<Vec<u8>> {
    use std::collections::BTreeMap;

    let group_threshold = shares[0].group_threshold;

    let mut by_group: BTreeMap<u8, Vec<&Share>> = BTreeMap::new();
    for share in shares {
        by_group.entry(share.group_index).or_default().push(share);
    }
    if by_group.len() as u8 != group_threshold {
        return Err(Error::InsufficientShares);
    }

    let mut group_shares: Vec<(u8, Vec<u8>)> = Vec::with_capacity(by_group.len());
    for (group_index, members) in by_group {
        group_shares.push((group_index, recover_group_share(&members)?));
    }

    sss::recover_secret(group_threshold, &group_shares)
}

fn recover_group_share(members: &[&Share]) -> Result<Vec<u8>> {
    let member_threshold = members[0].member_threshold;
    if members
        .iter()
        .any(|m| m.member_threshold != member_threshold)
    {
        return Err(Error::Mismatch);
    }
    if members.len() as u8 != member_threshold {
        return Err(Error::InsufficientShares);
    }

    let mut points: Vec<(u8, Vec<u8>)> = Vec::with_capacity(members.len());
    for member in members {
        if points
            .iter()
            .any(|(index, _)| *index == member.member_index)
        {
            return Err(Error::DuplicateIndex);
        }
        points.push((member.member_index, member.share_value.clone()));
    }

    sss::recover_secret(member_threshold, &points)
}

fn random_identifier() -> Result<u16> {
    let mut bytes = [0u8; 2];
    getrandom::getrandom(&mut bytes)
        .map_err(|e| Error::InvalidParameters(format!("random number generator failed: {e}")))?;
    Ok(u16::from_be_bytes(bytes) & 0x7fff)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The official vectors, embedded so the conformance test is hermetic.
    const VECTORS: &str = include_str!("test_vectors.json");
    /// The passphrase the official vectors are encrypted with.
    const TREZOR: &[u8] = b"TREZOR";

    #[test]
    fn passes_every_official_slip0039_vector() {
        let cases: Vec<(String, Vec<String>, String, String)> =
            serde_json::from_str(VECTORS).expect("the embedded vectors must parse");

        for (description, mnemonics, expected_hex, _xprv) in cases {
            let result = recover(&mnemonics, TREZOR);
            if expected_hex.is_empty() {
                assert!(
                    result.is_err(),
                    "expected '{description}' to fail, but it succeeded"
                );
            } else {
                let secret =
                    result.unwrap_or_else(|e| panic!("expected '{description}' to succeed: {e}"));
                assert_eq!(
                    hex::encode(&*secret),
                    expected_hex,
                    "wrong secret for '{description}'"
                );
            }
        }
    }

    #[test]
    fn any_quorum_reconstructs_the_secret() {
        let secret = [0x42u8; 32];
        let mnemonics = split(&secret, 2, 3).unwrap();

        for quorum in [[0, 1], [0, 2], [1, 2]] {
            let subset: Vec<String> = quorum.iter().map(|&i| mnemonics[i].clone()).collect();
            assert_eq!(&*combine(&subset).unwrap(), &secret);
        }
    }

    #[test]
    fn fewer_than_threshold_shares_cannot_recover() {
        let mnemonics = split(&[0x42u8; 32], 2, 3).unwrap();
        let single = vec![mnemonics[0].clone()];
        assert!(matches!(combine(&single), Err(Error::InsufficientShares)));
    }

    #[test]
    fn shares_from_different_backups_are_rejected() {
        let a = split(&[0x11u8; 32], 2, 3).unwrap();
        let b = split(&[0x22u8; 32], 2, 3).unwrap();
        let mixed = vec![a[0].clone(), b[1].clone()];
        assert!(matches!(combine(&mixed), Err(Error::Mismatch)));
    }
}
