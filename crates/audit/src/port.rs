use async_trait::async_trait;

use crate::entry::AuditEntry;

/// Infra-agnostic sink for audit entries. `infra-postgres` implements this
/// against the append-only `audit_logs` table (INSERT/SELECT-only DB grant).
#[async_trait]
pub trait AuditLogger: Send + Sync {
    async fn record(&self, entry: AuditEntry) -> anyhow::Result<()>;
}
