use std::sync::Arc;

use audit::AuditLogger;
use cqrs::{ItemSearchRepository, ItemWriteRepository};
use delegation::DelegationRepository;
use infra_postgres::UserRepository;
use notification::NotificationRepository;

use crate::config::AppConfig;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub users: UserRepository,
    pub audit: Arc<dyn AuditLogger>,
    pub delegations: Arc<dyn DelegationRepository>,
    pub notifications: Arc<dyn NotificationRepository>,
    pub items_write: Arc<dyn ItemWriteRepository>,
    pub items_search: Arc<dyn ItemSearchRepository>,
}
