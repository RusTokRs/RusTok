use std::collections::HashSet;
use std::time::Duration;

use sea_orm::{DatabaseConnection, TransactionTrait};
use thiserror::Error;

use crate::recovery::{
    apply_tenant_override_enabled, load_current_lifecycle_settings,
    read_tenant_override_enabled, record_operation_override_state,
};

/// Claim leases last 300 seconds; renewing every minute leaves several
/// missed-heartbeat opportunities before another command may reclaim the
/// aggregate.
const LIFECYCLE_CLAIM_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(60);
use crate::{
    ModuleCommandContext, ModuleControlPlaneError, ModuleEffectivePolicyTransitionCoordinator,
    ModuleExecutionDispatcher,
    ModuleLifecycleHookPhase, ModuleOperationJournal, ModuleOperationRecordOutcome,
    ModuleOperationRequest, ModuleOperationSnapshot, ModuleOperationStatus,
    ModulePolicyRevisionTransition, ModuleToggleValidationError, StaticTenantLifecycleStore,
    StaticTenantLifecycleStoreError, TenantModuleStateRecord, TenantModuleStateStore,
    validate_module_toggle,
};

#[derive(Clone, Debug)]
pub struct ModuleLifecycleToggleRequest {
    pub tenant_id: uuid::Uuid,
    pub module_slug: String,
    /// Lifecycle hook direction for this command. Recovery may restore an
    /// inherited override while still running the inverse hook phase.
    pub enabled: bool,
    /// Authenticated request evidence retained by the lifecycle journal and
    /// every derived outbox event.
    pub context: ModuleCommandContext,
    /// Every lifecycle command must echo the owner-issued tenant/module
    /// aggregate revision. Static/native and admitted-artifact definitions
    /// share this durable claim/CAS pipeline.
    pub expected_revision: u64,
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

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ModuleLifecycleToggleResult {
    /// Exact module identity selected by the owner command. A host must not
    /// reread the original operation merely to reconstruct this response fact.
    pub module_slug: String,
    /// Current explicit tenant row after the state commit. `None` means the
    /// operation restored inherited/default selection by removing the row.
    pub state: Option<TenantModuleStateRecord>,
    pub override_enabled: Option<bool>,
    pub operation_id: Option<uuid::Uuid>,
    /// Current tenant/module lifecycle aggregate revision after the
    /// operation.
    pub revision: u64,
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
    #[error(transparent)]
    InvalidCommandContext(#[from] ModuleControlPlaneError),
    #[error("module lifecycle actor and tenant identities must not be nil")]
    InvalidCommandIdentity,
    #[error("module lifecycle idempotency key was reused for a different command")]
    IdempotencyConflict,
    #[error(
        "module lifecycle revision conflict for `{module_slug}`: expected {expected}, current {current}"
    )]
    RevisionConflict {
        module_slug: String,
        expected: u64,
        current: u64,
    },
    #[error("module lifecycle operation is already active for `{module_slug}`")]
    OperationInProgress { module_slug: String },
    #[error("module lifecycle operation `{operation_id}` for `{module_slug}` was orphaned")]
    OperationOrphaned {
        module_slug: String,
        operation_id: uuid::Uuid,
    },
    #[error("module lifecycle claim was lost for `{module_slug}`")]
    ClaimLost { module_slug: String },
    #[error("module effective-policy transition could not be published: {0}")]
    PolicyTransition(String),
}

