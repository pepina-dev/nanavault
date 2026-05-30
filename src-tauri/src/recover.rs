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
use crate::metadata::{self, BackupDescriptor, Filename};
use crate::relay::MetadataStore;
use crate::secret::{DerivedKey, MasterKey, Password};

/// Recover using the master key and password.
///
/// `open_output` is handed the original file name and returns the sink to write
/// the recovered plaintext to, so the caller decides the destination *from* the
/// name the backup recorded. The recovered name is returned on success.
pub async fn recover_with_password<F, M, W>(
    master_nsec: &str,
    password: &str,
    kdf_params: &KdfParams,
    manifest: Option<&Manifest>,
    blob_factory: &F,
    metadata_store: &M,
    open_output: impl FnOnce(&Filename) -> Result<W>,
) -> Result<Filename>
where
    F: BlobStoreFactory,
    M: MetadataStore,
    W: Write,
{
    let master = MasterKey::parse(master_nsec)?;
    let derived = kdf::derive(&master, &Password::new(password.to_string()), kdf_params)?;
    recover(&derived, manifest, blob_factory, metadata_store, open_output).await
}

/// Recover using a quorum of Shamir shares (the master key and password are not
/// needed). See [`recover_with_password`] for the `open_output` contract.
pub async fn recover_with_shares<F, M, W>(
    mnemonics: &[String],
    manifest: Option<&Manifest>,
    blob_factory: &F,
    metadata_store: &M,
    open_output: impl FnOnce(&Filename) -> Result<W>,
) -> Result<Filename>
where
    F: BlobStoreFactory,
    M: MetadataStore,
    W: Write,
{
    let secret = slip39::combine(mnemonics)?;
    let derived = DerivedKey::from_secret_bytes(&secret[..])?;
    recover(&derived, manifest, blob_factory, metadata_store, open_output).await
}

async fn recover<F, M, W>(
    derived: &DerivedKey,
    manifest: Option<&Manifest>,
    blob_factory: &F,
    metadata_store: &M,
    open_output: impl FnOnce(&Filename) -> Result<W>,
) -> Result<Filename>
where
    F: BlobStoreFactory,
    M: MetadataStore,
    W: Write,
{
    let descriptor = locate(derived, manifest, metadata_store).await?;
    let file_key = kdf::derive_file_key(derived)?;

    // Download and verify the blob *before* opening the destination, so a
    // missing backup never leaves an empty output file behind.
    let blob = download(&descriptor, blob_factory).await?;
    let output = open_output(&descriptor.filename)?;
    cipher::decrypt(&file_key, &blob, output)?;
    Ok(descriptor.filename)
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
    const FILENAME: &str = "secret-notes.txt";

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
            Filename::parse(FILENAME).unwrap(),
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

        let name = recover_with_password(
            &f.nsec,
            &f.password,
            &fast_kdf(),
            None, // no manifest — must find the pointer on the relays
            &f.network,
            &f.relays,
            |_| Ok(&mut recovered),
        )
        .await
        .unwrap();

        assert_eq!(recovered, PLAINTEXT);
        assert_eq!(name.as_str(), FILENAME);
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
            |_| Ok(&mut recovered),
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

        let name = recover_with_shares(&quorum, None, &f.network, &f.relays, |_| Ok(&mut recovered))
            .await
            .unwrap();

        assert_eq!(recovered, PLAINTEXT);
        assert_eq!(name.as_str(), FILENAME);
    }

    #[tokio::test]
    async fn recovery_names_the_sink_from_the_backed_up_filename() {
        let f = backed_up().await;
        let mut chosen = None;
        let mut recovered = Vec::new();

        recover_with_password(
            &f.nsec,
            &f.password,
            &fast_kdf(),
            None,
            &f.network,
            &f.relays,
            |name| {
                chosen = Some(name.as_str().to_owned());
                Ok(&mut recovered)
            },
        )
        .await
        .unwrap();

        assert_eq!(chosen.as_deref(), Some(FILENAME));
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
            |_| Ok(&mut recovered),
        )
        .await;

        assert!(matches!(result, Err(Error::Decryption)));
    }

    #[tokio::test]
    async fn too_few_shares_cannot_recover() {
        let f = backed_up().await;
        let one = f.outcome.shares[..1].to_vec();
        let mut recovered = Vec::new();

        let result =
            recover_with_shares(&one, None, &f.network, &f.relays, |_| Ok(&mut recovered)).await;

        assert!(matches!(result, Err(Error::Shamir(_))));
    }
}
