use std::sync::Arc;

use async_graphql::{
    Context, Enum, ErrorExtensions, FieldError, InputObject, Object, Result as GraphqlResult,
    SimpleObject,
};
use rustok_api::graphql::GraphQLError;
use rustok_api::{Permission, has_effective_permission};
use rustok_core::ModuleRuntimeExtensions;
use thiserror::Error;
use uuid::Uuid;

use crate::context::{AuthContext, TenantContext};
use crate::services::index_replay_runtime_composition::{
    IndexDriftDiagnosisOperatorError, IndexDriftDiagnosisOperatorRuntime,
    IndexReconciliationOperatorContext,
};
use crate::services::rbac_request_scope::permissions_for;

const MAX_SCHEMA_IDENTIFIER_BYTES: usize = 128;
const MAX_SCHEMA_VERSION_BYTES: usize = 10;
const MAX_ENTITY_ID_BYTES: usize = 64;
const MAX_LOCALE_BYTES: usize = 128;

/// Untrusted GraphQL input for one exact Index entity.
///
/// Every field remains a string so request-bound authority can be checked before schema, UUID, or
/// locale parsing. Tenant and actor identities are never caller supplied.
#[derive(Debug, Clone, InputObject)]
pub struct IndexDriftDiagnosisInput {
    pub module_name: String,
    pub entity_name: String,
    pub schema_version: String,
    pub entity_id: String,
    pub locale: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum IndexDriftDiagnosisStatus {
    Consistent,
    MismatchRecorded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum IndexDriftFindingReceiptStatus {
    Created,
    Refreshed,
    Reopened,
    Suppressed,
}

/// Bounded diagnosis result. No owner payload, materialized payload, SQL, or database cause is
/// exposed through the transport.
#[derive(Debug, Clone, SimpleObject)]
pub struct IndexDriftDiagnosisPayload {
    pub status: IndexDriftDiagnosisStatus,
    pub digest: Option<String>,
    pub source_digest: Option<String>,
    pub materialized_digest: Option<String>,
    pub finding_id: Option<Uuid>,
    pub finding_key: Option<String>,
    pub finding_status: Option<IndexDriftFindingReceiptStatus>,
}

impl From<rustok_index::IndexDriftDigestOutcome> for IndexDriftDiagnosisPayload {
    fn from(outcome: rustok_index::IndexDriftDigestOutcome) -> Self {
        match outcome {
            rustok_index::IndexDriftDigestOutcome::Consistent { digest } => Self {
                status: IndexDriftDiagnosisStatus::Consistent,
                digest: Some(digest),
                source_digest: None,
                materialized_digest: None,
                finding_id: None,
                finding_key: None,
                finding_status: None,
            },
            rustok_index::IndexDriftDigestOutcome::MismatchRecorded {
                source_digest,
                materialized_digest,
                receipt,
            } => Self {
                status: IndexDriftDiagnosisStatus::MismatchRecorded,
                digest: None,
                source_digest: Some(source_digest),
                materialized_digest: Some(materialized_digest),
                finding_id: Some(receipt.finding_id()),
                finding_key: Some(receipt.finding_key().to_owned()),
                finding_status: Some(match receipt.status() {
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
                }),
            },
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
enum IndexDriftDiagnosisTransportPreparationError {
    #[error("Index drift diagnosis request context is invalid")]
    InvalidContext,
    #[error("Index drift diagnosis requires a request-bound effective permission snapshot")]
    MissingRequestAuthority,
    #[error("modules:manage is required for Index drift diagnosis")]
    Forbidden,
    #[error("Index drift diagnosis input field is invalid: {field}")]
    InvalidInput { field: &'static str },
}

#[derive(Default)]
pub struct IndexDriftDiagnosisMutation;

#[Object]
impl IndexDriftDiagnosisMutation {
    /// Diagnose one exact Index entity and record only a detected mismatch.
    async fn diagnose_index_entity(
        &self,
        ctx: &Context<'_>,
        input: IndexDriftDiagnosisInput,
    ) -> GraphqlResult<IndexDriftDiagnosisPayload> {
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        let (operator_context, key) = prepare_authorized_request(tenant.id, auth.user_id, input)
            .map_err(map_preparation_error)?;

        let runtime = ctx
            .data::<Arc<ModuleRuntimeExtensions>>()?
            .get::<IndexDriftDiagnosisOperatorRuntime>()
            .cloned()
            .ok_or_else(|| {
                <FieldError as GraphQLError>::internal_error(
                    "Index drift diagnosis runtime is not available",
                )
            })?;

        runtime
            .diagnose_entity(operator_context, key)
            .await
            .map(IndexDriftDiagnosisPayload::from)
            .map_err(map_operator_error)
    }
}

fn prepare_authorized_request(
    tenant_id: Uuid,
    actor_id: Uuid,
    input: IndexDriftDiagnosisInput,
) -> std::result::Result<
    (IndexReconciliationOperatorContext, rustok_index::EntityKey),
    IndexDriftDiagnosisTransportPreparationError,
> {
    let context = IndexReconciliationOperatorContext::new(tenant_id, actor_id)
        .map_err(|_| IndexDriftDiagnosisTransportPreparationError::InvalidContext)?;

    let permissions = permissions_for(&tenant_id, &actor_id)
        .ok_or(IndexDriftDiagnosisTransportPreparationError::MissingRequestAuthority)?;
    if !has_effective_permission(&permissions, &Permission::MODULES_MANAGE) {
        return Err(IndexDriftDiagnosisTransportPreparationError::Forbidden);
    }

    let key = parse_entity_key(tenant_id, input)?;
    Ok((context, key))
}

fn parse_entity_key(
    tenant_id: Uuid,
    input: IndexDriftDiagnosisInput,
) -> std::result::Result<rustok_index::EntityKey, IndexDriftDiagnosisTransportPreparationError> {
    let module_name = rustok_index::ModuleName::new(bounded_text(
        "module_name",
        &input.module_name,
        MAX_SCHEMA_IDENTIFIER_BYTES,
    )?)
    .map_err(
        |_| IndexDriftDiagnosisTransportPreparationError::InvalidInput {
            field: "module_name",
        },
    )?;
    let entity_name = rustok_index::EntityName::new(bounded_text(
        "entity_name",
        &input.entity_name,
        MAX_SCHEMA_IDENTIFIER_BYTES,
    )?)
    .map_err(
        |_| IndexDriftDiagnosisTransportPreparationError::InvalidInput {
            field: "entity_name",
        },
    )?;
    let schema_version = bounded_text(
        "schema_version",
        &input.schema_version,
        MAX_SCHEMA_VERSION_BYTES,
    )?
    .parse::<u32>()
    .ok()
    .filter(|value| *value > 0)
    .ok_or(IndexDriftDiagnosisTransportPreparationError::InvalidInput {
        field: "schema_version",
    })?;
    let entity_id = Uuid::parse_str(bounded_text(
        "entity_id",
        &input.entity_id,
        MAX_ENTITY_ID_BYTES,
    )?)
    .ok()
    .filter(|value| !value.is_nil())
    .ok_or(IndexDriftDiagnosisTransportPreparationError::InvalidInput { field: "entity_id" })?;
    let locale = input
        .locale
        .map(|locale| {
            let locale = bounded_text("locale", &locale, MAX_LOCALE_BYTES)?;
            rustok_index::LocaleKey::new(locale).map_err(|_| {
                IndexDriftDiagnosisTransportPreparationError::InvalidInput { field: "locale" }
            })
        })
        .transpose()?;

    Ok(rustok_index::EntityKey {
        tenant_id,
        schema: rustok_index::SchemaRef {
            module: module_name,
            entity: entity_name,
            version: rustok_index::SchemaVersion::new(schema_version),
        },
        entity_id,
        locale,
    })
}

fn bounded_text<'a>(
    field: &'static str,
    value: &'a str,
    max_bytes: usize,
) -> std::result::Result<&'a str, IndexDriftDiagnosisTransportPreparationError> {
    if value.is_empty() || value.len() > max_bytes {
        return Err(IndexDriftDiagnosisTransportPreparationError::InvalidInput { field });
    }
    Ok(value)
}

fn map_preparation_error(error: IndexDriftDiagnosisTransportPreparationError) -> FieldError {
    match error {
        IndexDriftDiagnosisTransportPreparationError::InvalidContext
        | IndexDriftDiagnosisTransportPreparationError::MissingRequestAuthority => {
            <FieldError as GraphQLError>::unauthenticated()
        }
        IndexDriftDiagnosisTransportPreparationError::Forbidden => {
            <FieldError as GraphQLError>::permission_denied(
                "Permission denied: modules:manage required",
            )
        }
        IndexDriftDiagnosisTransportPreparationError::InvalidInput { field } => {
            FieldError::new("Invalid Index drift diagnosis input").extend_with(|_, extensions| {
                extensions.set("code", "BAD_USER_INPUT");
                extensions.set("input_field", field);
            })
        }
    }
}

fn map_operator_error(error: IndexDriftDiagnosisOperatorError) -> FieldError {
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
        IndexDriftDiagnosisOperatorError::Diagnosis(error) => map_digest_error(error),
    }
}

fn map_digest_error(error: rustok_index::IndexDriftDigestError) -> FieldError {
    match error {
        rustok_index::IndexDriftDigestError::NilTenantId
        | rustok_index::IndexDriftDigestError::NilEntityId
        | rustok_index::IndexDriftDigestError::ZeroSchemaVersion => {
            <FieldError as GraphQLError>::bad_user_input("Invalid Index drift diagnosis key")
        }
        rustok_index::IndexDriftDigestError::SnapshotCaptureFailed(failure) => {
            map_dependency_failure("INDEX_DRIFT_SNAPSHOT_CAPTURE_FAILED", failure)
        }
        rustok_index::IndexDriftDigestError::MismatchRecordFailed(failure) => {
            map_dependency_failure("INDEX_DRIFT_MISMATCH_RECORD_FAILED", failure)
        }
        _ => <FieldError as GraphQLError>::internal_error("Index drift diagnosis failed"),
    }
}

fn map_dependency_failure(
    code: &'static str,
    failure: rustok_index::IndexDriftDependencyFailure,
) -> FieldError {
    let retryable = matches!(
        failure.kind(),
        rustok_index::IndexDriftDependencyFailureKind::Retryable
    );
    FieldError::new("Index drift diagnosis dependency failed").extend_with(|_, extensions| {
        extensions.set("code", code);
        extensions.set("retryable", retryable);
        extensions.set("dependency_code", failure.code());
    })
}

#[cfg(test)]
mod tests {
    use rustok_api::Permission;
    use rustok_core::UserRole;
    use uuid::Uuid;

    use super::{
        IndexDriftDiagnosisInput, IndexDriftDiagnosisTransportPreparationError,
        prepare_authorized_request,
    };
    use crate::services::rbac_request_scope::{RbacRequestScope, with_rbac_request_scope};

    fn malformed_input() -> IndexDriftDiagnosisInput {
        IndexDriftDiagnosisInput {
            module_name: "Rustok Product".to_owned(),
            entity_name: "product".to_owned(),
            schema_version: "zero".to_owned(),
            entity_id: "not-a-uuid".to_owned(),
            locale: Some("not a locale".to_owned()),
        }
    }

    #[tokio::test]
    async fn transport_authorizes_before_parsing_untrusted_input() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();

        let missing = with_rbac_request_scope(None, async {
            prepare_authorized_request(tenant_id, actor_id, malformed_input())
        })
        .await
        .expect_err("missing request authority must win over malformed input");
        assert_eq!(
            missing,
            IndexDriftDiagnosisTransportPreparationError::MissingRequestAuthority
        );

        let forbidden = with_rbac_request_scope(
            Some(RbacRequestScope::new(
                tenant_id,
                actor_id,
                vec![Permission::MODULES_READ],
                UserRole::Admin,
            )),
            async { prepare_authorized_request(tenant_id, actor_id, malformed_input()) },
        )
        .await
        .expect_err("modules:read must fail before malformed input parsing");
        assert_eq!(
            forbidden,
            IndexDriftDiagnosisTransportPreparationError::Forbidden
        );

        let invalid = with_rbac_request_scope(
            Some(RbacRequestScope::new(
                tenant_id,
                actor_id,
                vec![Permission::MODULES_MANAGE],
                UserRole::Admin,
            )),
            async { prepare_authorized_request(tenant_id, actor_id, malformed_input()) },
        )
        .await
        .expect_err("authorized malformed input must reach bounded parsing");
        assert_eq!(
            invalid,
            IndexDriftDiagnosisTransportPreparationError::InvalidInput {
                field: "module_name"
            }
        );
    }

    #[tokio::test]
    async fn transport_derives_tenant_and_builds_one_exact_key() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let entity_id = Uuid::new_v4();
        let input = IndexDriftDiagnosisInput {
            module_name: "rustok-product".to_owned(),
            entity_name: "product".to_owned(),
            schema_version: "2".to_owned(),
            entity_id: entity_id.to_string(),
            locale: Some("en-US".to_owned()),
        };

        let (_, key) = with_rbac_request_scope(
            Some(RbacRequestScope::new(
                tenant_id,
                actor_id,
                vec![Permission::MODULES_MANAGE],
                UserRole::Admin,
            )),
            async { prepare_authorized_request(tenant_id, actor_id, input) },
        )
        .await
        .expect("authorized exact key should parse");

        assert_eq!(key.tenant_id, tenant_id);
        assert_eq!(key.entity_id, entity_id);
        assert_eq!(key.schema.module.as_str(), "rustok-product");
        assert_eq!(key.schema.entity.as_str(), "product");
        assert_eq!(key.schema.version.get(), 2);
        assert_eq!(
            key.locale.as_ref().map(|locale| locale.as_str()),
            Some("en-US")
        );
    }
}
