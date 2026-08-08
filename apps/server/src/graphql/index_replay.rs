use std::{sync::Arc, time::Duration};

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
use crate::services::app_lifecycle::StopHandle;
use crate::services::index_replay_runtime_composition::{
    IndexReplayOperatorContext, IndexReplayOperatorError, IndexReplayOperatorRuntime,
    IndexReplayShadowOperatorError, IndexReplayShadowTransportError,
    IndexReplayShadowTransportOutcome, IndexReplayShadowTransportRuntime,
};
use crate::services::rbac_request_scope::permissions_for;

const MAX_SCHEMA_IDENTIFIER_BYTES: usize = 128;
const MAX_SCHEMA_VERSION_BYTES: usize = 10;
const MAX_LOCALE_BYTES: usize = 32;
const MAX_CONTINUATION_BYTES: usize = 16 * 1024;
const GRAPHQL_REPLAY_PAGE_LIMIT: usize = 100;
const GRAPHQL_REPLAY_MAX_PAGES: usize = 8;
const GRAPHQL_REPLAY_HEARTBEAT_EVERY_PAGES: usize = 1;
const GRAPHQL_REPLAY_LEASE_SECONDS: u64 = 60;

/// Untrusted GraphQL input for one bounded schema-wide or exact-locale replay run.
///
/// Tenant, actor, worker identity and replay resource budgets are server-owned and never caller supplied.
/// Omitting locale preserves the historical schema-wide replay identity.
#[derive(Debug, Clone, InputObject)]
pub struct IndexReplayRunInput {
    pub module_name: String,
    pub entity_name: String,
    pub schema_version: String,
    pub locale: Option<String>,
}

/// Untrusted input for one bounded schema-wide or exact-locale Shadow validation run.
///
/// The only resumable state is an authenticated confidential continuation token. Source identity,
/// page budget, jobs, checkpoints, leases, cancellation and retry controls are not caller fields.
/// Omitting locale preserves the schema-wide Shadow scan identity.
#[derive(Debug, Clone, InputObject)]
pub struct IndexReplayShadowRunInput {
    pub module_name: String,
    pub entity_name: String,
    pub schema_version: String,
    pub locale: Option<String>,
    pub continuation: Option<String>,
}

