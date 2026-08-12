use std::sync::Arc;

use async_graphql::{
    Context, ErrorExtensions, FieldError, InputObject, Object, Result as GraphqlResult,
    SimpleObject,
};
use rustok_api::graphql::GraphQLError;
use rustok_api::{Permission, has_effective_permission};
use rustok_core::ModuleRuntimeExtensions;
use thiserror::Error;
use uuid::Uuid;

use super::index_drift_diagnosis::IndexDriftFindingReceiptStatus;
use crate::context::{AuthContext, TenantContext};
use crate::services::index_replay_runtime_composition::{
    IndexDriftDiagnosisOperatorError, IndexDriftSourcePageDiagnosisError,
    IndexDriftSourcePageDiagnosisRuntime, IndexReconciliationOperatorContext,
};
use crate::services::rbac_request_scope::permissions_for;

const MAX_SCHEMA_IDENTIFIER_BYTES: usize = 128;
const MAX_SCHEMA_VERSION_BYTES: usize = 10;
const MAX_PAGE_LIMIT_BYTES: usize = 2;
const MAX_CONTINUATION_BYTES: usize = 16 * 1024;
const MAX_PAGE_LIMIT: usize = 32;

/// Untrusted input for exactly one bounded owner-source page.
///
/// Tenant, actor, source owner, and source name are never caller supplied. Schema version and limit
/// remain strings so request-bound authorization can run before parsing any untrusted value.
#[derive(Debug, Clone, InputObject)]
pub struct IndexDriftSourcePageDiagnosisInput {
    pub module_name: String,
    pub entity_name: String,
    pub schema_version: String,
    pub limit: String,
    pub continuation: Option<String>,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct IndexDriftSourcePageFindingReceipt {
    pub finding_id: Uuid,
    pub finding_key: String,
    pub status: IndexDriftFindingReceiptStatus,
}

impl From<&rustok_index::IndexDriftMismatchReceipt> for IndexDriftSourcePageFindingReceipt {
    fn from(receipt: &rustok_index::IndexDriftMismatchReceipt) -> Self {
        Self {
            finding_id: receipt.finding_id(),
            finding_key: receipt.finding_key().to_owned(),
            status: match receipt.status() {
                rustok_index::IndexDriftMismatchRecordStatus::Created => {
                    IndexDriftFindingReceiptStatus::Created
                }
                rustok_index::IndexDriftMismatchRecordStatus::Refreshed => {
                    IndexDriftFindingReceiptStatus::Refreshed
                }
                rustok_index::IndexDriftMismatchRecordStatus::Reopened => {
                    IndexDriftFindingReceiptStatus::Reopened
                }
                rustok_index::IndexDriftMismatchRecordStatus::Suppressed => {
                    IndexDriftFindingReceiptStatus::Suppressed
                }
            },
        }
    }
}

/// Bounded current-page result. It exposes no raw source cursor, entity identifier, owner payload,
/// source identity, registry handle, secret reference, SQL, or database cause.
#[derive(Debug, Clone, SimpleObject)]
pub struct IndexDriftSourcePageDiagnosisPayload {
    pub scanned_mutation_count: i32,
    pub candidate_count: i32,
    pub skipped_delete_count: i32,
    pub non_missing_count: i32,
    pub missing_recorded_count: i32,
    pub findings: Vec<IndexDriftSourcePageFindingReceipt>,
    pub complete: bool,
    pub continuation: Option<String>,
}

impl From<crate::services::index_replay_runtime_composition::IndexDriftSourcePageDiagnosisSealedOutcome>
    for IndexDriftSourcePageDiagnosisPayload
{
    fn from(
        outcome: crate::services::index_replay_runtime_composition::IndexDriftSourcePageDiagnosisSealedOutcome,
    ) -> Self {
        Self {
            scanned_mutation_count: outcome.scanned_mutation_count() as i32,
            candidate_count: outcome.candidate_count() as i32,
            skipped_delete_count: outcome.skipped_delete_count() as i32,
            non_missing_count: outcome.non_missing_count() as i32,
            missing_recorded_count: outcome.missing_recorded_count() as i32,
            findings: outcome
                .receipts()
                .iter()
                .map(IndexDriftSourcePageFindingReceipt::from)
                .collect(),
            complete: outcome.is_complete(),
            continuation: outcome
                .next_token()
                .map(|token| token.as_str().to_owned()),
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
enum IndexDriftSourcePageTransportPreparationError {
    #[error("Index drift source-page request context is invalid")]
    InvalidContext,
    #[error(
        "Index drift source-page diagnosis requires a request-bound effective permission snapshot"
    )]
    MissingRequestAuthority,
    #[error("modules:manage is required for Index drift source-page diagnosis")]
    Forbidden,
    #[error("Index drift source-page input field is invalid: {field}")]
    InvalidInput { field: &'static str },
}

#[derive(Default)]
pub struct IndexDriftSourcePageDiagnosisMutation;

#[Object]
impl IndexDriftSourcePageDiagnosisMutation {
    /// Diagnose exactly one bounded source page through the authenticated confidential continuation
    /// boundary.
    async fn diagnose_index_source_page(
        &self,
        ctx: &Context<'_>,
        input: IndexDriftSourcePageDiagnosisInput,
    ) -> GraphqlResult<IndexDriftSourcePageDiagnosisPayload> {
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        let (operator_context, schema, continuation, limit) =
            prepare_authorized_source_page_request(tenant.id, auth.user_id, input)
                .map_err(map_preparation_error)?;

        let runtime = ctx
            .data::<Arc<ModuleRuntimeExtensions>>()?
            .get::<IndexDriftSourcePageDiagnosisRuntime>()
            .cloned()
            .ok_or_else(|| {
                <FieldError as GraphQLError>::internal_error(
                    "Index drift source-page diagnosis runtime is not available",
                )
            })?;

        runtime
            .diagnose_source_page_sealed(operator_context, schema, continuation.as_deref(), limit)
            .await
            .map(IndexDriftSourcePageDiagnosisPayload::from)
            .map_err(map_source_page_error)
    }
}

fn prepare_authorized_source_page_request(
    tenant_id: Uuid,
    actor_id: Uuid,
    input: IndexDriftSourcePageDiagnosisInput,
) -> std::result::Result<
    (
        IndexReconciliationOperatorContext,
        rustok_index::SchemaRef,
        Option<String>,
        usize,
    ),
    IndexDriftSourcePageTransportPreparationError,
> {
    let context = IndexReconciliationOperatorContext::new(tenant_id, actor_id)
        .map_err(|_| IndexDriftSourcePageTransportPreparationError::InvalidContext)?;

    let permissions = permissions_for(&tenant_id, &actor_id)
        .ok_or(IndexDriftSourcePageTransportPreparationError::MissingRequestAuthority)?;
    if !has_effective_permission(&permissions, &Permission::MODULES_MANAGE) {
        return Err(IndexDriftSourcePageTransportPreparationError::Forbidden);
    }

    let schema = parse_schema(input.module_name, input.entity_name, input.schema_version)?;
    let limit = bounded_text("limit", &input.limit, MAX_PAGE_LIMIT_BYTES)?
        .parse::<usize>()
        .ok()
        .filter(|value| (1..=MAX_PAGE_LIMIT).contains(value))
        .ok_or(IndexDriftSourcePageTransportPreparationError::InvalidInput { field: "limit" })?;
    let continuation = input
        .continuation
        .map(|value| {
            bounded_text("continuation", &value, MAX_CONTINUATION_BYTES)?;
            Ok(value)
        })
        .transpose()?;

    Ok((context, schema, continuation, limit))
}

fn parse_schema(
    module_name: String,
    entity_name: String,
    schema_version: String,
) -> std::result::Result<rustok_index::SchemaRef, IndexDriftSourcePageTransportPreparationError> {
    let module_name = bounded_text("module_name", &module_name, MAX_SCHEMA_IDENTIFIER_BYTES)?;
    let entity_name = bounded_text("entity_name", &entity_name, MAX_SCHEMA_IDENTIFIER_BYTES)?;
    let schema_version = bounded_text("schema_version", &schema_version, MAX_SCHEMA_VERSION_BYTES)?
        .parse::<u32>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or(
            IndexDriftSourcePageTransportPreparationError::InvalidInput {
                field: "schema_version",
            },
        )?;

    Ok(rustok_index::SchemaRef {
        module: rustok_index::ModuleName::new(module_name).map_err(|_| {
            IndexDriftSourcePageTransportPreparationError::InvalidInput {
                field: "module_name",
            }
        })?,
        entity: rustok_index::EntityName::new(entity_name).map_err(|_| {
            IndexDriftSourcePageTransportPreparationError::InvalidInput {
                field: "entity_name",
            }
        })?,
        version: rustok_index::SchemaVersion::new(schema_version),
    })
}

fn bounded_text<'a>(
    field: &'static str,
    value: &'a str,
    max_bytes: usize,
) -> std::result::Result<&'a str, IndexDriftSourcePageTransportPreparationError> {
    if value.is_empty() || value.len() > max_bytes {
        return Err(IndexDriftSourcePageTransportPreparationError::InvalidInput { field });
    }
    Ok(value)
}

fn map_preparation_error(error: IndexDriftSourcePageTransportPreparationError) -> FieldError {
    match error {
        IndexDriftSourcePageTransportPreparationError::InvalidContext
        | IndexDriftSourcePageTransportPreparationError::MissingRequestAuthority => {
            <FieldError as GraphQLError>::unauthenticated()
        }
        IndexDriftSourcePageTransportPreparationError::Forbidden => {
            <FieldError as GraphQLError>::permission_denied(
                "Permission denied: modules:manage required",
            )
        }
        IndexDriftSourcePageTransportPreparationError::InvalidInput { field } => {
            FieldError::new("Invalid Index drift source-page input").extend_with(|_, extensions| {
                extensions.set("code", "BAD_USER_INPUT");
                extensions.set("input_field", field);
            })
        }
    }
}

fn map_source_page_error(error: IndexDriftSourcePageDiagnosisError) -> FieldError {
    match error {
        IndexDriftSourcePageDiagnosisError::MissingRequestAuthority => {
            <FieldError as GraphQLError>::unauthenticated()
        }
        IndexDriftSourcePageDiagnosisError::Forbidden => {
            <FieldError as GraphQLError>::permission_denied(
                "Permission denied: modules:manage required",
            )
        }
        IndexDriftSourcePageDiagnosisError::InvalidPageLimit { .. } => {
            <FieldError as GraphQLError>::bad_user_input("Invalid Index drift source-page limit")
        }
        IndexDriftSourcePageDiagnosisError::ContinuationUnavailable => fixed_dependency_error(
            "INDEX_SOURCE_CONTINUATION_UNAVAILABLE",
            false,
            "Index source continuation is unavailable",
        ),
        IndexDriftSourcePageDiagnosisError::ContinuationKeyringUnavailable => {
            fixed_dependency_error(
                "INDEX_SOURCE_CONTINUATION_KEYRING_UNAVAILABLE",
                true,
                "Index source continuation dependency failed",
            )
        }
        IndexDriftSourcePageDiagnosisError::Continuation(error) => map_continuation_error(error),
        IndexDriftSourcePageDiagnosisError::Source(error) => map_source_error(error),
        IndexDriftSourcePageDiagnosisError::Diagnosis { source, .. } => map_diagnosis_error(source),
    }
}

fn map_continuation_error(error: rustok_index::IndexSourceContinuationError) -> FieldError {
    use rustok_index::IndexSourceContinuationError as Error;

    match error {
        Error::UnknownSchemaSource(_) => {
            <FieldError as GraphQLError>::bad_user_input("Unknown Index source schema")
        }
        Error::Expired => continuation_input_error("INDEX_SOURCE_CONTINUATION_EXPIRED"),
        Error::EmptyToken
        | Error::TokenTooLarge { .. }
        | Error::DecodedTokenTooLarge { .. }
        | Error::PlaintextTooLarge { .. }
        | Error::Base64(_)
        | Error::MalformedEnvelope
        | Error::InvalidKeyId(_)
        | Error::KeyUnavailable(_)
        | Error::InvalidToken
        | Error::Postcard(_)
        | Error::TenantMismatch
        | Error::SchemaMismatch
        | Error::SourceOwnerMismatch
        | Error::SourceNameMismatch
        | Error::LocaleScopeMismatch
        | Error::InvalidClaimsLifetime
        | Error::IssuedAtInFuture => continuation_input_error("INDEX_SOURCE_CONTINUATION_INVALID"),
        _ => <FieldError as GraphQLError>::internal_error(
            "Index source continuation processing failed",
        ),
    }
}

fn continuation_input_error(code: &'static str) -> FieldError {
    FieldError::new("Invalid Index source continuation").extend_with(|_, extensions| {
        extensions.set("code", code);
    })
}

fn map_source_error(error: rustok_index::IndexSourceError) -> FieldError {
    match error {
        rustok_index::IndexSourceError::UnknownSchemaSource(_) => {
            <FieldError as GraphQLError>::bad_user_input("Unknown Index source schema")
        }
        rustok_index::IndexSourceError::SourceFailure { failure, .. } => {
            let retryable = matches!(
                failure.kind(),
                rustok_index::IndexSourceFailureKind::Retryable
            );
            FieldError::new("Index source page dependency failed").extend_with(|_, extensions| {
                extensions.set("code", "INDEX_SOURCE_PAGE_DEPENDENCY_FAILED");
                extensions.set("retryable", retryable);
                extensions.set("dependency_code", failure.code());
            })
        }
        _ => <FieldError as GraphQLError>::internal_error("Index source page failed"),
    }
}

fn map_diagnosis_error(error: IndexDriftDiagnosisOperatorError) -> FieldError {
    match error {
        IndexDriftDiagnosisOperatorError::TenantMismatch
        | IndexDriftDiagnosisOperatorError::Forbidden => {
            <FieldError as GraphQLError>::permission_denied(
                "Permission denied: modules:manage required",
            )
        }
        IndexDriftDiagnosisOperatorError::MissingRequestAuthority => {
            <FieldError as GraphQLError>::unauthenticated()
        }
        IndexDriftDiagnosisOperatorError::Diagnosis(error) => match error {
            rustok_index::IndexDriftDigestError::SnapshotCaptureFailed(failure) => {
                map_diagnosis_dependency("INDEX_DRIFT_SNAPSHOT_CAPTURE_FAILED", failure)
            }
            rustok_index::IndexDriftDigestError::MismatchRecordFailed(failure) => {
                map_diagnosis_dependency("INDEX_DRIFT_MISMATCH_RECORD_FAILED", failure)
            }
            _ => <FieldError as GraphQLError>::internal_error(
                "Index drift source-page candidate diagnosis failed",
            ),
        },
    }
}

fn map_diagnosis_dependency(
    code: &'static str,
    failure: rustok_index::IndexDriftDependencyFailure,
) -> FieldError {
    let retryable = matches!(
        failure.kind(),
        rustok_index::IndexDriftDependencyFailureKind::Retryable
    );
    FieldError::new("Index drift source-page dependency failed").extend_with(|_, extensions| {
        extensions.set("code", code);
        extensions.set("retryable", retryable);
        extensions.set("dependency_code", failure.code());
    })
}

fn fixed_dependency_error(
    code: &'static str,
    retryable: bool,
    message: &'static str,
) -> FieldError {
    FieldError::new(message).extend_with(|_, extensions| {
        extensions.set("code", code);
        extensions.set("retryable", retryable);
    })
}

#[cfg(test)]
mod tests {
    use rustok_api::Permission;
    use rustok_core::UserRole;
    use uuid::Uuid;

