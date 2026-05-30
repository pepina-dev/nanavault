//! Backup orchestration: encrypt → upload → publish the pointer → split into
//! shares → assemble the manifest.
//!
//! This layer is generic over the [`BlobStoreFactory`] and [`MetadataStore`]
//! ports, so it is exercised end to end against in-memory fakes; the Tauri
//! command supplies the real Blossom and relay adapters.

use std::io::Read;

use nostr::ToBech32;
use serde::Serialize;

use crate::blossom::{BlobStore, BlobStoreFactory};
use crate::crypto::cipher::{self, BlobHash};
use crate::crypto::kdf::{self, KdfParams};
use crate::crypto::slip39;
use crate::error::{Error, Result};
use crate::manifest::Manifest;
use crate::metadata::{self, BackupDescriptor, Filename};
use crate::relay::MetadataStore;
use crate::secret::DerivedKey;

/// What a successful backup hands back to the caller.
///
/// `shares` are secret — any `threshold` of them reconstruct the backup — so
/// this type intentionally has no `Debug`: it must never be logged.
#[derive(Serialize)]
pub struct BackupOutcome {
    /// The derived identity (npub) that owns this backup.
    pub derived_npub: String,
    /// The encrypted blob's hash, for reference.
    pub blob_sha256: String,
    /// The SLIP-0039 mnemonic shares, to be distributed to trusted people.
    pub shares: Vec<String>,
    /// The exportable recovery manifest.
    pub manifest: Manifest,
}

/// Run a full backup with an already-derived key.
///
/// The caller derives the key (and so owns the master key and password); this
/// keeps key derivation out of the orchestration layer and lets the caller
/// build the authorized Blossom factory from the same key without deriving it
/// twice. `filename` is the original name of the file, recorded so recovery can
/// restore it; `kdf_params` is recorded in the manifest for transparency.
#[allow(clippy::too_many_arguments)]
pub async fn run_backup<F, M>(
    derived: &DerivedKey,
    plaintext: impl Read,
    filename: Filename,
    servers: &[String],
    threshold: u8,
    share_count: u8,
    kdf_params: &KdfParams,
    blob_factory: &F,
    metadata_store: &M,
) -> Result<BackupOutcome>
where
    F: BlobStoreFactory,
    M: MetadataStore,
{
    let file_key = kdf::derive_file_key(derived)?;

    let blob = cipher::encrypt(&file_key, plaintext)?;
    let blob_hash = BlobHash::of(&blob);
    let blob_size = blob.len() as u64;

    let stored_servers = upload(&blob, servers, blob_factory).await?;

    let descriptor = BackupDescriptor::new(blob_hash, stored_servers, filename);
    let pointer = metadata::build_pointer(&descriptor, derived)?;
    metadata_store.publish(&pointer).await?;

    let secret = derived.secret_bytes();
    let shares = slip39::split(&secret[..], threshold, share_count)?;

    let derived_npub = derived
        .public_key()
        .to_bech32()
        .map_err(|e| Error::Pointer(e.to_string()))?;

    Ok(BackupOutcome {
        derived_npub,
        blob_sha256: blob_hash.to_hex(),
        shares,
        manifest: Manifest::new(descriptor, metadata_store.relays(), blob_size, *kdf_params),
    })
}

/// Upload the blob to each server, returning the ones that accepted it. At least
/// one must succeed.
async fn upload<F>(blob: &[u8], servers: &[String], factory: &F) -> Result<Vec<String>>
where
    F: BlobStoreFactory,
{
    let mut stored = Vec::new();
    let mut last_error = None;

    for server in servers {
        let result = match factory.store(server) {
            Ok(store) => store.upload(blob).await,
            Err(e) => Err(e),
        };
        match result {
            Ok(()) => stored.push(server.clone()),
            Err(e) => last_error = Some(e),
        }
    }

    if stored.is_empty() {
        return Err(
            last_error.unwrap_or_else(|| Error::Blossom("no Blossom servers were provided".into()))
        );
    }
    Ok(stored)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::kdf::KdfAlg;
    use crate::secret::{MasterKey, Password};
    use crate::testutil::{FakeBlobNetwork, FakeRelays};
    use nostr::Keys;
    use std::io::Cursor;

    /// Cheap KDF parameters so tests don't spend 256 MiB per derivation.
    fn fast_kdf() -> KdfParams {
        KdfParams {
            alg: KdfAlg::Argon2id,
            m: 32,
            t: 1,
            p: 1,
        }
    }

    fn derived() -> DerivedKey {
        let master = MasterKey::parse(&Keys::generate().secret_key().to_secret_hex()).unwrap();
        kdf::derive(&master, &Password::new("pw".into()), &fast_kdf()).unwrap()
    }

    #[tokio::test]
    async fn a_backup_produces_shares_a_manifest_and_uploads() {
        let network = FakeBlobNetwork::new();
        let relays = FakeRelays::new(vec!["wss://relay.one".into()]);
        let servers = vec!["https://blossom.one".into(), "https://blossom.two".into()];

        let outcome = run_backup(
            &derived(),
            Cursor::new(b"my important file".to_vec()),
            Filename::parse("important.txt").unwrap(),
            &servers,
            2,
            3,
            &fast_kdf(),
            &network,
            &relays,
        )
        .await
        .unwrap();

        assert_eq!(outcome.shares.len(), 3);
        assert_eq!(outcome.manifest.descriptor.servers, servers);
        assert_eq!(
            outcome.manifest.descriptor.filename.as_str(),
            "important.txt"
        );
        assert_eq!(outcome.manifest.relays, vec!["wss://relay.one".to_string()]);
        assert!(outcome.derived_npub.starts_with("npub1"));
    }

    #[tokio::test]
    async fn a_backup_tolerates_one_server_being_offline() {
        let network = FakeBlobNetwork::new();
        network.take_offline("https://blossom.down");
        let relays = FakeRelays::new(vec!["wss://relay.one".into()]);

        let outcome = run_backup(
            &derived(),
            Cursor::new(b"data".to_vec()),
            Filename::parse("data.bin").unwrap(),
            &["https://blossom.down".into(), "https://blossom.up".into()],
            2,
            3,
            &fast_kdf(),
            &network,
            &relays,
        )
        .await
        .unwrap();

        // Only the reachable server is recorded.
        assert_eq!(
            outcome.manifest.descriptor.servers,
            vec!["https://blossom.up"]
        );
    }

    #[tokio::test]
    async fn a_backup_fails_when_every_server_is_offline() {
        let network = FakeBlobNetwork::new();
        network.take_offline("https://blossom.down");
        let relays = FakeRelays::new(vec!["wss://relay.one".into()]);

        let result = run_backup(
            &derived(),
            Cursor::new(b"data".to_vec()),
            Filename::parse("data.bin").unwrap(),
            &["https://blossom.down".into()],
            2,
            3,
            &fast_kdf(),
            &network,
            &relays,
        )
        .await;

        assert!(matches!(result, Err(Error::Blossom(_))));
    }
}
