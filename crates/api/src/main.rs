mod config;
mod middleware;
mod openapi;
mod routes;
mod state;

use std::sync::Arc;

use audit::AuditLogger;
use cqrs::{ItemSearchRepository, ItemWriteRepository};
use delegation::DelegationRepository;
use infra_opensearch::OpenSearchItemRepository;
use infra_postgres::{
    PostgresAuditLogger, PostgresDelegationRepository, PostgresItemRepository,
    PostgresNotificationRepository, UserRepository,
};
use notification::NotificationRepository;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::config::AppConfig;
use crate::openapi::ApiDoc;
use crate::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = AppConfig::from_env()?;

    let db = infra_postgres::connect(&config.database_url).await?;
    infra_postgres::run_migrations(&db).await?;
    tracing::info!("database migrations applied");

    let opensearch_client = infra_opensearch::build_client(&config.opensearch_url)?;
    let item_search_repo = OpenSearchItemRepository::new(opensearch_client);
    item_search_repo.ensure_index().await?;
    let item_search_repo = Arc::new(item_search_repo);

    let state = AppState {
        config: Arc::new(config.clone()),
        users: UserRepository::new(db.clone()),
        audit: Arc::new(PostgresAuditLogger::new(db.clone())) as Arc<dyn AuditLogger>,
        delegations: Arc::new(PostgresDelegationRepository::new(db.clone()))
            as Arc<dyn DelegationRepository>,
        notifications: Arc::new(PostgresNotificationRepository::new(db.clone()))
            as Arc<dyn NotificationRepository>,
        items_write: Arc::new(PostgresItemRepository::new(db.clone()))
            as Arc<dyn ItemWriteRepository>,
        items_search: item_search_repo as Arc<dyn ItemSearchRepository>,
    };

    let app = axum::Router::new()
        .merge(routes::router())
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = format!("{}:{}", config.host, config.port);
    tracing::info!(%addr, "starting api");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