    use super::{
        IndexDriftSourcePageDiagnosisInput, IndexDriftSourcePageTransportPreparationError,
        prepare_authorized_source_page_request,
    };
    use crate::services::rbac_request_scope::{RbacRequestScope, with_rbac_request_scope};

    fn malformed_input() -> IndexDriftSourcePageDiagnosisInput {
        IndexDriftSourcePageDiagnosisInput {
            module_name: "Rustok Product".to_owned(),
            entity_name: "product".to_owned(),
            schema_version: "zero".to_owned(),
            limit: "999".to_owned(),
            continuation: Some(String::new()),
        }
    }

    #[tokio::test]
    async fn source_page_transport_authorizes_before_parsing_untrusted_input() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();

        let missing = with_rbac_request_scope(None, async {
            prepare_authorized_source_page_request(tenant_id, actor_id, malformed_input())
        })
        .await
        .expect_err("missing authority must win over malformed input");
        assert_eq!(
            missing,
            IndexDriftSourcePageTransportPreparationError::MissingRequestAuthority
        );

        let forbidden = with_rbac_request_scope(
            Some(RbacRequestScope::new(
                tenant_id,
                actor_id,
                vec![Permission::MODULES_READ],
                UserRole::Admin,
            )),
            async {
                prepare_authorized_source_page_request(tenant_id, actor_id, malformed_input())
            },
        )
        .await
        .expect_err("read-only authority must win over malformed input");
        assert_eq!(
            forbidden,
            IndexDriftSourcePageTransportPreparationError::Forbidden
        );

