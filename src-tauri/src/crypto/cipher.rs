//! File encryption and the on-disk blob format.
//!
//! Files are encrypted with XChaCha20-Poly1305 in the STREAM construction,
//! which authenticates the data one chunk at a time and binds the chunk order,
//! so a truncated or reordered blob is rejected.
//!
//! The encrypted blob is laid out as:
//!
//! ```text
//! ┌────────┬─────────┬───────────────────┬──────────────────────────┐
//! │ magic  │ version │ nonce prefix (19) │ AEAD chunks (1 MiB each)  │
//! │ 4 bytes│ 1 byte  │                   │ each chunk + 16-byte tag  │
//! └────────┴─────────┴───────────────────┴──────────────────────────┘
//! ```
//!
//! Encryption streams the plaintext from a reader so the whole file never has
//! to be resident at once; the ciphertext is collected into a `Vec` because the
//! Blossom upload takes the blob by value. Decryption streams the recovered
//! plaintext out to a writer.

use core::fmt;

use chacha20poly1305::aead::generic_array::GenericArray;
use chacha20poly1305::aead::stream::{DecryptorBE32, EncryptorBE32};
use chacha20poly1305::aead::KeyInit;
use chacha20poly1305::XChaCha20Poly1305;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};

use crate::error::{Error, Result};
use crate::secret::FileKey;

const MAGIC: &[u8; 4] = b"NVLT";
const VERSION: u8 = 1;
/// XChaCha20-Poly1305 has a 24-byte nonce; the STREAM construction reserves 5
/// bytes for its counter and last-block flag, leaving a 19-byte random prefix.
const NONCE_PREFIX_LEN: usize = 19;
const HEADER_LEN: usize = MAGIC.len() + 1 + NONCE_PREFIX_LEN;
const TAG_LEN: usize = 16;
/// Plaintext chunk size. The matching ciphertext chunk is this plus the tag.
const CHUNK_SIZE: usize = 1024 * 1024;
const ENCRYPTED_CHUNK_SIZE: usize = CHUNK_SIZE + TAG_LEN;

/// The AEAD scheme used for a blob. Recorded in the pointer and the manifest so
/// the format is self-describing and an unknown value fails loudly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CipherId {
    #[serde(rename = "xchacha20poly1305-stream")]
    XChaCha20Poly1305Stream,
}

/// The cipher this build produces.
pub const CIPHER: CipherId = CipherId::XChaCha20Poly1305Stream;

/// The SHA-256 of an encrypted blob: its Blossom address and its integrity tag.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct BlobHash([u8; 32]);

impl BlobHash {
    /// Hash an entire blob.
    pub fn of(blob: &[u8]) -> Self {
        Self(Sha256::digest(blob).into())
    }

    pub fn to_hex(self) -> String {
        hex::encode(self.0)
    }

    pub fn from_hex(hex: &str) -> Result<Self> {
        let bytes =
            hex::decode(hex).map_err(|_| Error::BlobFormat("hash is not valid hex".into()))?;
        let bytes: [u8; 32] = bytes
            .try_into()
            .map_err(|_| Error::BlobFormat("hash is not 32 bytes".into()))?;
        Ok(Self(bytes))
    }
}

impl fmt::Display for BlobHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl fmt::Debug for BlobHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BlobHash({})", self.to_hex())
    }
}

impl Serialize for BlobHash {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for BlobHash {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let hex = String::deserialize(deserializer)?;
        Self::from_hex(&hex).map_err(serde::de::Error::custom)
    }
}

/// Encrypt the plaintext read from `reader` into a self-contained blob.
pub fn encrypt(file_key: &FileKey, mut reader: impl Read) -> Result<Vec<u8>> {
    let mut nonce_prefix = [0u8; NONCE_PREFIX_LEN];
    getrandom::getrandom(&mut nonce_prefix).map_err(|_| Error::Encryption)?;

    let mut blob = Vec::with_capacity(HEADER_LEN + ENCRYPTED_CHUNK_SIZE);
    blob.extend_from_slice(MAGIC);
    blob.push(VERSION);
    blob.extend_from_slice(&nonce_prefix);

    let cipher =
        XChaCha20Poly1305::new_from_slice(file_key.as_bytes()).map_err(|_| Error::Encryption)?;
    let mut encryptor = EncryptorBE32::from_aead(cipher, GenericArray::from_slice(&nonce_prefix));

    // Read one chunk ahead so we know which chunk is the last and can seal it
    // with `encrypt_last`, which writes the STREAM end-of-stream marker.
    let mut current = read_chunk(&mut reader)?;
    loop {
        let next = read_chunk(&mut reader)?;
        if next.is_empty() {
            let sealed = encryptor
                .encrypt_last(current.as_slice())
                .map_err(|_| Error::Encryption)?;
            blob.extend_from_slice(&sealed);
            break;
        }
        let sealed = encryptor
            .encrypt_next(current.as_slice())
            .map_err(|_| Error::Encryption)?;
        blob.extend_from_slice(&sealed);
        current = next;
    }

    Ok(blob)
}

