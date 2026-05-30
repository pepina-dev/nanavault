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
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::crypto::cipher::BlobHash;
use crate::error::{Error, Result};
use crate::secret::DerivedKey;

/// The replaceable-event kind that carries a NanaVault pointer. App-specific,
/// in the replaceable range (10000–19999).
pub const BACKUP_EVENT_KIND: u16 = 10909;

/// The current pointer/manifest payload version.
pub const DESCRIPTOR_VERSION: u8 = 1;

/// The original name of a backed-up file: a single path component that is safe
/// to use, on its own, as a recovery destination name.
///
/// Construction is the only way to obtain one, and it rejects empty names, path
/// separators, `.`/`..`, control characters, and over-long names. A `Filename`
/// in hand therefore *cannot* escape the directory it is later joined into — the
/// path-traversal defense lives in the type, not at every call site. The bound
/// is enforced again on deserialization, so a hostile pointer or manifest is
/// refused as it is read, never as it is used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Filename(String);

/// The longest name we accept, in bytes. Comfortably under the 255-byte limit
/// that common filesystems impose on a single component.
const MAX_FILENAME_LEN: usize = 255;

impl Filename {
    /// Validate `value` as a standalone file name.
    pub fn parse(value: &str) -> Result<Self> {
        let reject = |reason: &str| {
            Err(Error::InvalidFilename(format!("{value:?}: {reason}")))
        };

        if value.is_empty() {
            return reject("must not be empty");
        }
        if value.len() > MAX_FILENAME_LEN {
            return reject("is too long");
        }
        if value == "." || value == ".." {
            return reject("refers to a directory");
        }
        if value.contains('/') || value.contains('\\') {
            return reject("must not contain a path separator");
        }
        if value.chars().any(char::is_control) {
            return reject("must not contain control characters");
        }

        Ok(Self(value.to_owned()))
    }

    /// Take the final component of a filesystem path and validate it as a name.
    /// This is how a backup records the name of the file the user chose.
    pub fn from_path(path: &str) -> Result<Self> {
        let name = std::path::Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                Error::InvalidFilename(format!("{path:?}: has no file-name component"))
            })?;
        Self::parse(name)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Serialize for Filename {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Filename {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

/// Everything recovery needs from the pointer: *where* the blob lives, *which*
/// blob it is, and *what to call it* once restored.
///
/// This is the payload of the encrypted pointer event. It carries only what the
/// recovery path actually consumes — nothing self-descriptive (the cipher) or
/// merely informational (the blob size), and nothing that recovery cannot use
/// (the KDF parameters, which the password path must already know before it can
/// reach this payload at all). Those live in the [`Manifest`](crate::manifest)
/// instead. It holds no secret material; the file name is sensitive, but the
/// payload is encrypted, so it never reaches a relay in the clear.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupDescriptor {
    /// Payload format version.
    pub v: u8,
    /// SHA-256 of the encrypted blob — its Blossom address and integrity tag.
    pub blob_sha256: BlobHash,
    /// Blossom servers the blob was uploaded to.
    pub servers: Vec<String>,
    /// The original name of the backed-up file, restored on recovery.
    pub filename: Filename,
}

impl BackupDescriptor {
    pub fn new(blob_sha256: BlobHash, servers: Vec<String>, filename: Filename) -> Self {
        Self {
            v: DESCRIPTOR_VERSION,
            blob_sha256,
            servers,
            filename,
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
    use nostr::Keys;

    fn derived_key() -> DerivedKey {
        DerivedKey::from_secret_bytes(&Keys::generate().secret_key().to_secret_bytes()).unwrap()
    }

    fn descriptor() -> BackupDescriptor {
        BackupDescriptor::new(
            BlobHash::of(b"some encrypted blob"),
            vec!["https://blossom.example".into()],
            Filename::parse("my-secret-file.txt").unwrap(),
        )
    }

    #[test]
    fn a_plain_filename_round_trips() {
        let name = Filename::parse("report.pdf").unwrap();
        assert_eq!(name.as_str(), "report.pdf");
    }

    #[test]
    fn a_filename_that_could_escape_its_directory_is_rejected() {
        for hostile in [
            "",
            ".",
            "..",
            "a/b.txt",
            "../secret",
            "/etc/passwd",
            "back\\slash.txt",
            "with\0null",
            "with\nnewline",
        ] {
            assert!(
                matches!(Filename::parse(hostile), Err(Error::InvalidFilename(_))),
                "expected {hostile:?} to be rejected"
            );
        }
    }

    #[test]
    fn an_over_long_filename_is_rejected() {
        let long = "a".repeat(256);
        assert!(matches!(
            Filename::parse(&long),
            Err(Error::InvalidFilename(_))
        ));
    }

    #[test]
    fn from_path_keeps_only_the_final_component() {
        assert_eq!(
            Filename::from_path("/home/alice/taxes/2025.pdf")
                .unwrap()
                .as_str(),
            "2025.pdf"
        );
    }

    #[test]
    fn from_path_rejects_a_path_with_no_file_name() {
        assert!(matches!(
            Filename::from_path("/"),
            Err(Error::InvalidFilename(_))
        ));
    }

    #[test]
    fn a_filename_serializes_as_a_bare_string() {
        let json = serde_json::to_string(&Filename::parse("a.txt").unwrap()).unwrap();
        assert_eq!(json, r#""a.txt""#);
    }

    #[test]
    fn deserializing_a_hostile_filename_is_refused() {
        assert!(serde_json::from_str::<Filename>(r#""../escape""#).is_err());
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

        // The blob hash, the server URL, and the file name must not appear in
        // the clear: a relay observer learns nothing about the backup.
        assert!(!event.content.contains(&descriptor.blob_sha256.to_hex()));
        assert!(!event.content.contains("blossom.example"));
        assert!(!event.content.contains("my-secret-file.txt"));
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
