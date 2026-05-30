//! The backup pointer: the small record that says *where* a backup lives and
//! *how* it was made, and its on-relay form.
//!
//! The pointer is published as a **replaceable** nostr event (so re-backing-up
//! overwrites the previous pointer rather than piling up) authored by the
//! derived key. Its payload is NIP-44 self-encrypted, so a relay observer never
//! learns the blob hash or the server list — only that some key published one
//! small encrypted event.

use nostr::nips::nip44;
use nostr::{Event, EventBuilder, Kind};
use serde::{Deserialize, Serialize};

use crate::crypto::cipher::{BlobHash, CipherId};
use crate::crypto::kdf::KdfParams;
use crate::error::{Error, Result};
use crate::secret::DerivedKey;

/// The replaceable-event kind that carries a NanaVault pointer. App-specific,
/// in the replaceable range (10000–19999).
pub const BACKUP_EVENT_KIND: u16 = 10909;

/// The current pointer/manifest payload version.
pub const DESCRIPTOR_VERSION: u8 = 1;

/// Everything needed to locate and interpret a backup, minus the key.
///
/// This is the payload of the pointer event; the recovery manifest is this plus
/// the relay list. It deliberately holds no secret material.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupDescriptor {
    /// Payload format version.
    pub v: u8,
    /// SHA-256 of the encrypted blob — its Blossom address.
    pub blob_sha256: BlobHash,
    /// Size of the encrypted blob, in bytes.
    pub blob_size: u64,
    /// Blossom servers the blob was uploaded to.
    pub servers: Vec<String>,
    /// The AEAD used for the blob.
    pub cipher: CipherId,
    /// The Argon2id parameters used to derive the key.
    pub kdf: KdfParams,
}

impl BackupDescriptor {
    pub fn new(
        blob_sha256: BlobHash,
        blob_size: u64,
        servers: Vec<String>,
        kdf: KdfParams,
    ) -> Self {
        Self {
            v: DESCRIPTOR_VERSION,
            blob_sha256,
            blob_size,
            servers,
            cipher: crate::crypto::cipher::CIPHER,
            kdf,
        }
    }
}

/// The nostr kind under which pointers are published.
pub fn kind() -> Kind {
    Kind::Custom(BACKUP_EVENT_KIND)
}

/// Build the signed, encrypted pointer event for a backup.
pub fn build_pointer(descriptor: &BackupDescriptor, derived: &DerivedKey) -> Result<Event> {
    let payload = serde_json::to_string(descriptor).map_err(|e| Error::Pointer(e.to_string()))?;
    let keys = derived.keys();

    let content = nip44::encrypt(
        keys.secret_key(),
        &keys.public_key(),
        payload,
        nip44::Version::V2,
    )
    .map_err(|e| Error::Pointer(e.to_string()))?;

    EventBuilder::new(kind(), content)
        .sign_with_keys(keys)
        .map_err(|e| Error::Pointer(e.to_string()))
}

/// Decrypt and parse a pointer event with the derived key that authored it.
pub fn parse_pointer(event: &Event, derived: &DerivedKey) -> Result<BackupDescriptor> {
    let keys = derived.keys();

    let payload = nip44::decrypt(keys.secret_key(), &keys.public_key(), &event.content)
        .map_err(|_| Error::Pointer("the pointer could not be decrypted with this key".into()))?;

    let descriptor: BackupDescriptor =
        serde_json::from_str(&payload).map_err(|e| Error::Pointer(e.to_string()))?;

    if descriptor.v != DESCRIPTOR_VERSION {
        return Err(Error::Pointer(format!(
            "unsupported pointer version {}",
            descriptor.v
        )));
    }
    Ok(descriptor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::kdf::KdfParams;
    use nostr::Keys;

    fn derived_key() -> DerivedKey {
        DerivedKey::from_secret_bytes(&Keys::generate().secret_key().to_secret_bytes()).unwrap()
    }

    fn descriptor() -> BackupDescriptor {
        BackupDescriptor::new(
            BlobHash::of(b"some encrypted blob"),
            4096,
            vec!["https://blossom.example".into()],
            KdfParams::default(),
        )
    }

    #[test]
    fn a_pointer_round_trips_through_an_event() {
        let key = derived_key();
        let original = descriptor();

        let event = build_pointer(&original, &key).unwrap();
        let parsed = parse_pointer(&event, &key).unwrap();

        assert_eq!(parsed, original);
    }

    #[test]
    fn the_event_is_replaceable_and_authored_by_the_derived_key() {
        let key = derived_key();
        let event = build_pointer(&descriptor(), &key).unwrap();

        assert!(kind().is_replaceable());
        assert_eq!(event.kind, kind());
        assert_eq!(event.pubkey, key.public_key());
        assert!(event.verify().is_ok());
    }

    #[test]
    fn the_payload_is_encrypted_not_plaintext() {
        let key = derived_key();
        let descriptor = descriptor();
        let event = build_pointer(&descriptor, &key).unwrap();

        // The blob hash and the server URL must not appear in the clear.
        assert!(!event.content.contains(&descriptor.blob_sha256.to_hex()));
        assert!(!event.content.contains("blossom.example"));
    }

    #[test]
    fn a_pointer_cannot_be_parsed_with_the_wrong_key() {
        let event = build_pointer(&descriptor(), &derived_key()).unwrap();
        assert!(matches!(
            parse_pointer(&event, &derived_key()),
            Err(Error::Pointer(_))
        ));
    }
}
