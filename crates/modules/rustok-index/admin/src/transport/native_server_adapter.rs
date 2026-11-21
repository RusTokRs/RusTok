use leptos::prelude::*;

use crate::model::{
    CancelActionResult, CancelJobInput, IndexAdminBootstrap, ReplayActionResult, RetryActionResult,
    RetryJobInput, TriggerReplayInput,
};

#[cfg(feature = "ssr")]
mod snapshot;

#[cfg(feature = "ssr")]
use super::native_http::{IndexAdminUpstream, IndexEndpoint};

/// Host-owned graceful-stop probe for replay; never supplied by a server-function caller.
#[cfg(feature = "ssr")]
#[derive(Clone)]
pub struct IndexAdminReplayStop(std::sync::Arc<dyn Fn() -> bool + Send + Sync>);

#[cfg(feature = "ssr")]
impl IndexAdminReplayStop {
    pub fn new(is_stopping: impl Fn() -> bool + Send + Sync + 'static) -> Self {
        Self(std::sync::Arc::new(is_stopping))
    }
}

#[cfg(feature = "ssr")]
async fn authenticated_context()
-> Result<(rustok_api::AuthContext, rustok_api::TenantContext), ServerFnError> {
    use rustok_api::{AuthContext, TenantContext};

    let auth = leptos_axum::extract::<AuthContext>()
        .await
        .map_err(ServerFnError::new)?;
    let tenant = leptos_axum::extract::<TenantContext>()
        .await
        .map_err(ServerFnError::new)?;
    validate_request_scope(&auth, &tenant)?;
    Ok((auth, tenant))
}

#[cfg(feature = "ssr")]
fn validate_request_scope(
    auth: &rustok_api::AuthContext,
    tenant: &rustok_api::TenantContext,
) -> Result<(), ServerFnError> {
    if auth.tenant_id.is_nil()
        || tenant.id.is_nil()
        || auth.user_id.is_nil()
        || auth.tenant_id != tenant.id
        || !tenant.is_active
    {
        tracing::warn!(auth_tenant_id = %auth.tenant_id, resolved_tenant_id = %tenant.id,
            code = "index.admin_invalid_tenant_scope", "Denied Index admin request");
        return Err(ServerFnError::new("Index admin access is denied"));
    }
    Ok(())
}

#[cfg(feature = "ssr")]
fn authorize_bootstrap(auth: &rustok_api::AuthContext) -> Result<bool, ServerFnError> {
    use rustok_api::{Permission, has_effective_permission};
    if !has_effective_permission(&auth.permissions, &Permission::SETTINGS_READ) {
        return Err(ServerFnError::new(
            "settings:read required to inspect Index",
        ));
    }
    Ok(has_effective_permission(
        &auth.permissions,
        &Permission::MODULES_MANAGE,
    ))
}

#[cfg(feature = "ssr")]
fn authorize_operator(
    auth: &rustok_api::AuthContext,
    tenant: &rustok_api::TenantContext,
) -> Result<rustok_index::IndexJobOperatorAuthority, ServerFnError> {
    rustok_index::IndexJobOperatorAuthority::authorize(
        tenant.id,
        auth.tenant_id,
        auth.user_id,
        &auth.permissions,
    )
    .map_err(operator_error)
}

#[cfg(feature = "ssr")]
fn replay_runtime(
    host: &rustok_api::HostRuntimeContext,
) -> Result<rustok_index::SharedIndexReplayRuntime, ServerFnError> {
    host.shared_get::<rustok_index::SharedIndexReplayRuntime>()
        .ok_or_else(|| ServerFnError::new("Index replay runtime is unavailable"))
}

#[cfg(feature = "ssr")]
fn host_context() -> Result<rustok_api::HostRuntimeContext, ServerFnError> {
    use_context::<rustok_api::HostRuntimeContext>()
        .ok_or_else(|| ServerFnError::new("Index host runtime is unavailable"))
}

