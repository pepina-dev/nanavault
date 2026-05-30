//! Key derivation.
//!
//! Two derivations live here:
//!
//! 1. `master nsec + password → derived key`. The password is the weak factor,
//!    so it is run through Argon2id (memory-hard) before being mixed, via HKDF,
//!    with the high-entropy master secret. Reproducing the derived key requires
//!    *both* the master secret and the password.
//!
//! 2. `derived key → file key`. A dedicated symmetric key for the AEAD, so the
//!    signing key and the encryption key are never the same bytes.
//!
//! The Argon2id parameters are a fixed application constant (the default
//! `KdfParams`). They have to be: the password-only recovery path must
//! re-derive the key before it can read anything, so there is nowhere to learn
//! the parameters from first. They are still recorded in the pointer and the
//! manifest for transparency. `derive` takes them as an argument only so tests
//! can run with cheap settings.

use argon2::{Algorithm, Argon2, Params, Version};
use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::error::{Error, Result};
use crate::secret::{DerivedKey, FileKey, MasterKey, Password};

/// Domain-separation labels. Distinct labels guarantee the three HKDF/Argon2
/// uses can never collide, even though they draw on overlapping inputs.
const KDF_SALT_LABEL: &[u8] = b"nanavault/kdf/v1";
const DERIVED_KEY_INFO: &str = "nanavault/derived-key/v1";
const FILE_KEY_INFO: &[u8] = b"nanavault/file-key/v1";

/// An upper bound on the scalar-mapping retry loop. The probability of even one
/// retry is ~2⁻¹²⁸, so this is a defensive backstop that can never be hit in
/// practice; it exists only so the loop is provably finite.
const MAX_SCALAR_ATTEMPTS: u32 = 64;

/// The password-hashing parameters, recorded alongside the backup so recovery
/// can reproduce the derivation.
///
/// Serializes as `{ "alg": "argon2id", "m": <KiB>, "t": <iters>, "p": <lanes> }`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KdfParams {
    pub alg: KdfAlg,
    /// Memory cost, in KiB.
    pub m: u32,
    /// Time cost (iterations).
    pub t: u32,
    /// Parallelism (lanes).
    pub p: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KdfAlg {
    Argon2id,
}

impl Default for KdfParams {
    /// Desktop-grade defaults: 256 MiB, 4 iterations, single lane.
    fn default() -> Self {
        Self {
            alg: KdfAlg::Argon2id,
            m: 256 * 1024,
            t: 4,
            p: 1,
        }
    }
}

/// Derive the backup key from the master key and password.
///
/// Deterministic: the same `(master, password, params)` always yields the same
/// derived key, which is what makes the password-only recovery path possible.
pub fn derive(master: &MasterKey, password: &Password, params: &KdfParams) -> Result<DerivedKey> {
    let salt = argon2_salt(master);

    let mut pw_key = Zeroizing::new([0u8; 32]);
    argon2(params)?
        .hash_password_into(password.as_bytes(), &salt, pw_key.as_mut())
        .map_err(|e| Error::KeyDerivation(e.to_string()))?;

    let master_secret = master.secret_bytes();
    let hkdf = Hkdf::<Sha256>::new(Some(pw_key.as_ref()), master_secret.as_ref());

    // Map the HKDF output to a valid secp256k1 scalar. Invalid outputs are
    // astronomically unlikely, but if one occurs we deterministically derive a
    // fresh candidate by extending the `info` label, so the result stays
    // reproducible at recovery time.
    for attempt in 0..MAX_SCALAR_ATTEMPTS {
        let info = scalar_info(attempt);
        let mut seed = Zeroizing::new([0u8; 32]);
        hkdf.expand(info.as_bytes(), seed.as_mut())
            .map_err(|e| Error::KeyDerivation(e.to_string()))?;

        if let Ok(key) = DerivedKey::from_secret_bytes(&seed[..]) {
            return Ok(key);
        }
    }

    Err(Error::KeyDerivation(
        "exhausted scalar-mapping attempts".to_string(),
    ))
}

/// Derive the symmetric file-encryption key from the derived key.
pub fn derive_file_key(derived: &DerivedKey) -> Result<FileKey> {
    let derived_secret = derived.secret_bytes();
    let hkdf = Hkdf::<Sha256>::new(None, derived_secret.as_ref());

    let mut file_key = [0u8; 32];
    hkdf.expand(FILE_KEY_INFO, &mut file_key)
        .map_err(|e| Error::KeyDerivation(e.to_string()))?;

    Ok(FileKey::new(file_key))
}

fn argon2(params: &KdfParams) -> Result<Argon2<'static>> {
    let KdfAlg::Argon2id = params.alg;
    let params = Params::new(params.m, params.t, params.p, Some(32))
        .map_err(|e| Error::KeyDerivation(e.to_string()))?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
}

fn argon2_salt(master: &MasterKey) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(KDF_SALT_LABEL);
    hasher.update(master.public_key().to_bytes());
    hasher.finalize().into()
}

fn scalar_info(attempt: u32) -> String {
    if attempt == 0 {
        DERIVED_KEY_INFO.to_string()
    } else {
        format!("{DERIVED_KEY_INFO}/{attempt}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::Keys;

    /// Cheap parameters so the tests don't spend 256 MiB per derivation.
    fn test_params() -> KdfParams {
        KdfParams {
            alg: KdfAlg::Argon2id,
            m: 32,
            t: 1,
            p: 1,
        }
    }

    fn master() -> MasterKey {
        MasterKey::parse(&Keys::generate().secret_key().to_secret_hex()).unwrap()
    }

    #[test]
    fn derivation_is_deterministic() {
        let master = master();
        let params = test_params();

        let a = derive(&master, &Password::new("hunter2".into()), &params).unwrap();
        let b = derive(&master, &Password::new("hunter2".into()), &params).unwrap();

        assert_eq!(a.public_key(), b.public_key());
    }

    #[test]
    fn different_password_yields_different_key() {
        let master = master();
        let params = test_params();

        let a = derive(&master, &Password::new("hunter2".into()), &params).unwrap();
        let b = derive(&master, &Password::new("hunter3".into()), &params).unwrap();

        assert_ne!(a.public_key(), b.public_key());
    }

    #[test]
    fn different_master_yields_different_key() {
        let params = test_params();
        let password = Password::new("hunter2".into());

        let a = derive(&master(), &password, &params).unwrap();
        let b = derive(&master(), &password, &params).unwrap();

        assert_ne!(a.public_key(), b.public_key());
    }

    #[test]
    fn derived_key_is_not_the_master_key() {
        let master = master();
        let master_pub = master.public_key();
        let derived = derive(&master, &Password::new("hunter2".into()), &test_params()).unwrap();

        assert_ne!(derived.public_key(), master_pub);
    }

    #[test]
    fn file_key_is_deterministic_and_distinct_from_signing_key() {
        let derived = derive(&master(), &Password::new("pw".into()), &test_params()).unwrap();

        let k1 = derive_file_key(&derived).unwrap();
        let k2 = derive_file_key(&derived).unwrap();
        assert_eq!(k1.as_bytes(), k2.as_bytes());

        // The file key must not simply be the signing scalar.
        assert_ne!(k1.as_bytes(), derived.secret_bytes().as_ref());
    }

    #[test]
    fn kdf_params_round_trip_through_json() {
        let params = KdfParams::default();
        let json = serde_json::to_string(&params).unwrap();
        assert_eq!(json, r#"{"alg":"argon2id","m":262144,"t":4,"p":1}"#);
        assert_eq!(serde_json::from_str::<KdfParams>(&json).unwrap(), params);
    }
}
