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
