//! Serializes lifecycle claim admission with governed checkpoint creation.
//!
//! This protects a committed aggregate claim, not arbitrary side effects of a
//! hook that continues after losing or releasing its claim. In particular,
//! lease expiry and orphan recovery still need an external-effect fence.

use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, Statement,
};
use thiserror::Error;
use uuid::Uuid;

use crate::backend::SupportedBackend;
use crate::{
    ArtifactInstallationResolveError, ModuleDefinition, ModuleDefinitionSource,
    ModuleInstallationScope, StaticTenantLifecycleStore, StaticTenantLifecycleStoreError,
};

#[derive(Debug, Error)]
pub(crate) enum LifecycleAdmissionError {
    #[error("an unresolved module release transition blocks lifecycle changes")]
    TransitionInProgress,
    #[error("the selected artifact definition no longer matches the active installation")]
    StaleDefinition,
    #[error(transparent)]
    Claim(#[from] StaticTenantLifecycleStoreError),
    #[error("lifecycle admission failed: {0}")]
    Database(String),
}

/// Atomically checks for transitions and acquires a durable journal claim. The
/// platform lock is always acquired before the aggregate row; it is released
/// only after the claim transaction commits or rolls back.
pub(crate) async fn claim(
    db: &DatabaseConnection,
    tenant: Uuid,
    slug: &str,
    definition: &ModuleDefinition,
    revision: u64,
    key: Uuid,
) -> Result<(), LifecycleAdmissionError> {
    if definition.slug != slug {
        return Err(LifecycleAdmissionError::StaleDefinition);
    }
    let tx = crate::backend::begin_fenced_transaction(db)
        .await
        .map_err(|error| LifecycleAdmissionError::Database(error.to_string()))?;
    let result = async {
        crate::backend::set_tenant_scope(&tx, tenant)
            .await
            .map_err(|error| LifecycleAdmissionError::Database(error.to_string()))?;
        crate::installation::acquire_artifact_activation_lock(
            &tx,
            &ModuleInstallationScope::Platform,
            slug,
        )
        .await
        .map_err(|error| LifecycleAdmissionError::Database(error.to_string()))?;
        let checkpoints = crate::TransitionCheckpointStore::list_active_checkpoints_for_slug_and_tenant_or_platform(
            &tx, slug, tenant,
        )
        .await
        .map_err(|error| LifecycleAdmissionError::Database(error.to_string()))?;
        if !checkpoints.is_empty() {
            return Err(LifecycleAdmissionError::TransitionInProgress);
        }
        StaticTenantLifecycleStore::claim(&tx, tenant, slug, revision, key).await?;
        // The catalog may have been constructed before a serving mutation.
        // Re-resolve the exact target inside this transaction after taking the
        // same slug lock and before committing the durable journal claim.
        if let ModuleDefinitionSource::Artifact { release } = &definition.source {
            ensure_artifact_selection_visibility(&tx).await?;
            let mut selected = crate::installation::resolve_active_many_in_transaction(
                &tx, std::slice::from_ref(release), tenant,
            ).await.map_err(|error| LifecycleAdmissionError::Database(error.to_string()))?;
            let selected = selected.pop().ok_or_else(|| LifecycleAdmissionError::Database(
                "artifact selection returned no result".to_string(),
            ))?.map_err(|error| match error {
                ArtifactInstallationResolveError::Unavailable
                | ArtifactInstallationResolveError::NotActiveSelection => LifecycleAdmissionError::StaleDefinition,
                error => LifecycleAdmissionError::Database(error.to_string()),
            })?;
            if ModuleDefinition::from_artifact_descriptor(&selected.descriptor) != *definition {
                return Err(LifecycleAdmissionError::StaleDefinition);
            }
        }
        Ok(())
    }
    .await;
    match result {
        Ok(()) => tx.commit().await.map_err(|error| {
            LifecycleAdmissionError::Database(format!("claim commit outcome is uncertain: {error}"))
        }),
        Err(error) => {
            tx.rollback().await.map_err(|rollback| {
                LifecycleAdmissionError::Database(format!(
                    "{error}; additionally, claim rollback failed: {rollback}"
                ))
            })?;
            Err(error)
        }
    }
}

/// A hidden tenant winner or uninstall row can make an older platform row
/// appear to be the effective selection. Until an audited cross-tenant owner
/// policy is part of this admission protocol, do not trust filtered views.
/// The visibility query references all four tables, acquiring transaction-held
/// AccessShareLocks before evaluating RLS so policy DDL cannot race admission.
async fn ensure_artifact_selection_visibility(
    tx: &DatabaseTransaction,
) -> Result<(), LifecycleAdmissionError> {
    let backend = SupportedBackend::new(tx.get_database_backend())
        .map_err(|error| LifecycleAdmissionError::Database(error.to_string()))?;
    if backend == SupportedBackend::Postgres {
        let row = tx.query_one_raw(Statement::from_string(backend.db_backend(),
            "SELECT (row_security_active('module_artifact_installations'::regclass) OR \
             row_security_active('module_artifact_admissions'::regclass) OR \
             row_security_active('module_artifact_tenant_lifecycle'::regclass) OR \
             row_security_active('module_artifact_uninstall_operations'::regclass)) AS enforced \
             FROM (SELECT count(*) FROM module_artifact_installations WHERE FALSE) i \
             CROSS JOIN (SELECT count(*) FROM module_artifact_admissions WHERE FALSE) a \
             CROSS JOIN (SELECT count(*) FROM module_artifact_tenant_lifecycle WHERE FALSE) l \
             CROSS JOIN (SELECT count(*) FROM module_artifact_uninstall_operations WHERE FALSE) u".to_string(),
        )).await.map_err(|error| LifecycleAdmissionError::Database(error.to_string()))?
            .ok_or_else(|| LifecycleAdmissionError::Database("could not verify artifact selection row security".to_string()))?;
        if row.try_get::<bool>("", "enforced")
            .map_err(|error| LifecycleAdmissionError::Database(error.to_string()))? {
            return Err(LifecycleAdmissionError::Database(
                "artifact lifecycle admission cannot prove complete selection visibility under active RLS".to_string(),
            ));
        }
    }
    Ok(())
}

/// Called under the platform slug lock. Even expired claims block staging until
/// owner reconciliation clears them; an expired lease is not proof that the
/// old hook's external work has stopped.
pub(crate) async fn has_journal_claim(
    tx: &DatabaseTransaction,
    scope: &ModuleInstallationScope,
    slug: &str,
) -> Result<bool, sea_orm::DbErr> {
    let backend = SupportedBackend::new(tx.get_database_backend())?;
    if backend == SupportedBackend::Postgres {
        // A tenant-filtered policy could silently hide another tenant's active
        // journal claim from a platform transition. Until an auditable
        // cross-tenant owner policy is part of the protocol, reject active RLS
        // rather than treating an incomplete query as proof of no claims. The
        // table reference also retains an AccessShareLock through the caller's
        // transaction, preventing policy DDL between this check and the read.
        let row = tx.query_one_raw(Statement::from_string(
            backend.db_backend(),
            "SELECT row_security_active('module_static_tenant_lifecycle'::regclass) AS enforced \
             FROM (SELECT count(*) FROM module_static_tenant_lifecycle WHERE FALSE) AS locked".to_string(),
        )).await?.ok_or_else(|| sea_orm::DbErr::Custom("could not verify lifecycle aggregate row security".to_string()))?;
        if row.try_get::<bool>("", "enforced")? {
            return Err(sea_orm::DbErr::Custom(
                "lifecycle aggregate RLS visibility cannot be proven for the transition/serving fence".to_string(),
            ));
        }
    }
    let mut values: Vec<sea_orm::Value> = vec![slug.into()];
    let tenant_filter = match (backend, scope) {
        (_, ModuleInstallationScope::Platform) => String::new(),
        (SupportedBackend::Postgres, ModuleInstallationScope::Tenant { tenant_id }) => {
            values.push((*tenant_id).into());
            " AND tenant_id = $2".to_string()
        }
        (SupportedBackend::Sqlite, ModuleInstallationScope::Tenant { tenant_id }) => {
            values.push(tenant_id.simple().to_string().into());
            " AND replace(replace(replace(replace(lower(CASE WHEN typeof(tenant_id) = 'blob' THEN hex(tenant_id) ELSE CAST(tenant_id AS TEXT) END), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') = ?2".to_string()
        }
    };
    let sql = format!("SELECT 1 AS present FROM module_static_tenant_lifecycle WHERE module_slug = {} AND active_idempotency_key IS NOT NULL AND claim_owner = 'journal'{tenant_filter} LIMIT 1", backend.placeholder(1));
    Ok(tx.query_one_raw(Statement::from_sql_and_values(backend.db_backend(), sql, values)).await?.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::TransactionTrait;
    use std::time::Duration;

    #[tokio::test]
    #[ignore = "requires RUSTOK_MODULES_TEST_DATABASE_URL and PostgreSQL schema creation privileges"]
    async fn postgres_staging_sees_lifecycle_claim_after_lock_wait() {
        let url = std::env::var("RUSTOK_MODULES_TEST_DATABASE_URL").unwrap();
        let db = sea_orm::Database::connect(url).await.unwrap();
        assert_eq!(db.get_database_backend(), sea_orm::DbBackend::Postgres);
        let schema = format!("lifecycle_fence_test_{}", Uuid::new_v4().simple());
        db.execute_unprepared(&format!("CREATE SCHEMA {schema}")).await.unwrap();
        let setup = db.begin().await.unwrap();
        setup.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
        setup.execute_unprepared(
            "CREATE TABLE module_artifact_activation_locks (scope_kind TEXT NOT NULL, scope_tenant_key TEXT NOT NULL, slug TEXT NOT NULL, PRIMARY KEY (scope_kind, scope_tenant_key, slug)); \
             CREATE TABLE module_static_tenant_lifecycle (tenant_id UUID NOT NULL, module_slug TEXT NOT NULL, revision BIGINT NOT NULL, active_idempotency_key UUID, active_claimed_at TIMESTAMPTZ, claim_owner TEXT NOT NULL DEFAULT 'journal', created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), PRIMARY KEY (tenant_id, module_slug))",
        ).await.unwrap();
        setup.commit().await.unwrap();
        let tenant = Uuid::new_v4();
        let claimer = crate::backend::begin_fenced_transaction(&db).await.unwrap();
        claimer.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
        crate::backend::set_tenant_scope(&claimer, tenant).await.unwrap();
        crate::installation::acquire_artifact_activation_lock(&claimer, &ModuleInstallationScope::Platform, "forum").await.unwrap();
        StaticTenantLifecycleStore::claim(&claimer, tenant, "forum", 0, Uuid::new_v4()).await.unwrap();
        let worker_db = db.clone();
        let worker_schema = schema.clone();
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let mut worker = tokio::spawn(async move {
            let tx = crate::backend::begin_fenced_transaction(&worker_db).await.unwrap();
            tx.execute_unprepared(&format!("SET LOCAL search_path TO {worker_schema}")).await.unwrap();
            crate::backend::set_platform_scope(&tx).await.unwrap();
            ready_tx.send(()).unwrap();
            crate::installation::acquire_artifact_activation_lock(&tx, &ModuleInstallationScope::Platform, "forum").await.unwrap();
            let visible = has_journal_claim(&tx, &ModuleInstallationScope::Platform, "forum").await.unwrap();
            tx.rollback().await.unwrap();
            visible
        });
        ready_rx.await.unwrap();
        assert!(tokio::time::timeout(Duration::from_millis(150), &mut worker).await.is_err(), "staging must wait for the claim transaction");
        claimer.commit().await.unwrap();
        assert!(worker.await.unwrap(), "staging must observe the committed claim after the lock wait");
        db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE")).await.unwrap();
    }
}
