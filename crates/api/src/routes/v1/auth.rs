use audit::{AuditAction, AuditEntry};
use auth::password::PasswordPolicy;
use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use chrono::{Duration, Utc};
use erp_core::{ApiResponse, AppError, User as UserView};
use infra_postgres::NewUser;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use crate::middleware::AuthContext;
use crate::state::AppState;

#[derive(Deserialize, Validate, ToSchema)]
pub struct RegisterRequest {
    #[validate(length(min = 3, max = 64))]
    pub username: String,
    #[validate(email)]
    pub email: String,
    /// Checked against the ISO 27001-aligned password policy (min length,
    /// character classes) — see `auth::password::PasswordPolicy`.
    pub password: String,
}

#[derive(Deserialize, ToSchema)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize, ToSchema)]
pub struct LoginResponse {
    pub mfa_required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_in: Option<i64>,
}

impl LoginResponse {
    fn mfa_required(pending_token: String) -> Self {
        Self {
            mfa_required: true,
            pending_token: Some(pending_token),
            access_token: None,
            refresh_token: None,
            expires_in: None,
        }
    }

    fn tokens(access_token: String, refresh_token: String, expires_in: i64) -> Self {
        Self {
            mfa_required: false,
            pending_token: None,
            access_token: Some(access_token),
            refresh_token: Some(refresh_token),
            expires_in: Some(expires_in),
        }
    }
}

#[derive(Deserialize, ToSchema)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Deserialize, ToSchema)]
pub struct MfaVerifyRequest {
    pub pending_token: String,
    pub code: String,
}

#[derive(Serialize, ToSchema)]
pub struct MfaSetupResponse {
    pub secret: String,
    pub provisioning_uri: String,
}

#[derive(Deserialize, ToSchema)]
pub struct MfaEnableRequest {
    pub code: String,
}

#[derive(Serialize, ToSchema)]
pub struct MfaEnableResponse {
    /// Shown once — store these somewhere safe, each is single-use.
    pub backup_codes: Vec<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct ActAsRequest {
    pub delegator_id: Uuid,
}

#[utoipa::path(
    post, path = "/api/v1/auth/register",
    request_body = RegisterRequest,
    responses((status = 200, body = ApiResponse<UserView>)),
    tag = "auth"
)]
pub(crate) async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<ApiResponse<UserView>>, AppError> {
    req.validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;
    PasswordPolicy::validate(&req.password).map_err(AppError::Validation)?;

    let password_hash = auth::hash_password(&req.password).map_err(AppError::Internal)?;
    let user = state
        .users
        .create(NewUser {
            username: req.username,
            email: req.email,
            password_hash,
        })
        .await
        .map_err(map_unique_violation)?;

    let mut entry = AuditEntry::activity(Some(user.id), AuditAction::DataCreate);
    entry.resource_type = Some("users".into());
    entry.resource_id = Some(user.id.to_string());
    let _ = state.audit.record(entry).await;

    Ok(Json(ApiResponse::ok(user.to_domain())))
}

#[utoipa::path(
    post, path = "/api/v1/auth/login",
    request_body = LoginRequest,
    responses((status = 200, body = ApiResponse<LoginResponse>)),
    tag = "auth"
)]
pub(crate) async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<ApiResponse<LoginResponse>>, AppError> {
    let user = state
        .users
        .find_by_username(&req.username)
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::Unauthorized)?;

    if user.is_locked() {
        return Err(AppError::Forbidden);
    }

    if !auth::verify_password(&req.password, &user.password_hash) {
        state
            .users
            .record_login_failure(user.id)
            .await
            .map_err(AppError::Internal)?;
        let _ = state
            .audit
            .record(AuditEntry::activity(
                Some(user.id),
                AuditAction::LoginFailed,
            ))
            .await;
        return Err(AppError::Unauthorized);
    }
    state
        .users
        .record_login_success(user.id)
        .await
        .map_err(AppError::Internal)?;

    if user.mfa_enabled {
        let pending = auth::jwt::issue_mfa_pending_token(&state.config.jwt_secret, user.id)
            .map_err(AppError::Internal)?;
        return Ok(Json(ApiResponse::ok(LoginResponse::mfa_required(pending))));
    }

    let _ = state
        .audit
        .record(AuditEntry::activity(
            Some(user.id),
            AuditAction::LoginSuccess,
        ))
        .await;
    let (access, expires_in, refresh) = issue_token_pair(&state, user.id).await?;
    Ok(Json(ApiResponse::ok(LoginResponse::tokens(
        access, refresh, expires_in,
    ))))
}

