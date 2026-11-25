pub(crate) mod auth;
pub(crate) mod delegations;
pub(crate) mod health;
pub(crate) mod items;
pub(crate) mod notifications;

use axum::Router;

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .merge(health::router())
        .merge(auth::router())
        .merge(items::router())
        .merge(delegations::router())
        .merge(notifications::router())
}