#[derive(Debug, Clone, InputObject)]
pub struct IndexReplayCancelInput {
    pub job_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum IndexReplayGraphqlRunStatus {
    Busy,
    AlreadyComplete,
    Complete,
    Cancelled,
    Yielded,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct IndexReplayRunPayload {
    pub status: IndexReplayGraphqlRunStatus,
    pub job_id: Option<Uuid>,
    pub pages_processed: i32,
    pub mutations_processed: i32,
    pub applied_count: i32,
    pub duplicate_count: i32,
    pub stale_count: i32,
}

impl TryFrom<rustok_index::IndexReplayRunOutcome> for IndexReplayRunPayload {
    type Error = FieldError;

    fn try_from(outcome: rustok_index::IndexReplayRunOutcome) -> Result<Self, Self::Error> {
        Ok(Self {
            status: match outcome.status() {
                rustok_index::IndexReplayRunStatus::Busy => IndexReplayGraphqlRunStatus::Busy,
                rustok_index::IndexReplayRunStatus::AlreadyComplete => {
                    IndexReplayGraphqlRunStatus::AlreadyComplete
                }
                rustok_index::IndexReplayRunStatus::Complete => IndexReplayGraphqlRunStatus::Complete,
                rustok_index::IndexReplayRunStatus::Cancelled => {
                    IndexReplayGraphqlRunStatus::Cancelled
                }
                rustok_index::IndexReplayRunStatus::Yielded => IndexReplayGraphqlRunStatus::Yielded,
            },
            job_id: outcome.job_id(),
            pages_processed: bounded_count(outcome.pages_processed())?,
            mutations_processed: bounded_count(outcome.mutation_count())?,
            applied_count: bounded_count(outcome.applied_count())?,
            duplicate_count: bounded_count(outcome.duplicate_count())?,
            stale_count: bounded_count(outcome.stale_count())?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum IndexReplayGraphqlShadowStatus {
    Complete,
    Yielded,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct IndexReplayShadowRunPayload {
    pub status: IndexReplayGraphqlShadowStatus,
    pub pages_scanned: i32,
    pub mutations_scanned: i32,
    pub upsert_count: i32,
    pub delete_count: i32,
    pub continuation: Option<String>,
}

impl TryFrom<IndexReplayShadowTransportOutcome> for IndexReplayShadowRunPayload {
    type Error = FieldError;

    fn try_from(outcome: IndexReplayShadowTransportOutcome) -> Result<Self, Self::Error> {
        Ok(Self {
            status: match outcome.status() {
                rustok_index::IndexReplayDryRunStatus::Complete => {
                    IndexReplayGraphqlShadowStatus::Complete
                }
                rustok_index::IndexReplayDryRunStatus::Yielded => {
                    IndexReplayGraphqlShadowStatus::Yielded
                }
            },
            pages_scanned: bounded_count(outcome.pages_scanned())?,
            mutations_scanned: bounded_count(outcome.mutation_count())?,
            upsert_count: bounded_count(outcome.upsert_count())?,
            delete_count: bounded_count(outcome.delete_count())?,
            continuation: outcome
                .next_token()
                .map(|token| token.as_str().to_owned()),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum IndexReplayGraphqlCancelStatus {
    Requested,
    Cancelled,
    AlreadySucceeded,
    AlreadyFailed,
    AlreadyCancelled,
    NotFound,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct IndexReplayCancelPayload {
    pub status: IndexReplayGraphqlCancelStatus,
}

impl From<rustok_index::IndexReplayCancelOutcome> for IndexReplayCancelPayload {
    fn from(outcome: rustok_index::IndexReplayCancelOutcome) -> Self {
        Self {
            status: match outcome {
                rustok_index::IndexReplayCancelOutcome::Requested => {
                    IndexReplayGraphqlCancelStatus::Requested
                }
                rustok_index::IndexReplayCancelOutcome::Cancelled => {
                    IndexReplayGraphqlCancelStatus::Cancelled
                }
                rustok_index::IndexReplayCancelOutcome::AlreadyTerminal(
                    rustok_index::IndexReplayTerminalState::Succeeded,
                ) => IndexReplayGraphqlCancelStatus::AlreadySucceeded,
                rustok_index::IndexReplayCancelOutcome::AlreadyTerminal(
                    rustok_index::IndexReplayTerminalState::Failed,
                ) => IndexReplayGraphqlCancelStatus::AlreadyFailed,
                rustok_index::IndexReplayCancelOutcome::AlreadyTerminal(
                    rustok_index::IndexReplayTerminalState::Cancelled,
                ) => IndexReplayGraphqlCancelStatus::AlreadyCancelled,
                rustok_index::IndexReplayCancelOutcome::NotFound => {
                    IndexReplayGraphqlCancelStatus::NotFound
                }
            },
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
enum IndexReplayTransportPreparationError {
    #[error("Index replay request context is invalid")]
    InvalidContext,
    #[error("Index replay requires a request-bound effective permission snapshot")]
    MissingRequestAuthority,
    #[error("modules:manage is required for Index replay operations")]
    Forbidden,
    #[error("Index replay input field is invalid: {field}")]
    InvalidInput { field: &'static str },
}

#[derive(Default)]
pub struct IndexReplayMutation;

#[Object]
impl IndexReplayMutation {
    /// Run one server-bounded chunk of the exact schema-wide or locale replay job.
    async fn run_index_replay(
        &self,
        ctx: &Context<'_>,
        input: IndexReplayRunInput,
    ) -> GraphqlResult<IndexReplayRunPayload> {
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        let (operator_context, request) =
            prepare_authorized_run(tenant.id, auth.user_id, input).map_err(map_preparation_error)?;
        let runtime = replay_runtime(ctx)?;
        let stop_handle = ctx.data::<StopHandle>()?.clone();
        runtime
            .run_interruptible(operator_context, request, || stop_handle.is_stopping())
            .await
            .map_err(map_operator_error)?
            .try_into()
    }

    /// Run one server-bounded schema-wide or exact-locale side-effect-free Shadow validation chunk.
    async fn run_index_replay_shadow(
        &self,
        ctx: &Context<'_>,
        input: IndexReplayShadowRunInput,
    ) -> GraphqlResult<IndexReplayShadowRunPayload> {
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        let (operator_context, schema, locale, continuation) =
            prepare_authorized_shadow_run(tenant.id, auth.user_id, input)
                .map_err(map_preparation_error)?;
        let runtime = shadow_replay_runtime(ctx)?;
        runtime
            .run(
                operator_context,
                schema,
                locale,
                continuation.as_deref(),
                GRAPHQL_REPLAY_PAGE_LIMIT,
                GRAPHQL_REPLAY_MAX_PAGES,
            )
            .await
            .map_err(map_shadow_transport_error)?
            .try_into()
    }

    /// Request cancellation of one replay job in the authenticated tenant.
    async fn cancel_index_replay(
        &self,
        ctx: &Context<'_>,
        input: IndexReplayCancelInput,
    ) -> GraphqlResult<IndexReplayCancelPayload> {
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        let (operator_context, job_id) =
            prepare_authorized_cancel(tenant.id, auth.user_id, input)
                .map_err(map_preparation_error)?;
        let runtime = replay_runtime(ctx)?;
        runtime
            .request_cancel(operator_context, job_id)
            .await
            .map(IndexReplayCancelPayload::from)
            .map_err(map_operator_error)
    }
}

fn replay_runtime(ctx: &Context<'_>) -> GraphqlResult<IndexReplayOperatorRuntime> {
    ctx.data::<Arc<ModuleRuntimeExtensions>>()?
        .get::<IndexReplayOperatorRuntime>()
        .cloned()
        .ok_or_else(|| {
            <FieldError as GraphQLError>::internal_error("Index replay runtime is not available")
        })
}

fn shadow_replay_runtime(ctx: &Context<'_>) -> GraphqlResult<IndexReplayShadowTransportRuntime> {
    ctx.data::<Arc<ModuleRuntimeExtensions>>()?
        .get::<IndexReplayShadowTransportRuntime>()
        .cloned()
        .ok_or_else(|| {
            <FieldError as GraphQLError>::internal_error(
                "Index replay Shadow transport runtime is not available",
            )
        })
}

fn prepare_authorized_run(
    tenant_id: Uuid,
    actor_id: Uuid,
    input: IndexReplayRunInput,
) -> std::result::Result<
    (IndexReplayOperatorContext, rustok_index::IndexReplayRunRequest),
    IndexReplayTransportPreparationError,
> {
    let context = authorize(tenant_id, actor_id)?;
    let schema = parse_schema(input.module_name, input.entity_name, input.schema_version)?;
    let locale = parse_locale(input.locale)?;
    let worker_id = format!("graphql-replay-{}", Uuid::new_v4().simple());
    let request = if let Some(locale) = locale {
        rustok_index::IndexReplayRunRequest::for_locale(
            tenant_id,
            schema,
            locale,
            worker_id,
            GRAPHQL_REPLAY_PAGE_LIMIT,
            GRAPHQL_REPLAY_MAX_PAGES,
            GRAPHQL_REPLAY_HEARTBEAT_EVERY_PAGES,
            Duration::from_secs(GRAPHQL_REPLAY_LEASE_SECONDS),
        )
    } else {
        rustok_index::IndexReplayRunRequest::new(
            tenant_id,
            schema,
            worker_id,
            GRAPHQL_REPLAY_PAGE_LIMIT,
            GRAPHQL_REPLAY_MAX_PAGES,
            GRAPHQL_REPLAY_HEARTBEAT_EVERY_PAGES,
            Duration::from_secs(GRAPHQL_REPLAY_LEASE_SECONDS),
        )
    }
    .map_err(|_| IndexReplayTransportPreparationError::InvalidInput {
        field: "schema_version",
    })?;
    Ok((context, request))
}

fn prepare_authorized_shadow_run(
    tenant_id: Uuid,
    actor_id: Uuid,
    input: IndexReplayShadowRunInput,
) -> std::result::Result<
    (
        IndexReplayOperatorContext,
        rustok_index::SchemaRef,
        Option<rustok_index::LocaleKey>,
        Option<String>,
    ),
    IndexReplayTransportPreparationError,
> {
    let context = authorize(tenant_id, actor_id)?;
    let schema = parse_schema(input.module_name, input.entity_name, input.schema_version)?;
    let locale = parse_locale(input.locale)?;
    let continuation = input
        .continuation
        .map(|value| {
            bounded_text("continuation", &value, MAX_CONTINUATION_BYTES)?;
            Ok(value)
        })
        .transpose()?;
    Ok((context, schema, locale, continuation))
}

fn prepare_authorized_cancel(
    tenant_id: Uuid,
    actor_id: Uuid,
    input: IndexReplayCancelInput,
) -> std::result::Result<(IndexReplayOperatorContext, Uuid), IndexReplayTransportPreparationError> {
    let context = authorize(tenant_id, actor_id)?;
    let job_id = Uuid::parse_str(bounded_text("job_id", &input.job_id, 64)?)
        .ok()
        .filter(|value| !value.is_nil())
        .ok_or(IndexReplayTransportPreparationError::InvalidInput { field: "job_id" })?;
    Ok((context, job_id))
}

fn authorize(
    tenant_id: Uuid,
    actor_id: Uuid,
) -> std::result::Result<IndexReplayOperatorContext, IndexReplayTransportPreparationError> {
    let context = IndexReplayOperatorContext::new(tenant_id, actor_id)
        .map_err(|_| IndexReplayTransportPreparationError::InvalidContext)?;
    let permissions = permissions_for(&tenant_id, &actor_id)
        .ok_or(IndexReplayTransportPreparationError::MissingRequestAuthority)?;
    if !has_effective_permission(&permissions, &Permission::MODULES_MANAGE) {
        return Err(IndexReplayTransportPreparationError::Forbidden);
    }
    Ok(context)
}

fn parse_schema(
    module_name: String,
    entity_name: String,
    schema_version: String,
) -> std::result::Result<rustok_index::SchemaRef, IndexReplayTransportPreparationError> {
    let module_name = bounded_text("module_name", &module_name, MAX_SCHEMA_IDENTIFIER_BYTES)?;
    let entity_name = bounded_text("entity_name", &entity_name, MAX_SCHEMA_IDENTIFIER_BYTES)?;
    let schema_version = bounded_text(
        "schema_version",
        &schema_version,
        MAX_SCHEMA_VERSION_BYTES,
    )?
    .parse::<u32>()
    .ok()
    .filter(|value| *value > 0)
    .ok_or(IndexReplayTransportPreparationError::InvalidInput {
        field: "schema_version",
    })?;

    Ok(rustok_index::SchemaRef {
        module: rustok_index::ModuleName::new(module_name).map_err(|_| {
            IndexReplayTransportPreparationError::InvalidInput {
                field: "module_name",
            }
        })?,
        entity: rustok_index::EntityName::new(entity_name).map_err(|_| {
            IndexReplayTransportPreparationError::InvalidInput {
                field: "entity_name",
            }
        })?,
        version: rustok_index::SchemaVersion::new(schema_version),
    })
}

fn parse_locale(
    locale: Option<String>,
) -> std::result::Result<Option<rustok_index::LocaleKey>, IndexReplayTransportPreparationError> {
    locale
        .map(|locale| {
            let locale = bounded_text("locale", &locale, MAX_LOCALE_BYTES)?;
            rustok_index::LocaleKey::new(locale).map_err(|_| {
                IndexReplayTransportPreparationError::InvalidInput { field: "locale" }
            })
        })
        .transpose()
}

fn bounded_text<'a>(
    field: &'static str,
    value: &'a str,
    max_bytes: usize,
) -> std::result::Result<&'a str, IndexReplayTransportPreparationError> {
    if value.is_empty() || value.len() > max_bytes {
        return Err(IndexReplayTransportPreparationError::InvalidInput { field });
    }
    Ok(value)
}

fn bounded_count(value: usize) -> GraphqlResult<i32> {
    i32::try_from(value)
        .map_err(|_| <FieldError as GraphQLError>::internal_error("Index replay count overflow"))
}

fn map_preparation_error(error: IndexReplayTransportPreparationError) -> FieldError {
    match error {
        IndexReplayTransportPreparationError::InvalidContext
        | IndexReplayTransportPreparationError::MissingRequestAuthority => {
            <FieldError as GraphQLError>::unauthenticated()
        }
        IndexReplayTransportPreparationError::Forbidden => {
            <FieldError as GraphQLError>::permission_denied(
                "Permission denied: modules:manage required",
            )
        }
        IndexReplayTransportPreparationError::InvalidInput { field } => {
            FieldError::new("Invalid Index replay input").extend_with(|_, extensions| {
                extensions.set("code", "BAD_USER_INPUT");
                extensions.set("input_field", field);
            })
        }
    }
}

fn map_operator_error(error: IndexReplayOperatorError) -> FieldError {
    match error {
        IndexReplayOperatorError::InvalidContext
        | IndexReplayOperatorError::MissingRequestAuthority => {
            <FieldError as GraphQLError>::unauthenticated()
        }
        IndexReplayOperatorError::TenantMismatch | IndexReplayOperatorError::Forbidden => {
            <FieldError as GraphQLError>::permission_denied(
                "Permission denied: modules:manage required",
            )
        }
        IndexReplayOperatorError::Replay(rustok_index::IndexReplayRunError::UnknownSchemaSource(_)) => {
            <FieldError as GraphQLError>::bad_user_input("Unknown Index replay schema")
        }
        IndexReplayOperatorError::Replay(_) => {
            <FieldError as GraphQLError>::internal_error("Index replay command failed")
        }
    }
}

fn map_shadow_transport_error(error: IndexReplayShadowTransportError) -> FieldError {
    match error {
        IndexReplayShadowTransportError::Authorization(error) => map_operator_error(error),
        IndexReplayShadowTransportError::ContinuationUnavailable => fixed_shadow_dependency_error(
            "INDEX_REPLAY_SHADOW_CONTINUATION_UNAVAILABLE",
            false,
            "Index replay Shadow continuation is unavailable",
        ),
        IndexReplayShadowTransportError::ContinuationKeyringUnavailable => {
            fixed_shadow_dependency_error(
                "INDEX_REPLAY_SHADOW_CONTINUATION_KEYRING_UNAVAILABLE",
                true,
                "Index replay Shadow continuation dependency failed",
            )
        }
        IndexReplayShadowTransportError::Continuation(error) => {
            map_shadow_continuation_error(error)
        }
        IndexReplayShadowTransportError::Request(error) => map_shadow_dry_run_error(error),
        IndexReplayShadowTransportError::Shadow(
            IndexReplayShadowOperatorError::Authorization(error),
        ) => map_operator_error(error),
        IndexReplayShadowTransportError::Shadow(IndexReplayShadowOperatorError::DryRun(error)) => {
            map_shadow_dry_run_error(error)
        }
    }
}

fn map_shadow_continuation_error(error: rustok_index::IndexSourceContinuationError) -> FieldError {
    use rustok_index::IndexSourceContinuationError as Error;

    match error {
        Error::UnknownSchemaSource(_) => {
            <FieldError as GraphQLError>::bad_user_input("Unknown Index replay schema")
        }
        Error::Expired => shadow_continuation_input_error("INDEX_REPLAY_SHADOW_CONTINUATION_EXPIRED"),
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
        | Error::IssuedAtInFuture => {
            shadow_continuation_input_error("INDEX_REPLAY_SHADOW_CONTINUATION_INVALID")
        }
        _ => <FieldError as GraphQLError>::internal_error(
            "Index replay Shadow continuation processing failed",
        ),
    }
}

fn shadow_continuation_input_error(code: &'static str) -> FieldError {
    FieldError::new("Invalid Index replay Shadow continuation").extend_with(|_, extensions| {
        extensions.set("code", code);
    })
}

fn map_shadow_dry_run_error(error: rustok_index::IndexReplayDryRunError) -> FieldError {
    match error {
        rustok_index::IndexReplayDryRunError::UnknownSchemaSource(_) => {
            <FieldError as GraphQLError>::bad_user_input("Unknown Index replay schema")
        }
        rustok_index::IndexReplayDryRunError::LocaleScopeUnsupported(_) => {
            <FieldError as GraphQLError>::bad_user_input(
                "Index replay Shadow schema does not support locale scope",
            )
        }
        _ => <FieldError as GraphQLError>::internal_error("Index replay Shadow command failed"),
    }
}

fn fixed_shadow_dependency_error(
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
        GRAPHQL_REPLAY_HEARTBEAT_EVERY_PAGES, GRAPHQL_REPLAY_LEASE_SECONDS,
        GRAPHQL_REPLAY_MAX_PAGES, GRAPHQL_REPLAY_PAGE_LIMIT, MAX_CONTINUATION_BYTES,
        IndexReplayCancelInput, IndexReplayRunInput, IndexReplayShadowRunInput,
        IndexReplayTransportPreparationError, prepare_authorized_cancel, prepare_authorized_run,
        prepare_authorized_shadow_run,
    };
    use crate::services::rbac_request_scope::{RbacRequestScope, with_rbac_request_scope};

    fn malformed_run() -> IndexReplayRunInput {
        IndexReplayRunInput {
            module_name: "Rustok Product".to_owned(),
            entity_name: "product".to_owned(),
            schema_version: "zero".to_owned(),
            locale: Some("not a locale!!!".to_owned()),
        }
    }

    fn malformed_shadow_run() -> IndexReplayShadowRunInput {
        IndexReplayShadowRunInput {
            module_name: "Rustok Product".to_owned(),
            entity_name: "product".to_owned(),
            schema_version: "zero".to_owned(),
            locale: Some("not a locale!!!".to_owned()),
            continuation: Some("not-a-token".to_owned()),
        }
    }

    #[tokio::test]
    async fn replay_transport_authorizes_before_parsing_untrusted_run_input() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();

        let missing = with_rbac_request_scope(None, async {
            prepare_authorized_run(tenant_id, actor_id, malformed_run())
        })
        .await
        .expect_err("missing authority must win over malformed input");
        assert_eq!(
            missing,
            IndexReplayTransportPreparationError::MissingRequestAuthority
        );

        let forbidden = with_rbac_request_scope(
            Some(RbacRequestScope::new(
                tenant_id,
                actor_id,
                vec![Permission::MODULES_READ],
                UserRole::Admin,
            )),
            async { prepare_authorized_run(tenant_id, actor_id, malformed_run()) },
        )
        .await
        .expect_err("modules:read must fail before replay input parsing");
        assert_eq!(forbidden, IndexReplayTransportPreparationError::Forbidden);
    }

    #[tokio::test]
    async fn shadow_transport_authorizes_before_schema_locale_and_continuation_parsing() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();

        let missing = with_rbac_request_scope(None, async {
            prepare_authorized_shadow_run(tenant_id, actor_id, malformed_shadow_run())
        })
        .await
        .expect_err("missing authority must win over malformed Shadow input");
        assert_eq!(
            missing,
            IndexReplayTransportPreparationError::MissingRequestAuthority
        );

        let forbidden = with_rbac_request_scope(
            Some(RbacRequestScope::new(
                tenant_id,
                actor_id,
                vec![Permission::MODULES_READ],
                UserRole::Admin,
            )),
            async {
                prepare_authorized_shadow_run(tenant_id, actor_id, malformed_shadow_run())
            },
        )
        .await
        .expect_err("modules:read must fail before Shadow input parsing");
        assert_eq!(forbidden, IndexReplayTransportPreparationError::Forbidden);
    }

    #[tokio::test]
    async fn shadow_transport_accepts_schema_locale_and_bounded_sealed_continuation() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let (_, schema, locale, continuation) = with_rbac_request_scope(
            Some(RbacRequestScope::new(
                tenant_id,
                actor_id,
                vec![Permission::MODULES_MANAGE],
                UserRole::Admin,
            )),
            async {
                prepare_authorized_shadow_run(
                    tenant_id,
                    actor_id,
                    IndexReplayShadowRunInput {
                        module_name: "rustok-product".to_owned(),
                        entity_name: "product".to_owned(),
                        schema_version: "4".to_owned(),
                        locale: Some("EN-us".to_owned()),
                        continuation: Some("sealed-token".to_owned()),
                    },
                )
            },
        )
        .await
        .expect("authorized Shadow input should parse");

        assert_eq!(schema.module.as_str(), "rustok-product");
        assert_eq!(schema.entity.as_str(), "product");
        assert_eq!(schema.version.get(), 4);
        assert_eq!(locale.as_ref().map(|locale| locale.as_str()), Some("en-US"));
        assert_eq!(continuation.as_deref(), Some("sealed-token"));

        let oversized = "x".repeat(MAX_CONTINUATION_BYTES + 1);
        let error = with_rbac_request_scope(
            Some(RbacRequestScope::new(
                tenant_id,
                actor_id,
                vec![Permission::MODULES_MANAGE],
                UserRole::Admin,
            )),
            async {
                prepare_authorized_shadow_run(
                    tenant_id,
                    actor_id,
                    IndexReplayShadowRunInput {
                        module_name: "rustok-product".to_owned(),
                        entity_name: "product".to_owned(),
                        schema_version: "4".to_owned(),
                        locale: None,
                        continuation: Some(oversized),
                    },
                )
            },
        )
        .await
        .expect_err("oversized Shadow continuation must fail closed");
        assert_eq!(
            error,
            IndexReplayTransportPreparationError::InvalidInput {
                field: "continuation"
            }
        );
    }

    #[tokio::test]
    async fn replay_transport_derives_authority_worker_and_server_owned_budgets() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let (_, request) = with_rbac_request_scope(
            Some(RbacRequestScope::new(
                tenant_id,
                actor_id,
                vec![Permission::MODULES_MANAGE],
                UserRole::Admin,
            )),
            async {
                prepare_authorized_run(
                    tenant_id,
                    actor_id,
                    IndexReplayRunInput {
                        module_name: "rustok-product".to_owned(),
                        entity_name: "product".to_owned(),
                        schema_version: "4".to_owned(),
                        locale: None,
                    },
                )
            },
        )
        .await
        .expect("authorized replay request should parse");

        assert_eq!(request.page_request().tenant_id(), tenant_id);
        assert_eq!(request.page_request().schema().module.as_str(), "rustok-product");
        assert_eq!(request.page_request().schema().entity.as_str(), "product");
        assert_eq!(request.page_request().schema().version.get(), 4);
        assert!(request.locale().is_none());
        assert!(request.worker_id().starts_with("graphql-replay-"));
        assert_eq!(request.page_request().limit(), GRAPHQL_REPLAY_PAGE_LIMIT);
        assert_eq!(request.max_pages(), GRAPHQL_REPLAY_MAX_PAGES);
        assert_eq!(
            request.heartbeat_every_pages(),
            GRAPHQL_REPLAY_HEARTBEAT_EVERY_PAGES
        );
        assert_eq!(
            request.lease_duration(),
            std::time::Duration::from_secs(GRAPHQL_REPLAY_LEASE_SECONDS)
        );
    }

    #[tokio::test]
    async fn replay_transport_canonicalizes_optional_locale_after_authorization() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let (_, request) = with_rbac_request_scope(
            Some(RbacRequestScope::new(
                tenant_id,
                actor_id,
                vec![Permission::MODULES_MANAGE],
                UserRole::Admin,
            )),
            async {
                prepare_authorized_run(
                    tenant_id,
                    actor_id,
                    IndexReplayRunInput {
                        module_name: "rustok-product".to_owned(),
                        entity_name: "product".to_owned(),
                        schema_version: "4".to_owned(),
                        locale: Some("EN-us".to_owned()),
                    },
                )
            },
        )
        .await
        .expect("authorized locale replay request should parse");

        assert_eq!(request.locale().map(|locale| locale.as_str()), Some("en-US"));
    }

    #[tokio::test]
    async fn replay_cancel_authorizes_before_job_id_parsing_and_derives_tenant() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let malformed = IndexReplayCancelInput {
            job_id: "not-a-uuid".to_owned(),
        };
        let forbidden = with_rbac_request_scope(
            Some(RbacRequestScope::new(
                tenant_id,
                actor_id,
                vec![Permission::MODULES_READ],
                UserRole::Admin,
            )),
            async { prepare_authorized_cancel(tenant_id, actor_id, malformed) },
        )
        .await
        .expect_err("authorization must precede job id parsing");
        assert_eq!(forbidden, IndexReplayTransportPreparationError::Forbidden);

        let job_id = Uuid::new_v4();
        let (_, parsed) = with_rbac_request_scope(
            Some(RbacRequestScope::new(
                tenant_id,
                actor_id,
                vec![Permission::MODULES_MANAGE],
                UserRole::Admin,
            )),
            async {
                prepare_authorized_cancel(
                    tenant_id,
                    actor_id,
                    IndexReplayCancelInput {
                        job_id: job_id.to_string(),
                    },
                )
            },
        )
        .await
        .expect("authorized job id should parse");
        assert_eq!(parsed, job_id);
    }
}