#[cfg(feature = "ssr")]
async fn operator_context() -> Result<
    (
        rustok_index::IndexJobOperatorAuthority,
        rustok_index::SharedIndexReplayRuntime,
        uuid::Uuid,
    ),
    ServerFnError,
> {
    let (auth, tenant) = authenticated_context().await?;
    let authority = authorize_operator(&auth, &tenant)?;
    let runtime = replay_runtime(&host_context()?)?;
    Ok((authority, runtime, tenant.id))
}

#[cfg(feature = "ssr")]
fn operator_error(error: rustok_index::IndexJobOperatorError) -> ServerFnError {
    use rustok_index::{IndexJobOperatorError as Operator, IndexJobRecoveryError as Recovery};
    let message = match error {
        Operator::Forbidden => "modules:manage required for Index jobs",
        Operator::Recovery(Recovery::InvalidReason(_) | Recovery::NilJobId) => {
            "Invalid job ID or recovery reason"
        }
        Operator::Replay(rustok_index::IndexReplayRunError::Job(
            rustok_index::IndexReplayJobError::DeadLettered { .. },
        )) => "Replay is dead-lettered; resolve the failure and requeue the original job",
        Operator::Replay(rustok_index::IndexReplayRunError::UnknownSchemaSource(_)) => {
            "No selected replay source owns this schema"
        }
        error => {
            tracing::warn!(%error, "Index job operation failed");
            "Index command result unavailable; refresh status before retrying"
        }
    };
    ServerFnError::new(message)
}

#[cfg(feature = "ssr")]
fn exact_job_id(input: &str) -> Result<uuid::Uuid, ServerFnError> {
    uuid::Uuid::parse_str(input.trim())
        .ok()
        .filter(|id| !id.is_nil())
        .ok_or_else(|| ServerFnError::new("Job ID must be a non-nil UUID"))
}

#[server(prefix = "/api/fn", endpoint = "index/bootstrap", input = leptos::server_fn::codec::Json, client = rustok_auth_ui::ui::leptos::AuthorizedBrowserClient)]
pub async fn fetch_bootstrap_native() -> Result<IndexAdminBootstrap, ServerFnError> {
    if let Some(upstream) = use_context::<IndexAdminUpstream>() {
        return upstream
            .relay(IndexEndpoint::Bootstrap, serde_json::json!({}))
            .await;
    }

    let (auth, tenant) = authenticated_context().await?;
    let can_manage = authorize_bootstrap(&auth)?;
    let runtime = host_context()?;
    snapshot::read(&runtime, tenant, can_manage)
        .await
        .map_err(|error| {
            tracing::warn!(%error, "Index admin snapshot failed");
            ServerFnError::new("Index storage snapshot is unavailable")
        })
}

