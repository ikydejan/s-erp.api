use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Permission ladder: holding a level grants every level below it too, so
/// `Delete` on a resource implies `View`/`Create`/`Edit` on that same resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema)]
#[repr(i16)]
pub enum PermissionLevel {
    View = 1,
    Create = 2,
    Edit = 3,
    Delete = 4,
}

impl PermissionLevel {
    pub fn from_i16(value: i16) -> Option<Self> {
        match value {
            1 => Some(Self::View),
            2 => Some(Self::Create),
            3 => Some(Self::Edit),
            4 => Some(Self::Delete),
            _ => None,
        }
    }

    pub fn satisfies(&self, required: PermissionLevel) -> bool {
        *self >= required
    }
}
