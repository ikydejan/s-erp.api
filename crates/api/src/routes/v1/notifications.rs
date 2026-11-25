use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use erp_core::{ApiResponse, AppError};
use notification::NotificationPreference;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::middleware::AuthContext;
use crate::state::AppState;

#[derive(Deserialize, ToSchema)]
pub struct SubscribeRequest {
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
    pub user_agent: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct UnsubscribeRequest {
    pub endpoint: String,
}

#[derive(Deserialize, ToSchema)]
pub struct SetPreferenceRequest {
    pub notification_type: String,
    pub enabled: bool,
}

#[utoipa::path(
    post, path = "/api/v1/notifications/subscribe",
    request_body = SubscribeRequest,
    responses((status = 200, body = ApiResponse<()>)),
    security(("bearer_auth" = [])),
    tag = "notifications"
)]
pub(crate) async fn subscribe(
    AuthContext(ctx): AuthContext,
    State(state): State<AppState>,
    Json(req): Json<SubscribeRequest>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    state
        .notifications
        .save_subscription(
            ctx.user_id,
            &req.endpoint,
            &req.p256dh,
            &req.auth,
            req.user_agent.as_deref(),
        )
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(ApiResponse::ok(())))
}

#[utoipa::path(
    delete, path = "/api/v1/notifications/subscribe",
    request_body = UnsubscribeRequest,
    responses((status = 200, body = ApiResponse<()>)),
    security(("bearer_auth" = [])),
    tag = "notifications"
)]
pub(crate) async fn unsubscribe(
    AuthContext(ctx): AuthContext,
    State(state): State<AppState>,
    Json(req): Json<UnsubscribeRequest>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    state
        .notifications
        .revoke_subscription(ctx.user_id, &req.endpoint)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(ApiResponse::ok(())))
}

#[utoipa::path(
    get, path = "/api/v1/notifications/preferences",
    responses((status = 200, body = ApiResponse<Vec<NotificationPreference>>)),
    security(("bearer_auth" = [])),
    tag = "notifications"
)]
pub(crate) async fn list_preferences(
    AuthContext(ctx): AuthContext,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<NotificationPreference>>>, AppError> {
    let prefs = state
        .notifications
        .list_preferences(ctx.user_id)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(ApiResponse::ok(prefs)))
}

#[utoipa::path(
    patch, path = "/api/v1/notifications/preferences",
    request_body = SetPreferenceRequest,
    responses((status = 200, body = ApiResponse<()>)),
    security(("bearer_auth" = [])),
    tag = "notifications"
)]
pub(crate) async fn set_preference(
    AuthContext(ctx): AuthContext,
    State(state): State<AppState>,
    Json(req): Json<SetPreferenceRequest>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    state
        .notifications
        .set_preference(ctx.user_id, &req.notification_type, req.enabled)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(ApiResponse::ok(())))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/notifications/subscribe",
            post(subscribe).delete(unsubscribe),
        )
        .route(
            "/notifications/preferences",
            get(list_preferences).patch(set_preference),
        )
}
