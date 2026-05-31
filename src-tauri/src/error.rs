//! The crate-wide error type.
//!
//! Every fallible operation in the backend funnels into [`Error`]. When an
//! error crosses the Tauri boundary it is serialized to its `Display` string,
//! so the frontend always receives a single human-readable message and never a
//! structured payload it would have to interpret.

use serde::{Serialize, Serializer};

/// The result type used throughout the backend.
pub type Result<T> = std::result::Result<T, Error>;

/// Everything that can go wrong in the NanaVault backend.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The supplied master key could not be parsed as an `nsec` or hex key.
    #[error("invalid nostr secret key")]
    InvalidSecretKey,

    /// Key derivation failed. In practice only reachable if Argon2 is given
    /// invalid parameters; the scalar-mapping retry makes the secp256k1 step
    /// effectively infallible.
    #[error("key derivation failed: {0}")]
    KeyDerivation(String),

    /// The AEAD refused to encrypt. Reachable only on absurd inputs (e.g. a
    /// plaintext larger than the stream construction can address).
    #[error("encryption failed")]
    Encryption,

    /// The AEAD refused to decrypt: the key is wrong or the ciphertext was
    /// modified. The message is intentionally vague — distinguishing the two
    /// would leak information.
    #[error("decryption failed: wrong key or corrupted data")]
    Decryption,

    /// The encrypted blob is not in the expected format.
    #[error("malformed encrypted blob: {0}")]
    BlobFormat(String),

    /// A text backup decrypted to bytes that are not valid UTF-8. Practically
    /// unreachable — the app only ever stores UTF-8 as text and the AEAD
    /// guarantees integrity — but the decode is fallible, so it gets an honest
    /// error rather than a panic.
    #[error("the recovered text was not valid UTF-8")]
    NotUtf8Text,

    /// A downloaded blob did not hash to the value we were looking for.
    #[error("integrity check failed: expected {expected}, got {actual}")]
    IntegrityMismatch { expected: String, actual: String },

    /// Splitting or combining Shamir shares failed.
    #[error("secret sharing: {0}")]
    Shamir(#[from] crate::crypto::slip39::Error),

    /// A file name was empty, contained a path separator, or otherwise could
    /// not stand in as a single, safe destination name.
    #[error("invalid file name: {0}")]
    InvalidFilename(String),

    /// Typed text exceeded the size limit for an in-app text backup.
    #[error("the text is too large: the limit is {limit} bytes")]
    TextTooLarge { limit: usize },

    /// Building or reading the encrypted pointer event failed.
    #[error("backup pointer error: {0}")]
    Pointer(String),

    /// The backup pointer could not be found on any relay and no manifest was
    /// supplied.
    #[error("no backup found on the given relays, and no recovery manifest was provided")]
    BackupNotFound,

    /// Talking to the nostr relays failed.
    #[error("relay error: {0}")]
    Relay(String),

    /// Talking to the Blossom servers failed (every server was tried).
    #[error("blossom error: {0}")]
    Blossom(String),

    /// A recovery manifest could not be read or written.
    #[error("recovery manifest error: {0}")]
    Manifest(String),

    /// Recovery could not find an unused name for the file in the destination
    /// folder, even after trying many numbered variants.
    #[error("could not find an available name for {0:?} in the destination folder")]
    DestinationUnavailable(String),

    /// The key rederived from the password (or reconstructed from the shares)
    /// does not match the identity the manifest was made with — the wrong master
    /// key, password, or shares for this backup.
    #[error("the key or password does not match this recovery manifest")]
    ManifestKeyMismatch,

    /// Reading the plaintext file or writing the recovered file failed.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
