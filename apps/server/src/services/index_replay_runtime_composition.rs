#[path = "index_replay_runtime_composition_base.rs"]
mod base;

pub use base::{
    IndexReconciliationOperatorContext, IndexReconciliationOperatorError,
    IndexReconciliationOperatorRuntime, IndexReplayOperatorContext, IndexReplayOperatorError,
    IndexReplayOperatorRuntime,
};

use std::fmt;

use rustok_api::{Permission, has_effective_permission};
use rustok_core::ModuleRuntimeExtensions;
use sea_orm::DatabaseConnection;
use thiserror::Error;
use uuid::Uuid;

use crate::error::{Error as ServerError, Result};
use crate::services::rbac_request_scope::permissions_for;

#[derive(Debug, Error)]
pub enum IndexReconciliationDeadLetterOperatorError {
    #[error(
        "Index reconciliation dead-letter inspection requires a request-bound effective permission snapshot"
    )]
    MissingRequestAuthority,
    #[error("modules:manage is required for Index reconciliation dead-letter inspection")]
    Forbidden,
    #[error(transparent)]
    Inspection(#[from] rustok_index::IndexReconciliationDeadLetterInspectionError),
}

/// Server-owned guarded read-only boundary for one failed reconciliation job.
///
/// The tenant is always derived from the request-bound reconciliation operator context.
/// Callers provide only the failed job UUID; database access occurs only after the current
/// effective RBAC snapshot proves `modules:manage` for the exact context tenant and actor.
#[derive(Clone)]
pub struct IndexReconciliationDeadLetterOperatorRuntime {
    inner: rustok_index::PostgresIndexReconciliationDeadLetterInspector,
}

impl IndexReconciliationDeadLetterOperatorRuntime {
    fn new(inner: rustok_index::PostgresIndexReconciliationDeadLetterInspector) -> Self {
        Self { inner }
    }

    pub async fn inspect_dead_letter(
        &self,
        context: IndexReconciliationOperatorContext,
        job_id: Uuid,
    ) -> std::result::Result<
        Option<rustok_index::IndexReconciliationDeadLetterInspection>,
        IndexReconciliationDeadLetterOperatorError,
    > {
        authorize_dead_letter_inspection(&context)?;
        self.inner
            .inspect(context.tenant_id(), job_id)
            .await
            .map_err(Into::into)
    }
}

impl fmt::Debug for IndexReconciliationDeadLetterOperatorRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IndexReconciliationDeadLetterOperatorRuntime")
            .finish_non_exhaustive()
    }
}

fn authorize_dead_letter_inspection(
    context: &IndexReconciliationOperatorContext,
) -> std::result::Result<(), IndexReconciliationDeadLetterOperatorError> {
    let permissions = permissions_for(&context.tenant_id(), &context.actor_id())
        .ok_or(IndexReconciliationDeadLetterOperatorError::MissingRequestAuthority)?;
    if !has_effective_permission(&permissions, &Permission::MODULES_MANAGE) {
        return Err(IndexReconciliationDeadLetterOperatorError::Forbidden);
    }
    Ok(())
}

/// Materializes the existing replay/reconciliation operators and, only when the guarded
/// reconciliation capability exists, publishes the exact-tenant dead-letter inspector.
pub(crate) fn materialize_index_replay_runtime(
    extensions: &mut ModuleRuntimeExtensions,
    db: DatabaseConnection,
) -> Result<()> {
    if extensions.contains::<IndexReconciliationDeadLetterOperatorRuntime>() {
        return Err(ServerError::Message(
            "guarded Index reconciliation dead-letter runtime is already materialized".to_string(),
        ));
    }

    base::materialize_index_replay_runtime(extensions, db.clone())?;
    if extensions.contains::<IndexReconciliationOperatorRuntime>() {
        extensions.insert(IndexReconciliationDeadLetterOperatorRuntime::new(
            rustok_index::PostgresIndexReconciliationDeadLetterInspector::new(db),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use rustok_api::Permission;
    use rustok_core::UserRole;
    use sea_orm::Database;
    use uuid::Uuid;

    use super::{
        IndexReconciliationDeadLetterOperatorError,
        IndexReconciliationDeadLetterOperatorRuntime, IndexReconciliationOperatorContext,
    };
    use crate::services::rbac_request_scope::{RbacRequestScope, with_rbac_request_scope};

    async fn runtime() -> IndexReconciliationDeadLetterOperatorRuntime {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("test database without Index tables");
        IndexReconciliationDeadLetterOperatorRuntime::new(
            rustok_index::PostgresIndexReconciliationDeadLetterInspector::new(db),
        )
    }

    #[tokio::test]
    async fn dead_letter_inspection_requires_request_bound_authority_before_database_access() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let context = IndexReconciliationOperatorContext::new(tenant_id, actor_id).unwrap();
        let error = runtime()
            .await
            .inspect_dead_letter(context, Uuid::new_v4())
            .await
            .expect_err("missing request authority must fail before database access");
        assert!(matches!(
            error,
            IndexReconciliationDeadLetterOperatorError::MissingRequestAuthority
        ));
    }

    #[tokio::test]
    async fn dead_letter_inspection_requires_modules_manage() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let context = IndexReconciliationOperatorContext::new(tenant_id, actor_id).unwrap();
        let error = with_rbac_request_scope(
            Some(RbacRequestScope::new(
                tenant_id,
                actor_id,
                vec![Permission::MODULES_READ],
                UserRole::Admin,
            )),
            runtime()
                .await
                .inspect_dead_letter(context, Uuid::new_v4()),
        )
        .await
        .expect_err("modules:read must not authorize dead-letter inspection");
        assert!(matches!(
            error,
            IndexReconciliationDeadLetterOperatorError::Forbidden
        ));
    }

    #[tokio::test]
    async fn authorized_dead_letter_inspection_uses_context_tenant_and_delegates() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let context = IndexReconciliationOperatorContext::new(tenant_id, actor_id).unwrap();
        let error = with_rbac_request_scope(
            Some(RbacRequestScope::new(
                tenant_id,
                actor_id,
                vec![Permission::MODULES_MANAGE],
                UserRole::Admin,
            )),
            runtime()
                .await
                .inspect_dead_letter(context, Uuid::new_v4()),
        )
        .await
        .expect_err("authorized request should reach the table-less inspector fixture");
        assert!(matches!(
            error,
            IndexReconciliationDeadLetterOperatorError::Inspection(
                rustok_index::IndexReconciliationDeadLetterInspectionError::Storage
            )
        ));
    }
}
