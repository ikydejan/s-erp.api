use audit::{AuditAction, AuditEntry};
use erp_core::{AppError, PermissionLevel, RequestContext};

use crate::state::AppState;

/// Checks `ctx` against the permission ladder for `resource`; on denial,
/// records a `PERMISSION_DENIED` audit entry before returning `Forbidden`.
pub async fn require_permission(
    state: &AppState,
    ctx: &RequestContext,
    resource: &str,
    level: PermissionLevel,
) -> Result<(), AppError> {
    if ctx.has_permission(resource, level) {
        return Ok(());
    }

    let mut entry = AuditEntry::activity(Some(ctx.user_id), AuditAction::PermissionDenied);
    entry.resource_type = Some(resource.to_string());
    entry.metadata = serde_json::json!({ "required_level": format!("{level:?}") });
    let _ = state.audit.record(entry).await;

    Err(AppError::Forbidden)
}
