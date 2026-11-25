use std::collections::HashMap;

use uuid::Uuid;

use crate::entities::permission::PermissionLevel;

/// Resolved identity for one authenticated request: who is making the call,
/// who they are acting on behalf of (if any active delegation applies), and
/// their effective permission level per resource (roles merged, max level wins).
#[derive(Debug, Clone)]
pub struct RequestContext {
    pub user_id: Uuid,
    pub acted_on_behalf_of: Option<Uuid>,
    pub permissions: HashMap<String, PermissionLevel>,
}

impl RequestContext {
    /// The identity whose data is being acted on: the delegator if delegating,
    /// otherwise the caller themselves.
    pub fn effective_subject(&self) -> Uuid {
        self.acted_on_behalf_of.unwrap_or(self.user_id)
    }

    pub fn has_permission(&self, resource: &str, required: PermissionLevel) -> bool {
        self.permissions
            .get(resource)
            .is_some_and(|level| level.satisfies(required))
    }
}