#[server(prefix = "/api/fn", endpoint = "index/trigger-replay", input = leptos::server_fn::codec::Json, client = rustok_auth_ui::ui::leptos::AuthorizedBrowserClient)]
pub async fn trigger_replay_native(
    input: TriggerReplayInput,
) -> Result<ReplayActionResult, ServerFnError> {
    if let Some(upstream) = use_context::<IndexAdminUpstream>() {
        return upstream
            .relay(
                IndexEndpoint::Trigger,
                serde_json::json!({ "input": input }),
            )
            .await;
    }

    use crate::model::ReplayActionStatus;
    use rustok_index::IndexReplayRunStatus;
    use std::time::Duration;

    let (authority, runtime, tenant_id) = operator_context().await?;
    let stop = host_context()?
        .shared_get::<IndexAdminReplayStop>()
        .ok_or_else(|| ServerFnError::new("Index replay shutdown signal is unavailable"))?;
    if !(1..=i32::MAX as u32).contains(&input.schema_version) {
        return Err(ServerFnError::new("Invalid Index schema version"));
    }
    let schema = rustok_index::SchemaRef {
        module: rustok_index::ModuleName::new(input.schema_module.trim())
            .map_err(|_| ServerFnError::new("Invalid Index schema module"))?,
        entity: rustok_index::EntityName::new(input.schema_entity.trim())
            .map_err(|_| ServerFnError::new("Invalid Index schema entity"))?,
        version: rustok_index::SchemaVersion::new(input.schema_version),
    };
    let worker_id = format!("admin-replay-{}", uuid::Uuid::new_v4().simple());
    let lease_duration = Duration::from_secs(60);
    let request = if let Some(locale) = input.locale {
        let locale = rustok_index::LocaleKey::new(locale)
            .map_err(|_| ServerFnError::new("Invalid Index replay locale"))?;
        rustok_index::IndexReplayRunRequest::for_locale(
            tenant_id,
            schema,
            locale,
            worker_id,
            100,
            8,
            1,
            lease_duration,
        )
    } else {
        rustok_index::IndexReplayRunRequest::new(
            tenant_id,
            schema,
            worker_id,
            100,
            8,
            1,
            lease_duration,
        )
    }
    .map_err(|_| ServerFnError::new("Invalid Index replay request"))?;
    let outcome = runtime
        .run_operator(authority, request, || (stop.0)())
        .await
        .map_err(operator_error)?;
    Ok(ReplayActionResult {
        job_id: outcome.job_id().map(|id| id.to_string()),
        status: match outcome.status() {
            IndexReplayRunStatus::Busy => ReplayActionStatus::Busy,
            IndexReplayRunStatus::AlreadyComplete => ReplayActionStatus::AlreadyComplete,
            IndexReplayRunStatus::Complete => ReplayActionStatus::Complete,
            IndexReplayRunStatus::Cancelled => ReplayActionStatus::Cancelled,
            IndexReplayRunStatus::Yielded => ReplayActionStatus::Yielded,
            IndexReplayRunStatus::RetryScheduled => ReplayActionStatus::RetryScheduled,
            IndexReplayRunStatus::FailedPermanent => ReplayActionStatus::FailedPermanent,
            IndexReplayRunStatus::FailedExhausted => ReplayActionStatus::FailedExhausted,
        },
        retry_after_seconds: outcome.retry_after().map(|delay| delay.as_secs()),
        pages_processed: outcome.pages_processed() as u64,
        mutations_applied: outcome.applied_count() as u64,
    })
}

#[server(prefix = "/api/fn", endpoint = "index/cancel-job", input = leptos::server_fn::codec::Json, client = rustok_auth_ui::ui::leptos::AuthorizedBrowserClient)]
pub async fn cancel_job_native(input: CancelJobInput) -> Result<CancelActionResult, ServerFnError> {
    if let Some(upstream) = use_context::<IndexAdminUpstream>() {
        return upstream
            .relay(IndexEndpoint::Cancel, serde_json::json!({ "input": input }))
            .await;
    }

    use crate::model::CancelActionStatus;
    use rustok_index::{IndexReplayCancelOutcome as Cancel, IndexReplayTerminalState as Terminal};

    let (authority, runtime, _) = operator_context().await?;
    let job_id = exact_job_id(&input.job_id)?;
    let outcome = runtime
        .cancel_operator_replay(authority, job_id)
        .await
        .map_err(operator_error)?;
    Ok(CancelActionResult {
        job_id: job_id.to_string(),
        outcome: match outcome {
            Cancel::Requested => CancelActionStatus::Requested,
            Cancel::Cancelled => CancelActionStatus::Cancelled,
            Cancel::AlreadyTerminal(Terminal::Succeeded) => CancelActionStatus::AlreadySucceeded,
            Cancel::AlreadyTerminal(Terminal::Failed) => CancelActionStatus::AlreadyFailed,
            Cancel::AlreadyTerminal(Terminal::Cancelled) => CancelActionStatus::AlreadyCancelled,
            Cancel::NotFound => CancelActionStatus::NotFound,
        },
    })
}