pub async fn execute_module_toggle(
    db: &DatabaseConnection,
    dispatcher: &ModuleExecutionDispatcher<'_>,
    policy_transition_coordinator: Option<ModuleEffectivePolicyTransitionCoordinator>,
    request: ModuleLifecycleToggleRequest,
) -> Result<ModuleLifecycleToggleResult, ModuleLifecycleExecutionError> {
    let mut settings = request.current_settings.clone();
    if request.tenant_id.is_nil()
        || request.context.tenant_id != Some(request.tenant_id)
        || request.context.validate().is_err()
    {
        return Err(ModuleLifecycleExecutionError::InvalidCommandIdentity);
    }
    let previous_effective_enabled = request
        .effective_enabled_modules
        .contains(request.module_slug.as_str());
    let operation_request = ModuleOperationRequest {
        tenant_id: request.tenant_id,
        module_slug: request.module_slug.clone(),
        requested_enabled: request.enabled,
        previous_effective_enabled,
        requested_by: Some(request.context.actor_id.to_string()),
        trace_id: Some(request.context.trace_id.clone()),
        correlation_id: request.context.correlation_id.to_string(),
        idempotency_key: Some(request.context.idempotency_key),
        expected_revision: Some(request.expected_revision),
    };
    if let Some(existing) =
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
    let operation = match ModuleOperationJournal::record_idempotent(db, operation_request.clone())
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
            return replay_lifecycle_operation(db, &operation_request, existing, settings).await;
        }
    };
    let expected_revision = request.expected_revision;
    let idempotency_key = request.context.idempotency_key;
    if let Err(error) = StaticTenantLifecycleStore::claim(
        db,
        request.tenant_id,
        &request.module_slug,
        expected_revision,
        idempotency_key,
    )
    .await
    {
        let contention = matches!(
            error,
            StaticTenantLifecycleStoreError::RevisionConflict { .. }
                | StaticTenantLifecycleStoreError::OperationInProgress { .. }
        );
        let mapped = map_static_lifecycle_store_error(error);
        if contention {
            // The journal row was inserted first so its durable command
            // fingerprint fences same-key reuse, but this command never
            // acquired the aggregate and performed no lifecycle work.
            // Discarding only the still-Validated row leaves the caller's
            // key reusable after it observes the current revision/claim.
            return Err(
                match ModuleOperationJournal::discard_validated(db, operation.id).await {
                    Ok(true) => mapped,
                    Ok(false) => ModuleLifecycleExecutionError::Persistence(format!(
                        "{mapped}; additionally, the unclaimed lifecycle operation \
                         was no longer in the validated state"
                    )),
                    Err(discard_error) => ModuleLifecycleExecutionError::Persistence(format!(
                        "{mapped}; additionally, discarding the unclaimed lifecycle \
                         operation failed: {discard_error}"
                    )),
                },
            );
        }
        let message = format!("aggregate-claim: {mapped}");
        ModuleOperationJournal::mark_failed(db, operation.id, &message)
            .await
            .map_err(|error| ModuleLifecycleExecutionError::Persistence(error.to_string()))?;
        return Err(mapped);
    }
    let lifecycle_claim = (expected_revision, idempotency_key);
    if let Err(error) = record_override_state_or_fail(
        db,
        operation.id,
        request.previous_override_enabled,
        request.requested_override_enabled,
    )
    .await
    {
        return Err(release_claim_after_error(
            db,
            request.tenant_id,
            &request.module_slug,
            idempotency_key,
            error,
        )
        .await);
    }
    settings = match load_current_lifecycle_settings(
        db,
        dispatcher,
        request.tenant_id,
        &request.module_slug,
    )
    .await
    {
        Ok(current_settings) => current_settings,
        Err(error) => {
            let primary = ModuleLifecycleExecutionError::Persistence(error.to_string());
            let primary =
                match ModuleOperationJournal::mark_failed(db, operation.id, &primary.to_string())
                    .await
                {
                    Ok(()) => primary,
                    Err(journal_error) => ModuleLifecycleExecutionError::Persistence(format!(
                        "{primary}; additionally, marking the lifecycle operation \
                         failed did not persist: {journal_error}"
                    )),
                };
            return Err(release_claim_after_error(
                db,
                request.tenant_id,
                &request.module_slug,
                idempotency_key,
                primary,
            )
            .await);
        }
    };

    if previous_effective_enabled == request.enabled && request.policy_transition.is_none() {
        let tenant_id = request.tenant_id;
        let module_slug = request.module_slug.clone();
        let requested_override_enabled = request.requested_override_enabled;
        let result_settings = settings.clone();
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
                        let committed_revision =
                            expected_revision.checked_add(1).ok_or_else(|| {
                                ModuleLifecycleExecutionError::Persistence(
                                    "module lifecycle revision overflow".to_string(),
                                )
                            })?;
                        let result = ModuleLifecycleToggleResult {
                            module_slug: module_slug.clone(),
                            state: state.clone(),
                            override_enabled: requested_override_enabled,
                            operation_id: Some(operation.id),
                            revision: committed_revision,
                            settings: result_settings,
                        };
                        let result_snapshot =
                            serde_json::to_string(&result).map_err(|error| {
                                ModuleLifecycleExecutionError::Persistence(format!(
                                    "serializing the lifecycle operation result \
                                     snapshot failed: {error}"
                                ))
                            })?;
                        ModuleOperationJournal::mark_committed_with_result(
                            transaction,
                            operation.id,
                            &result_snapshot,
                        )
                        .await
                        .map_err(|error| {
                            ModuleLifecycleExecutionError::Persistence(error.to_string())
                        })?;
                        let (expected_revision, idempotency_key) = lifecycle_claim;
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
                let error = match ModuleOperationJournal::mark_failed(
                    db,
                    operation.id,
                    &journal_message,
                )
                .await
                {
                    Ok(()) => error,
                    Err(journal_error) => ModuleLifecycleExecutionError::Persistence(format!(
                        "{error}; additionally, marking the lifecycle operation \
                         failed did not persist: {journal_error}"
                    )),
                };
                let error = release_claim_after_error(
                    db,
                    request.tenant_id,
                    &request.module_slug,
                    idempotency_key,
                    error,
                )
                .await;
                return Err(error);
            }
        };
        return Ok(ModuleLifecycleToggleResult {
            module_slug: request.module_slug.clone(),
            state,
            override_enabled: request.requested_override_enabled,
            operation_id: Some(operation.id),
            revision: expected_revision.checked_add(1).ok_or_else(|| {
                ModuleLifecycleExecutionError::Persistence(
                    "module lifecycle revision overflow".to_string(),
                )
            })?,
            settings,
        });
    }

    if let Err(error) = ModuleOperationJournal::mark_running(db, operation.id).await {
        return Err(release_claim_after_error(
            db,
            request.tenant_id,
            &request.module_slug,
            idempotency_key,
            ModuleLifecycleExecutionError::Persistence(error.to_string()),
        )
        .await);
    }

    let pre_phase = if request.enabled {
        ModuleLifecycleHookPhase::PreEnable
    } else {
        ModuleLifecycleHookPhase::PreDisable
    };
    if let Err(error) = dispatch_lifecycle_with_claim_heartbeat(
        db,
        dispatcher,
        request.tenant_id,
        &request.module_slug,
        &settings,
        pre_phase,
        expected_revision,
        idempotency_key,
    )
    .await
    {
        let execution_error = match error {
            ClaimHeartbeatDispatchError::Hook(message) => {
                ModuleLifecycleExecutionError::PreHook(message)
            }
            ClaimHeartbeatDispatchError::ClaimLost { module_slug } => {
                ModuleLifecycleExecutionError::ClaimLost { module_slug }
            }
            ClaimHeartbeatDispatchError::Persistence(message) => {
                ModuleLifecycleExecutionError::Persistence(message)
            }
        };
        let journal_message = match &execution_error {
            ModuleLifecycleExecutionError::PreHook(message) => message.clone(),
            error => error.to_string(),
        };
        let execution_error =
            match ModuleOperationJournal::mark_failed(db, operation.id, &journal_message).await {
                Ok(()) => execution_error,
                Err(journal_error) => ModuleLifecycleExecutionError::Persistence(format!(
                    "{execution_error}; additionally, marking the lifecycle operation \
                     failed did not persist: {journal_error}"
                )),
            };
        return Err(release_claim_after_error(
            db,
            request.tenant_id,
            &request.module_slug,
            idempotency_key,
            execution_error,
        )
        .await);
    }

    let requested_override_enabled = request.requested_override_enabled;
    let module_slug = request.module_slug.clone();
    let policy_transition = request.policy_transition.clone();
    let tenant_id = request.tenant_id;
    let command_context = request.context.clone();
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
                    ModuleOperationJournal::mark_committed_hook_pending(
                        transaction,
                        operation.id,
                    )
                    .await
                    .map_err(|error| {
                        ModuleLifecycleExecutionError::Persistence(error.to_string())
                    })?;
                    if let (Some(coordinator), Some(transition)) = (coordinator, policy_transition)
                    {
                        coordinator
                            .publish_and_advance(
                                transaction,
                                &command_context,
                                "module.lifecycle",
                                &transition,
                            )
                            .await
                            .map_err(|error| {
                                ModuleLifecycleExecutionError::PolicyTransition(error.to_string())
                            })?;
                    }
                    StaticTenantLifecycleStore::advance(
                        transaction,
                        tenant_id,
                        &module_slug,
                        expected_revision,
                        idempotency_key,
                    )
                    .await
                    .map_err(map_static_lifecycle_store_error)?;
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
            let error =
                match ModuleOperationJournal::mark_failed(db, operation.id, &journal_message)
                    .await
                {
                    Ok(()) => error,
                    Err(journal_error) => ModuleLifecycleExecutionError::Persistence(format!(
                        "{error}; additionally, marking the lifecycle operation \
                         failed did not persist: {journal_error}"
                    )),
                };
            let error = release_claim_after_error(
                db,
                request.tenant_id,
                &request.module_slug,
                idempotency_key,
                error,
            )
            .await;
            return Err(error);
        }
    };

    let committed_revision = expected_revision.checked_add(1).ok_or_else(|| {
        ModuleLifecycleExecutionError::Persistence(
            "module lifecycle revision overflow".to_string(),
        )
    })?;
    let post_phase = if request.enabled {
        ModuleLifecycleHookPhase::PostEnable
    } else {
        ModuleLifecycleHookPhase::PostDisable
    };
    if let Err(error) = dispatch_lifecycle_with_claim_heartbeat(
        db,
        dispatcher,
        request.tenant_id,
        &request.module_slug,
        &settings,
        post_phase,
        committed_revision,
        idempotency_key,
    )
    .await
    {
        let execution_error = match error {
            ClaimHeartbeatDispatchError::Hook(message) => {
                ModuleLifecycleExecutionError::PostHook(message)
            }
            ClaimHeartbeatDispatchError::ClaimLost { module_slug } => {
                ModuleLifecycleExecutionError::ClaimLost { module_slug }
            }
            ClaimHeartbeatDispatchError::Persistence(message) => {
                ModuleLifecycleExecutionError::Persistence(message)
            }
        };
        // The override state commit is already durable: record the distinct
        // committed-but-hook-failed outcome instead of rewriting history to
        // `Failed`. Recovery retries only the post-hook for this status. The
        // raw message is stored without the historical `post-hook:` prefix:
        // classification reads the typed status, and only pre-split legacy
        // rows still carry (and are recognized by) the prefix.
        let journal_message = match &execution_error {
            ModuleLifecycleExecutionError::PostHook(message) => message.clone(),
            error => error.to_string(),
        };
        let execution_error = match ModuleOperationJournal::mark_committed_hook_failed(
            db,
            operation.id,
            &journal_message,
        )
        .await
        {
            Ok(()) => execution_error,
            Err(journal_error) => ModuleLifecycleExecutionError::Persistence(format!(
                "{execution_error}; additionally, marking the committed lifecycle \
                 operation hook-failed did not persist: {journal_error}"
            )),
        };
        return Err(release_claim_after_error(
            db,
            request.tenant_id,
            &request.module_slug,
            idempotency_key,
            execution_error,
        )
        .await);
    }

    // Terminal success and claim release commit atomically. A crash before
    // this transaction leaves `CommittedHookPending` plus the exact claim
    // for recovery; a committed transaction can never leave a terminal
    // operation that still appears to own the aggregate.
    let result = ModuleLifecycleToggleResult {
        module_slug: request.module_slug.clone(),
        state,
        override_enabled: request.requested_override_enabled,
        operation_id: Some(operation.id),
        revision: committed_revision,
        settings,
    };
    let result_snapshot = match serde_json::to_string(&result) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            return Err(release_claim_after_error(
                db,
                request.tenant_id,
                &request.module_slug,
                idempotency_key,
                ModuleLifecycleExecutionError::Persistence(format!(
                    "serializing the lifecycle operation result snapshot failed: {error}"
                )),
            )
            .await);
        }
    };
    let operation_id = operation.id;
    let completion_tenant_id = request.tenant_id;
    let completion_module_slug = request.module_slug.clone();
    db.transaction::<_, (), ModuleLifecycleExecutionError>(|transaction| {
        Box::pin(async move {
            ModuleOperationJournal::mark_committed_with_result(
                transaction,
                operation_id,
                &result_snapshot,
            )
            .await
            .map_err(|error| {
                ModuleLifecycleExecutionError::Persistence(error.to_string())
            })?;
            StaticTenantLifecycleStore::release(
                transaction,
                completion_tenant_id,
                &completion_module_slug,
                idempotency_key,
            )
            .await
            .map_err(map_static_lifecycle_store_error)?;
            Ok(())
        })
    })
    .await
    .map_err(transaction_error)?;

    Ok(result)
}