#[utoipa::path(
    post, path = "/api/v1/auth/mfa/verify",
    request_body = MfaVerifyRequest,
    responses((status = 200, body = ApiResponse<LoginResponse>)),
    tag = "auth"
)]
pub(crate) async fn mfa_verify(
    State(state): State<AppState>,
    Json(req): Json<MfaVerifyRequest>,
) -> Result<Json<ApiResponse<LoginResponse>>, AppError> {
    let claims = auth::verify_access_token(&state.config.jwt_secret, &req.pending_token)
        .map_err(|_| AppError::Unauthorized)?;
    if !claims.mfa_pending {
        return Err(AppError::Unauthorized);
    }
    let user = state
        .users
        .find_by_id(claims.sub)
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::Unauthorized)?;
    let secret_enc = user
        .mfa_secret_encrypted
        .clone()
        .ok_or(AppError::Unauthorized)?;
    let secret = auth::mfa::encryption::decrypt(&secret_enc, &state.config.mfa_encryption_key)
        .map_err(AppError::Internal)?;

    let mut ok = auth::mfa::totp::verify(&secret, &user.email, &req.code).unwrap_or(false);
    if !ok {
        let hash = auth::mfa::backup_codes::hash_code(&req.code);
        ok = state
            .users
            .consume_backup_code(user.id, &hash)
            .await
            .map_err(AppError::Internal)?;
    }
    if !ok {
        let _ = state
            .audit
            .record(AuditEntry::activity(
                Some(user.id),
                AuditAction::MfaChallengeFailed,
            ))
            .await;
        return Err(AppError::Unauthorized);
    }

    let _ = state
        .audit
        .record(AuditEntry::activity(
            Some(user.id),
            AuditAction::MfaChallengeSuccess,
        ))
        .await;
    let (access, expires_in, refresh) = issue_token_pair(&state, user.id).await?;
    Ok(Json(ApiResponse::ok(LoginResponse::tokens(
        access, refresh, expires_in,
    ))))
}

#[utoipa::path(
    post, path = "/api/v1/auth/refresh",
    request_body = RefreshRequest,
    responses((status = 200, body = ApiResponse<LoginResponse>)),
    tag = "auth"
)]
pub(crate) async fn refresh(
    State(state): State<AppState>,
    Json(req): Json<RefreshRequest>,
) -> Result<Json<ApiResponse<LoginResponse>>, AppError> {
    let hash = auth::refresh_token::hash(&req.refresh_token);
    let user_id = state
        .users
        .find_valid_refresh_token(&hash)
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::Unauthorized)?;
    // Rotate: the old refresh token is single-use.
    state
        .users
        .revoke_refresh_token(&hash)
        .await
        .map_err(AppError::Internal)?;

    let (access, expires_in, refresh) = issue_token_pair(&state, user_id).await?;
    Ok(Json(ApiResponse::ok(LoginResponse::tokens(
        access, refresh, expires_in,
    ))))
}

#[utoipa::path(
    post, path = "/api/v1/auth/logout",
    request_body = RefreshRequest,
    responses((status = 200, body = ApiResponse<()>)),
    security(("bearer_auth" = [])),
    tag = "auth"
)]
pub(crate) async fn logout(
    AuthContext(ctx): AuthContext,
    State(state): State<AppState>,
    Json(req): Json<RefreshRequest>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let hash = auth::refresh_token::hash(&req.refresh_token);
    state
        .users
        .revoke_refresh_token(&hash)
        .await
        .map_err(AppError::Internal)?;
    let _ = state
        .audit
        .record(AuditEntry::activity(Some(ctx.user_id), AuditAction::Logout))
        .await;
    Ok(Json(ApiResponse::ok(())))
}

#[utoipa::path(
    post, path = "/api/v1/auth/mfa/setup",
    responses((status = 200, body = ApiResponse<MfaSetupResponse>)),
    security(("bearer_auth" = [])),
    tag = "auth"
)]
pub(crate) async fn mfa_setup(
    AuthContext(ctx): AuthContext,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<MfaSetupResponse>>, AppError> {
    let user = state
        .users
        .find_by_id(ctx.user_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::Unauthorized)?;

    let secret = auth::mfa::totp::generate_secret_base32();
    let encrypted = auth::mfa::encryption::encrypt(&secret, &state.config.mfa_encryption_key)
        .map_err(AppError::Internal)?;
    state
        .users
        .set_mfa_secret(user.id, &encrypted)
        .await
        .map_err(AppError::Internal)?;
    let provisioning_uri =
        auth::mfa::totp::provisioning_uri(&secret, &user.email).map_err(AppError::Internal)?;

    Ok(Json(ApiResponse::ok(MfaSetupResponse {
        secret,
        provisioning_uri,
    })))
}

#[utoipa::path(
    post, path = "/api/v1/auth/mfa/enable",
    request_body = MfaEnableRequest,
    responses((status = 200, body = ApiResponse<MfaEnableResponse>)),
    security(("bearer_auth" = [])),
    tag = "auth"
)]
pub(crate) async fn mfa_enable(
    AuthContext(ctx): AuthContext,
    State(state): State<AppState>,
    Json(req): Json<MfaEnableRequest>,
) -> Result<Json<ApiResponse<MfaEnableResponse>>, AppError> {
    let user = state
        .users
        .find_by_id(ctx.user_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::Unauthorized)?;
    let secret_enc = user
        .mfa_secret_encrypted
        .clone()
        .ok_or_else(|| AppError::Validation("call POST /auth/mfa/setup first".into()))?;
    let secret = auth::mfa::encryption::decrypt(&secret_enc, &state.config.mfa_encryption_key)
        .map_err(AppError::Internal)?;

    if !auth::mfa::totp::verify(&secret, &user.email, &req.code).unwrap_or(false) {
        return Err(AppError::Validation("invalid MFA code".into()));
    }

    state
        .users
        .set_mfa_enabled(user.id, true)
        .await
        .map_err(AppError::Internal)?;
    let codes = auth::mfa::backup_codes::generate_codes();
    let hashes: Vec<String> = codes
        .iter()
        .map(|c| auth::mfa::backup_codes::hash_code(c))
        .collect();
    state
        .users
        .store_backup_codes(user.id, &hashes)
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(ApiResponse::ok(MfaEnableResponse {
        backup_codes: codes,
    })))
}

