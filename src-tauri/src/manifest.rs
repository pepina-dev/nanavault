//! The recovery manifest.
//!
//! A small JSON file the user can export so a recovery still works even if every
//! relay has dropped the pointer event. It is the backup descriptor plus the
//! relay list — the *where* and *how* of a backup. It deliberately holds **no
//! secret material**: the key still comes from the password derivation or from
//! the Shamir shares. The `no_secret_material` test guards that invariant.

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::metadata::BackupDescriptor;

/// An exportable, secret-free description of where a backup lives.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    #[serde(flatten)]
    pub descriptor: BackupDescriptor,
    /// The relays the pointer was published to.
    pub relays: Vec<String>,
}

impl Manifest {
    pub fn new(descriptor: BackupDescriptor, relays: Vec<String>) -> Self {
        Self { descriptor, relays }
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
    use crate::crypto::kdf::KdfParams;

    fn manifest() -> Manifest {
        let descriptor = BackupDescriptor::new(
            BlobHash::of(b"ciphertext"),
            2048,
            vec!["https://blossom.example".into()],
            KdfParams::default(),
        );
        Manifest::new(descriptor, vec!["wss://relay.example".into()])
    }

    #[test]
    fn round_trips_through_json() {
        let original = manifest();
        let parsed = Manifest::from_json(&original.to_json().unwrap()).unwrap();
        assert_eq!(parsed, original);
    }

    #[test]
    fn carries_no_secret_material() {
        // The manifest's top-level keys must be drawn only from this
        // non-secret allowlist. If anyone ever adds a secret field, this fails.
        const ALLOWED: &[&str] = &[
            "v",
            "blob_sha256",
            "blob_size",
            "servers",
            "cipher",
            "kdf",
            "relays",
        ];

        let value: serde_json::Value =
            serde_json::from_str(&manifest().to_json().unwrap()).unwrap();
        let object = value.as_object().expect("manifest serializes to an object");
        for key in object.keys() {
            assert!(
                ALLOWED.contains(&key.as_str()),
                "unexpected manifest field: {key}"
            );
        }
    }
}
