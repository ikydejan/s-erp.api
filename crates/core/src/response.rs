use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

/// Standard envelope every v1 API response is wrapped in, success or error.
#[derive(Serialize, ToSchema)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiError>,
}

#[derive(Serialize, ToSchema, Default)]
pub struct Meta {
    pub request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pagination: Option<PaginationMeta>,
}

#[derive(Serialize, ToSchema, Clone)]
pub struct PaginationMeta {
    pub page: u32,
    pub per_page: u32,
    pub total: u64,
    pub total_pages: u32,
}

#[derive(Serialize, ToSchema)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            meta: Meta {
                request_id: Uuid::new_v4().to_string(),
                pagination: None,
            },
            error: None,
        }
    }

    pub fn ok_paginated(data: T, pagination: PaginationMeta) -> Self {
        Self {
            success: true,
            data: Some(data),
            meta: Meta {
                request_id: Uuid::new_v4().to_string(),
                pagination: Some(pagination),
            },
            error: None,
        }
    }

    pub fn error(err: impl Into<ApiError>) -> Self {
        Self {
            success: false,
            data: None,
            meta: Meta {
                request_id: Uuid::new_v4().to_string(),
                pagination: None,
            },
            error: Some(err.into()),
        }
    }
}
