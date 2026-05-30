//! Recovery orchestration: obtain the derived key (from the password or from
//! Shamir shares), locate the backup (via the manifest or the relays), download
//! the blob, verify it, and decrypt.
//!
//! The two entry points differ only in how they reconstruct the derived key;
//! everything after that is shared.

use std::io::Write;

use crate::blossom::{BlobStore, BlobStoreFactory};
use crate::crypto::cipher::{self, BlobHash};
use crate::crypto::kdf::{self, KdfParams};
use crate::crypto::slip39;
use crate::error::{Error, Result};
use crate::manifest::Manifest;
use crate::metadata::{self, BackupDescriptor};
use crate::relay::MetadataStore;
use crate::secret::{DerivedKey, MasterKey, Password};

/// Recover using the master key and password.
pub async fn recover_with_password<F, M>(
    master_nsec: &str,
    password: &str,
    kdf_params: &KdfParams,
    manifest: Option<&Manifest>,
    blob_factory: &F,
    metadata_store: &M,
    output: impl Write,
) -> Result<()>
where
    F: BlobStoreFactory,
    M: MetadataStore,
{
    let master = MasterKey::parse(master_nsec)?;
    let derived = kdf::derive(&master, &Password::new(password.to_string()), kdf_params)?;
    recover(&derived, manifest, blob_factory, metadata_store, output).await
}

/// Recover using a quorum of Shamir shares (the master key and password are not
/// needed).
pub async fn recover_with_shares<F, M>(
    mnemonics: &[String],
    manifest: Option<&Manifest>,
    blob_factory: &F,
    metadata_store: &M,
    output: impl Write,
) -> Result<()>
where
    F: BlobStoreFactory,
    M: MetadataStore,
{
    let secret = slip39::combine(mnemonics)?;
    let derived = DerivedKey::from_secret_bytes(&secret[..])?;
    recover(&derived, manifest, blob_factory, metadata_store, output).await
}

async fn recover<F, M>(
    derived: &DerivedKey,
    manifest: Option<&Manifest>,
    blob_factory: &F,
    metadata_store: &M,
    output: impl Write,
) -> Result<()>
where
    F: BlobStoreFactory,
    M: MetadataStore,
{
    let descriptor = locate(derived, manifest, metadata_store).await?;
    let file_key = kdf::derive_file_key(derived)?;
    let blob = download(&descriptor, blob_factory).await?;
    cipher::decrypt(&file_key, &blob, output)
}

/// Find the backup descriptor: prefer the offline manifest, otherwise fetch the
/// pointer event from the relays and decrypt it.
async fn locate<M>(
    derived: &DerivedKey,
    manifest: Option<&Manifest>,
    metadata_store: &M,
) -> Result<BackupDescriptor>
where
    M: MetadataStore,
{
    if let Some(manifest) = manifest {
        return Ok(manifest.descriptor.clone());
    }

    let event = metadata_store
        .fetch_latest(&derived.public_key(), metadata::kind())
        .await?
        .ok_or(Error::BackupNotFound)?;
    metadata::parse_pointer(&event, derived)
}

