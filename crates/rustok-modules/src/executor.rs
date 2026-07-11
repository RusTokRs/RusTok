use std::collections::HashSet;

use sea_orm::{DatabaseConnection, TransactionTrait};
use thiserror::Error;

use rustok_core::ModuleRegistry;

use crate::{
    ModuleLifecycleHookPhase, ModuleOperationJournal, ModuleOperationRequest,
    ModuleToggleValidationError, TenantModuleStateRecord, TenantModuleStateRequest,
    TenantModuleStateStore, run_module_lifecycle_hook, validate_module_toggle,
};

#[derive(Clone, Debug)]
pub struct ModuleLifecycleToggleRequest {
    pub tenant_id: uuid::Uuid,
    pub module_slug: String,
    pub enabled: bool,
    pub requested_by: Option<String>,
    pub effective_enabled_modules: HashSet<String>,
    pub current_settings: serde_json::Value,
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
}

pub async fn execute_module_toggle(
    db: &DatabaseConnection,
    registry: &ModuleRegistry,
    request: ModuleLifecycleToggleRequest,
) -> Result<ModuleLifecycleToggleResult, ModuleLifecycleExecutionError> {
    validate_module_toggle(
        registry,
        &request.effective_enabled_modules,
        &request.module_slug,
        request.enabled,
    )?;
    let previous_effective_enabled = request
        .effective_enabled_modules
        .contains(request.module_slug.as_str());

    if previous_effective_enabled == request.enabled {
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

    let operation = ModuleOperationJournal::record(
        db,
        ModuleOperationRequest {
            tenant_id: request.tenant_id,
            module_slug: request.module_slug.clone(),
            requested_enabled: request.enabled,
            previous_effective_enabled,
            requested_by: request.requested_by,
            correlation_id: uuid::Uuid::new_v4().to_string(),
        },
    )
    .await
    .map_err(|error| ModuleLifecycleExecutionError::Persistence(error.to_string()))?;
    ModuleOperationJournal::mark_running(db, operation.id)
        .await
        .map_err(|error| ModuleLifecycleExecutionError::Persistence(error.to_string()))?;

    let pre_phase = if request.enabled {
        ModuleLifecycleHookPhase::PreEnable
    } else {
        ModuleLifecycleHookPhase::PreDisable
    };
    if let Err(error) = run_module_lifecycle_hook(
        registry,
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
    let state = db
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
                Ok(state)
            })
        })
        .await
        .map_err(|error| match error {
            sea_orm::TransactionError::Connection(error) => {
                ModuleLifecycleExecutionError::Persistence(error.to_string())
            }
            sea_orm::TransactionError::Transaction(error) => error,
        })?;

    let post_phase = if request.enabled {
        ModuleLifecycleHookPhase::PostEnable
    } else {
        ModuleLifecycleHookPhase::PostDisable
    };
    if let Err(error) = run_module_lifecycle_hook(
        registry,
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