#[derive(Debug)]
pub(crate) enum ClaimHeartbeatDispatchError {
    Hook(String),
    ClaimLost { module_slug: String },
    Persistence(String),
}

/// One claim-lease renewal tick. Extracted as a named future so the
/// dispatch loop can re-arm the same pinned future type after each
/// successful renewal (distinct `async move` blocks are distinct types).
async fn lifecycle_claim_heartbeat_tick(
    db: &DatabaseConnection,
    tenant_id: uuid::Uuid,
    module_slug: &str,
    expected_revision: u64,
    idempotency_key: uuid::Uuid,
) -> Result<bool, StaticTenantLifecycleStoreError> {
    tokio::time::sleep(LIFECYCLE_CLAIM_HEARTBEAT_INTERVAL).await;
    StaticTenantLifecycleStore::heartbeat(
        db,
        tenant_id,
        module_slug,
        expected_revision,
        idempotency_key,
    )
    .await
}

/// Dispatches one lifecycle hook while renewing the exact aggregate claim.
/// If the claim lease has already been reclaimed or the revision advanced,
/// dispatch is abandoned before the caller performs any further durable
/// lifecycle write under the stale claim.
pub(crate) async fn dispatch_lifecycle_with_claim_heartbeat(
    db: &DatabaseConnection,
    dispatcher: &ModuleExecutionDispatcher<'_>,
    tenant_id: uuid::Uuid,
    module_slug: &str,
    settings: &serde_json::Value,
    phase: ModuleLifecycleHookPhase,
    expected_revision: u64,
    idempotency_key: uuid::Uuid,
) -> Result<(), ClaimHeartbeatDispatchError> {
    let dispatch = dispatcher.dispatch_lifecycle(db, tenant_id, module_slug, settings, phase);
    tokio::pin!(dispatch);
    // Poll the heartbeat as an independent future rather than awaiting it
    // inside the timer branch. A hook may temporarily hold the only pooled
    // SQLite connection; the dispatch future must keep making progress so
    // it can release that connection for the heartbeat query.
    let mut heartbeat = Box::pin(lifecycle_claim_heartbeat_tick(
        db,
        tenant_id,
        module_slug,
        expected_revision,
        idempotency_key,
    ));
    loop {
        tokio::select! {
            biased;
            result = &mut dispatch => {
                return result.map_err(|error| {
                    ClaimHeartbeatDispatchError::Hook(error.to_string())
                });
            }
            heartbeat_result = &mut heartbeat => {
                match heartbeat_result {
                    Ok(true) => {}
                    Ok(false) => {
                        return Err(ClaimHeartbeatDispatchError::ClaimLost {
                            module_slug: module_slug.to_string(),
                        });
                    }
                    Err(error) => {
                        return Err(ClaimHeartbeatDispatchError::Persistence(
                            map_static_lifecycle_store_error(error).to_string(),
                        ));
                    }
                }
                heartbeat = Box::pin(lifecycle_claim_heartbeat_tick(
                    db,
                    tenant_id,
                    module_slug,
                    expected_revision,
                    idempotency_key,
                ));
            }
        }
    }
}

