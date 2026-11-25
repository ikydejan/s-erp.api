use async_trait::async_trait;
use chrono::{DateTime, Utc};
use delegation::{Delegation, DelegationRepository, DelegationStatus};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(FromRow)]
struct DelegationRow {
    id: Uuid,
    delegator_id: Uuid,
    delegate_id: Uuid,
    scope: Option<serde_json::Value>,
    reason: Option<String>,
    start_at: DateTime<Utc>,
    end_at: DateTime<Utc>,
    status: String,
    created_at: DateTime<Utc>,
}

impl DelegationRow {
    fn into_domain(self) -> Delegation {
        Delegation {
            id: self.id,
            delegator_id: self.delegator_id,
            delegate_id: self.delegate_id,
            scope: self.scope,
            reason: self.reason,
            start_at: self.start_at,
            end_at: self.end_at,
            status: DelegationStatus::parse(&self.status).unwrap_or(DelegationStatus::Expired),
            created_at: self.created_at,
        }
    }
}

#[derive(Clone)]
pub struct PostgresDelegationRepository {
    pool: PgPool,
}

impl PostgresDelegationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DelegationRepository for PostgresDelegationRepository {
    async fn create(&self, delegation: &Delegation) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO delegations (id, delegator_id, delegate_id, scope, reason, start_at, end_at, status)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(delegation.id)
        .bind(delegation.delegator_id)
        .bind(delegation.delegate_id)
        .bind(&delegation.scope)
        .bind(&delegation.reason)
        .bind(delegation.start_at)
        .bind(delegation.end_at)
        .bind(delegation.status.as_str())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn active_for_delegate(
        &self,
        delegate_id: Uuid,
        now: DateTime<Utc>,
    ) -> anyhow::Result<Vec<Delegation>> {
        let rows: Vec<DelegationRow> = sqlx::query_as(
            r#"
            SELECT * FROM delegations
            WHERE delegate_id = $1 AND status = 'active' AND start_at <= $2 AND end_at > $2
            "#,
        )
        .bind(delegate_id)
        .bind(now)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(DelegationRow::into_domain).collect())
    }

    async fn revoke(&self, id: Uuid, delegator_id: Uuid) -> anyhow::Result<bool> {
        let result =
            sqlx::query("UPDATE delegations SET status = 'revoked' WHERE id = $1 AND delegator_id = $2 AND status = 'active'")
                .bind(id)
                .bind(delegator_id)
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn expire_due(&self, now: DateTime<Utc>) -> anyhow::Result<Vec<Delegation>> {
        let rows: Vec<DelegationRow> = sqlx::query_as(
            r#"
            UPDATE delegations
            SET status = 'expired'
            WHERE status = 'active' AND end_at <= $1
            RETURNING *
            "#,
        )
        .bind(now)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(DelegationRow::into_domain).collect())
    }
}
