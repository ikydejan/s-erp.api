use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use cqrs::{ItemSearchQuery, ItemView, NewItemCommand};
use erp_core::{ApiResponse, AppError, PaginationMeta, PermissionLevel};
use uuid::Uuid;

use crate::middleware::{require_permission, AuthContext};
use crate::state::AppState;

const RESOURCE: &str = "items";

#[utoipa::path(
    post, path = "/api/v1/items",
    request_body = NewItemCommand,
    responses((status = 200, body = ApiResponse<ItemView>)),
    security(("bearer_auth" = [])),
    tag = "items"
)]
pub(crate) async fn create_item(
    AuthContext(ctx): AuthContext,
    State(state): State<AppState>,
    Json(cmd): Json<NewItemCommand>,
) -> Result<Json<ApiResponse<ItemView>>, AppError> {
    require_permission(&state, &ctx, RESOURCE, PermissionLevel::Create).await?;

    let item = state
        .items_write
        .create(cmd, ctx.user_id, ctx.acted_on_behalf_of)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(ApiResponse::ok(item)))
}

#[utoipa::path(
    get, path = "/api/v1/items",
    params(ItemSearchQuery),
    responses((status = 200, body = ApiResponse<Vec<ItemView>>)),
    security(("bearer_auth" = [])),
    tag = "items"
)]
pub(crate) async fn list_items(
    AuthContext(ctx): AuthContext,
    State(state): State<AppState>,
    Query(query): Query<ItemSearchQuery>,
) -> Result<Json<ApiResponse<Vec<ItemView>>>, AppError> {
    require_permission(&state, &ctx, RESOURCE, PermissionLevel::View).await?;

    let page = query.page.max(1);
    let per_page = query.per_page.clamp(1, 100);
    let (items, total) = state
        .items_search
        .search(query)
        .await
        .map_err(AppError::Internal)?;

    let pagination = PaginationMeta {
        page,
        per_page,
        total,
        total_pages: ((total as f64) / (per_page as f64)).ceil() as u32,
    };
    Ok(Json(ApiResponse::ok_paginated(items, pagination)))
}

#[utoipa::path(
    get, path = "/api/v1/items/{id}",
    responses((status = 200, body = ApiResponse<ItemView>), (status = 404, body = ApiResponse<()>)),
    security(("bearer_auth" = [])),
    tag = "items"
)]
pub(crate) async fn get_item(
    AuthContext(ctx): AuthContext,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<ItemView>>, AppError> {
    require_permission(&state, &ctx, RESOURCE, PermissionLevel::View).await?;

    // Read straight from Postgres (not OpenSearch) so a record is visible
    // immediately after creation, before the outbox has indexed it.
    let item = state
        .items_write
        .get_by_id(id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound("item".into()))?;
    Ok(Json(ApiResponse::ok(item)))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/items", post(create_item).get(list_items))
        .route("/items/:id", get(get_item))
}
