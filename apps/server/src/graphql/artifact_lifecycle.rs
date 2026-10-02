use async_graphql::{ErrorExtensions, FieldError};

use rustok_api::graphql::GraphQLError;
use rustok_modules::ModuleInstallationError;

fn conflict(code: &'static str, message: &'static str) -> FieldError {
    FieldError::new(message).extend_with(|_, extensions| {
        extensions.set("code", code);
        extensions.set("retryable_issue", false);
    })
}

/// Maps tenant intent errors without exposing admission or storage internals.
pub(crate) fn map_artifact_tenant_lifecycle_error(error: ModuleInstallationError) -> FieldError {
    match error {
        ModuleInstallationError::AdmissionRevisionConflict(_) => conflict(
            "ARTIFACT_TENANT_LIFECYCLE_CONFLICT",
            "Artifact tenant lifecycle command conflicts with the current owner state",
        ),
        _ => {
            <FieldError as GraphQLError>::internal_error("Artifact tenant lifecycle is unavailable")
        }
    }
}

/// Maps scoped installation lifecycle errors without exposing the admitted
/// descriptor, dependency graph, or owner storage details to GraphQL callers.
pub(crate) fn map_artifact_installation_lifecycle_error(
    error: ModuleInstallationError,
) -> FieldError {
    match error {
        ModuleInstallationError::AdmissionRevisionConflict(_) => conflict(
            "ARTIFACT_INSTALLATION_LIFECYCLE_CONFLICT",
            "Artifact installation lifecycle command conflicts with the current owner state",
        ),
        _ => <FieldError as GraphQLError>::internal_error(
            "Artifact installation lifecycle is unavailable",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{map_artifact_installation_lifecycle_error, map_artifact_tenant_lifecycle_error};
    use async_graphql::ErrorExtensions;
    use rustok_modules::ModuleInstallationError;

    #[test]
    fn installation_revision_conflict_keeps_stable_conflict_contract() {
        let error = map_artifact_installation_lifecycle_error(
            ModuleInstallationError::AdmissionRevisionConflict(
                "database password=secret".to_string(),
            ),
        );

        assert_eq!(
            error.message,
            "Artifact installation lifecycle command conflicts with the current owner state"
        );
        let extended = error.extend();
        let code = extended
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get("code"))
            .cloned()
            .and_then(|value| value.into_json().ok())
            .and_then(|value| value.as_str().map(ToOwned::to_owned));
        assert_eq!(
            code.as_deref(),
            Some("ARTIFACT_INSTALLATION_LIFECYCLE_CONFLICT")
        );
        assert!(!extended.message.contains("database password=secret"));
    }

    #[test]
    fn installation_storage_errors_are_generic() {
        let error = map_artifact_installation_lifecycle_error(
            ModuleInstallationError::Store("database password=secret".to_string()),
        );

        assert_eq!(
            error.message,
            "Artifact installation lifecycle is unavailable"
        );
        assert!(!error.message.contains("database password=secret"));
    }

    #[test]
    fn tenant_storage_errors_are_generic() {
        let error = map_artifact_tenant_lifecycle_error(
            ModuleInstallationError::Outbox("database password=secret".to_string()),
        );

        assert_eq!(
            error.message,
            "Artifact tenant lifecycle is unavailable"
        );
        assert!(!error.message.contains("database password=secret"));
    }
}
