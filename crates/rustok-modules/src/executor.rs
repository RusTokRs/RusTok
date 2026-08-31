use std::collections::HashSet;

use sea_orm::{DatabaseConnection, TransactionTrait};
use thiserror::Error;

use crate::recovery::{
    apply_tenant_override_enabled, read_tenant_override_enabled, record_operation_override_state,
};
use crate::{
    ControlPlaneInfrastructure, ModuleEffectivePolicyTransitionCoordinator,
    ModuleExecutionDispatcher, ModuleLifecycleHookPhase, ModuleOperationJournal,
    ModuleOperationRecordOutcome, ModuleOperationRequest, ModuleOperationSnapshot,
    ModuleOperationStatus, ModulePolicyRevisionTransition, ModuleToggleValidationError,
    StaticTenantLifecycleStore, StaticTenantLifecycleStoreError, TenantModuleStateRecord,
    TenantModuleStateStore, validate_module_toggle,
};

#[derive(Clone, Debug)]
pub struct ModuleLifecycleToggleRequest {
    pub tenant_id: uuid::Uuid,
    pub module_slug: String,
    /// Lifecycle hook direction for this command. Recovery may restore an
    /// inherited override while still running the inverse hook phase.
    pub enabled: bool,
    pub requested_by: Option<String>,
    pub trace_id: Option<String>,
    pub correlation_id: Option<String>,
    pub idempotency_key: Option<uuid::Uuid>,
    /// Static native lifecycle commands must echo the owner-issued aggregate
    /// revision. Dynamic artifact lifecycle uses a separate aggregate and
    /// leaves this field absent.
    pub expected_revision: Option<u64>,
    /// Selects the static native lifecycle aggregate rather than the separate
    /// admitted-artifact tenant lifecycle owner.
    pub static_lifecycle: bool,
    /// Canonical serving availability, including co-requisite constraints.
    pub effective_enabled_modules: HashSet<String>,
    /// Ordinary dependency-order selection used only to validate sequential
    /// lifecycle transitions. Co-requisites must not create a second ordering graph.
    pub ordering_enabled_modules: HashSet<String>,
    /// Exact tenant override before this command. `None` means inherit defaults.
    pub previous_override_enabled: Option<bool>,
    /// Exact tenant override to persist. `None` removes the explicit override.
    pub requested_override_enabled: Option<bool>,
    pub current_settings: serde_json::Value,
    pub policy_transition: Option<ModulePolicyRevisionTransition>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModuleLifecycleToggleResult {
    /// Exact module identity selected by the owner command. A host must not
    /// reread the original operation merely to reconstruct this response fact.
    pub module_slug: String,
    /// Current explicit tenant row after the state commit. `None` means the
    /// operation restored inherited/default selection by removing the row.
    pub state: Option<TenantModuleStateRecord>,
    pub override_enabled: Option<bool>,
    pub operation_id: Option<uuid::Uuid>,
    /// Current static lifecycle aggregate revision after the operation. It is
    /// absent only for non-static lifecycle dispatch.
    pub revision: Option<u64>,
    /// Exact settings supplied to lifecycle hooks and retained by the current
    /// explicit state. Host transports return this owner-issued fact instead
    /// of rereading the tenant-module persistence model.
    pub settings: serde_json::Value,
}

#[derive(Debug, Error)]
pub enum ModuleLifecycleExecutionError {
    #[error(transparent)]
    Validation(#[from] ModuleToggleValidationError),
    #[error("module lifecycle persistence failed: {0}")]
    Persistence(String),
    #[error("module pre-hook failed: {0}")]
    PreHook(String),
    #[error("module post-hook failed: {0}")]
    PostHook(String),
    #[error("module lifecycle idempotency key must not be nil")]
    InvalidIdempotencyKey,
    #[error("module lifecycle actor and tenant identities must not be nil")]
    InvalidCommandIdentity,
    #[error("module lifecycle idempotency key was reused for a different command")]
    IdempotencyConflict,
    #[error("static module lifecycle command requires an expected revision")]
    MissingExpectedRevision,
    #[error(
        "static module lifecycle revision conflict for `{module_slug}`: expected {expected}, current {current}"
    )]
    RevisionConflict {
        module_slug: String,
        expected: u64,
        current: u64,
    },
    #[error("static module lifecycle operation is already active for `{module_slug}")]
    OperationInProgress { module_slug: String },
    #[error("module effective-policy transition could not be published: {0}")]
    PolicyTransition(String),
}

