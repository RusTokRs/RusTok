use std::collections::HashSet;

use sea_orm::{DatabaseConnection, TransactionTrait};
use thiserror::Error;

use crate::{
    validate_module_toggle, ControlPlaneInfrastructure, ModuleEffectivePolicyTransitionCoordinator,
    ModuleExecutionDispatcher, ModuleLifecycleHookPhase, ModuleOperationJournal,
    ModuleOperationRecordOutcome, ModuleOperationRequest, ModuleOperationSnapshot,
    ModuleOperationStatus, ModulePolicyRevisionTransition, ModuleToggleValidationError,
    TenantModuleStateRecord, TenantModuleStateRequest, TenantModuleStateStore,
};

#[derive(Clone, Debug)]
pub struct ModuleLifecycleToggleRequest {
    pub tenant_id: uuid::Uuid,
    pub module_slug: String,
    pub enabled: bool,
    pub requested_by: Option<String>,
    pub correlation_id: Option<String>,
    pub idempotency_key: Option<uuid::Uuid>,
    pub effective_enabled_modules: HashSet<String>,
    pub current_settings: serde_json::Value,
    pub policy_transition: Option<ModulePolicyRevisionTransition>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleLifecycleToggleResult {
    pub state: TenantModuleStateRecord,
    pub operation_id: Option<uuid::Uuid>,
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
    #[error("module lifecycle idempotency key was reused for a different command")]
    IdempotencyConflict,
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
        correlation_id: request
            .correlation_id
            .unwrap_or_else(|| infrastructure.new_id().to_string()),
        idempotency_key: request.idempotency_key,
    };
    if operation_request.idempotency_key.is_some() {
        if let Some(existing) =
            ModuleOperationJournal::replay_idempotent_command(db, &operation_request)
                .await
                .map_err(map_idempotency_store_error)?
        {
            return replay_lifecycle_operation(db, &operation_request, existing).await;
        }
    }
    validate_module_toggle(
        dispatcher.catalog(),
        &request.effective_enabled_modules,
        &request.module_slug,
        request.enabled,
    )?;
    if previous_effective_enabled == request.enabled && request.policy_transition.is_none() {
        let state = TenantModuleStateStore::persist(
            db,
            TenantModuleStateRequest {
                tenant_id: request.tenant_id,
                module_slug: request.module_slug,
                enabled: request.enabled,
            },
        )
        .await
        .map_err(|error| ModuleLifecycleExecutionError::Persistence(error.to_string()))?;
        return Ok(ModuleLifecycleToggleResult {
            state,
            operation_id: None,
        });
    }

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
                return replay_lifecycle_operation(db, &operation_request, existing).await;
            }
        }
    } else {
        ModuleOperationJournal::record(db, operation_request)
            .await
            .map_err(|error| ModuleLifecycleExecutionError::Persistence(error.to_string()))?
    };
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
        return Err(ModuleLifecycleExecutionError::PreHook(message));
    }

    let state_request = TenantModuleStateRequest {
        tenant_id: request.tenant_id,
        module_slug: request.module_slug.clone(),
        enabled: request.enabled,
    };
    let policy_transition = request.policy_transition.clone();
    let tenant_id = request.tenant_id;
    let coordinator = policy_transition_coordinator;
    let state = match db
        .transaction::<_, TenantModuleStateRecord, ModuleLifecycleExecutionError>(|transaction| {
            Box::pin(async move {
                let state = TenantModuleStateStore::persist(transaction, state_request)
                    .await
                    .map_err(|error| {
                        ModuleLifecycleExecutionError::Persistence(error.to_string())
                    })?;
                ModuleOperationJournal::mark_committed(transaction, operation.id)
                    .await
                    .map_err(|error| {
                        ModuleLifecycleExecutionError::Persistence(error.to_string())
                    })?;
                if let (Some(coordinator), Some(transition)) = (coordinator, policy_transition) {
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
                Ok(state)
            })
        })
        .await
    {
        Ok(state) => state,
        Err(error) => {
            let error = match error {
                sea_orm::TransactionError::Connection(error) => {
                    ModuleLifecycleExecutionError::Persistence(error.to_string())
                }
                sea_orm::TransactionError::Transaction(error) => error,
            };
            let journal_message = format!("state-commit: {error}");
            ModuleOperationJournal::mark_failed(db, operation.id, &journal_message)
                .await
                .map_err(|error| ModuleLifecycleExecutionError::Persistence(error.to_string()))?;
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
        return Err(ModuleLifecycleExecutionError::PostHook(message));
    }

    Ok(ModuleLifecycleToggleResult {
        state,
        operation_id: Some(operation.id),
    })
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

async fn replay_lifecycle_operation(
    db: &DatabaseConnection,
    request: &ModuleOperationRequest,
    operation: ModuleOperationSnapshot,
) -> Result<ModuleLifecycleToggleResult, ModuleLifecycleExecutionError> {
    match operation.status {
        ModuleOperationStatus::Committed => {
            let state = TenantModuleStateStore::read(db, request.tenant_id, &request.module_slug)
                .await
                .map_err(|error| ModuleLifecycleExecutionError::Persistence(error.to_string()))?
                .ok_or_else(|| {
                    ModuleLifecycleExecutionError::Persistence(
                        "committed lifecycle operation has no tenant state".to_string(),
                    )
                })?;
            Ok(ModuleLifecycleToggleResult {
                state,
                operation_id: Some(operation.id),
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
                    correlation_id TEXT, \
                    idempotency_key TEXT, \
                    error_message TEXT, \
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, \
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP\
                 )"
                .to_string(),
            ))
            .await
            .expect("module operations table");
        database
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
                correlation_id: None,
                idempotency_key: None,
                effective_enabled_modules: HashSet::new(),
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
