//! Encryption of the master secret.
//!
//! Before it is shared, the master secret is run through a four-round Feistel
//! network whose round function is PBKDF2-HMAC-SHA256 (SLIP-0039 §"Encryption
//! of the master secret"). This is a wide-block pseudorandom permutation: it
//! ensures that learning part of fewer-than-threshold shares reveals nothing
//! about the secret, and it folds in the passphrase. Decryption is the same
//! network with the round order reversed.

use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;

const ROUNDS: [u8; 4] = [0, 1, 2, 3];
const BASE_ITERATIONS: u32 = 2500;

/// Encrypt the master secret into the encrypted master secret that is shared.
pub fn encrypt(master_secret: &[u8], passphrase: &[u8], e: u8, id: u16, ext: bool) -> Vec<u8> {
    let mut rounds = ROUNDS;
    network(master_secret, passphrase, e, id, ext, &mut rounds)
}

/// Recover the master secret from the encrypted master secret.
pub fn decrypt(encrypted: &[u8], passphrase: &[u8], e: u8, id: u16, ext: bool) -> Vec<u8> {
    let mut rounds = ROUNDS;
    rounds.reverse();
    network(encrypted, passphrase, e, id, ext, &mut rounds)
}

fn network(
    input: &[u8],
    passphrase: &[u8],
    e: u8,
    id: u16,
    ext: bool,
    rounds: &mut [u8],
) -> Vec<u8> {
    let half = input.len() / 2;
    let mut left = input[..half].to_vec();
    let mut right = input[half..].to_vec();
    let salt_prefix = salt_prefix(id, ext);

    for &round in rounds.iter() {
        let f = round_function(round, passphrase, e, &salt_prefix, &right);
        let next_right: Vec<u8> = left.iter().zip(&f).map(|(a, b)| a ^ b).collect();
        left = std::mem::replace(&mut right, next_right);
    }

    // The result is R ‖ L.
    let mut output = right;
    output.extend_from_slice(&left);
    output
}

fn round_function(
    round: u8,
    passphrase: &[u8],
    e: u8,
    salt_prefix: &[u8],
    right: &[u8],
) -> Vec<u8> {
    let mut password = Vec::with_capacity(1 + passphrase.len());
    password.push(round);
    password.extend_from_slice(passphrase);

    let mut salt = Vec::with_capacity(salt_prefix.len() + right.len());
    salt.extend_from_slice(salt_prefix);
    salt.extend_from_slice(right);

    let mut output = vec![0u8; right.len()];
    pbkdf2_hmac::<Sha256>(&password, &salt, BASE_ITERATIONS << e, &mut output);
    output
}

fn salt_prefix(id: u16, ext: bool) -> Vec<u8> {
    if ext {
        // Extendable backups use no identifier salt.
        Vec::new()
    } else {
        let mut prefix = b"shamir".to_vec();
        prefix.extend_from_slice(&id.to_be_bytes());
        prefix
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decrypt_inverts_encrypt() {
        let secret = [0x33u8; 32];
        for ext in [false, true] {
            let encrypted = encrypt(&secret, b"TREZOR", 0, 0x1234, ext);
            assert_ne!(encrypted, secret, "encryption should not be the identity");
            assert_eq!(decrypt(&encrypted, b"TREZOR", 0, 0x1234, ext), secret);
        }
    }

    #[test]
    fn the_wrong_passphrase_yields_a_different_secret() {
        let secret = [0x33u8; 32];
        let encrypted = encrypt(&secret, b"TREZOR", 0, 0x1234, true);
        assert_ne!(decrypt(&encrypted, b"wrong", 0, 0x1234, true), secret);
    }
}
