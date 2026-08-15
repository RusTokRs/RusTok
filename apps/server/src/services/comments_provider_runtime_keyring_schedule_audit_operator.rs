use std::fmt;

use rustok_api::{Permission, has_effective_permission};
use thiserror::Error;
use uuid::Uuid;

use crate::error::{Error as ServerError, Result};
use crate::services::rbac_request_scope::permissions_for;
use crate::services::server_runtime_context::ServerRuntimeContext;

use super::{
    keyring_schedule_audit_handoff_worker::{
        CommentsTcpDelegationScheduleAuditHandoffWorkerConfig,
        start_comments_tcp_delegation_schedule_audit_handoff_worker_with_source_retry_if_enabled,
    },
    keyring_schedule_audit_recovery_postgres::{
        CommentsTcpDelegationScheduleAuditRecoveryError,
        CommentsTcpDelegationScheduleAuditRecoveryInspection,
        CommentsTcpDelegationScheduleAuditRecoveryOutcome,
        CommentsTcpDelegationScheduleAuditRecoveryRequest,
        PostgresCommentsTcpDelegationScheduleAuditRecoveryStore,
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommentsTcpDelegationScheduleAuditOperatorContext {
    tenant_id: Uuid,
    actor_id: Uuid,
}

impl CommentsTcpDelegationScheduleAuditOperatorContext {
    pub fn new(
        tenant_id: Uuid,
        actor_id: Uuid,
    ) -> std::result::Result<Self, CommentsTcpDelegationScheduleAuditOperatorError> {
        if tenant_id.is_nil() || actor_id.is_nil() {
            return Err(CommentsTcpDelegationScheduleAuditOperatorError::InvalidContext);
        }
        Ok(Self {
            tenant_id,
            actor_id,
        })
    }

    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub fn actor_id(&self) -> Uuid {
        self.actor_id
    }

    fn authorize_for(
        &self,
        control_plane_tenant_id: Uuid,
    ) -> std::result::Result<(), CommentsTcpDelegationScheduleAuditOperatorError> {
        if self.tenant_id != control_plane_tenant_id {
            return Err(CommentsTcpDelegationScheduleAuditOperatorError::TenantMismatch);
        }
        let permissions = permissions_for(&self.tenant_id, &self.actor_id)
            .ok_or(CommentsTcpDelegationScheduleAuditOperatorError::MissingRequestAuthority)?;
        if !has_effective_permission(&permissions, &Permission::MODULES_MANAGE) {
            return Err(CommentsTcpDelegationScheduleAuditOperatorError::Forbidden);
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum CommentsTcpDelegationScheduleAuditOperatorError {
    #[error("Comments schedule audit operator tenant and actor must not be nil")]
    InvalidContext,
    #[error(
        "Comments schedule audit operator tenant does not match the configured control-plane tenant"
    )]
    TenantMismatch,
    #[error(
        "Comments schedule audit operations require a request-bound effective permission snapshot"
    )]
    MissingRequestAuthority,
    #[error("modules:manage is required for Comments schedule audit operations")]
    Forbidden,
    #[error(transparent)]
    Recovery(#[from] CommentsTcpDelegationScheduleAuditRecoveryError),
}

/// Server-owned request-bound boundary for bounded source dead-letter inspection
/// and actor/reason-audited source requeue.
///
/// The caller supplies only a validated operator context and exact source-row
/// recovery facts. Tenant and actor identities delegated to storage are derived
/// exclusively from the authorized context. This capability starts no task and
/// exposes no database connection, source claim token, payload, SQL, or transport.
#[derive(Clone)]
pub struct CommentsTcpDelegationScheduleAuditOperatorRuntime {
    control_plane_tenant_id: Uuid,
    recovery: PostgresCommentsTcpDelegationScheduleAuditRecoveryStore,
}

impl CommentsTcpDelegationScheduleAuditOperatorRuntime {
    fn new(
        control_plane_tenant_id: Uuid,
        recovery: PostgresCommentsTcpDelegationScheduleAuditRecoveryStore,
    ) -> Self {
        Self {
            control_plane_tenant_id,
            recovery,
        }
    }

    pub async fn inspect_dead_letter(
        &self,
        context: CommentsTcpDelegationScheduleAuditOperatorContext,
        request_id: Uuid,
    ) -> std::result::Result<
        Option<CommentsTcpDelegationScheduleAuditRecoveryInspection>,
        CommentsTcpDelegationScheduleAuditOperatorError,
    > {
        context.authorize_for(self.control_plane_tenant_id)?;
        self.recovery
            .inspect_dead_letter(request_id)
            .await
            .map_err(Into::into)
    }

    pub async fn requeue_dead_letter(
        &self,
        context: CommentsTcpDelegationScheduleAuditOperatorContext,
        request_id: Uuid,
        expected_attempt_count: i64,
        expected_recovery_epoch: i64,
        reason: impl Into<String>,
    ) -> std::result::Result<
        CommentsTcpDelegationScheduleAuditRecoveryOutcome,
        CommentsTcpDelegationScheduleAuditOperatorError,
    > {
        context.authorize_for(self.control_plane_tenant_id)?;
        let request = CommentsTcpDelegationScheduleAuditRecoveryRequest::new(
            context.tenant_id,
            request_id,
            context.actor_id,
            expected_attempt_count,
            expected_recovery_epoch,
            reason,
        )?;
        self.recovery
            .requeue_dead_letter(request)
            .await
            .map_err(Into::into)
    }
}

impl fmt::Debug for CommentsTcpDelegationScheduleAuditOperatorRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommentsTcpDelegationScheduleAuditOperatorRuntime")
            .finish_non_exhaustive()
    }
}

