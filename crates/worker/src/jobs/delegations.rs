use std::time::Duration;

use chrono::Utc;
use delegation::DelegationRepository;
use infra_postgres::{PostgresDelegationRepository, PostgresNotificationRepository};
use notification::NotificationRepository;

const POLL_INTERVAL: Duration = Duration::from_secs(60);

/// Flips past-due delegations to `expired` and lets the delegator know their
/// stand-in's access just ended.
pub async fn run(
    delegations: PostgresDelegationRepository,
    notifications: PostgresNotificationRepository,
) {
    loop {
        match delegations.expire_due(Utc::now()).await {
            Ok(expired) => {
                for d in expired {
                    let _ = notifications
                        .enqueue(
                            d.delegator_id,
                            "delegation_expired",
                            "Your delegation has ended",
                            &format!("Delegation to user {} has expired", d.delegate_id),
                            serde_json::json!({ "delegation_id": d.id }),
                        )
                        .await;
                }
            }
            Err(e) => tracing::error!(error = %e, "failed to sweep expired delegations"),
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}
