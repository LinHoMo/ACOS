//! In-memory implementation of the event and artifact stores.
//!
//! Used for the MVP and tests. SQLite-backed implementations land in Phase 2.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;

use acos_core::error::AcosError;
use acos_core::id::{ArtifactId, RunId};
use acos_core::traits::{ArtifactStore, Event, EventStore};

/// (run id, artifact kind, artifact bytes)
type ArtifactEntry = (RunId, String, Vec<u8>);

/// An in-memory event + artifact store.
#[derive(Debug, Clone)]
pub struct InMemoryStore {
    events: Arc<Mutex<HashMap<RunId, Vec<Event>>>>,
    artifacts: Arc<Mutex<HashMap<ArtifactId, ArtifactEntry>>>,
    seq: Arc<Mutex<HashMap<RunId, u64>>>,
}

impl InMemoryStore {
    /// Creates a new, empty in-memory store.
    pub fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(HashMap::new())),
            artifacts: Arc::new(Mutex::new(HashMap::new())),
            seq: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Returns all events for a run (test helper).
    pub async fn events_for(&self, run_id: RunId) -> Vec<Event> {
        self.events
            .lock()
            .await
            .get(&run_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Returns all artifact names for a run (test helper).
    pub async fn artifact_names(&self, run_id: RunId) -> Vec<String> {
        self.artifacts
            .lock()
            .await
            .values()
            .filter(|(r, _, _)| *r == run_id)
            .map(|(_, name, _)| name.clone())
            .collect()
    }
}

impl Default for InMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventStore for InMemoryStore {
    async fn append(
        &self,
        run_id: RunId,
        event_type: String,
        payload: serde_json::Value,
    ) -> Result<Event, AcosError> {
        let mut seq = self.seq.lock().await;
        let mut events = self.events.lock().await;

        let run_seq = seq.entry(run_id).or_insert(0);
        *run_seq += 1;
        let seq_num = *run_seq;

        let event = Event {
            seq: seq_num,
            run_id,
            event_type,
            payload,
        };

        events
            .entry(run_id)
            .or_default()
            .push(event.clone());

        Ok(event)
    }

    async fn query(&self, run_id: RunId) -> Result<Vec<Event>, AcosError> {
        Ok(self.events_for(run_id).await)
    }

    async fn replay(&self, run_id: RunId) -> Result<Vec<Event>, AcosError> {
        Ok(self.events_for(run_id).await)
    }
}

#[async_trait]
impl ArtifactStore for InMemoryStore {
    async fn put(
        &self,
        run_id: RunId,
        name: String,
        content: Vec<u8>,
    ) -> Result<ArtifactId, AcosError> {
        let id = ArtifactId::new();
        self.artifacts
            .lock()
            .await
            .insert(id, (run_id, name, content));
        Ok(id)
    }

    async fn get(&self, id: ArtifactId) -> Result<Vec<u8>, AcosError> {
        self.artifacts
            .lock()
            .await
            .get(&id)
            .map(|(_, _, content)| content.clone())
            .ok_or_else(|| AcosError::ValidationFailure {
                message: format!("artifact {id:?} not found"),
            })
    }

    async fn get_by_name(&self, run_id: RunId, name: &str) -> Result<Vec<u8>, AcosError> {
        self.artifacts
            .lock()
            .await
            .iter()
            .find(|(_, (rid, n, _))| *rid == run_id && n == name)
            .map(|(_, (_, _, content))| content.clone())
            .ok_or_else(|| AcosError::ValidationFailure {
                message: format!("artifact '{name}' not found for run {run_id:?}"),
            })
    }
}