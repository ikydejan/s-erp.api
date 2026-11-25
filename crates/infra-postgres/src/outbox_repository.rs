use sqlx::{FromRow, PgPool};
use uuid::Uuid;

const MAX_ATTEMPTS: i16 = 10;

#[derive(Debug, Clone, FromRow)]
pub struct PendingSearchEvent {
    pub id: i64,
    pub aggregate_type: String,
    pub aggregate_id: Uuid,
    pub event_type: String,
    pub payload: serde_json::Value,
}

/// Drains `search_outbox`, written transactionally alongside domain writes by
/// each module's write repository (see `item_repository.rs`). Polled by the
/// `worker` binary's outbox dispatcher.
#[derive(Clone)]
pub struct SearchOutboxRepository {
    pool: PgPool,
}

impl SearchOutboxRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn fetch_pending(&self, limit: i64) -> anyhow::Result<Vec<PendingSearchEvent>> {
        let rows = sqlx::query_as::<_, PendingSearchEvent>(
            r#"
            SELECT id, aggregate_type, aggregate_id, event_type, payload
            FROM search_outbox
            WHERE dispatched_at IS NULL AND NOT failed
            ORDER BY created_at
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn mark_dispatched(&self, id: i64) -> anyhow::Result<()> {
        sqlx::query("UPDATE search_outbox SET dispatched_at = now() WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Records a failed attempt; dead-letters the event (stops retrying) once
    /// `MAX_ATTEMPTS` is reached, so a persistently broken event can't spin
    /// the dispatcher forever — it needs manual investigation at that point.
    pub async fn mark_failed(&self, id: i64, error: &str) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE search_outbox
            SET attempts = attempts + 1,
                last_error = $2,
                failed = (attempts + 1) >= $3
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(error)
        .bind(MAX_ATTEMPTS)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
