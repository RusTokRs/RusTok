use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
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
            rustok_commerce::CommerceError::ShippingProfileNotFound(_) => {
                (StatusCode::NOT_FOUND, "SHIPPING_PROFILE_NOT_FOUND")
            }
            rustok_commerce::CommerceError::DuplicateHandle { .. } => {
                (StatusCode::CONFLICT, "DUPLICATE_HANDLE")
            }
            rustok_commerce::CommerceError::DuplicateSku(_) => {
                (StatusCode::CONFLICT, "DUPLICATE_SKU")
            }
            rustok_commerce::CommerceError::DuplicateShippingProfileSlug(_) => {
                (StatusCode::CONFLICT, "DUPLICATE_SHIPPING_PROFILE_SLUG")
            }
            rustok_commerce::CommerceError::InvalidPrice(_)
            | rustok_commerce::CommerceError::InvalidOptionCombination
            | rustok_commerce::CommerceError::NoVariants => {
                (StatusCode::BAD_REQUEST, "VALIDATION_ERROR")
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
            rustok_commerce::CommerceError::Core(core) => match core {
                rustok_core::Error::NotFound(_) => (StatusCode::NOT_FOUND, "NOT_FOUND"),
                rustok_core::Error::Forbidden(_) => (StatusCode::FORBIDDEN, "FORBIDDEN"),
                rustok_core::Error::Validation(_) | rustok_core::Error::InvalidIdFormat(_) => {
                    (StatusCode::BAD_REQUEST, "VALIDATION_ERROR")
                }
                rustok_core::Error::Auth(_) => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED"),
                _ => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR"),
            },
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR"),
        };

        // Public validation/conflict messages are useful to clients. Internal
        // error variants can contain database URLs, connection details or
        // third-party provider diagnostics and must never cross HTTP.
        let message = if status.is_server_error() {
            tracing::error!(error = %err, "Commerce request failed");
            "Internal server error".to_string()
        } else {
            err.to_string()
        };

        Self::new(status, Json(ApiResponse::<()>::error(code, message)))
    }
}

#[cfg(all(test, feature = "mod-commerce"))]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use rustok_commerce::CommerceError;

    async fn commerce_error_payload(error: CommerceError) -> (StatusCode, serde_json::Value) {
        let response = ApiErrorResponse::from(error).into_response();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 4096).await.expect("read response");
        let payload = serde_json::from_slice(&bytes).expect("decode response");
        (status, payload)
    }

    #[tokio::test]
    async fn commerce_database_error_never_leaks_database_details() {
        let error = CommerceError::Database(sea_orm::DbErr::Custom(
            "postgres://admin:secret@db.internal/private".into(),
        ));
        let (status, payload) = commerce_error_payload(error).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(payload["error"]["code"], "INTERNAL_ERROR");
        assert_eq!(payload["error"]["message"], "Internal server error");
        assert!(!payload.to_string().contains("secret"));
        assert!(!payload.to_string().contains("db.internal"));
    }

    #[tokio::test]
    async fn commerce_user_errors_have_actionable_statuses() {
        let cases = [
            (
                CommerceError::InvalidPrice("negative amount".into()),
                StatusCode::BAD_REQUEST,
                "VALIDATION_ERROR",
            ),
            (
                CommerceError::InvalidOptionCombination,
                StatusCode::BAD_REQUEST,
                "VALIDATION_ERROR",
            ),
            (
                CommerceError::Core(rustok_core::Error::NotFound("missing".into())),
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
            ),
            (
                CommerceError::Core(rustok_core::Error::Forbidden("blocked".into())),
                StatusCode::FORBIDDEN,
                "FORBIDDEN",
            ),
            (
                CommerceError::Core(rustok_core::Error::Validation("bad".into())),
                StatusCode::BAD_REQUEST,
                "VALIDATION_ERROR",
            ),
            (
                CommerceError::NoVariants,
                StatusCode::BAD_REQUEST,
                "VALIDATION_ERROR",
            ),
            (
                CommerceError::ShippingProfileNotFound(uuid::Uuid::new_v4()),
                StatusCode::NOT_FOUND,
                "SHIPPING_PROFILE_NOT_FOUND",
            ),
            (
                CommerceError::DuplicateShippingProfileSlug("duplicate".into()),
                StatusCode::CONFLICT,
                "DUPLICATE_SHIPPING_PROFILE_SLUG",
            ),
        ];
        for (error, expected_status, code) in cases {
            let (status, payload) = commerce_error_payload(error).await;
            assert_eq!(status, expected_status);
            assert_eq!(payload["error"]["code"], code);
        }
    }
}