pub async fn execute_module_toggle(
    infrastructure: &ControlPlaneInfrastructure,
    db: &DatabaseConnection,
    dispatcher: &ModuleExecutionDispatcher<'_>,
    policy_transition_coordinator: Option<ModuleEffectivePolicyTransitionCoordinator>,
    request: ModuleLifecycleToggleRequest,
) -> Result<ModuleLifecycleToggleResult, ModuleLifecycleExecutionError> {
    let settings = request.current_settings.clone();
    if request.idempotency_key == Some(uuid::Uuid::nil()) {
        return Err(ModuleLifecycleExecutionError::InvalidIdempotencyKey);
    }
    let previous_effective_enabled = request
        .effective_enabled_modules
        .contains(request.module_slug.as_str());
    let operation_request = ModuleOperationRequest {
        tenant_id: request.tenant_id,
        module_slug: request.module_slug.clone(),
        requested_enabled: request.enabled,
        previous_effective_enabled,
        requested_by: request.requested_by.clone(),
        trace_id: request.trace_id.clone(),
        correlation_id: request
            .correlation_id
            .clone()
            .or_else(|| request.idempotency_key.map(|key| key.to_string()))
            .unwrap_or_else(|| infrastructure.new_id().to_string()),
        idempotency_key: request.idempotency_key,
        expected_revision: request.expected_revision,
    };
    if operation_request.idempotency_key.is_some()
        && let Some(existing) =
            ModuleOperationJournal::replay_idempotent_command(db, &operation_request)
                .await
                .map_err(map_idempotency_store_error)?
    {
        return replay_lifecycle_operation(db, &operation_request, existing, settings).await;
    }
    validate_module_toggle(
        dispatcher.catalog(),
        &request.ordering_enabled_modules,
        &request.module_slug,
        request.enabled,
    )?;
    if request.policy_transition.is_some() && policy_transition_coordinator.is_none() {
        return Err(ModuleLifecycleExecutionError::PolicyTransition(
            "publisher is required for an effective-policy transition".to_string(),
        ));
    }
    let operation = if operation_request.idempotency_key.is_some() {
        match ModuleOperationJournal::record_idempotent(db, operation_request.clone())
            .await
            .map_err(map_idempotency_store_error)?
        {
            ModuleOperationRecordOutcome::Recorded(operation) => operation,
            ModuleOperationRecordOutcome::Replayed(operation) => {
                let existing = ModuleOperationJournal::find(db, operation.id)
                    .await
                    .map_err(|error| ModuleLifecycleExecutionError::Persistence(error.to_string()))?
                    .ok_or_else(|| {
                        ModuleLifecycleExecutionError::Persistence(
                            "idempotent lifecycle operation disappeared".to_string(),
                        )
                    })?;
                return replay_lifecycle_operation(db, &operation_request, existing, settings)
                    .await;
            }
        }
    } else {
        ModuleOperationJournal::record(db, operation_request.clone())
            .await
            .map_err(|error| ModuleLifecycleExecutionError::Persistence(error.to_string()))?
    };
    let static_lifecycle = if request.static_lifecycle {
        let expected_revision = request
            .expected_revision
            .ok_or(ModuleLifecycleExecutionError::MissingExpectedRevision)?;
        let idempotency_key = operation_request
            .idempotency_key
            .ok_or(ModuleLifecycleExecutionError::InvalidIdempotencyKey)?;
        if let Err(error) = StaticTenantLifecycleStore::claim(
            db,
            request.tenant_id,
            &request.module_slug,
            expected_revision,
            idempotency_key,
        )
        .await
        {
            let message = format!("aggregate-claim: {error}");
            ModuleOperationJournal::mark_failed(db, operation.id, &message)
                .await
                .map_err(|error| ModuleLifecycleExecutionError::Persistence(error.to_string()))?;
            return Err(map_static_lifecycle_store_error(error));
        }
        Some((expected_revision, idempotency_key))
    } else {
        None
    };
    if let Err(error) = record_override_state_or_fail(
        db,
        operation.id,
        request.previous_override_enabled,
        request.requested_override_enabled,
    )
    .await
    {
        if let Some((_, idempotency_key)) = static_lifecycle {
            let _ = StaticTenantLifecycleStore::release(
                db,
                request.tenant_id,
                &request.module_slug,
                idempotency_key,
            )
            .await;
        }
        return Err(error);
    }

    if previous_effective_enabled == request.enabled && request.policy_transition.is_none() {
        let tenant_id = request.tenant_id;
        let module_slug = request.module_slug.clone();
        let requested_override_enabled = request.requested_override_enabled;
        let state = match db
            .transaction::<_, Option<TenantModuleStateRecord>, ModuleLifecycleExecutionError>(
                |transaction| {
                    Box::pin(async move {
                        let state = apply_tenant_override_enabled(
                            transaction,
                            tenant_id,
                            &module_slug,
                            requested_override_enabled,
                        )
                        .await
                        .map_err(|error| {
                            ModuleLifecycleExecutionError::Persistence(error.to_string())
                        })?;
                        ModuleOperationJournal::mark_committed(transaction, operation.id)
                            .await
                            .map_err(|error| {
                                ModuleLifecycleExecutionError::Persistence(error.to_string())
                            })?;
                        if let Some((expected_revision, idempotency_key)) = static_lifecycle {
                            StaticTenantLifecycleStore::advance(
                                transaction,
                                tenant_id,
                                &module_slug,
                                expected_revision,
                                idempotency_key,
                            )
                            .await
                            .map_err(map_static_lifecycle_store_error)?;
                            StaticTenantLifecycleStore::release(
                                transaction,
                                tenant_id,
                                &module_slug,
                                idempotency_key,
                            )
                            .await
                            .map_err(map_static_lifecycle_store_error)?;
                        }
                        Ok(state)
                    })
                },
            )
            .await
        {
            Ok(state) => state,
            Err(error) => {
                let error = transaction_error(error);
                let journal_message = format!("state-commit: {error}");
                ModuleOperationJournal::mark_failed(db, operation.id, &journal_message)
                    .await
                    .map_err(|error| {
                        ModuleLifecycleExecutionError::Persistence(error.to_string())
                    })?;
                if let Some((_, idempotency_key)) = static_lifecycle {
                    let _ = StaticTenantLifecycleStore::release(
                        db,
                        request.tenant_id,
                        &request.module_slug,
                        idempotency_key,
                    )
                    .await;
                }
                return Err(error);
            }
        };
        return Ok(ModuleLifecycleToggleResult {
            module_slug: request.module_slug.clone(),
            state,
            override_enabled: request.requested_override_enabled,
            operation_id: Some(operation.id),
            revision: static_lifecycle
                .map(|(expected_revision, _)| {
                    expected_revision.checked_add(1).ok_or_else(|| {
                        ModuleLifecycleExecutionError::Persistence(
                            "static module lifecycle revision overflow".to_string(),
                        )
                    })
                })
                .transpose()?,
            settings,
        });
    }

    ModuleOperationJournal::mark_running(db, operation.id)
        .await
        .map_err(|error| ModuleLifecycleExecutionError::Persistence(error.to_string()))?;

    let pre_phase = if request.enabled {
        ModuleLifecycleHookPhase::PreEnable
    } else {
        ModuleLifecycleHookPhase::PreDisable
    };
    if let Err(error) = dispatcher
        .dispatch_lifecycle(
            db,
            request.tenant_id,
            &request.module_slug,
            &request.current_settings,
            pre_phase,
        )
        .await
    {
        let message = error.to_string();
        ModuleOperationJournal::mark_failed(db, operation.id, &message)
            .await
            .map_err(|error| ModuleLifecycleExecutionError::Persistence(error.to_string()))?;
        if let Some((_, idempotency_key)) = static_lifecycle {
            StaticTenantLifecycleStore::release(
                db,
                request.tenant_id,
                &request.module_slug,
                idempotency_key,
            )
            .await
            .map_err(map_static_lifecycle_store_error)?;
        }
        return Err(ModuleLifecycleExecutionError::PreHook(message));
    }

    let requested_override_enabled = request.requested_override_enabled;
    let module_slug = request.module_slug.clone();
    let policy_transition = request.policy_transition.clone();
    let tenant_id = request.tenant_id;
    let coordinator = policy_transition_coordinator;
    let state = match db
        .transaction::<_, Option<TenantModuleStateRecord>, ModuleLifecycleExecutionError>(
            |transaction| {
                Box::pin(async move {
                    let state = apply_tenant_override_enabled(
                        transaction,
                        tenant_id,
                        &module_slug,
                        requested_override_enabled,
                    )
                    .await
                    .map_err(|error| {
                        ModuleLifecycleExecutionError::Persistence(error.to_string())
                    })?;
                    ModuleOperationJournal::mark_committed(transaction, operation.id)
                        .await
                        .map_err(|error| {
                            ModuleLifecycleExecutionError::Persistence(error.to_string())
                        })?;
                    if let (Some(coordinator), Some(transition)) = (coordinator, policy_transition)
                    {
                        coordinator
                            .publish_and_advance(
                                transaction,
                                tenant_id,
                                None,
                                "module.lifecycle",
                                &transition,
                            )
                            .await
                            .map_err(|error| {
                                ModuleLifecycleExecutionError::PolicyTransition(error.to_string())
                            })?;
                    }
                    if let Some((expected_revision, idempotency_key)) = static_lifecycle {
                        StaticTenantLifecycleStore::advance(
                            transaction,
                            tenant_id,
                            &module_slug,
                            expected_revision,
                            idempotency_key,
                        )
                        .await
                        .map_err(map_static_lifecycle_store_error)?;
                    }
                    Ok(state)
                })
            },
        )
        .await
    {
        Ok(state) => state,
        Err(error) => {
            let error = transaction_error(error);
            let journal_message = format!("state-commit: {error}");
            ModuleOperationJournal::mark_failed(db, operation.id, &journal_message)
                .await
                .map_err(|error| ModuleLifecycleExecutionError::Persistence(error.to_string()))?;
            if let Some((_, idempotency_key)) = static_lifecycle {
                let _ = StaticTenantLifecycleStore::release(
                    db,
                    request.tenant_id,
                    &request.module_slug,
                    idempotency_key,
                )
                .await;
            }
            return Err(error);
        }
    };

    let post_phase = if request.enabled {
        ModuleLifecycleHookPhase::PostEnable
    } else {
        ModuleLifecycleHookPhase::PostDisable
    };
    if let Err(error) = dispatcher
        .dispatch_lifecycle(
            db,
            request.tenant_id,
            &request.module_slug,
            &request.current_settings,
            post_phase,
        )
        .await
    {
        let message = error.to_string();
        let journal_message = format!("post-hook: {message}");
        ModuleOperationJournal::mark_failed(db, operation.id, &journal_message)
            .await
            .map_err(|error| ModuleLifecycleExecutionError::Persistence(error.to_string()))?;
        if let Some((_, idempotency_key)) = static_lifecycle {
            StaticTenantLifecycleStore::release(
                db,
                request.tenant_id,
                &request.module_slug,
                idempotency_key,
            )
            .await
            .map_err(map_static_lifecycle_store_error)?;
        }
        return Err(ModuleLifecycleExecutionError::PostHook(message));
    }

    if let Some((_, idempotency_key)) = static_lifecycle {
        StaticTenantLifecycleStore::release(
            db,
            request.tenant_id,
            &request.module_slug,
            idempotency_key,
        )
        .await
        .map_err(map_static_lifecycle_store_error)?;
    }

    Ok(ModuleLifecycleToggleResult {
        module_slug: request.module_slug.clone(),
        state,
        override_enabled: request.requested_override_enabled,
        operation_id: Some(operation.id),
        revision: static_lifecycle
            .map(|(expected_revision, _)| {
                expected_revision.checked_add(1).ok_or_else(|| {
                    ModuleLifecycleExecutionError::Persistence(
                        "static module lifecycle revision overflow".to_string(),
                    )
                })
            })
            .transpose()?,
        settings,
    })
}

