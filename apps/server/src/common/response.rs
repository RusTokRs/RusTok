use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiError>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(code: impl Into<String>, message: impl Into<String>) -> ApiResponse<()> {
        ApiResponse {
            success: false,
            data: None,
            error: Some(ApiError {
                code: code.into(),
                message: message.into(),
                details: None,
            }),
        }
    }
}

pub struct ApiErrorResponse {
    status: StatusCode,
    body: Json<ApiResponse<()>>,
}

impl ApiErrorResponse {
    pub fn new(status: StatusCode, body: Json<ApiResponse<()>>) -> Self {
        Self { status, body }
    }
}

impl IntoResponse for ApiErrorResponse {
    fn into_response(self) -> Response {
        (self.status, self.body).into_response()
    }
}

impl From<(StatusCode, Json<ApiResponse<()>>)> for ApiErrorResponse {
    fn from(value: (StatusCode, Json<ApiResponse<()>>)) -> Self {
        Self::new(value.0, value.1)
    }
}

impl From<(StatusCode, &'static str)> for ApiErrorResponse {
    fn from(value: (StatusCode, &'static str)) -> Self {
        let (status, message) = value;
        let code = match status {
            StatusCode::BAD_REQUEST => "BAD_REQUEST",
            StatusCode::UNAUTHORIZED => "UNAUTHORIZED",
            StatusCode::FORBIDDEN => "FORBIDDEN",
            StatusCode::NOT_FOUND => "NOT_FOUND",
            _ => "ERROR",
        };

        Self::new(status, Json(ApiResponse::<()>::error(code, message)))
    }
}

#[cfg(feature = "mod-commerce")]
impl From<rustok_commerce::CommerceError> for ApiErrorResponse {
    fn from(err: rustok_commerce::CommerceError) -> Self {
        let (status, code) = match &err {
            rustok_commerce::CommerceError::ProductNotFound(_) => {
                (StatusCode::NOT_FOUND, "PRODUCT_NOT_FOUND")
            }
            rustok_commerce::CommerceError::VariantNotFound(_) => {
                (StatusCode::NOT_FOUND, "VARIANT_NOT_FOUND")
            }
            rustok_commerce::CommerceError::DuplicateHandle { .. } => {
                (StatusCode::CONFLICT, "DUPLICATE_HANDLE")
            }
            rustok_commerce::CommerceError::DuplicateSku(_) => {
                (StatusCode::CONFLICT, "DUPLICATE_SKU")
            }
            rustok_commerce::CommerceError::InsufficientInventory { .. } => {
                (StatusCode::UNPROCESSABLE_ENTITY, "INSUFFICIENT_INVENTORY")
            }
            rustok_commerce::CommerceError::Validation(_) => {
                (StatusCode::BAD_REQUEST, "VALIDATION_ERROR")
            }
            rustok_commerce::CommerceError::CannotDeletePublished => {
                (StatusCode::CONFLICT, "CANNOT_DELETE_PUBLISHED")
            }
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR"),
        };

        Self::new(
            status,
            Json(ApiResponse::<()>::error(code, err.to_string())),
        )
    }
}