#[utoipa::path(
    post, path = "/api/v1/auth/mfa/disable",
    request_body = MfaEnableRequest,
    responses((status = 200, body = ApiResponse<()>)),
    security(("bearer_auth" = [])),
    tag = "auth"
)]
pub(crate) async fn mfa_disable(
    AuthContext(ctx): AuthContext,
    State(state): State<AppState>,
    Json(req): Json<MfaEnableRequest>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let user = state
        .users
        .find_by_id(ctx.user_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::Unauthorized)?;
    let secret_enc = user
        .mfa_secret_encrypted
        .clone()
        .ok_or(AppError::Unauthorized)?;
    let secret = auth::mfa::encryption::decrypt(&secret_enc, &state.config.mfa_encryption_key)
        .map_err(AppError::Internal)?;

    if !auth::mfa::totp::verify(&secret, &user.email, &req.code).unwrap_or(false) {
        return Err(AppError::Validation("invalid MFA code".into()));
    }
    state
        .users
        .set_mfa_enabled(user.id, false)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(ApiResponse::ok(())))
}

#[utoipa::path(
    post, path = "/api/v1/auth/act-as",
    request_body = ActAsRequest,
    responses((status = 200, body = ApiResponse<LoginResponse>)),
    security(("bearer_auth" = [])),
    tag = "auth"
)]
pub(crate) async fn act_as(
    AuthContext(ctx): AuthContext,
    State(state): State<AppState>,
    Json(req): Json<ActAsRequest>,
) -> Result<Json<ApiResponse<LoginResponse>>, AppError> {
    let active = state
        .delegations
        .active_for_delegate(ctx.user_id, Utc::now())
        .await
        .map_err(AppError::Internal)?;
    if !active.iter().any(|d| d.delegator_id == req.delegator_id) {
        return Err(AppError::Forbidden);
    }

    let access = auth::issue_access_token(
        &state.config.jwt_secret,
        ctx.user_id,
        Some(req.delegator_id),
        state.config.jwt_access_ttl_minutes,
    )
    .map_err(AppError::Internal)?;
    Ok(Json(ApiResponse::ok(LoginResponse {
        mfa_required: false,
        pending_token: None,
        access_token: Some(access),
        refresh_token: None,
        expires_in: Some(state.config.jwt_access_ttl_minutes * 60),
    })))
}

pub(crate) async fn issue_token_pair(
    state: &AppState,
    user_id: Uuid,
) -> Result<(String, i64, String), AppError> {
    let access = auth::issue_access_token(
        &state.config.jwt_secret,
        user_id,
        None,
        state.config.jwt_access_ttl_minutes,
    )
    .map_err(AppError::Internal)?;
    let (raw_refresh, refresh_hash) = auth::refresh_token::generate();
    let expires_at = Utc::now() + Duration::days(state.config.jwt_refresh_ttl_days);
    state
        .users
        .create_refresh_token(user_id, &refresh_hash, expires_at, None, None)
        .await
        .map_err(AppError::Internal)?;
    Ok((
        access,
        state.config.jwt_access_ttl_minutes * 60,
        raw_refresh,
    ))
}

fn map_unique_violation(e: anyhow::Error) -> AppError {
    if e.to_string().contains("duplicate key") {
        AppError::Conflict("username or email already taken".into())
    } else {
        AppError::Internal(e)
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
        .route("/auth/mfa/verify", post(mfa_verify))
        .route("/auth/refresh", post(refresh))
        .route("/auth/logout", post(logout))
        .route("/auth/mfa/setup", post(mfa_setup))
        .route("/auth/mfa/enable", post(mfa_enable))
        .route("/auth/mfa/disable", post(mfa_disable))
        .route("/auth/act-as", post(act_as))
}
