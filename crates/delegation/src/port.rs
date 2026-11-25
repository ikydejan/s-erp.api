use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::model::Delegation;

#[async_trait]
pub trait DelegationRepository: Send + Sync {
    async fn create(&self, delegation: &Delegation) -> anyhow::Result<()>;

    /// All delegations where `delegate_id` is currently acting for someone else,
    /// active at `now`. Used to resolve `RequestContext::acted_on_behalf_of`.
    async fn active_for_delegate(
        &self,
        delegate_id: Uuid,
        now: DateTime<Utc>,
    ) -> anyhow::Result<Vec<Delegation>>;

    async fn revoke(&self, id: Uuid, delegator_id: Uuid) -> anyhow::Result<bool>;

    /// Flips expired-but-still-`active` rows to `expired`; returns the ones just expired
    /// (so the caller can enqueue "delegation ended" notifications).
    async fn expire_due(&self, now: DateTime<Utc>) -> anyhow::Result<Vec<Delegation>>;
}