        let invalid = with_rbac_request_scope(
            Some(RbacRequestScope::new(
                tenant_id,
                actor_id,
                vec![Permission::MODULES_MANAGE],
                UserRole::Admin,
            )),
            async {
                prepare_authorized_source_page_request(tenant_id, actor_id, malformed_input())
            },
        )
        .await
        .expect_err("authorized malformed input must reach parsing");
        assert_eq!(
            invalid,
            IndexDriftSourcePageTransportPreparationError::InvalidInput {
                field: "module_name"
            }
        );
    }

    #[tokio::test]
    async fn source_page_transport_builds_one_schema_and_bounded_page_request() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let input = IndexDriftSourcePageDiagnosisInput {
            module_name: "rustok-product".to_owned(),
            entity_name: "product".to_owned(),
            schema_version: "2".to_owned(),
            limit: "32".to_owned(),
            continuation: Some("opaque-token".to_owned()),
        };

        let (context, schema, continuation, limit) = with_rbac_request_scope(
            Some(RbacRequestScope::new(
                tenant_id,
                actor_id,
                vec![Permission::MODULES_MANAGE],
                UserRole::Admin,
            )),
            async { prepare_authorized_source_page_request(tenant_id, actor_id, input) },
        )
        .await
        .expect("authorized bounded page input should parse");

        assert_eq!(context.tenant_id(), tenant_id);
        assert_eq!(context.actor_id(), actor_id);
        assert_eq!(schema.module.as_str(), "rustok-product");
        assert_eq!(schema.entity.as_str(), "product");
        assert_eq!(schema.version.get(), 2);
        assert_eq!(continuation.as_deref(), Some("opaque-token"));
        assert_eq!(limit, 32);
    }
}
