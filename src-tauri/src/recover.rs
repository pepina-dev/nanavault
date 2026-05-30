//! Recovery orchestration: obtain the derived key (from the password or from
//! Shamir shares), locate the backup (via the manifest or the relays), download
//! and verify the blob, decrypt it, and write it into the destination directory
//! under its original name — atomically, so a failure never leaves a partial or
//! empty file and never clobbers an existing one.
//!
//! The two entry points differ only in how they reconstruct the derived key;
//! everything after that is shared.

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use tempfile::NamedTempFile;

use crate::blossom::{BlobStore, BlobStoreFactory};
use crate::crypto::cipher::{self, BlobHash};
use crate::crypto::kdf::{self, KdfParams};
use crate::crypto::slip39;
use crate::error::{Error, Result};
use crate::manifest::Manifest;
use crate::metadata::{self, BackupDescriptor, Filename};
use crate::relay::MetadataStore;
use crate::secret::{DerivedKey, MasterKey, Password};

/// Recover using the master key and password, writing the recovered file into
/// `output_dir` under its original name. Returns the path it was written to.
pub async fn recover_with_password<F, M>(
    master_nsec: &str,
    password: &str,
    kdf_params: &KdfParams,
    manifest: Option<&Manifest>,
    blob_factory: &F,
    metadata_store: &M,
    output_dir: &Path,
) -> Result<PathBuf>
where
    F: BlobStoreFactory,
    M: MetadataStore,
{
    let master = MasterKey::parse(master_nsec)?;
    let derived = kdf::derive(&master, &Password::new(password.to_string()), kdf_params)?;
    recover(&derived, manifest, blob_factory, metadata_store, output_dir).await
}

/// Recover using a quorum of Shamir shares (the master key and password are not
/// needed), writing the recovered file into `output_dir` under its original name.
pub async fn recover_with_shares<F, M>(
    mnemonics: &[String],
    manifest: Option<&Manifest>,
    blob_factory: &F,
    metadata_store: &M,
    output_dir: &Path,
) -> Result<PathBuf>
where
    F: BlobStoreFactory,
    M: MetadataStore,
{
    let secret = slip39::combine(mnemonics)?;
    let derived = DerivedKey::from_secret_bytes(&secret[..])?;
    recover(&derived, manifest, blob_factory, metadata_store, output_dir).await
}

async fn recover<F, M>(
    derived: &DerivedKey,
    manifest: Option<&Manifest>,
    blob_factory: &F,
    metadata_store: &M,
    output_dir: &Path,
) -> Result<PathBuf>
where
    F: BlobStoreFactory,
    M: MetadataStore,
{
    let descriptor = locate(derived, manifest, metadata_store).await?;
    let file_key = kdf::derive_file_key(derived)?;
    let blob = download(&descriptor, blob_factory).await?;

    write_atomically(output_dir, &descriptor.filename, |file| {
        cipher::decrypt(&file_key, &blob, file)
    })
}

/// How many numbered variants of a name to try before giving up. Reaching this
/// would mean the folder already holds a thousand same-named files — it is a
/// defensive bound, not a limit anyone should hit.
const MAX_DISAMBIGUATION_ATTEMPTS: u32 = 1000;

/// Write a file into `dir` under `name`, atomically: the plaintext is streamed to
/// a temporary file in the same directory and promoted by an atomic rename only
/// once `write` has fully succeeded. Any earlier failure — a wrong key, an I/O
/// error, a crash — discards the temporary file (it is removed on drop) and
/// leaves the destination untouched.
///
/// An existing file is never overwritten: if the name is taken, the next free
/// `stem (N).ext` is used instead. The already-written temporary file is reused
/// across attempts, so only the rename is retried, and `persist_noclobber` keeps
/// each attempt race-free.
fn write_atomically(
    dir: &Path,
    name: &Filename,
    write: impl FnOnce(&File) -> Result<()>,
) -> Result<PathBuf> {
    let mut temp = NamedTempFile::new_in(dir)?;
    write(temp.as_file())?;
    temp.as_file().sync_all()?;

    for attempt in 0..MAX_DISAMBIGUATION_ATTEMPTS {
        let destination = dir.join(candidate_name(name.as_str(), attempt));
        match temp.persist_noclobber(&destination) {
            Ok(_) => return Ok(destination),
            Err(e) if e.error.kind() == io::ErrorKind::AlreadyExists => temp = e.file,
            Err(e) => return Err(Error::Io(e.error)),
        }
    }
    Err(Error::DestinationUnavailable(name.as_str().to_owned()))
}

