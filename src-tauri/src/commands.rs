//! Tauri command handlers: the boundary between the webview and the backend.
//!
//! Each command builds the real relay and Blossom adapters, derives or
//! reconstructs the key, and delegates to the orchestration layer. Secrets are
//! held only for the duration of the call; the `nsec` is wrapped so its buffer
//! is wiped on return.

use std::io::{Cursor, Write};
use std::path::Path;

use nostr::Keys;
use serde::Serialize;
use zeroize::Zeroizing;

use crate::backup::{self, BackupOutcome, UpdateOutcome};
use crate::blossom::BlossomStoreFactory;
use crate::crypto::kdf::{self, KdfParams};
use crate::crypto::slip39;
use crate::error::{Error, Result};
use crate::manifest::Manifest;
use crate::metadata::{ContentKind, Filename};
use crate::recover;
use crate::relay::RelayStore;
use crate::secret::{DerivedKey, MasterKey, Password};

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

    let master = MasterKey::from_key_or_backup_code(&nsec)?;
    let derived = kdf::derive(&master, &Password::new(password), &params)?;
    let factory = BlossomStoreFactory::authorized(derived.keys().clone());
    let store = RelayStore::connect(relays).await?;

    let plaintext = std::fs::File::open(&file_path)?;
    let filename = Filename::from_path(&file_path)?;
    let outcome = backup::run_backup(
        &derived,
        plaintext,
        filename,
        ContentKind::File,
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

/// The largest typed-text secret accepted for an in-app text backup, in bytes (its
/// UTF-8 length). Generous for any hand-typed or pasted note while keeping the
/// in-memory buffers and the upload bounded. The frontend mirrors this for a live
/// counter, but it is enforced here — the backend is the real boundary.
const MAX_TEXT_BYTES: usize = 10 * 1024 * 1024;

/// Reject typed text larger than [`MAX_TEXT_BYTES`], before any key or network work.
fn ensure_within_text_limit(text: &str) -> Result<()> {
    if text.len() > MAX_TEXT_BYTES {
        return Err(Error::TextTooLarge {
            limit: MAX_TEXT_BYTES,
        });
    }
    Ok(())
}

/// Encrypt the typed `text`, upload it, publish the pointer, and split the derived
/// key into shares — the text-input counterpart to [`backup`]. The text is stored
/// under a fixed name ([`Filename::text_backup`]) and recovers as in-app text
/// rather than a file.
#[tauri::command]
pub async fn backup_text(
    nsec: String,
    password: String,
    text: String,
    relays: Vec<String>,
    servers: Vec<String>,
    threshold: u8,
    share_count: u8,
) -> Result<BackupOutcome> {
    let nsec = Zeroizing::new(nsec);
    let text = Zeroizing::new(text);
    ensure_within_text_limit(&text)?;
    let params = KdfParams::default();

    let master = MasterKey::from_key_or_backup_code(&nsec)?;
    let derived = kdf::derive(&master, &Password::new(password), &params)?;
    let factory = BlossomStoreFactory::authorized(derived.keys().clone());
    let store = RelayStore::connect(relays).await?;

    let outcome = backup::run_backup(
        &derived,
        Cursor::new(text.as_bytes()),
        Filename::text_backup(),
        ContentKind::Text,
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

/// Easy mode: mint a fresh master secret and return it as a **backup code** — a
/// SLIP-0039 1-of-1 mnemonic, the same word format the guardians' shares use.
///
/// Key generation is a secret operation, so it stays in the backend. The code is
/// shown once in the UI, never persisted, and handed straight back to [`backup`]
/// (and later [`recover_with_password`]) in place of an `nsec`.
#[tauri::command]
pub fn generate_backup_code() -> Result<String> {
    let keys = Keys::generate();
    let secret = Zeroizing::new(keys.secret_key().to_secret_bytes());
    let mut codes = slip39::split(&secret[..], 1, 1)?;
    codes
        .pop()
        .ok_or(Error::Shamir(slip39::Error::InsufficientShares))
}

/// Write a recovery manifest to disk as JSON.
#[tauri::command]
pub fn export_manifest(manifest: Manifest, path: String) -> Result<()> {
    std::fs::write(&path, manifest.to_json()?)?;
    Ok(())
}

/// What a recovery returns to the webview: a file backup written to disk (its
/// path, to be finalized by [`save_recovered`]) or a text backup held in memory
/// for the app to display and edit. The string form of [`recover::Recovered`].
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum RecoveredResponse {
    File { path: String },
    Text { filename: String, content: String },
}

/// Recover the backup with the master key and password. Uses the manifest at
/// `manifest_path` if given, otherwise the relays. A file backup is written to a
/// temporary location and its path returned, to be finalized via
/// [`save_recovered`] (so the user never faces a folder picker before the file
/// exists); a text backup is returned in memory for the app to display and edit.
#[tauri::command]
pub async fn recover_with_password(
    nsec: String,
    password: String,
    relays: Vec<String>,
    manifest_path: Option<String>,
) -> Result<RecoveredResponse> {
    let nsec = Zeroizing::new(nsec);
    let params = KdfParams::default();
    let manifest = load_manifest(manifest_path)?;
    let temp_dir = temp_recover_dir()?;

    let factory = BlossomStoreFactory::read_only();
    let store = RelayStore::connect(relays).await?;

    let result = recover::recover_with_password(
        &nsec,
        &password,
        &params,
        manifest.as_ref(),
        &factory,
        &store,
        &temp_dir,
    )
    .await;

    store.shutdown().await;
    finish_recovery(&temp_dir, result)
}

/// Recover the backup from a quorum of Shamir shares; the master key and password
/// are not needed. Like [`recover_with_password`], a file backup is written to a
/// temporary location to be finalized via [`save_recovered`], and a text backup is
/// returned in memory.
#[tauri::command]
pub async fn recover_with_shares(
    shares: Vec<String>,
    relays: Vec<String>,
    manifest_path: Option<String>,
) -> Result<RecoveredResponse> {
    let manifest = load_manifest(manifest_path)?;
    let temp_dir = temp_recover_dir()?;

    let factory = BlossomStoreFactory::read_only();
    let store = RelayStore::connect(relays).await?;

    let result =
        recover::recover_with_shares(&shares, manifest.as_ref(), &factory, &store, &temp_dir).await;

    store.shutdown().await;
    finish_recovery(&temp_dir, result)
}

/// Shape a recovery result for the webview, releasing the per-recovery temporary
/// directory unless it now holds a recovered file. A text recovery keeps its
/// plaintext in memory and a failed recovery writes nothing, so in both of those
/// cases the directory is empty and is removed.
fn finish_recovery(
    temp_dir: &Path,
    result: Result<recover::Recovered>,
) -> Result<RecoveredResponse> {
    match result {
        Ok(recover::Recovered::File(path)) => Ok(RecoveredResponse::File {
            path: path.to_string_lossy().into_owned(),
        }),
        Ok(recover::Recovered::Text { filename, content }) => {
            let _ = std::fs::remove_dir_all(temp_dir);
            Ok(RecoveredResponse::Text {
                filename: filename.as_str().to_owned(),
                content,
            })
        }
        Err(e) => {
            let _ = std::fs::remove_dir_all(temp_dir);
            Err(e)
        }
    }
}

/// Re-encrypt edited `text` and republish the pointer under the same identity,
/// reached with the master key and password. The shares stay valid — the identity
/// is unchanged — so none are produced. Used when the user edits a recovered text
/// backup and saves it.
#[tauri::command]
pub async fn resave_text_with_password(
    nsec: String,
    password: String,
    text: String,
    relays: Vec<String>,
    servers: Vec<String>,
) -> Result<UpdateOutcome> {
    let nsec = Zeroizing::new(nsec);
    let text = Zeroizing::new(text);
    ensure_within_text_limit(&text)?;
    let params = KdfParams::default();

    let master = MasterKey::from_key_or_backup_code(&nsec)?;
    let derived = kdf::derive(&master, &Password::new(password), &params)?;
    let factory = BlossomStoreFactory::authorized(derived.keys().clone());
    let store = RelayStore::connect(relays).await?;

    let outcome = backup::update(
        &derived,
        Cursor::new(text.as_bytes()),
        Filename::text_backup(),
        ContentKind::Text,
        &servers,
        &params,
        &factory,
        &store,
    )
    .await;

    store.shutdown().await;
    outcome
}

/// Re-encrypt edited `text` and republish the pointer under the same identity,
/// reached with a quorum of Shamir shares. As with [`resave_text_with_password`],
/// the shares are unchanged, so none are produced.
#[tauri::command]
pub async fn resave_text_with_shares(
    shares: Vec<String>,
    text: String,
    relays: Vec<String>,
    servers: Vec<String>,
) -> Result<UpdateOutcome> {
    let text = Zeroizing::new(text);
    ensure_within_text_limit(&text)?;
    let params = KdfParams::default();

    let secret = slip39::combine(&shares)?;
    let derived = DerivedKey::from_secret_bytes(&secret[..])?;
    let factory = BlossomStoreFactory::authorized(derived.keys().clone());
    let store = RelayStore::connect(relays).await?;

    let outcome = backup::update(
        &derived,
        Cursor::new(text.as_bytes()),
        Filename::text_backup(),
        ContentKind::Text,
        &servers,
        &params,
        &factory,
        &store,
    )
    .await;

    store.shutdown().await;
    outcome
}

/// Move a freshly recovered file out of its private temporary directory into the
/// folder the user just chose, keeping its original name. On a name clash the next
/// free `name (1).ext`, `name (2).ext`, … is used, so an existing file is never
/// overwritten. Returns the final path.
#[tauri::command]
pub fn save_recovered(source_path: String, output_dir: String) -> Result<String> {
    let source = Path::new(&source_path);
    let name = Filename::from_path(&source_path)?;

    let saved = recover::write_atomically(Path::new(&output_dir), &name, |out| {
        let mut destination = out; // `&File` implements `Write`
        std::io::copy(&mut std::fs::File::open(source)?, &mut destination)?;
        Ok(())
    })?;

    // The recovered file lived alone in a private per-recovery directory; now that
    // it has a permanent home, drop both the file and that directory.
    let _ = std::fs::remove_file(source);
    if let Some(temp_dir) = source.parent() {
        let _ = std::fs::remove_dir(temp_dir);
    }

    Ok(saved.to_string_lossy().into_owned())
}

/// Write recovered or edited text into the folder the user chose, under the fixed
/// text-backup name. On a name clash the next free `name (1).txt`, … is used, so an
/// existing file is never overwritten. Returns the final path. This is the
/// counterpart to [`save_recovered`] for a text backup, which lives in memory until
/// the user asks to keep a copy on disk.
#[tauri::command]
pub fn save_text(content: String, output_dir: String) -> Result<String> {
    let content = Zeroizing::new(content);

    let saved =
        recover::write_atomically(Path::new(&output_dir), &Filename::text_backup(), |out| {
            let mut destination = out; // `&File` implements `Write`
            destination.write_all(content.as_bytes())?;
            Ok(())
        })?;

    Ok(saved.to_string_lossy().into_owned())
}

/// A fresh, private directory for a single recovery, under a per-app scratch
/// folder in the OS temp dir. Each recovery gets its own, so the recovered file
/// always keeps its exact original name — never one disambiguated against another
/// recovery's leftovers — and [`save_recovered`] can reclaim it by simply removing
/// this directory.
fn temp_recover_dir() -> Result<std::path::PathBuf> {
    let base = std::env::temp_dir().join("nanavault-recovered");
    std::fs::create_dir_all(&base)?;
    Ok(tempfile::Builder::new()
        .prefix("r-")
        .tempdir_in(&base)?
        .keep())
}

/// Read and parse a recovery manifest from disk, if a path was given.
fn load_manifest(path: Option<String>) -> Result<Option<Manifest>> {
    match path {
        Some(path) => Ok(Some(Manifest::from_json(&std::fs::read_to_string(path)?)?)),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn backup_text_rejects_text_over_the_limit() {
        // The size check runs before any key or network work, so this returns
        // without touching the (empty) relay and server lists.
        let oversized = "a".repeat(MAX_TEXT_BYTES + 1);

        let result = backup_text(
            "ignored".into(),
            "ignored".into(),
            oversized,
            vec![],
            vec![],
            2,
            3,
        )
        .await;

        assert!(matches!(result, Err(Error::TextTooLarge { .. })));
    }

    #[test]
    fn save_text_writes_the_fixed_name_and_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_string_lossy().into_owned();

        let first = save_text("hello".into(), dir_path.clone()).unwrap();
        assert_eq!(
            Path::new(&first),
            dir.path().join(Filename::text_backup().as_str())
        );
        assert_eq!(std::fs::read_to_string(&first).unwrap(), "hello");

        // A second save must not clobber the first; it lands under a numbered name.
        let second = save_text("world".into(), dir_path).unwrap();
        assert_eq!(
            Path::new(&second),
            dir.path().join("nanavault-secret-file (1).txt")
        );
        assert_eq!(std::fs::read_to_string(&first).unwrap(), "hello");
        assert_eq!(std::fs::read_to_string(&second).unwrap(), "world");
    }

    #[test]
    fn save_recovered_keeps_the_original_name_and_removes_the_temp_source() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("report.pdf");
        std::fs::write(&source, b"the recovered bytes").unwrap();

        let dest_dir = tempfile::tempdir().unwrap();
        let saved = save_recovered(
            source.to_string_lossy().into_owned(),
            dest_dir.path().to_string_lossy().into_owned(),
        )
        .unwrap();

        assert_eq!(Path::new(&saved), dest_dir.path().join("report.pdf"));
        assert_eq!(std::fs::read(&saved).unwrap(), b"the recovered bytes");
        assert!(
            !source.exists(),
            "the temporary copy is removed once it is saved"
        );
    }

    #[test]
    fn save_recovered_never_overwrites_an_existing_file() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("report.pdf");
        std::fs::write(&source, b"the recovered bytes").unwrap();

        let dest_dir = tempfile::tempdir().unwrap();
        let occupied = dest_dir.path().join("report.pdf");
        std::fs::write(&occupied, b"a different file already here").unwrap();

        let saved = save_recovered(
            source.to_string_lossy().into_owned(),
            dest_dir.path().to_string_lossy().into_owned(),
        )
        .unwrap();

        // The recovered file lands beside the existing one under a numbered name,
        // and the file that was already there is left exactly as it was.
        assert_eq!(Path::new(&saved), dest_dir.path().join("report (1).pdf"));
        assert_eq!(std::fs::read(&saved).unwrap(), b"the recovered bytes");
        assert_eq!(
            std::fs::read(&occupied).unwrap(),
            b"a different file already here"
        );
    }

    #[test]
    fn each_recovery_gets_its_own_empty_temp_directory() {
        // A private directory per recovery is what lets the recovered file keep its
        // exact original name: a fresh directory can never collide, so `save_recovered`
        // always sees the real name rather than a `name (1)` left by an earlier one.
        let first = temp_recover_dir().unwrap();
        let second = temp_recover_dir().unwrap();

        assert_ne!(first, second, "two recoveries must not share a directory");
        assert!(
            std::fs::read_dir(&first).unwrap().next().is_none(),
            "a fresh recovery dir is empty"
        );
        assert!(
            std::fs::read_dir(&second).unwrap().next().is_none(),
            "a fresh recovery dir is empty"
        );

        let _ = std::fs::remove_dir(&first);
        let _ = std::fs::remove_dir(&second);
    }
}
