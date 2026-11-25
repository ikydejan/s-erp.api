use async_trait::async_trait;
use chrono::{DateTime, Utc};
use cqrs::{ItemView, ItemWriteRepository, NewItemCommand};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(FromRow)]
struct ItemRow {
    id: Uuid,
    sku: String,
    name: String,
    description: Option<String>,
    is_active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<ItemRow> for ItemView {
    fn from(r: ItemRow) -> Self {
        Self {
            id: r.id,
            sku: r.sku,
            name: r.name,
            description: r.description,
            is_active: r.is_active,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

/// Reference write-side implementation for the "items" CQRS module. `create`
/// writes the domain row, its audit trail entry, and its search_outbox event
/// in one transaction — copy this shape for the next ERP module.
#[derive(Clone)]
pub struct PostgresItemRepository {
    pool: PgPool,
}

impl PostgresItemRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ItemWriteRepository for PostgresItemRepository {
    async fn create(
        &self,
        cmd: NewItemCommand,
        actor_id: Uuid,
        acted_on_behalf_of: Option<Uuid>,
    ) -> anyhow::Result<ItemView> {
        let mut tx = self.pool.begin().await?;

        let row: ItemRow = sqlx::query_as(
            r#"
            INSERT INTO items (sku, name, description, created_by)
            VALUES ($1, $2, $3, $4)
            RETURNING id, sku, name, description, is_active, created_at, updated_at
            "#,
        )
        .bind(&cmd.sku)
        .bind(&cmd.name)
        .bind(&cmd.description)
        .bind(actor_id)
        .fetch_one(&mut *tx)
        .await?;

        let item: ItemView = row.into();
        let payload = serde_json::to_value(&item)?;

        sqlx::query(
            r#"
            INSERT INTO audit_logs (actor_user_id, acted_on_behalf_of, action, resource_type, resource_id, after_state)
            VALUES ($1, $2, 'DATA_CREATE', 'items', $3, $4)
            "#,
        )
        .bind(actor_id)
        .bind(acted_on_behalf_of)
        .bind(item.id.to_string())
        .bind(&payload)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO search_outbox (aggregate_type, aggregate_id, event_type, payload)
            VALUES ('items', $1, 'upsert', $2)
            "#,
        )
        .bind(item.id)
        .bind(&payload)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(item)
    }

    async fn get_by_id(&self, id: Uuid) -> anyhow::Result<Option<ItemView>> {
        let row: Option<ItemRow> =
            sqlx::query_as("SELECT id, sku, name, description, is_active, created_at, updated_at FROM items WHERE id = $1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(Into::into))
    }
}
