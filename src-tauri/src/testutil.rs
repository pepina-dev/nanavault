//! In-memory doubles for the relay and Blossom ports, used by the orchestration
//! tests. They model the two system boundaries (nostr relays, Blossom servers)
//! so backup and recovery can be tested end to end without a network.

#![cfg(test)]

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use nostr::{Event, Kind, PublicKey};

use crate::blossom::{BlobStore, BlobStoreFactory};
use crate::crypto::cipher::BlobHash;
use crate::error::{Error, Result};
use crate::relay::MetadataStore;

/// Blobs held by one server, keyed by hash hex.
type ServerBlobs = HashMap<String, Vec<u8>>;

/// A set of fake Blossom servers sharing one backing store. Cloning shares the
/// same storage, so a store handed out by [`BlobStoreFactory::store`] sees blobs
/// uploaded through any other handle to the same server.
#[derive(Clone, Default)]
pub struct FakeBlobNetwork {
    blobs: Arc<Mutex<HashMap<String, ServerBlobs>>>,
    offline: Arc<Mutex<HashSet<String>>>,
}

impl FakeBlobNetwork {
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark a server as offline: uploads and downloads to it then fail.
    pub fn take_offline(&self, server_url: &str) {
        self.offline.lock().unwrap().insert(server_url.to_string());
    }
}

impl BlobStoreFactory for FakeBlobNetwork {
    type Store = FakeBlobStore;

    fn store(&self, server_url: &str) -> Result<FakeBlobStore> {
        Ok(FakeBlobStore {
            server_url: server_url.to_string(),
            network: self.clone(),
        })
    }
}

pub struct FakeBlobStore {
    server_url: String,
    network: FakeBlobNetwork,
}

impl FakeBlobStore {
    fn is_offline(&self) -> bool {
        self.network
            .offline
            .lock()
            .unwrap()
            .contains(&self.server_url)
    }
}

#[async_trait]
impl BlobStore for FakeBlobStore {
    async fn upload(&self, blob: &[u8]) -> Result<()> {
        if self.is_offline() {
            return Err(Error::Blossom(format!("{}: offline", self.server_url)));
        }
        self.network
            .blobs
            .lock()
            .unwrap()
            .entry(self.server_url.clone())
            .or_default()
            .insert(BlobHash::of(blob).to_hex(), blob.to_vec());
        Ok(())
    }

    async fn download(&self, hash: &BlobHash) -> Result<Vec<u8>> {
        if self.is_offline() {
            return Err(Error::Blossom(format!("{}: offline", self.server_url)));
        }
        self.network
            .blobs
            .lock()
            .unwrap()
            .get(&self.server_url)
            .and_then(|server| server.get(&hash.to_hex()).cloned())
            .ok_or_else(|| Error::Blossom(format!("{}: blob not found", self.server_url)))
    }
}

/// A fake relay set: an append-only event log with replaceable-event semantics
/// on read (newest matching event wins).
#[derive(Clone)]
pub struct FakeRelays {
    events: Arc<Mutex<Vec<Event>>>,
    relays: Vec<String>,
}

impl FakeRelays {
    pub fn new(relays: Vec<String>) -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
            relays,
        }
    }
}

#[async_trait]
impl MetadataStore for FakeRelays {
    fn relays(&self) -> Vec<String> {
        self.relays.clone()
    }

    async fn publish(&self, event: &Event) -> Result<()> {
        self.events.lock().unwrap().push(event.clone());
        Ok(())
    }

    async fn fetch_latest(&self, author: &PublicKey, kind: Kind) -> Result<Option<Event>> {
        Ok(self
            .events
            .lock()
            .unwrap()
            .iter()
            .filter(|event| event.pubkey == *author && event.kind == kind)
            .max_by_key(|event| event.created_at)
            .cloned())
    }
}
