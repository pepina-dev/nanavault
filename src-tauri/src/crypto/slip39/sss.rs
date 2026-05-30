//! Shamir's secret sharing over GF(256), in the SLIP-0039 dialect: the secret
//! lives at f(255) and a digest of it at f(254), so a reconstruction can be
//! checked for integrity rather than blindly trusted.

use hmac::{Hmac, Mac};
use sha2::Sha256;

use super::gf256;
use super::{Error, Result};

const SECRET_INDEX: u8 = 255;
const DIGEST_INDEX: u8 = 254;
const DIGEST_LEN: usize = 4;

/// Split `secret` into `count` shares at x-indices `0..count`, any `threshold`
/// of which reconstruct it.
pub fn split_secret(threshold: u8, count: u8, secret: &[u8]) -> Result<Vec<(u8, Vec<u8>)>> {
    validate(threshold, count, secret)?;

    if threshold == 1 {
        return Ok((0..count).map(|i| (i, secret.to_vec())).collect());
    }

    let n = secret.len();

    // `threshold - 2` random shares, plus the digest at 254 and the secret at
    // 255, fully determine the degree `threshold - 1` polynomial.
    let mut base: Vec<(u8, Vec<u8>)> = Vec::with_capacity(threshold as usize);
    for x in 0..threshold - 2 {
        base.push((x, random_bytes(n)?));
    }

    let random = random_bytes(n - DIGEST_LEN)?;
    let mut digest_share = digest(&random, secret).to_vec();
    digest_share.extend_from_slice(&random);
    base.push((DIGEST_INDEX, digest_share));
    base.push((SECRET_INDEX, secret.to_vec()));

    let mut shares: Vec<(u8, Vec<u8>)> = base[..(threshold - 2) as usize].to_vec();
    for x in (threshold - 2)..count {
        shares.push((x, gf256::interpolate(x, &base)));
    }
    Ok(shares)
}

/// Reconstruct the secret from exactly `threshold` index/value pairs and verify
/// its digest.
pub fn recover_secret(threshold: u8, shares: &[(u8, Vec<u8>)]) -> Result<Vec<u8>> {
    if threshold == 1 {
        return shares
            .first()
            .map(|(_, value)| value.clone())
            .ok_or(Error::InsufficientShares);
    }

    let secret = gf256::interpolate(SECRET_INDEX, shares);
    let digest_share = gf256::interpolate(DIGEST_INDEX, shares);

    let (claimed, random) = digest_share.split_at(DIGEST_LEN);
    if &digest(random, &secret)[..] != claimed {
        return Err(Error::DigestMismatch);
    }
    Ok(secret)
}

fn validate(threshold: u8, count: u8, secret: &[u8]) -> Result<()> {
    if threshold == 0 || threshold > count || count > 16 {
        return Err(Error::InvalidParameters(format!(
            "need 1 ≤ threshold ≤ shares ≤ 16, got {threshold}-of-{count}"
        )));
    }
    if secret.len() < 16 || !secret.len().is_multiple_of(2) {
        return Err(Error::InvalidParameters(
            "secret must be at least 128 bits and a whole number of 16-bit units".into(),
        ));
    }
    Ok(())
}

fn digest(key: &[u8], message: &[u8]) -> [u8; DIGEST_LEN] {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC accepts a key of any length");
    mac.update(message);
    let tag = mac.finalize().into_bytes();
    tag[..DIGEST_LEN]
        .try_into()
        .expect("SHA-256 output is longer than the digest")
}

fn random_bytes(len: usize) -> Result<Vec<u8>> {
    let mut buffer = vec![0u8; len];
    getrandom::getrandom(&mut buffer)
        .map_err(|e| Error::InvalidParameters(format!("random number generator failed: {e}")))?;
    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_for_a_range_of_thresholds() {
        let secret = [0x5au8; 32];
        for (threshold, count) in [(1u8, 1u8), (2, 3), (3, 5), (5, 5)] {
            let shares = split_secret(threshold, count, &secret).unwrap();
            let quorum = &shares[..threshold as usize];
            assert_eq!(recover_secret(threshold, quorum).unwrap(), secret);
        }
    }

    #[test]
    fn a_tampered_share_fails_the_digest() {
        let secret = [0x5au8; 32];
        let mut shares = split_secret(2, 3, &secret).unwrap();
        shares[0].1[0] ^= 0xff;
        assert!(matches!(
            recover_secret(2, &shares[..2]),
            Err(Error::DigestMismatch)
        ));
    }
}
