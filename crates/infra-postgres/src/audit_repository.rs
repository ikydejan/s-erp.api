use async_trait::async_trait;
use audit::{AuditEntry, AuditLogger};
use sqlx::PgPool;

/// Writes to the append-only `audit_logs` table. In production this pool
/// should be opened with the restricted `erp_app` role (INSERT+SELECT only on
/// this table — see migration 0003) so even a compromised application process
/// cannot tamper with past entries.
#[derive(Clone)]
pub struct PostgresAuditLogger {
    pool: PgPool,
}

impl PostgresAuditLogger {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AuditLogger for PostgresAuditLogger {
    async fn record(&self, entry: AuditEntry) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO audit_logs
                (actor_user_id, acted_on_behalf_of, action, resource_type, resource_id,
                 before_state, after_state, ip_address, user_agent, metadata, occurred_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8::text::inet, $9, $10, $11)
            "#,
        )
        .bind(entry.actor_user_id)
        .bind(entry.acted_on_behalf_of)
        .bind(entry.action.as_str())
        .bind(entry.resource_type)
        .bind(entry.resource_id)
        .bind(entry.before_state)
        .bind(entry.after_state)
        .bind(entry.ip_address)
        .bind(entry.user_agent)
        .bind(entry.metadata)
        .bind(entry.occurred_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
