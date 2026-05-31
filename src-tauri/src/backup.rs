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
use crate::metadata::{self, BackupDescriptor, ContentKind, Filename};
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

/// What a content update hands back: the new blob's hash and the refreshed
/// recovery manifest.
///
/// Unlike [`BackupOutcome`] it carries no shares — an update keeps the same
/// derived identity, so the shares produced at backup time still reconstruct it
/// and are deliberately not regenerated.
#[derive(Serialize)]
pub struct UpdateOutcome {
    /// The new encrypted blob's hash, for reference.
    pub blob_sha256: String,
    /// The recovery manifest, refreshed to point at the new blob.
    pub manifest: Manifest,
}

/// Encrypt the plaintext, upload it to the servers that accept it, publish the
/// pointer, and assemble the recovery manifest. The shared core of a first
/// backup ([`run_backup`]) and a later content update ([`update`]).
#[allow(clippy::too_many_arguments)]
async fn store<F, M>(
    derived: &DerivedKey,
    plaintext: impl Read,
    filename: Filename,
    kind: ContentKind,
    servers: &[String],
    kdf_params: &KdfParams,
    blob_factory: &F,
    metadata_store: &M,
) -> Result<Manifest>
where
    F: BlobStoreFactory,
    M: MetadataStore,
{
    let file_key = kdf::derive_file_key(derived)?;

    let blob = cipher::encrypt(&file_key, plaintext)?;
    let blob_size = blob.len() as u64;

    let stored_servers = upload(&blob, servers, blob_factory).await?;

    let descriptor = BackupDescriptor::new(BlobHash::of(&blob), stored_servers, filename, kind);
    let pointer = metadata::build_pointer(&descriptor, derived)?;
    metadata_store.publish(&pointer).await?;

    Ok(Manifest::new(
        descriptor,
        metadata_store.relays(),
        derived.public_key(),
        blob_size,
        *kdf_params,
    ))
}

/// Run a full backup with an already-derived key.
///
/// The caller derives the key (and so owns the master key and password); this
/// keeps key derivation out of the orchestration layer and lets the caller
/// build the authorized Blossom factory from the same key without deriving it
/// twice. `filename` is the name recovery restores the secret under and `kind`
/// records whether it is a file or typed text; `kdf_params` is recorded in the
/// manifest for transparency.
#[allow(clippy::too_many_arguments)]
pub async fn run_backup<F, M>(
    derived: &DerivedKey,
    plaintext: impl Read,
    filename: Filename,
    kind: ContentKind,
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
    let manifest = store(
        derived,
        plaintext,
        filename,
        kind,
        servers,
        kdf_params,
        blob_factory,
        metadata_store,
    )
    .await?;

    let secret = derived.secret_bytes();
    let shares = slip39::split(&secret[..], threshold, share_count)?;

    let derived_npub = derived
        .public_key()
        .to_bech32()
        .map_err(|e| Error::Pointer(e.to_string()))?;

    Ok(BackupOutcome {
        derived_npub,
        blob_sha256: manifest.descriptor.blob_sha256.to_hex(),
        shares,
        manifest,
    })
}

/// Re-store new `plaintext` for an existing backup: encrypt the new content,
/// upload it, and republish the pointer under the same derived identity. No
/// shares are produced — the identity is unchanged, so the shares handed out at
/// backup time still reconstruct it. This is how an edited text backup is saved.
#[allow(clippy::too_many_arguments)]
pub async fn update<F, M>(
    derived: &DerivedKey,
    plaintext: impl Read,
    filename: Filename,
    kind: ContentKind,
    servers: &[String],
    kdf_params: &KdfParams,
    blob_factory: &F,
    metadata_store: &M,
) -> Result<UpdateOutcome>
where
    F: BlobStoreFactory,
    M: MetadataStore,
{
    let manifest = store(
        derived,
        plaintext,
        filename,
        kind,
        servers,
        kdf_params,
        blob_factory,
        metadata_store,
    )
    .await?;

    Ok(UpdateOutcome {
        blob_sha256: manifest.descriptor.blob_sha256.to_hex(),
        manifest,
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
    async fn a_text_backup_records_its_kind_and_fixed_name() {
        let network = FakeBlobNetwork::new();
        let relays = FakeRelays::new(vec!["wss://relay.one".into()]);

        let outcome = run_backup(
            &derived(),
            Cursor::new(b"a secret typed straight into the app".to_vec()),
            Filename::text_backup(),
            ContentKind::Text,
            &["https://blossom.one".into()],
            2,
            3,
            &fast_kdf(),
            &network,
            &relays,
        )
        .await
        .unwrap();

        assert_eq!(outcome.manifest.descriptor.kind, ContentKind::Text);
        assert_eq!(
            outcome.manifest.descriptor.filename.as_str(),
            "nanavault-secret-file.txt"
        );
    }

    #[tokio::test]
    async fn an_update_republishes_new_content_under_the_same_identity() {
        let network = FakeBlobNetwork::new();
        let relays = FakeRelays::new(vec!["wss://relay.one".into()]);
        let derived = derived();
        let servers = ["https://blossom.one".to_string()];

        let first = run_backup(
            &derived,
            Cursor::new(b"first version".to_vec()),
            Filename::text_backup(),
            ContentKind::Text,
            &servers,
            2,
            3,
            &fast_kdf(),
            &network,
            &relays,
        )
        .await
        .unwrap();

        let updated = update(
            &derived,
            Cursor::new(b"second version".to_vec()),
            Filename::text_backup(),
            ContentKind::Text,
            &servers,
            &fast_kdf(),
            &network,
            &relays,
        )
        .await
        .unwrap();

        // New content means a new blob, but the same identity, kind, and name.
        assert_ne!(first.blob_sha256, updated.blob_sha256);
        assert_eq!(updated.manifest.derived_pubkey, derived.public_key());
        assert_eq!(updated.manifest.descriptor.kind, ContentKind::Text);
        assert_eq!(
            updated.manifest.descriptor.blob_sha256.to_hex(),
            updated.blob_sha256
        );

        // The relays now serve a pointer to the new blob, so recovery finds the
        // updated content rather than the original.
        let event = relays
            .fetch_latest(&derived.public_key(), metadata::kind())
            .await
            .unwrap()
            .expect("the update republished a pointer");
        let descriptor = metadata::parse_pointer(&event, &derived).unwrap();
        assert_eq!(descriptor.blob_sha256.to_hex(), updated.blob_sha256);
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
            ContentKind::File,
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
            ContentKind::File,
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
            ContentKind::File,
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
