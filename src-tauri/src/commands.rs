//! Tauri command handlers: the boundary between the webview and the backend.
//!
//! Each command builds the real relay and Blossom adapters, derives or
//! reconstructs the key, and delegates to the orchestration layer. Secrets are
//! held only for the duration of the call; the `nsec` is wrapped so its buffer
//! is wiped on return.

use std::path::Path;

use zeroize::Zeroizing;

use crate::backup::{self, BackupOutcome};
use crate::blossom::BlossomStoreFactory;
use crate::crypto::kdf::{self, KdfParams};
use crate::error::Result;
use crate::manifest::Manifest;
use crate::metadata::Filename;
use crate::recover;
use crate::relay::RelayStore;
use crate::secret::{MasterKey, Password};

/// Encrypt `file_path`, upload it to the Blossom `servers`, publish the pointer
/// to the `relays`, and split the derived key into `threshold`-of-`share_count`
/// mnemonic shares.
#[tauri::command]
pub async fn backup(
    nsec: String,
    password: String,
    file_path: String,
    relays: Vec<String>,
    servers: Vec<String>,
    threshold: u8,
    share_count: u8,
) -> Result<BackupOutcome> {
    let nsec = Zeroizing::new(nsec);
    let params = KdfParams::default();

    let master = MasterKey::parse(&nsec)?;
    let derived = kdf::derive(&master, &Password::new(password), &params)?;
    let factory = BlossomStoreFactory::authorized(derived.keys().clone());
    let store = RelayStore::connect(relays).await?;

    let plaintext = std::fs::File::open(&file_path)?;
    let filename = Filename::from_path(&file_path)?;
    let outcome = backup::run_backup(
        &derived,
        plaintext,
        filename,
        &servers,
        threshold,
        share_count,
        &params,
        &factory,
        &store,
    )
    .await;

    store.shutdown().await;
    outcome
}

/// Write a recovery manifest to disk as JSON.
#[tauri::command]
pub fn export_manifest(manifest: Manifest, path: String) -> Result<()> {
    std::fs::write(&path, manifest.to_json()?)?;
    Ok(())
}

/// Recover the backup with the master key and password into `output_dir`, using
/// the file's original name. Uses the manifest at `manifest_path` if given,
/// otherwise the relays. Returns the full path the file was written to.
#[tauri::command]
pub async fn recover_with_password(
    nsec: String,
    password: String,
    output_dir: String,
    relays: Vec<String>,
    manifest_path: Option<String>,
) -> Result<String> {
    let nsec = Zeroizing::new(nsec);
    let params = KdfParams::default();
    let manifest = load_manifest(manifest_path)?;

    let factory = BlossomStoreFactory::read_only();
    let store = RelayStore::connect(relays).await?;

    let result = recover::recover_with_password(
        &nsec,
        &password,
        &params,
        manifest.as_ref(),
        &factory,
        &store,
        Path::new(&output_dir),
    )
    .await;

    store.shutdown().await;
    Ok(result?.to_string_lossy().into_owned())
}

/// Recover the backup from a quorum of Shamir shares into `output_dir`, using the
/// file's original name. The master key and password are not needed. Returns the
/// full path the file was written to.
#[tauri::command]
pub async fn recover_with_shares(
    shares: Vec<String>,
    output_dir: String,
    relays: Vec<String>,
    manifest_path: Option<String>,
) -> Result<String> {
    let manifest = load_manifest(manifest_path)?;

    let factory = BlossomStoreFactory::read_only();
    let store = RelayStore::connect(relays).await?;

    let result = recover::recover_with_shares(
        &shares,
        manifest.as_ref(),
        &factory,
        &store,
        Path::new(&output_dir),
    )
    .await;

    store.shutdown().await;
    Ok(result?.to_string_lossy().into_owned())
}

/// Read and parse a recovery manifest from disk, if a path was given.
fn load_manifest(path: Option<String>) -> Result<Option<Manifest>> {
    match path {
        Some(path) => Ok(Some(Manifest::from_json(&std::fs::read_to_string(path)?)?)),
        None => Ok(None),
    }
}
