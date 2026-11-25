use async_trait::async_trait;
use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use chrono::Utc;
use erp_core::{AppError, RequestContext};

use crate::state::AppState;

/// Resolves the bearer JWT into a `RequestContext`: who the caller is, which
/// permissions they effectively hold, and — if the token carries an `act_as`
/// claim backed by a currently-active delegation — whose authority they are
/// acting under.
pub struct AuthContext(pub RequestContext);

#[async_trait]
impl FromRequestParts<AppState> for AuthContext {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or(AppError::Unauthorized)?;
        let token = header
            .strip_prefix("Bearer ")
            .ok_or(AppError::Unauthorized)?;

        let claims = auth::verify_access_token(&state.config.jwt_secret, token)
            .map_err(|_| AppError::Unauthorized)?;
        if claims.mfa_pending {
            return Err(AppError::Unauthorized);
        }

        let user = state
            .users
            .find_by_id(claims.sub)
            .await
            .map_err(AppError::Internal)?
            .filter(|u| u.is_active)
            .ok_or(AppError::Unauthorized)?;

        let mut permissions = state
            .users
            .effective_permissions(user.id)
            .await
            .map_err(AppError::Internal)?;

        let mut acted_on_behalf_of = None;
        if let Some(delegator_id) = claims.act_as {
            let active = state
                .delegations
                .active_for_delegate(user.id, Utc::now())
                .await
                .map_err(AppError::Internal)?;
            let delegation = active
                .into_iter()
                .find(|d| d.delegator_id == delegator_id)
                .ok_or(AppError::Unauthorized)?;

            let delegator_permissions = state
                .users
                .effective_permissions(delegator_id)
                .await
                .map_err(AppError::Internal)?;
            permissions = delegator_permissions
                .into_iter()
                .filter(|(resource, _)| delegation.covers_resource(resource))
                .collect();
            acted_on_behalf_of = Some(delegator_id);
        }

        Ok(AuthContext(RequestContext {
            user_id: user.id,
            acted_on_behalf_of,
            permissions,
        }))
    }
}
