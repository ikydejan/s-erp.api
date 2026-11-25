use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    /// Authenticated user id.
    pub sub: Uuid,
    /// Set when the token was issued while an active delegation was selected.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub act_as: Option<Uuid>,
    /// `true` for the short-lived token issued right after password check,
    /// before the MFA challenge is completed — rejected by `AuthContext`,
    /// only accepted by the `/auth/mfa/verify` endpoint.
    #[serde(default)]
    pub mfa_pending: bool,
    pub iat: i64,
    pub exp: i64,
}

pub fn issue_access_token(
    secret: &str,
    user_id: Uuid,
    act_as: Option<Uuid>,
    ttl_minutes: i64,
) -> Result<String, anyhow::Error> {
    issue_token(secret, user_id, act_as, false, ttl_minutes)
}

/// Short-TTL token proving the password check passed; only valid for
/// completing the MFA challenge, not for calling the rest of the API.
pub fn issue_mfa_pending_token(secret: &str, user_id: Uuid) -> Result<String, anyhow::Error> {
    issue_token(secret, user_id, None, true, 5)
}

fn issue_token(
    secret: &str,
    user_id: Uuid,
    act_as: Option<Uuid>,
    mfa_pending: bool,
    ttl_minutes: i64,
) -> Result<String, anyhow::Error> {
    let now = Utc::now();
    let claims = Claims {
        sub: user_id,
        act_as,
        mfa_pending,
        iat: now.timestamp(),
        exp: (now + Duration::minutes(ttl_minutes)).timestamp(),
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| anyhow::anyhow!("failed to issue access token: {e}"))
}

pub fn verify_access_token(secret: &str, token: &str) -> Result<Claims, anyhow::Error> {
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|e| anyhow::anyhow!("invalid access token: {e}"))?;
    Ok(data.claims)
}