async fn record_override_state_or_fail(
    db: &DatabaseConnection,
    operation_id: uuid::Uuid,
    previous_override_enabled: Option<bool>,
    requested_override_enabled: Option<bool>,
) -> Result<(), ModuleLifecycleExecutionError> {
    if let Err(error) = record_operation_override_state(
        db,
        operation_id,
        previous_override_enabled,
        requested_override_enabled,
    )
    .await
    {
        let message = format!("recovery-state: {error}");
        let _ = ModuleOperationJournal::mark_failed(db, operation_id, &message).await;
        return Err(ModuleLifecycleExecutionError::Persistence(
            error.to_string(),
        ));
    }
    Ok(())
}

fn map_idempotency_store_error(
    error: crate::ModuleOperationStoreError,
) -> ModuleLifecycleExecutionError {
    match error {
        crate::ModuleOperationStoreError::IdempotencyConflict => {
            ModuleLifecycleExecutionError::IdempotencyConflict
        }
        error => ModuleLifecycleExecutionError::Persistence(error.to_string()),
    }
}

fn map_static_lifecycle_store_error(
    error: StaticTenantLifecycleStoreError,
) -> ModuleLifecycleExecutionError {
    match error {
        StaticTenantLifecycleStoreError::RevisionConflict {
            module_slug,
            expected,
            current,
        } => ModuleLifecycleExecutionError::RevisionConflict {
            module_slug,
            expected,
            current,
        },
        StaticTenantLifecycleStoreError::OperationInProgress { module_slug } => {
            ModuleLifecycleExecutionError::OperationInProgress { module_slug }
        }
        StaticTenantLifecycleStoreError::RevisionOverflow(module_slug) => {
            ModuleLifecycleExecutionError::Persistence(format!(
                "static module lifecycle revision overflow for `{module_slug}`"
            ))
        }
        StaticTenantLifecycleStoreError::Database(error)
        | StaticTenantLifecycleStoreError::InconsistentState(error) => {
            ModuleLifecycleExecutionError::Persistence(error)
        }
    }
}

