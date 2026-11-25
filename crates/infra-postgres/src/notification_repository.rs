use async_trait::async_trait;
use chrono::{DateTime, Utc};
use notification::{
    NotificationMessage, NotificationPreference, NotificationRepository, PushSubscription,
};
use serde_json::Value;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(FromRow)]
struct PushSubscriptionRow {
    id: Uuid,
    user_id: Uuid,
    endpoint: String,
    p256dh_key: String,
    auth_key: String,
    user_agent: Option<String>,
    created_at: DateTime<Utc>,
    revoked_at: Option<DateTime<Utc>>,
}

impl From<PushSubscriptionRow> for PushSubscription {
    fn from(r: PushSubscriptionRow) -> Self {
        Self {
            id: r.id,
            user_id: r.user_id,
            endpoint: r.endpoint,
            p256dh_key: r.p256dh_key,
            auth_key: r.auth_key,
            user_agent: r.user_agent,
            created_at: r.created_at,
            revoked_at: r.revoked_at,
        }
    }
}

#[derive(FromRow)]
struct NotificationPreferenceRow {
    user_id: Uuid,
    notification_type: String,
    enabled: bool,
}

impl From<NotificationPreferenceRow> for NotificationPreference {
    fn from(r: NotificationPreferenceRow) -> Self {
        Self {
            user_id: r.user_id,
            notification_type: r.notification_type,
            enabled: r.enabled,
        }
    }
}

#[derive(FromRow)]
struct NotificationMessageRow {
    id: i64,
    user_id: Uuid,
    notification_type: String,
    title: String,
    body: String,
    data: Value,
    attempts: i16,
}

impl From<NotificationMessageRow> for NotificationMessage {
    fn from(r: NotificationMessageRow) -> Self {
        Self {
            id: r.id,
            user_id: r.user_id,
            notification_type: r.notification_type,
            title: r.title,
            body: r.body,
            data: r.data,
            attempts: r.attempts,
        }
    }
}

#[derive(Clone)]
pub struct PostgresNotificationRepository {
    pool: PgPool,
}

impl PostgresNotificationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl NotificationRepository for PostgresNotificationRepository {
    async fn save_subscription(
        &self,
        user_id: Uuid,
        endpoint: &str,
        p256dh: &str,
        auth: &str,
        user_agent: Option<&str>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO push_subscriptions (user_id, endpoint, p256dh_key, auth_key, user_agent)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (endpoint) DO UPDATE
                SET p256dh_key = EXCLUDED.p256dh_key,
                    auth_key = EXCLUDED.auth_key,
                    user_agent = EXCLUDED.user_agent,
                    revoked_at = NULL
            "#,
        )
        .bind(user_id)
        .bind(endpoint)
        .bind(p256dh)
        .bind(auth)
        .bind(user_agent)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn revoke_subscription(&self, user_id: Uuid, endpoint: &str) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE push_subscriptions SET revoked_at = now() WHERE user_id = $1 AND endpoint = $2",
        )
        .bind(user_id)
        .bind(endpoint)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn active_subscriptions(&self, user_id: Uuid) -> anyhow::Result<Vec<PushSubscription>> {
        let rows: Vec<PushSubscriptionRow> = sqlx::query_as(
            "SELECT id, user_id, endpoint, p256dh_key, auth_key, user_agent, created_at, revoked_at
             FROM push_subscriptions WHERE user_id = $1 AND revoked_at IS NULL",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn set_preference(
        &self,
        user_id: Uuid,
        notification_type: &str,
        enabled: bool,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO notification_preferences (user_id, notification_type, enabled)
            VALUES ($1, $2, $3)
            ON CONFLICT (user_id, notification_type) DO UPDATE SET enabled = EXCLUDED.enabled, updated_at = now()
            "#,
        )
        .bind(user_id)
        .bind(notification_type)
        .bind(enabled)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn is_enabled(&self, user_id: Uuid, notification_type: &str) -> anyhow::Result<bool> {
        let enabled: Option<bool> =
            sqlx::query_scalar("SELECT enabled FROM notification_preferences WHERE user_id = $1 AND notification_type = $2")
                .bind(user_id)
                .bind(notification_type)
                .fetch_optional(&self.pool)
                .await?;
        // No row yet means the user never toggled it — default to enabled.
        Ok(enabled.unwrap_or(true))
    }

    async fn list_preferences(&self, user_id: Uuid) -> anyhow::Result<Vec<NotificationPreference>> {
        let rows: Vec<NotificationPreferenceRow> =
            sqlx::query_as("SELECT user_id, notification_type, enabled FROM notification_preferences WHERE user_id = $1")
                .bind(user_id)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn enqueue(
        &self,
        user_id: Uuid,
        notification_type: &str,
        title: &str,
        body: &str,
        data: Value,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO notification_outbox (user_id, notification_type, title, body, data) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(user_id)
        .bind(notification_type)
        .bind(title)
        .bind(body)
        .bind(data)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn fetch_pending(&self, limit: i64) -> anyhow::Result<Vec<NotificationMessage>> {
        let rows: Vec<NotificationMessageRow> = sqlx::query_as(
            r#"
            SELECT id, user_id, notification_type, title, body, data, attempts
            FROM notification_outbox
            WHERE sent_at IS NULL AND NOT failed
            ORDER BY created_at
            LIMIT $1
            FOR UPDATE SKIP LOCKED
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn mark_sent(&self, id: i64) -> anyhow::Result<()> {
        sqlx::query("UPDATE notification_outbox SET sent_at = now() WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn mark_failed(&self, id: i64, error: &str, dead_letter: bool) -> anyhow::Result<()> {
        sqlx::query("UPDATE notification_outbox SET attempts = attempts + 1, last_error = $2, failed = $3 WHERE id = $1")
            .bind(id)
            .bind(error)
            .bind(dead_letter)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
