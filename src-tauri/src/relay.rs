//! Pointer transport: the [`MetadataStore`] port and its nostr-relay adapter.
//!
//! A `MetadataStore` is the set of relays a backup is published to and read
//! from. The orchestration layer depends only on the port; the real adapter
//! wraps a [`nostr_sdk::Client`] connected to the configured relays.

use std::time::Duration;

use async_trait::async_trait;
use nostr::{Event, Filter, Kind, PublicKey};
use nostr_sdk::Client;

use crate::error::{Error, Result};

/// How long to wait for relays to answer a fetch.
const FETCH_TIMEOUT: Duration = Duration::from_secs(10);

/// Publishes and retrieves backup pointer events across a set of relays.
#[async_trait]
pub trait MetadataStore: Send + Sync {
    /// The relays this store uses, recorded in the manifest.
    fn relays(&self) -> Vec<String>;

    /// Publish a signed pointer event.
    async fn publish(&self, event: &Event) -> Result<()>;

    /// Fetch the latest event by the given author and kind, if any. For a
    /// replaceable kind there is at most one current event per author, but we
    /// still take the newest defensively in case relays disagree.
    async fn fetch_latest(&self, author: &PublicKey, kind: Kind) -> Result<Option<Event>>;
}

/// A live connection to a set of nostr relays.
pub struct RelayStore {
    client: Client,
    relays: Vec<String>,
}

impl RelayStore {
    /// Connect to the given relays.
    pub async fn connect(relays: Vec<String>) -> Result<Self> {
        let client = Client::default();
        for relay in &relays {
            client
                .add_relay(relay.as_str())
                .await
                .map_err(|e| Error::Relay(format!("could not add relay {relay}: {e}")))?;
        }
        client.connect().await;
        Ok(Self { client, relays })
    }

    /// Disconnect from all relays.
    pub async fn shutdown(&self) {
        self.client.disconnect().await;
    }
}

#[async_trait]
impl MetadataStore for RelayStore {
    fn relays(&self) -> Vec<String> {
        self.relays.clone()
    }

    async fn publish(&self, event: &Event) -> Result<()> {
        self.client
            .send_event(event)
            .await
            .map(|_| ())
            .map_err(|e| Error::Relay(e.to_string()))
    }

    async fn fetch_latest(&self, author: &PublicKey, kind: Kind) -> Result<Option<Event>> {
        let filter = Filter::new().author(*author).kind(kind).limit(1);
        let events = self
            .client
            .fetch_events(filter, FETCH_TIMEOUT)
            .await
            .map_err(|e| Error::Relay(e.to_string()))?;
        Ok(events.into_iter().max_by_key(|event| event.created_at))
    }
}