fn transaction_error(
    error: sea_orm::TransactionError<ModuleLifecycleExecutionError>,
) -> ModuleLifecycleExecutionError {
    match error {
        sea_orm::TransactionError::Connection(error) => {
            ModuleLifecycleExecutionError::Persistence(error.to_string())
        }
        sea_orm::TransactionError::Transaction(error) => error,
    }
}

async fn replay_lifecycle_operation(
    db: &DatabaseConnection,
    request: &ModuleOperationRequest,
    operation: ModuleOperationSnapshot,
    settings: serde_json::Value,
) -> Result<ModuleLifecycleToggleResult, ModuleLifecycleExecutionError> {
    match operation.status {
        ModuleOperationStatus::Committed => {
            let override_enabled =
                read_tenant_override_enabled(db, request.tenant_id, &request.module_slug)
                    .await
                    .map_err(|error| {
                        ModuleLifecycleExecutionError::Persistence(error.to_string())
                    })?;
            let state = if override_enabled.is_some() {
                TenantModuleStateStore::read(db, request.tenant_id, &request.module_slug)
                    .await
                    .map_err(|error| {
                        ModuleLifecycleExecutionError::Persistence(error.to_string())
                    })?
            } else {
                None
            };
            Ok(ModuleLifecycleToggleResult {
                module_slug: request.module_slug.clone(),
                state,
                override_enabled,
                operation_id: Some(operation.id),
                revision: if request.expected_revision.is_some() {
                    Some(
                        StaticTenantLifecycleStore::snapshot(
                            db,
                            request.tenant_id,
                            &request.module_slug,
                        )
                        .await
                        .map_err(map_static_lifecycle_store_error)?
                        .revision,
                    )
                } else {
                    None
                },
                settings,
            })
        }
        ModuleOperationStatus::Failed => {
            let message = operation
                .error_message
                .unwrap_or_else(|| "unknown failure".to_string());
            if let Some(message) = message.strip_prefix("post-hook: ") {
                Err(ModuleLifecycleExecutionError::PostHook(message.to_string()))
            } else if let Some(message) = message.strip_prefix("state-commit: ") {
                Err(ModuleLifecycleExecutionError::Persistence(
                    message.to_string(),
                ))
            } else {
                Err(ModuleLifecycleExecutionError::PreHook(message))
            }
        }
        ModuleOperationStatus::Validated | ModuleOperationStatus::Running => {
            Err(ModuleLifecycleExecutionError::Persistence(
                "idempotent lifecycle operation is still in progress".to_string(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use rustok_core::{MigrationSource, ModuleRegistry, RusToKModule};
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
    use sea_orm_migration::MigrationTrait;

    use super::*;
    use crate::ModuleDefinitionCatalog;

    struct OptionalModule;

    impl MigrationSource for OptionalModule {
        fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
            Vec::new()
        }
    }

    #[async_trait]
    impl RusToKModule for OptionalModule {
        fn slug(&self) -> &'static str {
            "optional-test"
        }

        fn name(&self) -> &'static str {
            "Optional Test"
        }

        fn description(&self) -> &'static str {
            "Optional lifecycle test module"
        }

        fn version(&self) -> &'static str {
            "0.1.0"
        }
    }

    async fn journal_only_database() -> DatabaseConnection {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        database
            .execute(Statement::from_string(
                DbBackend::Sqlite,
                "CREATE TABLE module_operations (\
                    id TEXT PRIMARY KEY NOT NULL, \
                    tenant_id TEXT NOT NULL, \
                    module_slug TEXT NOT NULL, \
                    requested_enabled BOOLEAN NOT NULL, \
                    previous_effective_enabled BOOLEAN NOT NULL, \
                    status TEXT NOT NULL, \
                    requested_by TEXT, \
                    trace_id TEXT, \
                    correlation_id TEXT, \
                    idempotency_key TEXT, \
                    expected_revision INTEGER, \
                    error_message TEXT, \
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, \
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP\
                 )"
                .to_string(),
            ))
            .await
            .expect("module operations table");
        database
            .execute(Statement::from_string(
                DbBackend::Sqlite,
                "CREATE TABLE module_operation_override_states (\
                    operation_id TEXT PRIMARY KEY NOT NULL, \
                    previous_override_enabled BOOLEAN, \
                    requested_override_enabled BOOLEAN\
                 )"
                .to_string(),
            ))
            .await
            .expect("module operation override states table");
        database
    }

    async fn journal_and_state_database() -> DatabaseConnection {
        let database = journal_only_database().await;
        database
            .execute(Statement::from_string(
                DbBackend::Sqlite,
                "CREATE TABLE tenant_modules (\
                    id TEXT PRIMARY KEY NOT NULL, \
                    tenant_id TEXT NOT NULL, \
                    module_slug TEXT NOT NULL, \
                    enabled BOOLEAN NOT NULL, \
                    settings TEXT NOT NULL DEFAULT '{}', \
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, \
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, \
                    UNIQUE (tenant_id, module_slug)\
                 )"
                .to_string(),
            ))
            .await
            .expect("tenant module state table");
        database
            .execute(Statement::from_string(
                DbBackend::Sqlite,
                "CREATE TABLE module_static_tenant_lifecycle (\
                    tenant_id TEXT NOT NULL, \
                    module_slug TEXT NOT NULL, \
                    revision INTEGER NOT NULL, \
                    active_idempotency_key TEXT, \
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, \
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, \
                    PRIMARY KEY (tenant_id, module_slug)\
                 )"
                .to_string(),
            ))
            .await
            .expect("static lifecycle aggregate table");
        database
    }

    #[tokio::test]
    async fn idempotent_no_op_toggle_replays_one_journaled_operation() {
        let database = journal_and_state_database().await;
        let registry = ModuleRegistry::new().register(OptionalModule);
        let catalog = ModuleDefinitionCatalog::from_static_registry(&registry).expect("catalog");
        let dispatcher = ModuleExecutionDispatcher::new(&catalog, &registry);
        let tenant_id = uuid::Uuid::new_v4();
        let idempotency_key = uuid::Uuid::new_v4();
        let request = ModuleLifecycleToggleRequest {
            tenant_id,
            module_slug: "optional-test".to_string(),
            enabled: true,
            requested_by: Some("operator".to_string()),
            trace_id: Some("test:module-lifecycle".to_string()),
            correlation_id: None,
            idempotency_key: Some(idempotency_key),
            expected_revision: Some(0),
            static_lifecycle: true,
            effective_enabled_modules: HashSet::from(["optional-test".to_string()]),
            ordering_enabled_modules: HashSet::new(),
            previous_override_enabled: None,
            requested_override_enabled: Some(true),
            current_settings: serde_json::json!({}),
            policy_transition: None,
        };

        let first = execute_module_toggle(
            &ControlPlaneInfrastructure::default(),
            &database,
            &dispatcher,
            None,
            request.clone(),
        )
        .await
        .expect("first no-op command");
        let replay = execute_module_toggle(
            &ControlPlaneInfrastructure::default(),
            &database,
            &dispatcher,
            None,
            request,
        )
        .await
        .expect("exact retry replays");

        assert_eq!(first.operation_id, replay.operation_id);
        let row = database
            .query_one(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT COUNT(*) AS count FROM module_operations WHERE tenant_id = ?1 AND idempotency_key = ?2",
                vec![tenant_id.into(), idempotency_key.into()],
            ))
            .await
            .expect("journal count query")
            .expect("journal count row");
        let count: i64 = row.try_get("", "count").expect("journal count");
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn state_commit_failure_marks_running_operation_as_failed() {
        let database = journal_only_database().await;
        let registry = ModuleRegistry::new().register(OptionalModule);
        let catalog = ModuleDefinitionCatalog::from_static_registry(&registry).expect("catalog");
        let dispatcher = ModuleExecutionDispatcher::new(&catalog, &registry);

        let infrastructure = ControlPlaneInfrastructure::default();
        let result = execute_module_toggle(
            &infrastructure,
            &database,
            &dispatcher,
            None,
            ModuleLifecycleToggleRequest {
                tenant_id: uuid::Uuid::new_v4(),
                module_slug: "optional-test".to_string(),
                enabled: true,
                requested_by: Some("test".to_string()),
                trace_id: None,
                correlation_id: None,
                idempotency_key: None,
                expected_revision: None,
                static_lifecycle: false,
                effective_enabled_modules: HashSet::new(),
                ordering_enabled_modules: HashSet::new(),
                previous_override_enabled: None,
                requested_override_enabled: Some(true),
                current_settings: serde_json::json!({}),
                policy_transition: None,
            },
        )
        .await;

        assert!(matches!(
            result,
            Err(ModuleLifecycleExecutionError::Persistence(_))
        ));

        let row = database
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT status, error_message FROM module_operations LIMIT 1".to_string(),
            ))
            .await
            .expect("journal query")
            .expect("journal row");
        let status: String = row.try_get("", "status").expect("status");
        let error_message: String = row.try_get("", "error_message").expect("error message");

        assert_eq!(status, "failed");
        assert!(error_message.contains("state-commit:"));
    }
}
