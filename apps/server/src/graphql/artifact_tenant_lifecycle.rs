use async_graphql::{ErrorExtensions, FieldError};

use rustok_api::graphql::GraphQLError;
use rustok_modules::ModuleInstallationError;

/// Maps the owner lifecycle boundary without exposing admission or storage
/// internals to GraphQL callers.
pub(crate) fn map_artifact_tenant_lifecycle_error(error: ModuleInstallationError) -> FieldError {
    match error {
        ModuleInstallationError::AdmissionRevisionConflict(_) => FieldError::new(
            "Artifact tenant lifecycle command conflicts with the current owner state",
        )
        .extend_with(|_, extensions| {
            extensions.set("code", "ARTIFACT_TENANT_LIFECYCLE_CONFLICT");
            extensions.set("retryable_issue", false);
        }),
        _ => {
            <FieldError as GraphQLError>::internal_error("Artifact tenant lifecycle is unavailable")
        }
    }
}
