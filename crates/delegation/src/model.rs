use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DelegationStatus {
    Active,
    Expired,
    Revoked,
}

impl DelegationStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            DelegationStatus::Active => "active",
            DelegationStatus::Expired => "expired",
            DelegationStatus::Revoked => "revoked",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "active" => Some(DelegationStatus::Active),
            "expired" => Some(DelegationStatus::Expired),
            "revoked" => Some(DelegationStatus::Revoked),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Delegation {
    pub id: Uuid,
    pub delegator_id: Uuid,
    pub delegate_id: Uuid,
    /// `None` = delegate inherits every permission the delegator has.
    pub scope: Option<Value>,
    pub reason: Option<String>,
    pub start_at: DateTime<Utc>,
    pub end_at: DateTime<Utc>,
    pub status: DelegationStatus,
    pub created_at: DateTime<Utc>,
}

impl Delegation {
    pub fn is_active_at(&self, now: DateTime<Utc>) -> bool {
        self.status == DelegationStatus::Active && self.start_at <= now && now < self.end_at
    }

    /// Whether this delegation covers the given resource (used when a delegate
    /// acts on behalf of the delegator and we need to check scope, on top of
    /// the delegator's own RBAC permission for that resource).
    pub fn covers_resource(&self, resource: &str) -> bool {
        match &self.scope {
            None => true,
            Some(Value::Array(resources)) => resources.iter().any(|r| r.as_str() == Some(resource)),
            Some(_) => false,
        }
    }
}
