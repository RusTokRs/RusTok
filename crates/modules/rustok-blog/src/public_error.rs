use async_graphql::{Error as GraphqlError, ErrorExtensions};
use axum::http::StatusCode;
use rustok_core::error::{ErrorKind, RichError};
use rustok_web::HttpError;

use crate::BlogError;

fn safe_parts(error: BlogError) -> (u16, String, String) {
    let rich: RichError = error.into();
    let status = rich.status_code;
    let code = rich
        .error_code
        .unwrap_or_else(|| rich.kind.error_code().to_string());
    let message = rich.user_message.unwrap_or_else(|| match rich.kind {
        ErrorKind::Validation => "Invalid Blog request".to_string(),
        ErrorKind::Unauthenticated => "Authentication required".to_string(),
        ErrorKind::Forbidden => "Access denied".to_string(),
        ErrorKind::NotFound => "The requested Blog resource was not found".to_string(),
        ErrorKind::Conflict => "The Blog resource changed concurrently".to_string(),
        ErrorKind::RateLimited => "Too many Blog requests".to_string(),
        ErrorKind::Database | ErrorKind::Internal => {
            "The Blog operation could not be completed".to_string()
        }
        ErrorKind::ExternalService => "A required Blog dependency is unavailable".to_string(),
        ErrorKind::Timeout => "A required Blog dependency timed out".to_string(),
        ErrorKind::BusinessLogic => "The Blog operation is not allowed".to_string(),
    });
    (status, code, message)
}

pub(crate) fn to_http_error(error: BlogError) -> HttpError {
    let (status, code, message) = safe_parts(error);
    let status = StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    HttpError::new(status, code, message)
}

pub(crate) fn to_graphql_error(error: BlogError) -> GraphqlError {
    let (status, code, message) = safe_parts(error);
    GraphqlError::new(message).extend_with(|_, extensions| {
        extensions.set("code", code);
        extensions.set("httpStatus", status);
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn database_details_are_redacted() {
        let error = BlogError::Database(sea_orm::DbErr::Custom(
            "postgresql://secret-host/private".to_string(),
        ));

        let mapped = to_http_error(error);
        assert_eq!(mapped.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(!mapped.message.contains("secret-host"));
        assert!(!mapped.message.contains("postgresql"));
    }

    #[test]
    fn not_found_and_conflict_keep_transport_semantics() {
        let missing = to_http_error(BlogError::PostNotFound(Uuid::new_v4()));
        assert_eq!(missing.status, StatusCode::NOT_FOUND);

        let conflict = to_http_error(BlogError::conflict("internal predecessor detail"));
        assert_eq!(conflict.status, StatusCode::CONFLICT);
        assert!(!conflict.message.contains("predecessor"));
    }
}