#[server(prefix = "/api/fn", endpoint = "index/retry-job", input = leptos::server_fn::codec::Json, client = rustok_auth_ui::ui::leptos::AuthorizedBrowserClient)]
pub async fn retry_job_native(input: RetryJobInput) -> Result<RetryActionResult, ServerFnError> {
    if let Some(upstream) = use_context::<IndexAdminUpstream>() {
        return upstream
            .relay(IndexEndpoint::Retry, serde_json::json!({ "input": input }))
            .await;
    }

    use crate::model::{RetryActionStatus, RetryJobKind};
    use rustok_index::IndexJobRequeueOutcome;

    let (authority, runtime, _) = operator_context().await?;
    let job_id = exact_job_id(&input.job_id)?;
    let reason = input.reason.trim();
    let outcome = match input.kind {
        RetryJobKind::Replay => {
            runtime
                .requeue_operator_replay(authority, job_id, reason)
                .await
        }
        RetryJobKind::Reconciliation => {
            runtime
                .requeue_operator_reconciliation(authority, job_id, reason)
                .await
        }
    }
    .map_err(operator_error)?;
    let (outcome, retry_epoch) = match outcome {
        IndexJobRequeueOutcome::Requeued { retry_epoch, .. } => {
            (RetryActionStatus::Requeued, Some(retry_epoch))
        }
        IndexJobRequeueOutcome::NotFound => (RetryActionStatus::NotFound, None),
        IndexJobRequeueOutcome::NotFailed => (RetryActionStatus::NotFailed, None),
        IndexJobRequeueOutcome::ScopeUnavailable => (RetryActionStatus::ScopeUnavailable, None),
    };
    Ok(RetryActionResult {
        job_id: job_id.to_string(),
        outcome,
        retry_epoch,
    })
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use rustok_api::{AuthContext, HostRuntimeContext, Permission, TenantContext};
    use uuid::Uuid;

    use super::*;

    fn scope(permissions: Vec<Permission>) -> (AuthContext, TenantContext) {
        let tenant_id = Uuid::new_v4();
        let auth = AuthContext {
            user_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions,
            client_id: None,
            scopes: vec![],
            grant_type: "user".into(),
        };
        let tenant = TenantContext {
            id: tenant_id,
            name: "Fixture".into(),
            slug: "fixture".into(),
            domain: None,
            settings: serde_json::json!({}),
            default_locale: "en".into(),
            is_active: true,
        };
        (auth, tenant)
    }

    #[test]
    fn native_read_and_operator_permissions_are_separate() {
        let (mut auth, tenant) = scope(vec![Permission::SETTINGS_READ]);
        assert!(validate_request_scope(&auth, &tenant).is_ok());
        assert!(!authorize_bootstrap(&auth).unwrap());
        assert!(authorize_operator(&auth, &tenant).is_err());
        auth.permissions = vec![Permission::MODULES_MANAGE];
        assert!(authorize_bootstrap(&auth).is_err());
        assert!(authorize_operator(&auth, &tenant).is_ok());
        auth.permissions.push(Permission::SETTINGS_READ);
        assert!(authorize_bootstrap(&auth).unwrap());
    }

    #[test]
    fn native_rejects_cross_tenant_nil_actor_and_inactive_requests() {
        let (mut auth, mut tenant) =
            scope(vec![Permission::SETTINGS_READ, Permission::MODULES_MANAGE]);
        tenant.id = Uuid::new_v4();
        assert!(validate_request_scope(&auth, &tenant).is_err());
        assert!(authorize_operator(&auth, &tenant).is_err());
        tenant.id = auth.tenant_id;
        tenant.is_active = false;
        assert!(validate_request_scope(&auth, &tenant).is_err());
        tenant.is_active = true;
        auth.user_id = Uuid::nil();
        assert!(validate_request_scope(&auth, &tenant).is_err());
    }

    #[test]
    fn native_reports_absent_runtime_without_building_a_fallback() {
        let host = HostRuntimeContext::new(sea_orm::DatabaseConnection::default());
        let Err(error) = replay_runtime(&host) else {
            panic!("missing selected runtime must fail closed");
        };
        assert!(
            error
                .to_string()
                .contains("Index replay runtime is unavailable")
        );
    }
}
