//! Blob storage: the [`BlobStore`] port and its Blossom adapter.
//!
//! A `BlobStore` is a single Blossom server. The orchestration layer talks only
//! to the port, so it can be exercised with in-memory fakes; the real adapter
//! wraps [`nostr_blossom`], which builds the kind-24242 authorization event
//! internally from the signer we hand it. A [`BlobStoreFactory`] makes a store
//! for a given server URL — recovery needs this because it only learns the
//! server list after reading the backup pointer.

use async_trait::async_trait;
use std::str::FromStr;

use nostr::hashes::sha256::Hash as Sha256Hash;
use nostr::Keys;
use nostr_blossom::client::BlossomClient;
use url::Url;

use crate::crypto::cipher::BlobHash;
use crate::error::{Error, Result};

/// One Blossom server: upload a blob, or download one by its hash.
#[async_trait]
pub trait BlobStore: Send + Sync {
    /// Store a blob. The server addresses it by its SHA-256.
    async fn upload(&self, blob: &[u8]) -> Result<()>;

    /// Fetch the blob with the given hash. Integrity is the caller's
    /// responsibility — a server could return anything.
    async fn download(&self, hash: &BlobHash) -> Result<Vec<u8>>;
}

/// Builds a [`BlobStore`] for a server URL.
pub trait BlobStoreFactory: Send + Sync {
    type Store: BlobStore;

    fn store(&self, server_url: &str) -> Result<Self::Store>;
}

/// A real Blossom server. Uploads require the signing key (Blossom authorizes
/// them with a kind-24242 event); downloads are unauthenticated.
pub struct BlossomStore {
    server_url: String,
    client: BlossomClient,
    signer: Option<Keys>,
}

#[async_trait]
impl BlobStore for BlossomStore {
    async fn upload(&self, blob: &[u8]) -> Result<()> {
        let signer = self
            .signer
            .as_ref()
            .ok_or_else(|| Error::Blossom("this store is read-only; cannot upload".into()))?;
        self.client
            .upload_blob(blob.to_vec(), None, None, Some(signer))
            .await
            .map(|_| ())
            .map_err(|e| Error::Blossom(format!("{}: upload failed: {e}", self.server_url)))
    }

    async fn download(&self, hash: &BlobHash) -> Result<Vec<u8>> {
        let sha = Sha256Hash::from_str(&hash.to_hex())
            .map_err(|e| Error::Blossom(format!("invalid blob hash: {e}")))?;
        self.client
            .get_blob::<Keys>(sha, None, None, None)
            .await
            .map_err(|e| Error::Blossom(format!("{}: download failed: {e}", self.server_url)))
    }
}

/// Makes [`BlossomStore`]s. Backup uses [`authorized`](Self::authorized) so
/// uploads can be signed; recovery uses [`read_only`](Self::read_only), since
/// downloads need no key.
pub struct BlossomStoreFactory {
    signer: Option<Keys>,
}

impl BlossomStoreFactory {
    /// A factory whose stores can upload, signed by `signer`.
    pub fn authorized(signer: Keys) -> Self {
        Self {
            signer: Some(signer),
        }
    }

    /// A factory whose stores can only download.
    pub fn read_only() -> Self {
        Self { signer: None }
    }
}

impl BlobStoreFactory for BlossomStoreFactory {
    type Store = BlossomStore;

    fn store(&self, server_url: &str) -> Result<BlossomStore> {
        let url = Url::parse(server_url)
            .map_err(|e| Error::Blossom(format!("invalid server URL '{server_url}': {e}")))?;
        Ok(BlossomStore {
            server_url: server_url.to_string(),
            client: BlossomClient::new(url),
            signer: self.signer.clone(),
        })
    }
}
