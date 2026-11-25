mod config;
mod jobs;

use config::WorkerConfig;
use infra_opensearch::OpenSearchItemRepository;
use infra_postgres::{
    PostgresDelegationRepository, PostgresNotificationRepository, SearchOutboxRepository,
};
use notification::VapidConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = WorkerConfig::from_env()?;

    let db = infra_postgres::connect(&config.database_url).await?;
    infra_postgres::run_migrations(&db).await?;

    let opensearch_client = infra_opensearch::build_client(&config.opensearch_url)?;
    let item_search_repo = OpenSearchItemRepository::new(opensearch_client);
    item_search_repo.ensure_index().await?;

    let outbox_repo = SearchOutboxRepository::new(db.clone());
    let notification_repo = PostgresNotificationRepository::new(db.clone());
    let delegation_repo = PostgresDelegationRepository::new(db.clone());

    let vapid = VapidConfig {
        private_key_base64: config.vapid_private_key.clone(),
        subject: config.vapid_subject.clone(),
    };

    tracing::info!("worker starting: outbox dispatcher, notification sender, delegation sweeper");

    tokio::join!(
        jobs::outbox::run(outbox_repo, item_search_repo),
        jobs::notifications::run(notification_repo.clone(), vapid),
        jobs::delegations::run(delegation_repo, notification_repo),
    );

    Ok(())
}
