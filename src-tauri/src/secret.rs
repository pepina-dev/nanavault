//! Zeroizing newtypes for the secret material that flows through the backend.
//!
//! These wrappers make secret handling explicit and guarantee the bytes are
//! wiped from memory on drop. None of them implement a revealing `Debug`, so a
//! secret can never be written to a log by accident.
//!
//! The two trust boundaries worth stating plainly:
//!
//! - The master key and password arrive from the webview through a Tauri
//!   command, so they briefly exist in JavaScript memory. That is inherent to a
//!   webview UI; we minimize their lifetime on the Rust side and never persist
//!   them.
//! - [`nostr::SecretKey`] erases itself on drop (secp256k1's `non_secure_erase`),
//!   so the keys we hold inside [`MasterKey`] and [`DerivedKey`] are cleaned up
//!   without extra work here.

use core::fmt;

use nostr::{Keys, PublicKey, SecretKey};
use zeroize::Zeroizing;

use crate::crypto::slip39;
use crate::error::{Error, Result};

/// The user's password. Wiped on drop; never logged.
pub struct Password(Zeroizing<String>);

impl Password {
    pub fn new(value: String) -> Self {
        Self(Zeroizing::new(value))
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl fmt::Debug for Password {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Password(<redacted>)")
    }
}

/// The user's master nostr key.
///
/// It is used *only* to derive the backup key — never to sign anything and
/// never persisted. Knowing the derived key cannot reveal this one.
pub struct MasterKey {
    secret: SecretKey,
    public: PublicKey,
}

impl MasterKey {
    /// Parse a master key from an `nsec` (bech32) or hex string.
    pub fn parse(input: &str) -> Result<Self> {
        let keys = Keys::parse(input).map_err(|_| Error::InvalidSecretKey)?;
        Ok(Self {
            secret: keys.secret_key().clone(),
            public: keys.public_key(),
        })
    }

    /// Build a master key directly from its 32-byte secret scalar.
    pub fn from_secret_bytes(bytes: &[u8]) -> Result<Self> {
        let secret = SecretKey::from_slice(bytes).map_err(|_| Error::InvalidSecretKey)?;
        let keys = Keys::new(secret);
        Ok(Self {
            secret: keys.secret_key().clone(),
            public: keys.public_key(),
        })
    }

    /// Parse a master key from whatever the user supplies: an `nsec`/hex key
    /// (advanced mode), or an easy-mode **backup code** — a SLIP-0039 1-of-1
    /// mnemonic that carries the same secret. The key forms are tried first;
    /// anything else is decoded as a single-share backup code.
    pub fn from_key_or_backup_code(input: &str) -> Result<Self> {
        if let Ok(key) = Self::parse(input) {
            return Ok(key);
        }
        let secret = slip39::combine(&[input.to_string()]).map_err(|_| Error::InvalidSecretKey)?;
        Self::from_secret_bytes(&secret[..])
    }

    /// The x-only public key. Used as the (public, non-secret) salt source for
    /// key derivation.
    pub fn public_key(&self) -> PublicKey {
        self.public
    }

    /// The 32-byte secret scalar, wrapped so the copy is wiped after use.
    pub fn secret_bytes(&self) -> Zeroizing<[u8; 32]> {
        Zeroizing::new(self.secret.to_secret_bytes())
    }
}

impl fmt::Debug for MasterKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MasterKey")
            .field("public", &self.public)
            .field("secret", &"<redacted>")
            .finish()
    }
}

/// The key derived from the master key and password (or reconstructed from
/// Shamir shares).
///
/// It serves two cleanly separated roles: it is the nostr identity that signs
/// the pointer event and Blossom uploads, and it is the seed from which the
/// symmetric [`FileKey`] is derived. It is never persisted.
pub struct DerivedKey {
    keys: Keys,
}

impl DerivedKey {
    /// Build a derived key from its 32-byte secret scalar.
    ///
    /// Fails if the bytes are not exactly 32 bytes or do not represent a valid
    /// secp256k1 scalar — which is how a secret reconstructed from foreign
    /// shares of the wrong size is rejected.
    pub fn from_secret_bytes(bytes: &[u8]) -> Result<Self> {
        let secret = SecretKey::from_slice(bytes).map_err(|_| Error::InvalidSecretKey)?;
        Ok(Self {
            keys: Keys::new(secret),
        })
    }

    /// The signing keys, for nostr events and Blossom authorization.
    pub fn keys(&self) -> &Keys {
        &self.keys
    }

    pub fn public_key(&self) -> PublicKey {
        self.keys.public_key()
    }

    /// The 32-byte secret scalar, wrapped so the copy is wiped after use.
    pub fn secret_bytes(&self) -> Zeroizing<[u8; 32]> {
        Zeroizing::new(self.keys.secret_key().to_secret_bytes())
    }
}

impl fmt::Debug for DerivedKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DerivedKey")
            .field("public", &self.keys.public_key())
            .field("secret", &"<redacted>")
            .finish()
    }
}

/// A 256-bit symmetric key for file encryption. Wiped on drop; never logged.
pub struct FileKey(Zeroizing<[u8; 32]>);

impl FileKey {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(Zeroizing::new(bytes))
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for FileKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FileKey(<redacted>)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Advanced mode: an nsec/hex key still parses through the combined entry.
    #[test]
    fn accepts_a_plain_secret_key() {
        let keys = Keys::generate();
        let hex = keys.secret_key().to_secret_hex();
        let master = MasterKey::from_key_or_backup_code(&hex).unwrap();
        assert_eq!(master.public_key(), keys.public_key());
    }

    /// Easy mode: a generated backup code (a 1-of-1 SLIP-0039 mnemonic) decodes
    /// back to the very key it was minted from, so backup and recovery agree.
    #[test]
    fn backup_code_round_trips_to_its_master_key() {
        let keys = Keys::generate();
        let secret = keys.secret_key().to_secret_bytes();
        let code = slip39::split(&secret, 1, 1).unwrap().pop().unwrap();

        let master = MasterKey::from_key_or_backup_code(&code).unwrap();
        assert_eq!(master.public_key(), keys.public_key());
        assert_eq!(master.secret_bytes().as_ref(), &secret);
    }

    /// Garbage is neither a key nor a valid share.
    #[test]
    fn rejects_nonsense() {
        assert!(matches!(
            MasterKey::from_key_or_backup_code("not a real code"),
            Err(Error::InvalidSecretKey)
        ));
    }
}
