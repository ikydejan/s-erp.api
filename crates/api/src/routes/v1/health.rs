use axum::routing::get;
use axum::Router;
use erp_core::ApiResponse;
use serde::Serialize;
use utoipa::ToSchema;

use crate::state::AppState;

#[derive(Serialize, ToSchema)]
pub struct HealthStatus {
    status: &'static str,
}

#[utoipa::path(
    get,
    path = "/api/v1/health",
    responses((status = 200, description = "Service is up", body = ApiResponse<HealthStatus>)),
    tag = "health"
)]
pub(crate) async fn health() -> axum::Json<ApiResponse<HealthStatus>> {
    axum::Json(ApiResponse::ok(HealthStatus { status: "ok" }))
}

pub fn router() -> Router<AppState> {
    Router::new().route("/health", get(health))
}
