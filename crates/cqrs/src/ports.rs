use async_trait::async_trait;
use uuid::Uuid;

use crate::dto::{ItemSearchQuery, ItemView, NewItemCommand};

/// Write side: Postgres is the source of truth. `create` is expected to insert
/// the row, the matching `audit_logs` entry, and the `search_outbox` event in
/// one transaction.
#[async_trait]
pub trait ItemWriteRepository: Send + Sync {
    /// `acted_on_behalf_of` is set when the caller was acting under an active
    /// delegation, so the audit trail can record both the real actor and whose
    /// authority they were using.
    async fn create(
        &self,
        cmd: NewItemCommand,
        actor_id: Uuid,
        acted_on_behalf_of: Option<Uuid>,
    ) -> anyhow::Result<ItemView>;
    async fn get_by_id(&self, id: Uuid) -> anyhow::Result<Option<ItemView>>;
}

/// Read side: OpenSearch, kept in sync by the outbox dispatcher.
#[async_trait]
pub trait ItemSearchRepository: Send + Sync {
    async fn search(&self, query: ItemSearchQuery) -> anyhow::Result<(Vec<ItemView>, u64)>;
}

/// Implemented by `infra-opensearch`, driven by the `worker` outbox dispatcher.
#[async_trait]
pub trait SearchIndexer: Send + Sync {
    async fn upsert_item(&self, item: &ItemView) -> anyhow::Result<()>;
    async fn delete_item(&self, id: Uuid) -> anyhow::Result<()>;
}
