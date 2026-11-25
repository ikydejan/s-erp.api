pub mod v1;

use axum::Router;

use crate::state::AppState;

/// All versions nest under their own nest path — `/api/v1` today, `/api/v2`
/// later for a specific module — so a new version never touches old routes.
pub fn router() -> Router<AppState> {
    Router::new().nest("/api/v1", v1::router())
}