/// Try each server until one returns the blob we are looking for. A server that
/// returns the wrong bytes is skipped; only a hash match is accepted.
async fn download<F>(descriptor: &BackupDescriptor, factory: &F) -> Result<Vec<u8>>
where
    F: BlobStoreFactory,
{
    let mut last_error = None;

    for server in &descriptor.servers {
        let store = match factory.store(server) {
            Ok(store) => store,
            Err(e) => {
                last_error = Some(e);
                continue;
            }
        };

        match store.download(&descriptor.blob_sha256).await {
            Ok(blob) => {
                let actual = BlobHash::of(&blob);
                if actual == descriptor.blob_sha256 {
                    return Ok(blob);
                }
                last_error = Some(Error::IntegrityMismatch {
                    expected: descriptor.blob_sha256.to_hex(),
                    actual: actual.to_hex(),
                });
            }
            Err(e) => last_error = Some(e),
        }
    }

    Err(last_error.unwrap_or(Error::BackupNotFound))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::{run_backup, BackupOutcome};
    use crate::crypto::kdf::KdfAlg;
    use crate::testutil::{FakeBlobNetwork, FakeRelays};
    use nostr::Keys;
    use std::io::Cursor;

    const PLAINTEXT: &[u8] = b"the contents worth protecting";

    fn fast_kdf() -> KdfParams {
        KdfParams {
            alg: KdfAlg::Argon2id,
            m: 32,
            t: 1,
            p: 1,
        }
    }

    struct Fixture {
        nsec: String,
        password: String,
        network: FakeBlobNetwork,
        relays: FakeRelays,
        outcome: BackupOutcome,
    }

    async fn backed_up() -> Fixture {
        let nsec = Keys::generate().secret_key().to_secret_hex();
        let password = "correct horse battery staple".to_string();
        let network = FakeBlobNetwork::new();
        let relays = FakeRelays::new(vec!["wss://relay.one".into()]);

        let master = MasterKey::parse(&nsec).unwrap();
        let derived = kdf::derive(&master, &Password::new(password.clone()), &fast_kdf()).unwrap();
        let outcome = run_backup(
            &derived,
            Cursor::new(PLAINTEXT.to_vec()),
            &["https://blossom.one".into()],
            2,
            3,
            &fast_kdf(),
            &network,
            &relays,
        )
        .await
        .unwrap();

        Fixture {
            nsec,
            password,
            network,
            relays,
            outcome,
        }
    }

    #[tokio::test]
    async fn password_path_recovers_via_relays() {
        let f = backed_up().await;
        let mut recovered = Vec::new();

        recover_with_password(
            &f.nsec,
            &f.password,
            &fast_kdf(),
            None, // no manifest — must find the pointer on the relays
            &f.network,
            &f.relays,
            &mut recovered,
        )
        .await
        .unwrap();

        assert_eq!(recovered, PLAINTEXT);
    }

    #[tokio::test]
    async fn password_path_recovers_via_manifest_when_relays_are_empty() {
        let f = backed_up().await;
        let empty_relays = FakeRelays::new(vec!["wss://relay.dead".into()]);
        let mut recovered = Vec::new();

        recover_with_password(
            &f.nsec,
            &f.password,
            &fast_kdf(),
            Some(&f.outcome.manifest),
            &f.network,
            &empty_relays,
            &mut recovered,
        )
        .await
        .unwrap();

        assert_eq!(recovered, PLAINTEXT);
    }

    #[tokio::test]
    async fn shares_path_recovers() {
        let f = backed_up().await;
        let quorum = f.outcome.shares[..2].to_vec();
        let mut recovered = Vec::new();

        recover_with_shares(&quorum, None, &f.network, &f.relays, &mut recovered)
            .await
            .unwrap();

        assert_eq!(recovered, PLAINTEXT);
    }

    #[tokio::test]
    async fn the_wrong_password_cannot_decrypt() {
        let f = backed_up().await;
        let mut recovered = Vec::new();

        let result = recover_with_password(
            &f.nsec,
            "the wrong password",
            &fast_kdf(),
            Some(&f.outcome.manifest), // manifest locates it; the key is still wrong
            &f.network,
            &f.relays,
            &mut recovered,
        )
        .await;

        assert!(matches!(result, Err(Error::Decryption)));
    }

    #[tokio::test]
    async fn too_few_shares_cannot_recover() {
        let f = backed_up().await;
        let one = f.outcome.shares[..1].to_vec();
        let mut recovered = Vec::new();

        let result = recover_with_shares(&one, None, &f.network, &f.relays, &mut recovered).await;

        assert!(matches!(result, Err(Error::Shamir(_))));
    }
}