/// Decrypt a blob, streaming the recovered plaintext to `writer`.
pub fn decrypt(file_key: &FileKey, blob: &[u8], mut writer: impl Write) -> Result<()> {
    if blob.len() < HEADER_LEN {
        return Err(Error::BlobFormat("blob is shorter than its header".into()));
    }
    let (header, body) = blob.split_at(HEADER_LEN);

    if &header[..MAGIC.len()] != MAGIC {
        return Err(Error::BlobFormat("not a NanaVault blob".into()));
    }
    let version = header[MAGIC.len()];
    if version != VERSION {
        return Err(Error::BlobFormat(format!(
            "unsupported blob version {version}"
        )));
    }
    let nonce_prefix = &header[MAGIC.len() + 1..];

    if body.is_empty() {
        return Err(Error::BlobFormat("blob has no ciphertext".into()));
    }

    let cipher =
        XChaCha20Poly1305::new_from_slice(file_key.as_bytes()).map_err(|_| Error::Decryption)?;
    let mut decryptor = DecryptorBE32::from_aead(cipher, GenericArray::from_slice(nonce_prefix));

    // Every chunk but the last is exactly `ENCRYPTED_CHUNK_SIZE`; whatever
    // remains once no full chunk is left over is the sealed final chunk.
    let mut remaining = body;
    while remaining.len() > ENCRYPTED_CHUNK_SIZE {
        let (chunk, rest) = remaining.split_at(ENCRYPTED_CHUNK_SIZE);
        let plaintext = decryptor
            .decrypt_next(chunk)
            .map_err(|_| Error::Decryption)?;
        writer.write_all(&plaintext)?;
        remaining = rest;
    }
    let plaintext = decryptor
        .decrypt_last(remaining)
        .map_err(|_| Error::Decryption)?;
    writer.write_all(&plaintext)?;

    Ok(())
}

/// Read up to one chunk, coalescing short reads. A returned buffer shorter than
/// `CHUNK_SIZE` (including empty) signals end of input.
fn read_chunk(reader: &mut impl Read) -> Result<Vec<u8>> {
    let mut buffer = vec![0u8; CHUNK_SIZE];
    let mut filled = 0;
    while filled < CHUNK_SIZE {
        let read = reader.read(&mut buffer[filled..])?;
        if read == 0 {
            break;
        }
        filled += read;
    }
    buffer.truncate(filled);
    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn key() -> FileKey {
        FileKey::new([7u8; 32])
    }

    /// Deterministic pseudo-random bytes, so tests need no RNG.
    fn data(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i % 251) as u8).collect()
    }

    fn round_trip(len: usize) {
        let plaintext = data(len);
        let blob = encrypt(&key(), Cursor::new(&plaintext)).unwrap();

        let mut recovered = Vec::new();
        decrypt(&key(), &blob, &mut recovered).unwrap();

        assert_eq!(recovered, plaintext, "round trip failed for {len} bytes");
    }

    #[test]
    fn round_trips_across_chunk_boundaries() {
        for len in [
            0,
            1,
            1000,
            CHUNK_SIZE - 1,
            CHUNK_SIZE,
            CHUNK_SIZE + 1,
            2 * CHUNK_SIZE,
            2 * CHUNK_SIZE + 123,
        ] {
            round_trip(len);
        }
    }

    #[test]
    fn tampering_with_the_ciphertext_is_detected() {
        let blob = encrypt(&key(), Cursor::new(data(5000))).unwrap();

        let mut tampered = blob.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0x01;

        let mut out = Vec::new();
        assert!(matches!(
            decrypt(&key(), &tampered, &mut out),
            Err(Error::Decryption)
        ));
    }

    #[test]
    fn the_wrong_key_cannot_decrypt() {
        let blob = encrypt(&key(), Cursor::new(data(5000))).unwrap();

        let mut out = Vec::new();
        let wrong = FileKey::new([9u8; 32]);
        assert!(matches!(
            decrypt(&wrong, &blob, &mut out),
            Err(Error::Decryption)
        ));
    }

    #[test]
    fn a_malformed_header_is_rejected() {
        let mut out = Vec::new();
        assert!(matches!(
            decrypt(&key(), b"too short", &mut out),
            Err(Error::BlobFormat(_))
        ));

        let mut bad_magic = encrypt(&key(), Cursor::new(data(100))).unwrap();
        bad_magic[0] ^= 0xff;
        assert!(matches!(
            decrypt(&key(), &bad_magic, &mut out),
            Err(Error::BlobFormat(_))
        ));
    }

    #[test]
    fn blob_hash_round_trips_through_hex() {
        let hash = BlobHash::of(&data(64));
        assert_eq!(BlobHash::from_hex(&hash.to_hex()).unwrap(), hash);
    }
}