/// Preserves the single canonical bootstrap startup name while installing the
/// task-free operator capability immediately before the existing worker starts.
pub fn start_comments_tcp_delegation_schedule_audit_handoff_worker_with_operator_if_enabled(
    runtime_ctx: &ServerRuntimeContext,
) -> Result<()> {
    materialize_comments_tcp_delegation_schedule_audit_operator(runtime_ctx)
        .map_err(ServerError::BadRequest)?;
    start_comments_tcp_delegation_schedule_audit_handoff_worker_with_source_retry_if_enabled(
        runtime_ctx,
    )
}

/// Materializes one guarded operator capability when the canonical audit handoff
/// lane is enabled. Composition performs no database I/O and starts no task.
pub fn materialize_comments_tcp_delegation_schedule_audit_operator(
    runtime_ctx: &ServerRuntimeContext,
) -> std::result::Result<(), String> {
    let Some(config) = CommentsTcpDelegationScheduleAuditHandoffWorkerConfig::from_environment()?
    else {
        return Ok(());
    };
    if runtime_ctx
        .shared_get::<CommentsTcpDelegationScheduleAuditOperatorRuntime>()
        .is_some()
    {
        return Ok(());
    }
    let recovery =
        PostgresCommentsTcpDelegationScheduleAuditRecoveryStore::new(runtime_ctx.db_clone())?;
    let runtime = CommentsTcpDelegationScheduleAuditOperatorRuntime::new(
        config.control_plane_tenant_id(),
        recovery,
    );
    let _ = runtime_ctx.shared_insert_if_absent(runtime);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operator_context_rejects_nil_identity() {
        assert!(
            CommentsTcpDelegationScheduleAuditOperatorContext::new(Uuid::nil(), Uuid::new_v4(),)
                .is_err()
        );
        assert!(
            CommentsTcpDelegationScheduleAuditOperatorContext::new(Uuid::new_v4(), Uuid::nil(),)
                .is_err()
        );
    }

    #[test]
    fn tenant_mismatch_fails_before_request_authority_lookup() {
        let context =
            CommentsTcpDelegationScheduleAuditOperatorContext::new(Uuid::new_v4(), Uuid::new_v4())
                .unwrap();
        assert!(matches!(
            context.authorize_for(Uuid::new_v4()),
            Err(CommentsTcpDelegationScheduleAuditOperatorError::TenantMismatch)
        ));
    }
}
