use std::collections::HashMap;

use erp_core::PermissionLevel;

/// Collapses every `(resource, level)` row across all of a user's roles into
/// one effective map, keeping the highest level per resource.
pub fn merge_permissions(
    rows: impl IntoIterator<Item = (String, PermissionLevel)>,
) -> HashMap<String, PermissionLevel> {
    let mut merged: HashMap<String, PermissionLevel> = HashMap::new();
    for (resource, level) in rows {
        merged
            .entry(resource)
            .and_modify(|existing| {
                if level > *existing {
                    *existing = level;
                }
            })
            .or_insert(level);
    }
    merged
}
