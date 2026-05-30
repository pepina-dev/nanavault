//! The recovery manifest.
//!
//! A small JSON file the user can export so a recovery still works even if every
//! relay has dropped the pointer event. It wraps the backup descriptor — the
//! *where* and *which* of a backup — with the relays it was published to and the
//! self-descriptive details (cipher, KDF parameters, blob size) that the pointer
//! event deliberately omits. It holds **no secret material**: the key still
//! comes from the password derivation or from the Shamir shares. The
//! `carries_no_secret_material` test guards that invariant.

use nostr::PublicKey;
use serde::{Deserialize, Serialize};

use crate::crypto::cipher::{CipherId, CIPHER};
use crate::crypto::kdf::KdfParams;
use crate::error::{Error, Result};
use crate::metadata::BackupDescriptor;

/// An exportable, secret-free description of where a backup lives and how it was
/// made.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// What recovery needs: the blob's address, its servers, and the file name.
    pub descriptor: BackupDescriptor,
    /// The relays the pointer was published to.
    pub relays: Vec<String>,
    /// The public identity that authored the pointer. Recovery rederives the key
    /// and checks it against this before doing any work, so a wrong key or
    /// password is rejected immediately rather than after downloading the blob.
    /// This is public, not secret — it is already the pointer's author on the
    /// relays.
    pub derived_pubkey: PublicKey,
    /// Size of the encrypted blob, in bytes. Informational.
    pub blob_size: u64,
    /// The AEAD used for the blob. Self-descriptive only — the blob's own header
    /// is what recovery actually enforces.
    pub cipher: CipherId,
    /// The Argon2id parameters used to derive the key. Recorded for
    /// transparency; the password path must already know them to get this far.
    pub kdf: KdfParams,
}

impl Manifest {
    pub fn new(
        descriptor: BackupDescriptor,
        relays: Vec<String>,
        derived_pubkey: PublicKey,
        blob_size: u64,
        kdf: KdfParams,
    ) -> Self {
        Self {
            descriptor,
            relays,
            derived_pubkey,
            blob_size,
            cipher: CIPHER,
            kdf,
        }
    }

    /// Serialize for export to a file.
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self).map_err(|e| Error::Manifest(e.to_string()))
    }

    /// Parse a manifest the user supplied during recovery.
    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).map_err(|e| Error::Manifest(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::cipher::BlobHash;
    use crate::metadata::Filename;
    use nostr::Keys;

    fn manifest() -> Manifest {
        let descriptor = BackupDescriptor::new(
            BlobHash::of(b"ciphertext"),
            vec!["https://blossom.example".into()],
            Filename::parse("notes.txt").unwrap(),
        );
        Manifest::new(
            descriptor,
            vec!["wss://relay.example".into()],
            Keys::generate().public_key(),
            2048,
            KdfParams::default(),
        )
    }

    #[test]
    fn round_trips_through_json() {
        let original = manifest();
        let parsed = Manifest::from_json(&original.to_json().unwrap()).unwrap();
        assert_eq!(parsed, original);
    }

    #[test]
    fn carries_no_secret_material() {
        // Every field, at the top level and inside the nested descriptor, must
        // be drawn from these non-secret allowlists. If anyone ever adds a
        // secret field, this fails.
        const ALLOWED_TOP: &[&str] =
            &["descriptor", "relays", "derived_pubkey", "blob_size", "cipher", "kdf"];
        const ALLOWED_DESCRIPTOR: &[&str] = &["v", "blob_sha256", "servers", "filename"];

        let value: serde_json::Value = serde_json::from_str(&manifest().to_json().unwrap()).unwrap();
        let object = value.as_object().expect("manifest serializes to an object");
        for key in object.keys() {
            assert!(
                ALLOWED_TOP.contains(&key.as_str()),
                "unexpected manifest field: {key}"
            );
        }

        let descriptor = object["descriptor"]
            .as_object()
            .expect("descriptor serializes to an object");
        for key in descriptor.keys() {
            assert!(
                ALLOWED_DESCRIPTOR.contains(&key.as_str()),
                "unexpected descriptor field: {key}"
            );
        }
    }
}
