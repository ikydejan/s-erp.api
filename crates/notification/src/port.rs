use async_trait::async_trait;
use serde_json::Value;
use uuid::Uuid;

use crate::model::{NotificationMessage, NotificationPreference, PushSubscription};

#[async_trait]
pub trait NotificationRepository: Send + Sync {
    async fn save_subscription(
        &self,
        user_id: Uuid,
        endpoint: &str,
        p256dh: &str,
        auth: &str,
        user_agent: Option<&str>,
    ) -> anyhow::Result<()>;
    async fn revoke_subscription(&self, user_id: Uuid, endpoint: &str) -> anyhow::Result<()>;
    async fn active_subscriptions(&self, user_id: Uuid) -> anyhow::Result<Vec<PushSubscription>>;

    async fn set_preference(
        &self,
        user_id: Uuid,
        notification_type: &str,
        enabled: bool,
    ) -> anyhow::Result<()>;
    async fn is_enabled(&self, user_id: Uuid, notification_type: &str) -> anyhow::Result<bool>;
    async fn list_preferences(&self, user_id: Uuid) -> anyhow::Result<Vec<NotificationPreference>>;

    async fn enqueue(
        &self,
        user_id: Uuid,
        notification_type: &str,
        title: &str,
        body: &str,
        data: Value,
    ) -> anyhow::Result<()>;
    async fn fetch_pending(&self, limit: i64) -> anyhow::Result<Vec<NotificationMessage>>;
    async fn mark_sent(&self, id: i64) -> anyhow::Result<()>;
    async fn mark_failed(&self, id: i64, error: &str, dead_letter: bool) -> anyhow::Result<()>;
}
