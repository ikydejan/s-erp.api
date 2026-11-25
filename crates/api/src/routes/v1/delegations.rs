use audit::{AuditAction, AuditEntry};
use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use delegation::{Delegation, DelegationStatus};
use erp_core::{ApiResponse, AppError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::middleware::AuthContext;
use crate::state::AppState;

#[derive(Deserialize, ToSchema)]
pub struct CreateDelegationRequest {
    pub delegate_id: Uuid,
    /// Omit to delegate every resource the delegator has access to.
    pub scope: Option<Vec<String>>,
    pub reason: Option<String>,
    pub start_at: DateTime<Utc>,
    pub end_at: DateTime<Utc>,
}

#[derive(Serialize, ToSchema)]
pub struct DelegationView {
    pub id: Uuid,
    pub delegator_id: Uuid,
    pub delegate_id: Uuid,
    pub scope: Option<Value>,
    pub reason: Option<String>,
    pub start_at: DateTime<Utc>,
    pub end_at: DateTime<Utc>,
    pub status: String,
}

impl From<Delegation> for DelegationView {
    fn from(d: Delegation) -> Self {
        Self {
            id: d.id,
            delegator_id: d.delegator_id,
            delegate_id: d.delegate_id,
            scope: d.scope,
            reason: d.reason,
            start_at: d.start_at,
            end_at: d.end_at,
            status: d.status.as_str().to_string(),
        }
    }
}

#[utoipa::path(
    post, path = "/api/v1/delegations",
    request_body = CreateDelegationRequest,
    responses((status = 200, body = ApiResponse<DelegationView>)),
    security(("bearer_auth" = [])),
    tag = "delegations"
)]
pub(crate) async fn create_delegation(
    AuthContext(ctx): AuthContext,
    State(state): State<AppState>,
    Json(req): Json<CreateDelegationRequest>,
) -> Result<Json<ApiResponse<DelegationView>>, AppError> {
    if req.end_at <= req.start_at {
        return Err(AppError::Validation("end_at must be after start_at".into()));
    }
    if req.delegate_id == ctx.user_id {
        return Err(AppError::Validation("cannot delegate to yourself".into()));
    }

    let delegation = Delegation {
        id: Uuid::new_v4(),
        delegator_id: ctx.user_id,
        delegate_id: req.delegate_id,
        scope: req.scope.map(|resources| serde_json::json!(resources)),
        reason: req.reason,
        start_at: req.start_at,
        end_at: req.end_at,
        status: DelegationStatus::Active,
        created_at: Utc::now(),
    };
    state
        .delegations
        .create(&delegation)
        .await
        .map_err(AppError::Internal)?;

    let mut entry = AuditEntry::activity(Some(ctx.user_id), AuditAction::DelegationCreated);
    entry.resource_type = Some("delegations".into());
    entry.resource_id = Some(delegation.id.to_string());
    let _ = state.audit.record(entry).await;

    let _ = state
        .notifications
        .enqueue(
            req.delegate_id,
            "delegation_assigned",
            "You've been delegated tasks",
            &format!(
                "Active from {} to {}",
                delegation.start_at, delegation.end_at
            ),
            serde_json::json!({ "delegation_id": delegation.id }),
        )
        .await;

    Ok(Json(ApiResponse::ok(delegation.into())))
}

#[utoipa::path(
    delete, path = "/api/v1/delegations/{id}",
    responses((status = 200, body = ApiResponse<()>), (status = 404, body = ApiResponse<()>)),
    security(("bearer_auth" = [])),
    tag = "delegations"
)]
pub(crate) async fn revoke_delegation(
    AuthContext(ctx): AuthContext,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let revoked = state
        .delegations
        .revoke(id, ctx.user_id)
        .await
        .map_err(AppError::Internal)?;
    if !revoked {
        return Err(AppError::NotFound("delegation".into()));
    }

    let mut entry = AuditEntry::activity(Some(ctx.user_id), AuditAction::DelegationRevoked);
    entry.resource_type = Some("delegations".into());
    entry.resource_id = Some(id.to_string());
    let _ = state.audit.record(entry).await;

    Ok(Json(ApiResponse::ok(())))
}

#[utoipa::path(
    get, path = "/api/v1/delegations/mine",
    responses((status = 200, body = ApiResponse<Vec<DelegationView>>)),
    security(("bearer_auth" = [])),
    tag = "delegations"
)]
pub(crate) async fn my_delegations(
    AuthContext(ctx): AuthContext,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<DelegationView>>>, AppError> {
    let active = state
        .delegations
        .active_for_delegate(ctx.user_id, Utc::now())
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(ApiResponse::ok(
        active.into_iter().map(Into::into).collect(),
    )))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/delegations", post(create_delegation))
        .route("/delegations/mine", get(my_delegations))
        .route("/delegations/:id", axum::routing::delete(revoke_delegation))
}