/// Releases the lifecycle claim on an error path without ever
/// discarding either failure: the primary error stays visible, and a
/// release failure is reported alongside it (the claim lease remains as
/// the final self-healing bound).
async fn release_claim_after_error(
    db: &DatabaseConnection,
    tenant_id: uuid::Uuid,
    module_slug: &str,
    idempotency_key: uuid::Uuid,
    primary_error: ModuleLifecycleExecutionError,
) -> ModuleLifecycleExecutionError {
    match StaticTenantLifecycleStore::release(db, tenant_id, module_slug, idempotency_key).await {
        Ok(()) => primary_error,
        Err(release_error) => ModuleLifecycleExecutionError::Persistence(format!(
            "{primary_error}; additionally failed to release the lifecycle \
             claim (it will expire via its lease): {release_error}"
        )),
    }
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
        let journal_result = ModuleOperationJournal::mark_failed(db, operation_id, &message).await;
        let persistence = match journal_result {
            Ok(()) => error.to_string(),
            Err(journal_error) => format!(
                "{error} (additionally, marking the operation failed did not persist: \
                 {journal_error})"
            ),
        };
        return Err(ModuleLifecycleExecutionError::Persistence(persistence));
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
                "module lifecycle revision overflow for `{module_slug}`"
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
            // Prefer the exact result returned to the original caller:
            // state, revision, and settings are operation-time evidence,
            // and replaying them from current tenant state would silently
            // rewrite history after later operations. A stored snapshot
            // that does not decode or does not belong to this operation is
            // corrupt evidence and fails closed instead of falling back.
            if let Some(serialized) = ModuleOperationJournal::result_snapshot(db, operation.id)
                .await
                .map_err(|error| {
                    ModuleLifecycleExecutionError::Persistence(error.to_string())
                })?
            {
                let result: ModuleLifecycleToggleResult =
                    serde_json::from_str(&serialized).map_err(|error| {
                        ModuleLifecycleExecutionError::Persistence(format!(
                            "stored lifecycle result snapshot is invalid: {error}"
                        ))
                    })?;
                if result.operation_id != Some(operation.id)
                    || result.module_slug != request.module_slug
                {
                    return Err(ModuleLifecycleExecutionError::Persistence(
                        "stored lifecycle result snapshot does not belong to the \
                         replayed operation"
                            .to_string(),
                    ));
                }
                return Ok(result);
            }
            // Legacy rows committed before result snapshots existed (or via
            // recovery, which produces no caller result) reconstruct the
            // response from current state; this is a documented
            // approximation for pre-existing data only.
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
                revision: StaticTenantLifecycleStore::snapshot(
                    db,
                    request.tenant_id,
                    &request.module_slug,
                )
                .await
                .map_err(map_static_lifecycle_store_error)?
                .revision,
                settings,
            })
        }
        ModuleOperationStatus::CommittedHookFailed => {
            // State is committed; only the post-hook is outstanding. Replay
            // must not re-apply state — surface the hook failure so the
            // caller can route to post-hook recovery.
            let message = operation
                .error_message
                .unwrap_or_else(|| "unknown failure".to_string());
            let message = message.strip_prefix("post-hook: ").unwrap_or(&message);
            Err(ModuleLifecycleExecutionError::PostHook(message.to_string()))
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
        ModuleOperationStatus::Validated
        | ModuleOperationStatus::Running
        | ModuleOperationStatus::CommittedHookPending => {
            Err(ModuleLifecycleExecutionError::OperationInProgress {
                module_slug: request.module_slug.clone(),
            })
        }
        ModuleOperationStatus::Orphaned => {
            Err(ModuleLifecycleExecutionError::OperationOrphaned {
                module_slug: request.module_slug.clone(),
                operation_id: operation.id,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use rustok_core::{MigrationSource, ModuleRegistry, RusToKModule};
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
    use sea_orm_migration::MigrationTrait;

    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::{
        ArtifactBindingDispatch, ArtifactBindingDispatchError, ArtifactBindingExecutor,
        ArtifactPayloadKind, ArtifactReleaseRef, ModuleDefinition, ModuleDefinitionCatalog,
        ModuleDefinitionKind, ModuleDefinitionSource,
    };

    struct OptionalModule;

    struct CountingArtifactExecutor(AtomicUsize);

    #[async_trait]
    impl ArtifactBindingExecutor for CountingArtifactExecutor {
        fn supports_payload_kind(&self, _payload_kind: ArtifactPayloadKind) -> bool {
            true
        }

        async fn dispatch_binding(
            &self,
            _dispatch: ArtifactBindingDispatch<'_>,
        ) -> Result<serde_json::Value, ArtifactBindingDispatchError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(serde_json::Value::Null)
        }
    }

    impl MigrationSource for OptionalModule {
        fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
            Vec::new()
        }
    }

    #[async_trait]
    impl RusToKModule for OptionalModule {
        fn slug(&self) -> &'static str {
            "optional_test"
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
            .execute_raw(Statement::from_string(
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
                    result_snapshot TEXT, \
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, \
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP\
                 )"
                .to_string(),
            ))
            .await
            .expect("module operations table");
        database
            .execute_raw(Statement::from_string(
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
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                "CREATE TABLE tenant_modules (\
                    id TEXT PRIMARY KEY NOT NULL, \
                    tenant_id TEXT NOT NULL, \
                    module_slug TEXT NOT NULL, \
                    enabled BOOLEAN NULL, \
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
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                "CREATE TABLE module_static_tenant_lifecycle (\
                    tenant_id TEXT NOT NULL, \
                    module_slug TEXT NOT NULL, \
                    revision INTEGER NOT NULL, \
                    active_idempotency_key TEXT, \
                    active_claimed_at TEXT, \
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

    fn command_context(tenant_id: uuid::Uuid, idempotency_key: uuid::Uuid) -> ModuleCommandContext {
        ModuleCommandContext {
            actor_id: uuid::Uuid::new_v4(),
            tenant_id: Some(tenant_id),
            trace_id: "test:module-lifecycle".to_string(),
            correlation_id: uuid::Uuid::new_v4(),
            idempotency_key,
        }
    }

    async fn operation_count(
        database: &DatabaseConnection,
        tenant_id: uuid::Uuid,
        idempotency_key: uuid::Uuid,
    ) -> i64 {
        let row = database
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT COUNT(*) AS count FROM module_operations \
                 WHERE tenant_id = ?1 AND idempotency_key = ?2",
                vec![tenant_id.into(), idempotency_key.into()],
            ))
            .await
            .expect("operation count query")
            .expect("operation count row");
        row.try_get("", "count").expect("operation count")
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
            module_slug: "optional_test".to_string(),
            enabled: true,
            context: command_context(tenant_id, idempotency_key),
            expected_revision: 0,
            effective_enabled_modules: HashSet::from(["optional_test".to_string()]),
            ordering_enabled_modules: HashSet::new(),
            previous_override_enabled: None,
            requested_override_enabled: Some(true),
            current_settings: serde_json::json!({}),
            policy_transition: None,
        };

        let first = execute_module_toggle(&database, &dispatcher, None, request.clone())
            .await
            .expect("first no-op command");
        let replay = execute_module_toggle(&database, &dispatcher, None, request.clone())
            .await
            .expect("exact retry replays");

        let mut changed_trace_request = request.clone();
        changed_trace_request.context = ModuleCommandContext {
            trace_id: "test:module-lifecycle:changed-trace".to_string(),
            ..request.context.clone()
        };
        let changed_trace_replay =
            execute_module_toggle(&database, &dispatcher, None, changed_trace_request)
                .await
                .expect("changed observability context still replays the semantic command");
        assert_eq!(changed_trace_replay.operation_id, first.operation_id);

        assert_eq!(first.operation_id, replay.operation_id);
        // Replay returns the stored operation-time result verbatim, not a
        // reconstruction from current state.
        assert_eq!(replay, first);
        assert_eq!(changed_trace_replay, first);
        let row = database
            .query_one_raw(Statement::from_sql_and_values(
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
        let database = journal_and_state_database().await;
        database
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                "CREATE TRIGGER fail_tenant_module_insert \
                 BEFORE INSERT ON tenant_modules \
                 BEGIN SELECT RAISE(ABORT, 'forced state commit failure'); END"
                    .to_string(),
            ))
            .await
            .expect("state commit failure trigger");
        let registry = ModuleRegistry::new().register(OptionalModule);
        let catalog = ModuleDefinitionCatalog::from_static_registry(&registry).expect("catalog");
        let dispatcher = ModuleExecutionDispatcher::new(&catalog, &registry);
        let tenant_id = uuid::Uuid::new_v4();

        let result = execute_module_toggle(
            &database,
            &dispatcher,
            None,
            ModuleLifecycleToggleRequest {
                tenant_id,
                module_slug: "optional_test".to_string(),
                enabled: true,
                context: command_context(tenant_id, uuid::Uuid::new_v4()),
                expected_revision: 0,
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
            .query_one_raw(Statement::from_string(
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

    #[tokio::test]
    async fn claim_contention_and_revision_conflicts_do_not_poison_the_journal() {
        let database = journal_and_state_database().await;
        let registry = ModuleRegistry::new().register(OptionalModule);
        let catalog = ModuleDefinitionCatalog::from_static_registry(&registry).expect("catalog");
        let dispatcher = ModuleExecutionDispatcher::new(&catalog, &registry);
        let tenant_id = uuid::Uuid::new_v4();
        let holder_key = uuid::Uuid::new_v4();
        StaticTenantLifecycleStore::claim(
            &database,
            tenant_id,
            "optional_test",
            0,
            holder_key,
        )
        .await
        .expect("holder claim");

        let contender_key = uuid::Uuid::new_v4();
        let contended = execute_module_toggle(
            &database,
            &dispatcher,
            None,
            ModuleLifecycleToggleRequest {
                tenant_id,
                module_slug: "optional_test".to_string(),
                enabled: true,
                context: command_context(tenant_id, contender_key),
                expected_revision: 0,
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
            contended,
            Err(ModuleLifecycleExecutionError::OperationInProgress { .. })
        ));
        assert_eq!(operation_count(&database, tenant_id, contender_key).await, 0);

        StaticTenantLifecycleStore::release(
            &database,
            tenant_id,
            "optional_test",
            holder_key,
        )
        .await
        .expect("holder claim release");

        let stale_key = uuid::Uuid::new_v4();
        let stale_revision = execute_module_toggle(
            &database,
            &dispatcher,
            None,
            ModuleLifecycleToggleRequest {
                tenant_id,
                module_slug: "optional_test".to_string(),
                enabled: true,
                context: command_context(tenant_id, stale_key),
                expected_revision: 9,
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
            stale_revision,
            Err(ModuleLifecycleExecutionError::RevisionConflict {
                expected: 9,
                current: 0,
                ..
            })
        ));
        assert_eq!(operation_count(&database, tenant_id, stale_key).await, 0);
    }

    #[tokio::test]
    async fn orphaned_operation_replays_as_a_typed_orphan_error() {
        let database = journal_and_state_database().await;
        let registry = ModuleRegistry::new().register(OptionalModule);
        let catalog = ModuleDefinitionCatalog::from_static_registry(&registry).expect("catalog");
        let dispatcher = ModuleExecutionDispatcher::new(&catalog, &registry);
        let tenant_id = uuid::Uuid::new_v4();
        let idempotency_key = uuid::Uuid::new_v4();
        let context = command_context(tenant_id, idempotency_key);
        let operation = ModuleOperationJournal::record(
            &database,
            ModuleOperationRequest {
                tenant_id,
                module_slug: "optional_test".to_string(),
                requested_enabled: true,
                previous_effective_enabled: false,
                requested_by: Some(context.actor_id.to_string()),
                trace_id: Some(context.trace_id.clone()),
                correlation_id: context.correlation_id.to_string(),
                idempotency_key: Some(idempotency_key),
                expected_revision: Some(0),
            },
        )
        .await
        .expect("orphaned operation record");
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "UPDATE module_operations SET status = 'orphaned', \
                 error_message = 'owner did not finish' WHERE id = ?1",
                vec![operation.id.into()],
            ))
            .await
            .expect("mark operation orphaned");

        let replay = execute_module_toggle(
            &database,
            &dispatcher,
            None,
            ModuleLifecycleToggleRequest {
                tenant_id,
                module_slug: "optional_test".to_string(),
                enabled: true,
                context,
                expected_revision: 0,
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
            replay,
            Err(ModuleLifecycleExecutionError::OperationOrphaned {
                operation_id,
                ..
            }) if operation_id == operation.id
        ));
    }

    #[tokio::test]
    async fn settings_are_reread_after_claim_acquisition() {
        let database = journal_and_state_database().await;
        let registry = ModuleRegistry::new().register(OptionalModule);
        let catalog = ModuleDefinitionCatalog::from_static_registry(&registry).expect("catalog");
        let dispatcher = ModuleExecutionDispatcher::new(&catalog, &registry);
        let tenant_id = uuid::Uuid::new_v4();
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO tenant_modules \
                 (id, tenant_id, module_slug, enabled, settings) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                vec![
                    uuid::Uuid::new_v4().to_string().into(),
                    tenant_id.to_string().into(),
                    "optional_test".into(),
                    true.into(),
                    "{\"fresh\":true}".into(),
                ],
            ))
            .await
            .expect("fresh Settings insert");

        let result = execute_module_toggle(
            &database,
            &dispatcher,
            None,
            ModuleLifecycleToggleRequest {
                tenant_id,
                module_slug: "optional_test".to_string(),
                enabled: true,
                context: command_context(tenant_id, uuid::Uuid::new_v4()),
                expected_revision: 0,
                effective_enabled_modules: HashSet::from(["optional_test".to_string()]),
                ordering_enabled_modules: HashSet::new(),
                previous_override_enabled: Some(true),
                requested_override_enabled: Some(true),
                current_settings: serde_json::json!({ "stale": true }),
                policy_transition: None,
            },
        )
        .await
        .expect("toggle with reread settings");

        assert_eq!(result.settings, serde_json::json!({ "fresh": true }));
        assert_eq!(result.revision, 1);
    }

    #[tokio::test]
    async fn artifact_toggles_use_the_same_claim_and_revision_gate() {
        let database = journal_and_state_database().await;
        let release = ArtifactReleaseRef {
            slug: "artifact_test".to_string(),
            version: "1.0.0".to_string(),
            digest: format!("sha256:{}", "a".repeat(64)),
        };
        let mut catalog = ModuleDefinitionCatalog::default();
        catalog
            .insert(ModuleDefinition {
                slug: release.slug.clone(),
                version: release.version.clone(),
                kind: ModuleDefinitionKind::Optional,
                source: ModuleDefinitionSource::Artifact { release },
                dependencies: Vec::new(),
                permissions: Vec::new(),
                settings_schema_digest: None,
                schema_documents: Vec::new(),
                bindings: Vec::new(),
                ui: Vec::new(),
                capabilities: Vec::new(),
            })
            .expect("artifact definition");
        let artifact_executor = CountingArtifactExecutor(AtomicUsize::new(0));
        let dispatcher = ModuleExecutionDispatcher::artifact_only(&catalog, &artifact_executor);
        let tenant_id = uuid::Uuid::new_v4();
        let holder_key = uuid::Uuid::new_v4();
        StaticTenantLifecycleStore::claim(
            &database,
            tenant_id,
            "artifact_test",
            0,
            holder_key,
        )
        .await
        .expect("artifact aggregate holder claim");

        let contender_key = uuid::Uuid::new_v4();
        let contended = execute_module_toggle(
            &database,
            &dispatcher,
            None,
            ModuleLifecycleToggleRequest {
                tenant_id,
                module_slug: "artifact_test".to_string(),
                enabled: true,
                context: command_context(tenant_id, contender_key),
                expected_revision: 0,
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
            contended,
            Err(ModuleLifecycleExecutionError::OperationInProgress { .. })
        ));
        assert_eq!(artifact_executor.0.load(Ordering::SeqCst), 0);
        assert_eq!(operation_count(&database, tenant_id, contender_key).await, 0);

        StaticTenantLifecycleStore::release(
            &database,
            tenant_id,
            "artifact_test",
            holder_key,
        )
        .await
        .expect("artifact aggregate holder release");

        let stale_key = uuid::Uuid::new_v4();
        let stale_revision = execute_module_toggle(
            &database,
            &dispatcher,
            None,
            ModuleLifecycleToggleRequest {
                tenant_id,
                module_slug: "artifact_test".to_string(),
                enabled: true,
                context: command_context(tenant_id, stale_key),
                expected_revision: 4,
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
            stale_revision,
            Err(ModuleLifecycleExecutionError::RevisionConflict {
                expected: 4,
                current: 0,
                ..
            })
        ));
        assert_eq!(artifact_executor.0.load(Ordering::SeqCst), 0);
        assert_eq!(operation_count(&database, tenant_id, stale_key).await, 0);
    }
}
