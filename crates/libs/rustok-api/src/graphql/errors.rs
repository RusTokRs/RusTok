use async_graphql::{ErrorExtensions, FieldError};

#[derive(Debug, Clone, Copy)]
pub enum ErrorCode {
    Unauthenticated,
    PermissionDenied,
    InternalError,
    BadUserInput,
    NotFound,
}

impl ErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unauthenticated => "UNAUTHENTICATED",
            Self::PermissionDenied => "PERMISSION_DENIED",
            Self::InternalError => "INTERNAL_ERROR",
            Self::BadUserInput => "BAD_USER_INPUT",
            Self::NotFound => "NOT_FOUND",
        }
    }
}

pub trait GraphQLError {
    fn unauthenticated() -> FieldError;
    fn permission_denied(message: &str) -> FieldError;
    fn internal_error(message: &str) -> FieldError;
    fn bad_user_input(message: &str) -> FieldError;
    fn not_found(message: &str) -> FieldError;
}

impl GraphQLError for FieldError {
    fn unauthenticated() -> FieldError {
        FieldError::new("Authentication required").extend_with(|_, e| {
            e.set("code", ErrorCode::Unauthenticated.as_str());
        })
    }

    fn permission_denied(message: &str) -> FieldError {
        FieldError::new(message).extend_with(|_, e| {
            e.set("code", ErrorCode::PermissionDenied.as_str());
        })
    }

    fn internal_error(message: &str) -> FieldError {
        FieldError::new(message).extend_with(|_, e| {
            e.set("code", ErrorCode::InternalError.as_str());
        })
    }

    fn bad_user_input(message: &str) -> FieldError {
        FieldError::new(message).extend_with(|_, e| {
            e.set("code", ErrorCode::BadUserInput.as_str());
        })
    }

    fn not_found(message: &str) -> FieldError {
        FieldError::new(message).extend_with(|_, e| {
            e.set("code", ErrorCode::NotFound.as_str());
        })
    }
}

#[cfg(test)]
mod tests {
    use async_graphql::{ErrorExtensions, FieldError};

    use super::{ErrorCode, GraphQLError};

    #[test]
    fn error_code_strings_match_graphql_contract() {
        assert_eq!(ErrorCode::Unauthenticated.as_str(), "UNAUTHENTICATED");
        assert_eq!(ErrorCode::PermissionDenied.as_str(), "PERMISSION_DENIED");
        assert_eq!(ErrorCode::InternalError.as_str(), "INTERNAL_ERROR");
        assert_eq!(ErrorCode::BadUserInput.as_str(), "BAD_USER_INPUT");
        assert_eq!(ErrorCode::NotFound.as_str(), "NOT_FOUND");
    }

    #[test]
    fn graphql_error_helpers_set_expected_codes() {
        let cases = [
            (
                <FieldError as GraphQLError>::unauthenticated().extend(),
                ErrorCode::Unauthenticated.as_str(),
            ),
            (
                <FieldError as GraphQLError>::permission_denied("forbidden").extend(),
                ErrorCode::PermissionDenied.as_str(),
            ),
            (
                <FieldError as GraphQLError>::internal_error("boom").extend(),
                ErrorCode::InternalError.as_str(),
            ),
            (
                <FieldError as GraphQLError>::bad_user_input("bad").extend(),
                ErrorCode::BadUserInput.as_str(),
            ),
            (
                <FieldError as GraphQLError>::not_found("missing").extend(),
                ErrorCode::NotFound.as_str(),
            ),
        ];

        for (error, expected_code) in cases {
            let actual_code = error
                .extensions
                .as_ref()
                .and_then(|extensions| extensions.get("code"))
                .cloned()
                .and_then(|value| value.into_json().ok())
                .and_then(|value| value.as_str().map(ToOwned::to_owned));
            assert_eq!(actual_code.as_deref(), Some(expected_code));
        }
    }
}
