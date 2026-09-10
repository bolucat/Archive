//! Private effective configuration reporting. Hosts decide how snapshots are stored.
use std::sync::Arc;

use serde_yaml_ng::Mapping;

use crate::ConfigRevision;

/// A committed full configuration, never a graceful-switch bootstrap document.
/// Contains credentials: keep it out of general status events and Debug output.
#[derive(Clone)]
pub struct EffectiveConfigSnapshot {
    pub instance_id: uuid::Uuid,
    pub revision: ConfigRevision,
    pub config: Arc<Mapping>,
}

impl std::fmt::Debug for EffectiveConfigSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EffectiveConfigSnapshot")
            .field("instance_id", &self.instance_id)
            .field("revision", &self.revision.id())
            .finish_non_exhaustive()
    }
}

/// A coalescing commit notification reader. It exposes owned snapshots, never
/// watch guards that could hold a manager publication lock across user work.
pub struct ConfigCommitSubscription {
    pub(crate) receiver: tokio::sync::watch::Receiver<Option<EffectiveConfigSnapshot>>,
}
impl ConfigCommitSubscription {
    pub fn latest(&self) -> Option<EffectiveConfigSnapshot> {
        self.receiver.borrow().clone()
    }
    /// Returns None once the manager is dropped. Slow consumers see the latest
    /// commit; this is not a durable event log or a current-running-state feed.
    pub async fn changed(&mut self) -> Option<EffectiveConfigSnapshot> {
        self.receiver.changed().await.ok()?;
        self.receiver.borrow_and_update().clone()
    }
}