/// The name to try on the `attempt`-th rename: the original first, then
/// `stem (1).ext`, `stem (2).ext`, … The extension — the part after the last
/// dot, but never a leading dot — is preserved so the recovered file stays
/// openable.
fn candidate_name(name: &str, attempt: u32) -> String {
    if attempt == 0 {
        return name.to_owned();
    }
    let (stem, extension) = match name.rfind('.') {
        Some(dot) if dot > 0 => (&name[..dot], &name[dot..]),
        _ => (name, ""),
    };
    format!("{stem} ({attempt}){extension}")
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
        // The manifest skips the relay lookup that would otherwise prove the
        // key's identity, so check it here: reject a wrong key or password
        // before downloading the blob, exactly as the relay path does.
        if derived.public_key() != manifest.derived_pubkey {
            return Err(Error::ManifestKeyMismatch);
        }
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

    /// True when `dir` holds no entries — proof that a failed recovery left
    /// neither an output file nor a leftover temporary one.
    fn is_empty(dir: &Path) -> bool {
        std::fs::read_dir(dir).unwrap().next().is_none()
    }

    #[tokio::test]
    async fn password_path_recovers_via_relays() {
        let f = backed_up().await;
        let out = tempfile::tempdir().unwrap();

        let path = recover_with_password(
            &f.nsec,
            &f.password,
            &fast_kdf(),
            None, // no manifest — must find the pointer on the relays
            &f.network,
            &f.relays,
            out.path(),
        )
        .await
        .unwrap();

        assert_eq!(path, out.path().join(FILENAME));
        assert_eq!(std::fs::read(&path).unwrap(), PLAINTEXT);
    }

    #[tokio::test]
    async fn password_path_recovers_via_manifest_when_relays_are_empty() {
        let f = backed_up().await;
        let empty_relays = FakeRelays::new(vec!["wss://relay.dead".into()]);
        let out = tempfile::tempdir().unwrap();

        let path = recover_with_password(
            &f.nsec,
            &f.password,
            &fast_kdf(),
            Some(&f.outcome.manifest),
            &f.network,
            &empty_relays,
            out.path(),
        )
        .await
        .unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), PLAINTEXT);
    }

    #[tokio::test]
    async fn shares_path_recovers() {
        let f = backed_up().await;
        let quorum = f.outcome.shares[..2].to_vec();
        let out = tempfile::tempdir().unwrap();

        let path = recover_with_shares(&quorum, None, &f.network, &f.relays, out.path())
            .await
            .unwrap();

        assert_eq!(path, out.path().join(FILENAME));
        assert_eq!(std::fs::read(&path).unwrap(), PLAINTEXT);
    }

    #[tokio::test]
    async fn recovery_writes_a_numbered_copy_when_the_name_is_taken() {
        let f = backed_up().await;
        let out = tempfile::tempdir().unwrap();
        let taken = out.path().join(FILENAME);
        std::fs::write(&taken, b"the original, untouched").unwrap();

        let path = recover_with_password(
            &f.nsec,
            &f.password,
            &fast_kdf(),
            None,
            &f.network,
            &f.relays,
            out.path(),
        )
        .await
        .unwrap();

        // The recovered file lands beside the original under a numbered name,
        // keeping its extension; the original is left exactly as it was.
        assert_eq!(path, out.path().join("secret-notes (1).txt"));
        assert_eq!(std::fs::read(&path).unwrap(), PLAINTEXT);
        assert_eq!(std::fs::read(&taken).unwrap(), b"the original, untouched");
    }

    #[tokio::test]
    async fn recovery_increments_until_it_finds_a_free_name() {
        let f = backed_up().await;
        let out = tempfile::tempdir().unwrap();
        std::fs::write(out.path().join("secret-notes.txt"), b"first").unwrap();
        std::fs::write(out.path().join("secret-notes (1).txt"), b"second").unwrap();

        let path = recover_with_password(
            &f.nsec,
            &f.password,
            &fast_kdf(),
            None,
            &f.network,
            &f.relays,
            out.path(),
        )
        .await
        .unwrap();

        assert_eq!(path, out.path().join("secret-notes (2).txt"));
        assert_eq!(std::fs::read(&path).unwrap(), PLAINTEXT);
    }

    #[test]
    fn candidate_name_numbers_collisions_and_preserves_the_extension() {
        assert_eq!(candidate_name("report.pdf", 0), "report.pdf");
        assert_eq!(candidate_name("report.pdf", 1), "report (1).pdf");
        assert_eq!(candidate_name("report", 2), "report (2)");
        assert_eq!(candidate_name("archive.tar.gz", 1), "archive.tar (1).gz");
        assert_eq!(candidate_name(".bashrc", 1), ".bashrc (1)");
    }

    #[tokio::test]
    async fn a_wrong_password_is_rejected_without_downloading_the_blob() {
        let f = backed_up().await;
        // A blob network that never held the blob: had recovery tried to
        // download it, that would surface as a Blossom error. The manifest's
        // identity check must reject the wrong password before then.
        let no_blobs = FakeBlobNetwork::new();
        let out = tempfile::tempdir().unwrap();

        let result = recover_with_password(
            &f.nsec,
            "the wrong password",
            &fast_kdf(),
            Some(&f.outcome.manifest),
            &no_blobs,
            &f.relays,
            out.path(),
        )
        .await;

        assert!(matches!(result, Err(Error::ManifestKeyMismatch)));
        assert!(is_empty(out.path()));
    }

    #[tokio::test]
    async fn too_few_shares_cannot_recover() {
        let f = backed_up().await;
        let one = f.outcome.shares[..1].to_vec();
        let out = tempfile::tempdir().unwrap();

        let result = recover_with_shares(&one, None, &f.network, &f.relays, out.path()).await;

        assert!(matches!(result, Err(Error::Shamir(_))));
        assert!(is_empty(out.path()));
    }
}
