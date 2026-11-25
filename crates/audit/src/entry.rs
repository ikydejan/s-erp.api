use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditAction {
    LoginSuccess,
    LoginFailed,
    Logout,
    MfaChallengeSuccess,
    MfaChallengeFailed,
    PermissionDenied,
    DataCreate,
    DataUpdate,
    DataDelete,
    DelegationCreated,
    DelegationRevoked,
    DelegationExpired,
}

impl AuditAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            AuditAction::LoginSuccess => "LOGIN_SUCCESS",
            AuditAction::LoginFailed => "LOGIN_FAILED",
            AuditAction::Logout => "LOGOUT",
            AuditAction::MfaChallengeSuccess => "MFA_CHALLENGE_SUCCESS",
            AuditAction::MfaChallengeFailed => "MFA_CHALLENGE_FAILED",
            AuditAction::PermissionDenied => "PERMISSION_DENIED",
            AuditAction::DataCreate => "DATA_CREATE",
            AuditAction::DataUpdate => "DATA_UPDATE",
            AuditAction::DataDelete => "DATA_DELETE",
            AuditAction::DelegationCreated => "DELEGATION_CREATED",
            AuditAction::DelegationRevoked => "DELEGATION_REVOKED",
            AuditAction::DelegationExpired => "DELEGATION_EXPIRED",
        }
    }
}

/// One row destined for the append-only `audit_logs` table. Covers both
/// activity events (logins, permission checks) and data-change events
/// (before/after snapshots of create/update/delete).
#[derive(Debug, Clone, Serialize)]
pub struct AuditEntry {
    pub actor_user_id: Option<Uuid>,
    pub acted_on_behalf_of: Option<Uuid>,
    #[serde(skip)]
    pub action: AuditAction,
    pub resource_type: Option<String>,
    pub resource_id: Option<String>,
    pub before_state: Option<Value>,
    pub after_state: Option<Value>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub metadata: Value,
    pub occurred_at: DateTime<Utc>,
}

impl AuditEntry {
    pub fn activity(actor_user_id: Option<Uuid>, action: AuditAction) -> Self {
        Self {
            actor_user_id,
            acted_on_behalf_of: None,
            action,
            resource_type: None,
            resource_id: None,
            before_state: None,
            after_state: None,
            ip_address: None,
            user_agent: None,
            metadata: serde_json::json!({}),
            occurred_at: Utc::now(),
        }
    }

    pub fn data_change(
        actor_user_id: Uuid,
        action: AuditAction,
        resource_type: impl Into<String>,
        resource_id: impl Into<String>,
        before_state: Option<Value>,
        after_state: Option<Value>,
    ) -> Self {
        Self {
            actor_user_id: Some(actor_user_id),
            acted_on_behalf_of: None,
            action,
            resource_type: Some(resource_type.into()),
            resource_id: Some(resource_id.into()),
            before_state,
            after_state,
            ip_address: None,
            user_agent: None,
            metadata: serde_json::json!({}),
            occurred_at: Utc::now(),
        }
    }

    pub fn with_delegation(mut self, acted_on_behalf_of: Uuid) -> Self {
        self.acted_on_behalf_of = Some(acted_on_behalf_of);
        self
    }

    pub fn with_request_meta(
        mut self,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Self {
        self.ip_address = ip_address;
        self.user_agent = user_agent;
        self
    }
}
