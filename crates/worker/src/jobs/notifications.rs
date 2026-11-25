use std::time::Duration;

use infra_postgres::PostgresNotificationRepository;
use notification::{send_push, NotificationMessage, NotificationRepository, VapidConfig};

const POLL_INTERVAL: Duration = Duration::from_secs(5);
const BATCH_SIZE: i64 = 50;
const MAX_ATTEMPTS: i16 = 10;

/// Drains `notification_outbox` and delivers each message via Web Push,
/// honoring per-user, per-type preferences. Delivery works even with the
/// browser closed, as long as the OS/browser's push service keeps running.
pub async fn run(repo: PostgresNotificationRepository, vapid: VapidConfig) {
    if vapid.private_key_base64.is_empty() {
        tracing::warn!(
            "VAPID_PRIVATE_KEY not set — notification sender will dead-letter every message"
        );
    }

    loop {
        let messages = match repo.fetch_pending(BATCH_SIZE).await {
            Ok(messages) => messages,
            Err(e) => {
                tracing::error!(error = %e, "failed to fetch pending notifications");
                tokio::time::sleep(POLL_INTERVAL).await;
                continue;
            }
        };

        if messages.is_empty() {
            tokio::time::sleep(POLL_INTERVAL).await;
            continue;
        }

        for message in messages {
            if let Err(e) = send_one(&repo, &vapid, &message).await {
                tracing::error!(error = %e, notification_id = message.id, "notification dispatch bookkeeping failed");
            }
        }
    }
}

async fn send_one(
    repo: &PostgresNotificationRepository,
    vapid: &VapidConfig,
    message: &NotificationMessage,
) -> anyhow::Result<()> {
    if !repo
        .is_enabled(message.user_id, &message.notification_type)
        .await?
    {
        return repo.mark_sent(message.id).await;
    }

    let subscriptions = repo.active_subscriptions(message.user_id).await?;
    if subscriptions.is_empty() {
        return repo.mark_sent(message.id).await;
    }

    let payload = serde_json::json!({
        "title": message.title,
        "body": message.body,
        "data": message.data,
    })
    .to_string();

    let mut delivered = false;
    for subscription in &subscriptions {
        match send_push(subscription, vapid, &payload).await {
            Ok(true) => delivered = true,
            Ok(false) => {
                let _ = repo
                    .revoke_subscription(subscription.user_id, &subscription.endpoint)
                    .await;
            }
            Err(e) => {
                tracing::warn!(error = %e, endpoint = %subscription.endpoint, "push send failed")
            }
        }
    }

    if delivered {
        repo.mark_sent(message.id).await
    } else {
        let dead_letter = message.attempts + 1 >= MAX_ATTEMPTS;
        repo.mark_failed(message.id, "no subscription accepted delivery", dead_letter)
            .await
    }
}
