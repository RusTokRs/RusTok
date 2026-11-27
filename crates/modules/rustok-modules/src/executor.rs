use std::collections::{HashMap, HashSet};

use sea_orm::{DatabaseConnection, TransactionTrait};
use thiserror::Error;

use crate::recovery::{
    apply_tenant_override_enabled, hook_settings_digest, load_current_lifecycle_settings,
    read_tenant_override_enabled, record_operation_override_state, store_hook_settings_digest,
};

pub(crate) use crate::claim_heartbeat::{
    ClaimHeartbeatDispatchError, dispatch_lifecycle_with_claim_heartbeat,
};
use crate::{
    ModuleCommandContext, ModuleControlPlaneError, ModuleEffectivePolicyTransitionCoordinator,
    ModuleExecutionDispatcher, ModuleLifecycleHookPhase, ModuleOperationJournal,
    ModuleOperationRecordOutcome, ModuleOperationRequest, ModuleOperationSnapshot,
    ModuleOperationStatus, ModulePolicyRevisionTransition, ModuleToggleValidationError,
    StaticTenantLifecycleStore, StaticTenantLifecycleStoreError, TenantModuleSettingsRequest,
    TenantModuleStateRecord, TenantModuleStateStore, validate_module_toggle,
};

#[derive(Clone, Debug)]
pub(crate) struct ModuleLifecycleToggleRequest {
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
    /// Original failed operation when this is a compensation.
    pub recovery_source_id: Option<uuid::Uuid>,
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
    /// Host-composed schema or resolution failure. The owner checks the
    /// journal for replay before rejecting a new command with unavailable evidence.
    pub static_settings_schema: Option<
        Result<HashMap<String, crate::ModuleSettingSpec>, StaticSettingsSchemaResolutionError>,
    >,
}

/// Typed error wrapper for host static settings schema resolution failure.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error, serde::Serialize, serde::Deserialize)]
#[error("{0}")]
pub struct StaticSettingsSchemaResolutionError(pub String);

impl From<&str> for StaticSettingsSchemaResolutionError {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for StaticSettingsSchemaResolutionError {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl std::ops::Deref for StaticSettingsSchemaResolutionError {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
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
    #[error("settings migration required for module `{module_slug}`: {reason}")]
    SettingsMigrationRequired { module_slug: String, reason: String },
    #[error("module settings schema is unavailable: {0}")]
    SettingsSchemaUnavailable(String),
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

pub(crate) async fn execute_module_toggle(
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
    if request.static_settings_schema.is_some()
        && matches!(
            dispatcher
                .catalog()
                .get(&request.module_slug)
                .map(|definition| &definition.source),
            Some(crate::ModuleDefinitionSource::Artifact { .. })
        )
    {
        return Err(ModuleLifecycleExecutionError::Persistence(
            "a static settings schema cannot be applied to an artifact module".to_string(),
        ));
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
        recovery_source_id: request.recovery_source_id,
        expected_revision: Some(request.expected_revision),
    };
    if let Some(existing) =
        ModuleOperationJournal::replay_idempotent_command(db, &operation_request)
            .await
            .map_err(map_idempotency_store_error)?
    {
        return replay_lifecycle_operation(db, &operation_request, existing, settings).await;
    }
    if let Some(source_id) = request.recovery_source_id {
        // This entry point is public; a caller must not bypass the recovery
        // owner's evidence gate by constructing a compensation directly.
        // Replay above is read-only, while a missing receipt must never start
        // another hook merely because an earlier claim was cleared.
        let source = crate::recovery::module_operation_recovery_plan(db, source_id)
            .await
            .map_err(|error| ModuleLifecycleExecutionError::Persistence(error.to_string()))?;
        if source.tenant_id != request.tenant_id || source.module_slug != request.module_slug {
            return Err(ModuleLifecycleExecutionError::InvalidCommandIdentity);
        }
        if !source.retryable {
            return Err(ModuleLifecycleExecutionError::PolicyTransition(
                crate::recovery::RECOVERY_EVIDENCE_UNAVAILABLE.to_string(),
            ));
        }
    }
    validate_module_toggle(
        dispatcher.catalog(),
        &request.ordering_enabled_modules,
        &request.module_slug,
        request.enabled,
    )?;
    if request.enabled
        && let Some(Err(reason)) = &request.static_settings_schema
    {
        return Err(ModuleLifecycleExecutionError::SettingsSchemaUnavailable(
            reason.to_string(),
        ));
    }
    if request.policy_transition.is_some() && policy_transition_coordinator.is_none() {
        return Err(ModuleLifecycleExecutionError::PolicyTransition(
            "publisher is required for an effective-policy transition".to_string(),
        ));
    }
    let definition = dispatcher
        .catalog()
        .get(&request.module_slug)
        .ok_or_else(|| {
            ModuleLifecycleExecutionError::Persistence(
                "validated module definition disappeared".to_string(),
            )
        })?;
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
    if let Err(error) = crate::lifecycle_transition_fence::claim(
        db,
        request.tenant_id,
        &request.module_slug,
        definition,
        expected_revision,
        idempotency_key,
    )
    .await
    {
        use crate::lifecycle_transition_fence::LifecycleAdmissionError;
        let error = match error {
            LifecycleAdmissionError::TransitionInProgress => {
                let mapped = ModuleLifecycleExecutionError::PolicyTransition(
                    "an unresolved module release transition blocks lifecycle changes".to_string(),
                );
                return Err(match ModuleOperationJournal::mark_failed(
                    db, operation.id,
                    "transition-fence: an unresolved module release transition blocks lifecycle changes",
                ).await {
                    Ok(()) => mapped,
                    Err(journal_error) => ModuleLifecycleExecutionError::Persistence(format!(
                        "{mapped}; additionally, recording the transition failure failed: {journal_error}"
                    )),
                });
            }
            LifecycleAdmissionError::StaleDefinition => {
                let mapped = ModuleLifecycleExecutionError::PolicyTransition(
                    "the selected artifact definition is no longer the active installation"
                        .to_string(),
                );
                return Err(match ModuleOperationJournal::mark_failed(
                    db, operation.id,
                    "transition-fence: artifact definition no longer matches active installation",
                ).await {
                    Ok(()) => mapped,
                    Err(journal_error) => ModuleLifecycleExecutionError::Persistence(format!(
                        "{mapped}; additionally, recording the definition failure failed: {journal_error}"
                    )),
                });
            }
            LifecycleAdmissionError::Database(message) => {
                // Commit or rollback may have been acknowledged ambiguously.
                // Preserve the validated journal; never assume the claim was absent.
                return Err(ModuleLifecycleExecutionError::Persistence(message));
            }
            LifecycleAdmissionError::Claim(error) => error,
        };
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
    // Recheck as defense in depth; admission already checked under the shared
    // platform lock while acquiring the journal claim. This cannot fence a
    // hook that continues after its claim has been lost or released.
    let checkpoint_result =
        crate::TransitionCheckpointStore::list_active_checkpoints_for_slug_and_tenant_or_platform(
            db,
            &request.module_slug,
            request.tenant_id,
        )
        .await;
    let transition_error = match checkpoint_result {
        Ok(checkpoints) if !checkpoints.is_empty() => {
            Some(ModuleLifecycleExecutionError::PolicyTransition(
                "an unresolved module release transition blocks lifecycle changes".to_string(),
            ))
        }
        Ok(_) => None,
        Err(error) => Some(ModuleLifecycleExecutionError::Persistence(
            error.to_string(),
        )),
    };
    if let Some(error) = transition_error {
        let message = match &error {
            ModuleLifecycleExecutionError::Persistence(reason) => {
                format!("transition-read: {reason}")
            }
            ModuleLifecycleExecutionError::PolicyTransition(reason) => {
                format!("transition-fence: {reason}")
            }
            other => format!("transition-error: {other}"),
        };
        let error = match ModuleOperationJournal::mark_failed(db, operation.id, &message).await {
            Ok(()) => error,
            Err(journal_error) => ModuleLifecycleExecutionError::Persistence(format!(
                "{error}; additionally, recording the transition failure failed: {journal_error}"
            )),
        };
        return Err(release_claim_after_error(
            db,
            request.tenant_id,
            &request.module_slug,
            idempotency_key,
            error,
        )
        .await);
    }
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
            let primary = match ModuleOperationJournal::mark_failed(
                db,
                operation.id,
                &format!("settings-read: {primary}"),
            )
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

    let normalized_static_settings = if request.enabled {
        if let Some(Ok(schema)) = &request.static_settings_schema {
            match crate::normalize_module_settings(&request.module_slug, schema, settings.clone()) {
                Ok(normalized) => {
                    settings = normalized.clone();
                    Some(normalized)
                }
                Err(error) => {
                    let reason = error.to_string();
                    let primary = ModuleLifecycleExecutionError::SettingsMigrationRequired {
                        module_slug: request.module_slug.clone(),
                        reason: reason.clone(),
                    };
                    let primary = match ModuleOperationJournal::mark_failed(
                        db,
                        operation.id,
                        &format!("settings-preflight: {reason}"),
                    )
                    .await
                    {
                        Ok(()) => primary,
                        Err(error) => ModuleLifecycleExecutionError::Persistence(format!(
                            "{primary}; additionally, recording the failed preflight failed: {error}"
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
            }
        } else {
            None
        }
    } else {
        None
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
                        // Same platform slug fence as serving SQL admission. Take it
                        // before touching the override, journal or aggregate revision.
                        crate::installation::acquire_artifact_activation_lock(
                            transaction, &crate::ModuleInstallationScope::Platform, &module_slug,
                        ).await.map_err(|error| ModuleLifecycleExecutionError::Persistence(error.to_string()))?;
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
                        if let Some(settings) = normalized_static_settings {
                            TenantModuleStateStore::persist_settings(
                                transaction,
                                TenantModuleSettingsRequest {
                                    tenant_id,
                                    module_slug: module_slug.clone(),
                                    settings,
                                },
                            )
                            .await
                            .map_err(|error| {
                                ModuleLifecycleExecutionError::Persistence(error.to_string())
                            })?;
                        }
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
                        let result_snapshot = serde_json::to_string(&result).map_err(|error| {
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
        // A dispatch error, lost lease, or uncertain transport result does
        // not prove that an issued hook and its external effects stopped.
        // Keep the durable fence for explicit reconciliation.
        return Err(execution_error);
    }

    let requested_override_enabled = request.requested_override_enabled;
    let module_slug = request.module_slug.clone();
    let policy_transition = request.policy_transition.clone();
    let tenant_id = request.tenant_id;
    let command_context = request.context.clone();
    let normalized_static_settings = normalized_static_settings.clone();
    let hook_settings_digest = match hook_settings_digest(&settings) {
        Ok(digest) => digest,
        Err(error) => {
            let primary = ModuleLifecycleExecutionError::Persistence(error.to_string());
            let primary = match ModuleOperationJournal::mark_failed(
                db,
                operation.id,
                &format!("settings-read: {primary}"),
            )
            .await
            {
                Ok(()) => primary,
                Err(error) => ModuleLifecycleExecutionError::Persistence(format!(
                    "{primary}; recording the failure also failed: {error}"
                )),
            };
            // The pre-hook has already been issued. Preserve its fence.
            return Err(primary);
        }
    };
    let coordinator = policy_transition_coordinator;
    let state = match db
        .transaction::<_, Option<TenantModuleStateRecord>, ModuleLifecycleExecutionError>(
            |transaction| {
                Box::pin(async move {
                    lock_lifecycle_commit_authority(
                        transaction, tenant_id, &module_slug,
                        coordinator.as_ref(), policy_transition.as_ref(),
                    ).await?;
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
                    if let Some(settings) = normalized_static_settings {
                        TenantModuleStateStore::persist_settings(
                            transaction,
                            TenantModuleSettingsRequest {
                                tenant_id,
                                module_slug: module_slug.clone(),
                                settings,
                            },
                        )
                        .await
                        .map_err(|error| {
                            ModuleLifecycleExecutionError::Persistence(error.to_string())
                        })?;
                    }
                    store_hook_settings_digest(transaction, operation.id, &hook_settings_digest)
                        .await
                        .map_err(|error| {
                            ModuleLifecycleExecutionError::Persistence(error.to_string())
                        })?;
                    ModuleOperationJournal::mark_committed_hook_pending(transaction, operation.id)
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
                match ModuleOperationJournal::mark_failed(db, operation.id, &journal_message).await
                {
                    Ok(()) => error,
                    Err(journal_error) => ModuleLifecycleExecutionError::Persistence(format!(
                        "{error}; additionally, marking the lifecycle operation \
                         failed did not persist: {journal_error}"
                    )),
                };
            // A transaction/commit error can be ambiguous, and a pre-hook
            // has already been issued. Preserve the fence.
            return Err(error);
        }
    };

    let committed_revision = expected_revision.checked_add(1).ok_or_else(|| {
        ModuleLifecycleExecutionError::Persistence("module lifecycle revision overflow".to_string())
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
        // A dispatch error, lost lease, or uncertain transport result does
        // not prove that an issued hook and its external effects stopped.
        // Keep the durable fence for explicit reconciliation.
        return Err(execution_error);
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
            return Err(ModuleLifecycleExecutionError::Persistence(format!(
                "serializing the lifecycle operation result snapshot failed: {error}"
            )));
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
            .map_err(|error| ModuleLifecycleExecutionError::Persistence(error.to_string()))?;
            // Journal completion and claim release share the same transaction.
            // Recheck after locking the journal row, so a delayed completion
            // cannot commit success under an expired or replaced claim.
            let claim_live = StaticTenantLifecycleStore::heartbeat(
                transaction,
                completion_tenant_id,
                &completion_module_slug,
                committed_revision,
                idempotency_key,
            )
            .await
            .map_err(map_static_lifecycle_store_error)?;
            if !claim_live {
                return Err(ModuleLifecycleExecutionError::ClaimLost {
                    module_slug: completion_module_slug,
                });
            }
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

/// Releases the lifecycle claim on an error path without ever
/// discarding either failure: the primary error stays visible, and a
/// release failure is reported alongside it. A retained claim requires
/// explicit reconciliation; expiry alone does not clear it.
/// Shared ordering with administrative Auth/RBAC: cursor before slug.
/// No authority lock is held across a lifecycle hook.
async fn lock_lifecycle_commit_authority(
    transaction: &sea_orm::DatabaseTransaction,
    tenant_id: uuid::Uuid,
    module_slug: &str,
    coordinator: Option<&ModuleEffectivePolicyTransitionCoordinator>,
    transition: Option<&ModulePolicyRevisionTransition>,
) -> Result<(), ModuleLifecycleExecutionError> {
    if let Some(transition) = transition {
        let coordinator = coordinator.ok_or_else(|| ModuleLifecycleExecutionError::PolicyTransition(
            "publisher is required for an effective-policy transition".into(),
        ))?;
        let current = coordinator.lock_current_revision(
            transaction, tenant_id, "module.lifecycle",
        ).await.map_err(|error| ModuleLifecycleExecutionError::PolicyTransition(error.to_string()))?;
        if current != transition.previous_revision {
            return Err(ModuleLifecycleExecutionError::PolicyTransition(
                "effective-policy predecessor changed before lifecycle commit".into(),
            ));
        }
    }
    crate::installation::acquire_artifact_activation_lock(
        transaction, &crate::ModuleInstallationScope::Platform, module_slug,
    ).await.map_err(|error| ModuleLifecycleExecutionError::Persistence(error.to_string()))?;
    Ok(())
}

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
             claim (it remains fenced for reconciliation): {release_error}"
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

pub(crate) fn map_static_lifecycle_store_error(
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
                .map_err(|error| ModuleLifecycleExecutionError::Persistence(error.to_string()))?
            {
                let result: ModuleLifecycleToggleResult = serde_json::from_str(&serialized)
                    .map_err(|error| {
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
            if let Some(reason) = message.strip_prefix("settings-preflight: ") {
                Err(ModuleLifecycleExecutionError::SettingsMigrationRequired {
                    module_slug: request.module_slug.clone(),
                    reason: reason.to_string(),
                })
            } else if let Some(message) = message.strip_prefix("transition-fence: ") {
                Err(ModuleLifecycleExecutionError::PolicyTransition(
                    message.to_string(),
                ))
            } else if let Some(message) = message.strip_prefix("transition-read: ") {
                Err(ModuleLifecycleExecutionError::Persistence(
                    message.to_string(),
                ))
            } else if let Some(message) = message.strip_prefix("settings-read: ") {
                Err(ModuleLifecycleExecutionError::Persistence(
                    message.to_string(),
                ))
            } else if let Some(message) = message.strip_prefix("aggregate-claim: ") {
                Err(ModuleLifecycleExecutionError::Persistence(
                    message.to_string(),
                ))
            } else if let Some(message) = message.strip_prefix("recovery-state: ") {
                Err(ModuleLifecycleExecutionError::Persistence(
                    message.to_string(),
                ))
            } else if let Some(message) = message.strip_prefix("post-hook: ") {
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
        ModuleOperationStatus::Orphaned => Err(ModuleLifecycleExecutionError::OperationOrphaned {
            module_slug: request.module_slug.clone(),
            operation_id: operation.id,
        }),
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
        let mut options = sea_orm::ConnectOptions::new("sqlite::memory:");
        options
            .max_connections(1)
            .min_connections(1)
            .sqlx_logging(false);
        let database = Database::connect(options).await.expect("database");
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
                    recovery_source_id TEXT, \
                    expected_revision INTEGER, \
                    error_message TEXT, \
                    result_snapshot TEXT, \
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, \
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, \
                    FOREIGN KEY (tenant_id) REFERENCES tenants(id)\
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
                    requested_override_enabled BOOLEAN, \
                    hook_settings_digest TEXT\
                 )"
                .to_string(),
            ))
            .await
            .expect("module operation override states table");
        database
    }

    async fn journal_and_state_database() -> DatabaseConnection {
        let database = journal_only_database().await;
        database.execute_unprepared("CREATE TABLE module_artifact_activation_locks (scope_kind TEXT NOT NULL, scope_tenant_key TEXT NOT NULL, slug TEXT NOT NULL, PRIMARY KEY (scope_kind, scope_tenant_key, slug))")
            .await.expect("lifecycle admission lock fixture");
        database
            .execute_unprepared("CREATE TABLE tenants (id BLOB PRIMARY KEY NOT NULL); CREATE VIEW tenant_identity_keys AS SELECT id FROM tenants")
            .await
            .expect("tenant table");
        database
            .execute_unprepared("PRAGMA foreign_keys = ON")
            .await
            .expect("enable fixture foreign keys");
        crate::migrations::m20260902_000046_module_transition_and_retention_tables::Migration
            .up(&sea_orm_migration::SchemaManager::new(&database))
            .await
            .expect("transition checkpoint tables");
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
                    UNIQUE (tenant_id, module_slug), \
                    FOREIGN KEY (tenant_id) REFERENCES tenants(id)\
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
                    claim_owner TEXT NOT NULL DEFAULT 'journal', \
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, \
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, \
                    PRIMARY KEY (tenant_id, module_slug), \
                    FOREIGN KEY (tenant_id) REFERENCES tenants(id)\
                 )"
                .to_string(),
            ))
            .await
            .expect("static lifecycle aggregate table: invariant validated at startup");
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

    async fn seed_tenant(database: &DatabaseConnection, tenant_id: uuid::Uuid) {
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT OR IGNORE INTO tenants (id) VALUES (?1)",
                vec![tenant_id.to_string().into()],
            ))
            .await
            .expect("tenant fixture");
    }

    struct ReclaimDuringHook {
        calls: std::sync::Arc<AtomicUsize>,
        replacement: uuid::Uuid,
    }

    impl MigrationSource for ReclaimDuringHook {
        fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
            Vec::new()
        }
    }

    #[async_trait]
    impl RusToKModule for ReclaimDuringHook {
        fn slug(&self) -> &'static str {
            "claim_test"
        }
        fn name(&self) -> &'static str {
            "Claim test"
        }
        fn description(&self) -> &'static str {
            "Lifecycle claim test"
        }
        fn version(&self) -> &'static str {
            "0.1.0"
        }

        async fn pre_enable(&self, ctx: rustok_core::ModuleContext<'_>) -> rustok_core::Result<()> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            ctx.db.execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "UPDATE module_static_tenant_lifecycle SET active_claimed_at = '2000-01-01T00:00:00Z' \
                 WHERE tenant_id = ?1 AND module_slug = 'claim_test'",
                vec![ctx.tenant_id.to_string().into()],
            )).await.expect("expire old claim");
            assert!(matches!(
                StaticTenantLifecycleStore::claim(
                    ctx.db,
                    ctx.tenant_id,
                    "claim_test",
                    0,
                    self.replacement
                )
                .await,
                Err(StaticTenantLifecycleStoreError::OperationInProgress { .. })
            ));
            Ok(())
        }
    }

    #[tokio::test]
    async fn lifecycle_hook_checks_claim_before_and_after_dispatch() {
        let database = journal_and_state_database().await;
        let tenant = uuid::Uuid::new_v4();
        seed_tenant(&database, tenant).await;
        let old = uuid::Uuid::new_v4();
        let replacement = uuid::Uuid::new_v4();
        let calls = std::sync::Arc::new(AtomicUsize::new(0));
        let registry = ModuleRegistry::new().register(ReclaimDuringHook {
            calls: calls.clone(),
            replacement,
        });
        let catalog = ModuleDefinitionCatalog::from_static_registry(&registry).unwrap();
        let dispatcher = ModuleExecutionDispatcher::new(&catalog, &registry);
        StaticTenantLifecycleStore::claim(&database, tenant, "claim_test", 0, old)
            .await
            .unwrap();

        // A hook that outlives its lease cannot be reported successful or
        // have its fence stolen by another worker.
        assert!(matches!(
            dispatch_lifecycle_with_claim_heartbeat(
                &database,
                &dispatcher,
                tenant,
                "claim_test",
                &serde_json::json!({}),
                ModuleLifecycleHookPhase::PreEnable,
                0,
                old,
            )
            .await,
            Err(ClaimHeartbeatDispatchError::ClaimLost { .. })
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            StaticTenantLifecycleStore::snapshot(&database, tenant, "claim_test")
                .await
                .unwrap()
                .active_idempotency_key,
            Some(old)
        );

        // A stale holder must not start another hook at all.
        assert!(matches!(
            dispatch_lifecycle_with_claim_heartbeat(
                &database,
                &dispatcher,
                tenant,
                "claim_test",
                &serde_json::json!({}),
                ModuleLifecycleHookPhase::PreEnable,
                0,
                old,
            )
            .await,
            Err(ClaimHeartbeatDispatchError::ClaimLost { .. })
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
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
                vec![tenant_id.to_string().into(), idempotency_key.into()],
            ))
            .await
            .expect("operation count query")
            .expect("operation count row");
        row.try_get("", "count").expect("operation count")
    }

    #[tokio::test]
    async fn native_sqlite_lifecycle_toggles_and_replays_with_real_foreign_keys() {
        let database = journal_and_state_database().await;
        let tenant_id = uuid::Uuid::new_v4();
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO tenants (id) VALUES (?1)",
                vec![tenant_id.into()],
            ))
            .await
            .unwrap();
        let registry = ModuleRegistry::new().register(OptionalModule);
        let catalog = ModuleDefinitionCatalog::from_static_registry(&registry).unwrap();
        let dispatcher = ModuleExecutionDispatcher::new(&catalog, &registry);
        let enable = ModuleLifecycleToggleRequest {
            tenant_id,
            module_slug: "optional_test".into(),
            enabled: true,
            context: command_context(tenant_id, uuid::Uuid::new_v4()),
            expected_revision: 0,
            recovery_source_id: None,
            effective_enabled_modules: HashSet::new(),
            ordering_enabled_modules: HashSet::new(),
            previous_override_enabled: None,
            requested_override_enabled: Some(true),
            current_settings: serde_json::json!({}),
            policy_transition: None,
            static_settings_schema: None,
        };
        let writer = crate::ModuleLifecycleDbWriter::new(database.clone(), &registry, vec![]);
        assert_eq!(
            writer
                .original_toggle_revision(tenant_id, "optional_test", true, &enable.context)
                .await
                .unwrap(),
            None
        );
        assert_eq!(
            writer
                .tenant_override_enabled(tenant_id, "optional_test")
                .await
                .unwrap(),
            None
        );
        let enabled = execute_module_toggle(&database, &dispatcher, None, enable.clone())
            .await
            .unwrap();
        assert_eq!(
            writer
                .original_toggle_revision(tenant_id, "optional_test", true, &enable.context)
                .await
                .unwrap(),
            Some(0)
        );
        assert_eq!(
            writer
                .tenant_override_enabled(tenant_id, "optional_test")
                .await
                .unwrap(),
            Some(true)
        );
        assert!(matches!(
            writer
                .original_toggle_revision(tenant_id, "optional_test", false, &enable.context)
                .await,
            Err(crate::ModuleLifecycleDbWriterError::Lifecycle(
                ModuleLifecycleExecutionError::IdempotencyConflict
            ))
        ));
        let mut foreign_actor = enable.context.clone();
        foreign_actor.actor_id = uuid::Uuid::new_v4();
        assert!(matches!(
            writer
                .original_toggle_revision(tenant_id, "optional_test", true, &foreign_actor)
                .await,
            Err(crate::ModuleLifecycleDbWriterError::Lifecycle(
                ModuleLifecycleExecutionError::IdempotencyConflict
            ))
        ));
        let operation_id = enabled.operation_id.unwrap();
        let changed = database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "UPDATE module_operations SET expected_revision = NULL WHERE id = ?1",
                vec![operation_id.into()],
            ))
            .await
            .unwrap();
        assert_eq!(changed.rows_affected(), 1);
        assert!(matches!(
            writer
                .original_toggle_revision(tenant_id, "optional_test", true, &enable.context)
                .await,
            Err(crate::ModuleLifecycleDbWriterError::Lifecycle(
                ModuleLifecycleExecutionError::Persistence(_)
            ))
        ));
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "UPDATE module_operations SET expected_revision = 0 WHERE id = ?1",
                vec![operation_id.into()],
            ))
            .await
            .unwrap();
        let replay = execute_module_toggle(&database, &dispatcher, None, enable)
            .await
            .unwrap();
        assert_eq!(enabled, replay);
        assert_eq!(enabled.revision, 1);
        assert_eq!(
            TenantModuleStateStore::read(&database, tenant_id, "optional_test")
                .await
                .unwrap()
                .unwrap()
                .enabled,
            Some(true)
        );
        let disable = ModuleLifecycleToggleRequest {
            tenant_id,
            module_slug: "optional_test".into(),
            enabled: false,
            context: command_context(tenant_id, uuid::Uuid::new_v4()),
            expected_revision: 1,
            recovery_source_id: None,
            effective_enabled_modules: HashSet::from(["optional_test".into()]),
            ordering_enabled_modules: HashSet::new(),
            previous_override_enabled: Some(true),
            requested_override_enabled: Some(false),
            current_settings: serde_json::json!({}),
            policy_transition: None,
            static_settings_schema: None,
        };
        let disabled = execute_module_toggle(&database, &dispatcher, None, disable.clone())
            .await
            .unwrap();
        assert_eq!(
            execute_module_toggle(&database, &dispatcher, None, disable)
                .await
                .unwrap(),
            disabled
        );
        assert_eq!(disabled.revision, 2);
        assert_eq!(
            TenantModuleStateStore::read(&database, tenant_id, "optional_test")
                .await
                .unwrap()
                .unwrap()
                .enabled,
            Some(false)
        );
    }

    #[tokio::test]
    async fn idempotent_no_op_toggle_replays_one_journaled_operation() {
        let database = journal_and_state_database().await;
        let registry = ModuleRegistry::new().register(OptionalModule);
        let catalog = ModuleDefinitionCatalog::from_static_registry(&registry).expect("catalog");
        let dispatcher = ModuleExecutionDispatcher::new(&catalog, &registry);
        let tenant_id = uuid::Uuid::new_v4();
        seed_tenant(&database, tenant_id).await;
        let idempotency_key = uuid::Uuid::new_v4();
        let request = ModuleLifecycleToggleRequest {
            tenant_id,
            module_slug: "optional_test".to_string(),
            enabled: true,
            context: command_context(tenant_id, idempotency_key),
            expected_revision: 0,
            recovery_source_id: None,
            effective_enabled_modules: HashSet::from(["optional_test".to_string()]),
            ordering_enabled_modules: HashSet::new(),
            previous_override_enabled: None,
            requested_override_enabled: Some(true),
            current_settings: serde_json::json!({}),
            policy_transition: None,
            static_settings_schema: None,
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
                vec![tenant_id.to_string().into(), idempotency_key.into()],
            ))
            .await
            .expect("journal count query")
            .expect("journal count row");
        let count: i64 = row.try_get("", "count").expect("journal count");
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn enable_checks_claimed_settings_and_materializes_defaults_atomically() {
        struct TrackingModule(std::sync::Arc<AtomicUsize>);
        impl MigrationSource for TrackingModule {
            fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
                vec![]
            }
        }
        #[async_trait]
        impl RusToKModule for TrackingModule {
            fn slug(&self) -> &'static str {
                "optional_test"
            }
            fn name(&self) -> &'static str {
                "Tracking"
            }
            fn description(&self) -> &'static str {
                "Lifecycle test"
            }
            fn version(&self) -> &'static str {
                "0.1.0"
            }
            async fn pre_enable(
                &self,
                _ctx: rustok_core::ModuleContext<'_>,
            ) -> rustok_core::Result<()> {
                self.0.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        }
        let database = journal_and_state_database().await;
        let calls = std::sync::Arc::new(AtomicUsize::new(0));
        let registry = ModuleRegistry::new().register(TrackingModule(calls.clone()));
        let catalog = ModuleDefinitionCatalog::from_static_registry(&registry).unwrap();
        let dispatcher = ModuleExecutionDispatcher::new(&catalog, &registry);
        let tenant_id = uuid::Uuid::new_v4();
        seed_tenant(&database, tenant_id).await;
        TenantModuleStateStore::persist_settings(
            &database,
            TenantModuleSettingsRequest {
                tenant_id,
                module_slug: "optional_test".into(),
                settings: serde_json::json!({"limit": "invalid"}),
            },
        )
        .await
        .unwrap();
        let schema = HashMap::from([(
            "limit".to_string(),
            crate::ModuleSettingSpec {
                value_type: "integer".into(),
                default: Some(serde_json::json!(5)),
                ..Default::default()
            },
        )]);
        let mut request = ModuleLifecycleToggleRequest {
            tenant_id,
            module_slug: "optional_test".into(),
            enabled: true,
            context: command_context(tenant_id, uuid::Uuid::new_v4()),
            expected_revision: 0,
            recovery_source_id: None,
            effective_enabled_modules: HashSet::new(),
            ordering_enabled_modules: HashSet::new(),
            previous_override_enabled: None,
            requested_override_enabled: Some(true),
            // A stale caller snapshot must never authorize this operation.
            current_settings: serde_json::json!({}),
            policy_transition: None,
            static_settings_schema: Some(Ok(schema)),
        };
        assert!(matches!(
            execute_module_toggle(&database, &dispatcher, None, request.clone()).await,
            Err(ModuleLifecycleExecutionError::SettingsMigrationRequired { .. })
        ));
        let available_schema = request.static_settings_schema.clone();
        request.static_settings_schema = Some(Err("schema source unavailable".into()));
        assert!(matches!(
            execute_module_toggle(&database, &dispatcher, None, request.clone()).await,
            Err(ModuleLifecycleExecutionError::SettingsMigrationRequired { .. })
        ));
        request.static_settings_schema = available_schema;
        let snapshot = StaticTenantLifecycleStore::snapshot(&database, tenant_id, "optional_test")
            .await
            .unwrap();
        assert_eq!(snapshot.revision, 0);
        assert!(snapshot.active_idempotency_key.is_none());
        assert_eq!(calls.load(Ordering::SeqCst), 0);

        TenantModuleStateStore::persist_settings(
            &database,
            TenantModuleSettingsRequest {
                tenant_id,
                module_slug: "optional_test".into(),
                settings: serde_json::json!({}),
            },
        )
        .await
        .unwrap();
        request.context.idempotency_key = uuid::Uuid::new_v4();
        let result = execute_module_toggle(&database, &dispatcher, None, request.clone())
            .await
            .unwrap();
        assert_eq!(result.settings, serde_json::json!({"limit": 5}));
        let persisted =
            load_current_lifecycle_settings(&database, &dispatcher, tenant_id, "optional_test")
                .await
                .unwrap();
        assert_eq!(persisted, result.settings);
        assert_eq!(
            execute_module_toggle(&database, &dispatcher, None, request)
                .await
                .unwrap(),
            result
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn schema_resolution_failure_blocks_new_command_but_not_exact_replay() {
        let database = journal_and_state_database().await;
        let registry = ModuleRegistry::new().register(OptionalModule);
        let catalog = ModuleDefinitionCatalog::from_static_registry(&registry).unwrap();
        let dispatcher = ModuleExecutionDispatcher::new(&catalog, &registry);
        let tenant_id = uuid::Uuid::new_v4();
        seed_tenant(&database, tenant_id).await;
        let mut request = ModuleLifecycleToggleRequest {
            tenant_id,
            module_slug: "optional_test".into(),
            enabled: true,
            context: command_context(tenant_id, uuid::Uuid::new_v4()),
            expected_revision: 0,
            recovery_source_id: None,
            effective_enabled_modules: HashSet::new(),
            ordering_enabled_modules: HashSet::new(),
            previous_override_enabled: None,
            requested_override_enabled: Some(true),
            current_settings: serde_json::json!({}),
            policy_transition: None,
            static_settings_schema: Some(Err("schema source unavailable".into())),
        };
        assert!(matches!(
            execute_module_toggle(&database, &dispatcher, None, request.clone()).await,
            Err(ModuleLifecycleExecutionError::SettingsSchemaUnavailable(_))
        ));
        assert_eq!(
            operation_count(&database, tenant_id, request.context.idempotency_key).await,
            0
        );
        let snapshot = StaticTenantLifecycleStore::snapshot(&database, tenant_id, "optional_test")
            .await
            .unwrap();
        assert_eq!(snapshot.revision, 0);
        assert!(snapshot.active_idempotency_key.is_none());

        request.static_settings_schema = Some(Ok(HashMap::new()));
        let committed = execute_module_toggle(&database, &dispatcher, None, request.clone())
            .await
            .unwrap();
        request.static_settings_schema = Some(Err("schema source unavailable".into()));
        assert_eq!(
            execute_module_toggle(&database, &dispatcher, None, request.clone())
                .await
                .unwrap(),
            committed
        );
        request.expected_revision = 1;
        assert!(matches!(
            execute_module_toggle(&database, &dispatcher, None, request).await,
            Err(ModuleLifecycleExecutionError::IdempotencyConflict)
        ));
    }

    #[tokio::test]
    #[ignore = "requires RUSTOK_MODULES_TEST_DATABASE_URL and PostgreSQL schema creation privileges"]
    async fn postgres_lifecycle_commit_waits_for_cursor_without_holding_slug() {
        use std::time::Duration;
        let db = sea_orm::Database::connect(std::env::var("RUSTOK_MODULES_TEST_DATABASE_URL").unwrap()).await.unwrap();
        let schema = format!("lifecycle_order_{}", uuid::Uuid::new_v4().simple());
        db.execute_unprepared(&format!("CREATE SCHEMA {schema}")).await.unwrap();
        let setup = crate::backend::begin_fenced_transaction(&db).await.unwrap();
        setup.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
        setup.execute_unprepared(
            "CREATE TABLE module_policy_revision_cursors (tenant_id UUID NOT NULL, consumer_key TEXT NOT NULL, current_revision TEXT, current_sequence BIGINT NOT NULL, updated_at TIMESTAMPTZ NOT NULL, PRIMARY KEY (tenant_id, consumer_key));
             CREATE TABLE module_artifact_activation_locks (scope_kind TEXT NOT NULL, scope_tenant_key TEXT NOT NULL, slug TEXT NOT NULL, PRIMARY KEY (scope_kind, scope_tenant_key, slug))"
        ).await.unwrap();
        setup.commit().await.unwrap();
        let tenant = uuid::Uuid::new_v4();
        let consumer = crate::SeaOrmModulePolicyRevisionConsumer::new(db.clone());
        let admin = crate::backend::begin_fenced_transaction(&db).await.unwrap();
        admin.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
        consumer.lock_current_revision_in_transaction(&admin, tenant, "module.lifecycle").await.unwrap();
        let worker_db = db.clone();
        let worker_schema = schema.clone();
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let mut worker = tokio::spawn(async move {
            let tx = crate::backend::begin_fenced_transaction(&worker_db).await.unwrap();
            tx.execute_unprepared(&format!("SET LOCAL search_path TO {worker_schema}")).await.unwrap();
            let coordinator = ModuleEffectivePolicyTransitionCoordinator::new(
                crate::ControlPlaneInfrastructure::default(),
                crate::SeaOrmModulePolicyRevisionConsumer::new(worker_db.clone()),
            );
            let transition = ModulePolicyRevisionTransition {
                previous_revision: None,
                next_revision: format!("sha256:{}", "a".repeat(64)),
            };
            ready_tx.send(()).unwrap();
            let result = lock_lifecycle_commit_authority(&tx, tenant, "forum", Some(&coordinator), Some(&transition)).await;
            tx.rollback().await.unwrap();
            result
        });
        ready_rx.await.unwrap();
        assert!(tokio::time::timeout(Duration::from_millis(150), &mut worker).await.is_err());
        // An admin holding the cursor must be able to take the slug while
        // the lifecycle transaction is waiting, rather than form a cycle.
        tokio::time::timeout(Duration::from_secs(3),
            crate::installation::acquire_artifact_activation_lock(&admin, &crate::ModuleInstallationScope::Platform, "forum"),
        ).await.expect("slug must not be held by cursor waiter").unwrap();
        admin.commit().await.unwrap();
        worker.await.unwrap().unwrap();
        db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE")).await.unwrap();
    }

    #[tokio::test]
    async fn lifecycle_admission_and_checkpoint_fence_share_scope() {
        let db = journal_and_state_database().await;
        let tenant = uuid::Uuid::new_v4();
        let other = uuid::Uuid::new_v4();
        seed_tenant(&db, tenant).await;
        seed_tenant(&db, other).await;
        let key = uuid::Uuid::new_v4();
        let definition = ModuleDefinition::from_static_registry_module(&OptionalModule);
        crate::lifecycle_transition_fence::claim(&db, tenant, "optional_test", &definition, 0, key)
            .await
            .unwrap();
        let tx = crate::backend::begin_fenced_transaction(&db).await.unwrap();
        crate::installation::acquire_artifact_activation_lock(
            &tx,
            &crate::ModuleInstallationScope::Platform,
            "optional_test",
        )
        .await
        .unwrap();
        assert!(
            crate::lifecycle_transition_fence::has_journal_claim(
                &tx,
                &crate::ModuleInstallationScope::Platform,
                "optional_test",
            )
            .await
            .unwrap()
        );
        assert!(
            !crate::lifecycle_transition_fence::has_journal_claim(
                &tx,
                &crate::ModuleInstallationScope::Tenant { tenant_id: other },
                "optional_test",
            )
            .await
            .unwrap()
        );
        tx.rollback().await.unwrap();
        StaticTenantLifecycleStore::release(&db, tenant, "optional_test", key)
            .await
            .unwrap();
        let now = chrono::Utc::now();
        crate::TransitionCheckpointStore::save_checkpoint(
            &db,
            &crate::ModuleTransitionCheckpoint {
                operation_id: uuid::Uuid::new_v4(),
                revision: 1,
                module_slug: "optional_test".into(),
                tenant_id: None,
                predecessor_digest: None,
                candidate_digest: "candidate".into(),
                state: crate::ModuleTransitionState::Fenced,
                security_epoch: crate::GlobalSecurityEpoch::INITIAL,
                fences: crate::ConflictFenceSet::new(vec![]),
                recovery_attempt_count: 0,
                created_at: now,
                updated_at: now,
            },
        )
        .await
        .unwrap();
        assert!(matches!(
            crate::lifecycle_transition_fence::claim(
                &db,
                tenant,
                "optional_test",
                &definition,
                0,
                uuid::Uuid::new_v4(),
            )
            .await,
            Err(crate::lifecycle_transition_fence::LifecycleAdmissionError::TransitionInProgress)
        ));
        assert!(
            StaticTenantLifecycleStore::snapshot(&db, tenant, "optional_test")
                .await
                .unwrap()
                .active_idempotency_key
                .is_none()
        );
    }

    #[tokio::test]
    async fn lifecycle_replay_survives_transition_while_new_changes_are_blocked() {
        let database = journal_and_state_database().await;
        let registry = ModuleRegistry::new().register(OptionalModule);
        let catalog = ModuleDefinitionCatalog::from_static_registry(&registry).unwrap();
        let dispatcher = ModuleExecutionDispatcher::new(&catalog, &registry);
        let tenant_id = uuid::Uuid::new_v4();
        seed_tenant(&database, tenant_id).await;
        let mut request = ModuleLifecycleToggleRequest {
            tenant_id,
            module_slug: "optional_test".into(),
            enabled: true,
            context: command_context(tenant_id, uuid::Uuid::new_v4()),
            expected_revision: 0,
            recovery_source_id: None,
            effective_enabled_modules: HashSet::from(["optional_test".into()]),
            ordering_enabled_modules: HashSet::new(),
            previous_override_enabled: None,
            requested_override_enabled: Some(true),
            current_settings: serde_json::json!({}),
            policy_transition: None,
            static_settings_schema: Some(Ok(HashMap::new())),
        };
        let committed = execute_module_toggle(&database, &dispatcher, None, request.clone())
            .await
            .unwrap();
        let now = chrono::Utc::now();
        crate::TransitionCheckpointStore::save_checkpoint(
            &database,
            &crate::ModuleTransitionCheckpoint {
                operation_id: uuid::Uuid::new_v4(),
                revision: 1,
                module_slug: "optional_test".into(),
                tenant_id: None,
                predecessor_digest: Some("predecessor".into()),
                candidate_digest: "candidate".into(),
                state: crate::ModuleTransitionState::InterventionRequired {
                    failure_reason: "recovery needed".into(),
                },
                security_epoch: crate::GlobalSecurityEpoch::INITIAL,
                fences: crate::ConflictFenceSet::new(vec![]),
                recovery_attempt_count: 0,
                created_at: now,
                updated_at: now,
            },
        )
        .await
        .unwrap();
        assert_eq!(
            execute_module_toggle(&database, &dispatcher, None, request.clone())
                .await
                .unwrap(),
            committed
        );
        request.context.idempotency_key = uuid::Uuid::new_v4();
        request.expected_revision = committed.revision;
        assert!(matches!(
            execute_module_toggle(&database, &dispatcher, None, request.clone()).await,
            Err(ModuleLifecycleExecutionError::PolicyTransition(_))
        ));
        assert!(matches!(
            execute_module_toggle(&database, &dispatcher, None, request).await,
            Err(ModuleLifecycleExecutionError::PolicyTransition(_))
        ));
        let snapshot = StaticTenantLifecycleStore::snapshot(&database, tenant_id, "optional_test")
            .await
            .unwrap();
        assert_eq!(snapshot.revision, committed.revision);
        assert!(snapshot.active_idempotency_key.is_none());
    }

    #[tokio::test]
    async fn expired_claim_rolls_back_terminal_lifecycle_completion() {
        let db = journal_and_state_database().await;
        let tenant_id = uuid::Uuid::new_v4();
        seed_tenant(&db, tenant_id).await;
        let registry = ModuleRegistry::new().register(OptionalModule);
        let catalog = ModuleDefinitionCatalog::from_static_registry(&registry).unwrap();
        let dispatcher = ModuleExecutionDispatcher::new(&catalog, &registry);
        let key = uuid::Uuid::new_v4();
        let request = ModuleLifecycleToggleRequest {
            tenant_id,
            module_slug: "optional_test".into(),
            enabled: true,
            context: command_context(tenant_id, key),
            expected_revision: 0,
            recovery_source_id: None,
            effective_enabled_modules: HashSet::new(),
            ordering_enabled_modules: HashSet::new(),
            previous_override_enabled: None,
            requested_override_enabled: Some(true),
            current_settings: serde_json::json!({}),
            policy_transition: None,
            static_settings_schema: None,
        };
        db.execute_unprepared("CREATE TRIGGER expire_claim_at_completion AFTER UPDATE OF status ON module_operations WHEN OLD.status = 'committed_hook_pending' AND NEW.status = 'committed' BEGIN UPDATE module_static_tenant_lifecycle SET active_claimed_at = '2000-01-01T00:00:00Z' WHERE module_slug = 'optional_test'; END").await.unwrap();
        assert!(matches!(
            execute_module_toggle(&db, &dispatcher, None, request).await,
            Err(ModuleLifecycleExecutionError::ClaimLost { .. })
        ));
        let row = db
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT status FROM module_operations WHERE idempotency_key = ?1",
                vec![key.into()],
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            row.try_get::<String>("", "status").unwrap(),
            "committed_hook_pending"
        );
        let snapshot = StaticTenantLifecycleStore::snapshot(&db, tenant_id, "optional_test")
            .await
            .unwrap();
        assert_eq!(snapshot.revision, 1);
        assert_eq!(snapshot.active_idempotency_key, Some(key));
    }

    #[tokio::test]
    async fn retry_and_compensation_require_original_execution_evidence() {
        struct RecoveryModule(std::sync::Arc<std::sync::Mutex<Vec<serde_json::Value>>>);
        impl MigrationSource for RecoveryModule {
            fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
                vec![]
            }
        }
        #[async_trait]
        impl RusToKModule for RecoveryModule {
            fn slug(&self) -> &'static str {
                "optional_test"
            }
            fn name(&self) -> &'static str {
                "Recovery"
            }
            fn description(&self) -> &'static str {
                "Recovery test"
            }
            fn version(&self) -> &'static str {
                "0.1.0"
            }
            async fn post_enable(
                &self,
                ctx: rustok_core::ModuleContext<'_>,
            ) -> rustok_core::Result<()> {
                let mut seen = self.0.lock().unwrap();
                seen.push(ctx.config.clone());
                if seen.len() == 1 {
                    Err(rustok_core::Error::External("temporary failure".into()))
                } else {
                    Ok(())
                }
            }
        }
        let db = journal_and_state_database().await;
        let inputs = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let registry = ModuleRegistry::new().register(RecoveryModule(inputs.clone()));
        let catalog = ModuleDefinitionCatalog::from_static_registry(&registry).unwrap();
        let dispatcher = ModuleExecutionDispatcher::new(&catalog, &registry);
        let tenant_id = uuid::Uuid::new_v4();
        seed_tenant(&db, tenant_id).await;
        let settings = serde_json::json!({"limit": 1});
        TenantModuleStateStore::persist_settings(
            &db,
            TenantModuleSettingsRequest {
                tenant_id,
                module_slug: "optional_test".into(),
                settings: settings.clone(),
            },
        )
        .await
        .unwrap();
        let toggle_key = uuid::Uuid::new_v4();
        let toggle = ModuleLifecycleToggleRequest {
            tenant_id,
            module_slug: "optional_test".into(),
            enabled: true,
            context: command_context(tenant_id, toggle_key),
            expected_revision: 0,
            recovery_source_id: None,
            effective_enabled_modules: HashSet::new(),
            ordering_enabled_modules: HashSet::new(),
            previous_override_enabled: None,
            requested_override_enabled: Some(true),
            current_settings: settings.clone(),
            policy_transition: None,
            static_settings_schema: None,
        };
        let initial = execute_module_toggle(&db, &dispatcher, None, toggle).await;
        assert!(
            matches!(initial, Err(ModuleLifecycleExecutionError::PostHook(_))),
            "unexpected initial result: {initial:?}"
        );
        let original =
            ModuleOperationJournal::failed_for_tenant(&db, tenant_id, Some("optional_test"))
                .await
                .unwrap()
                .remove(0);
        let plan = crate::recovery::module_operation_recovery_plan(&db, original.id)
            .await
            .unwrap();
        assert!(!plan.retryable && plan.hook_settings_digest_recorded);
        assert_eq!(
            plan.recommended_action,
            crate::ModuleOperationRecoveryAction::None
        );
        assert_eq!(
            StaticTenantLifecycleStore::snapshot(&db, tenant_id, "optional_test")
                .await
                .unwrap()
                .active_idempotency_key,
            Some(toggle_key)
        );
        let collision = crate::recovery::ModulePostHookRetryRequest {
            operation_id: original.id,
            context: command_context(tenant_id, toggle_key),
            expected_revision: 0,
        };
        assert!(matches!(
            crate::recovery::retry_failed_post_hook_operation(&db, &dispatcher, collision).await,
            Err(crate::ModuleOperationRecoveryError::IdempotencyConflict)
        ));

        let retry_key = uuid::Uuid::new_v4();
        let retry = crate::recovery::ModulePostHookRetryRequest {
            operation_id: original.id,
            context: command_context(tenant_id, retry_key),
            expected_revision: 1,
        };
        // Both a retained claim and a legacy/missing claim must fail closed.
        // This fixture-only release is not a production reconciliation.
        for remove_claim in [false, true] {
            if remove_claim {
                StaticTenantLifecycleStore::release(&db, tenant_id, "optional_test", toggle_key)
                    .await
                    .unwrap();
            }
            assert!(
                matches!(crate::recovery::retry_failed_post_hook_operation(&db, &dispatcher, retry.clone()).await,
                Err(crate::ModuleOperationRecoveryError::NotRetryable(reason))
                    if reason == crate::recovery::RECOVERY_EVIDENCE_UNAVAILABLE)
            );
            assert_eq!(operation_count(&db, tenant_id, retry_key).await, 0);
        }
        // A caller cannot bypass the recovery service through the public
        // executor with a new recovery_source_id either.
        let compensation_key = uuid::Uuid::new_v4();
        let compensation = ModuleLifecycleToggleRequest {
            tenant_id,
            module_slug: "optional_test".into(),
            enabled: false,
            context: command_context(tenant_id, compensation_key),
            expected_revision: 1,
            recovery_source_id: Some(original.id),
            effective_enabled_modules: HashSet::new(),
            ordering_enabled_modules: HashSet::new(),
            previous_override_enabled: Some(true),
            requested_override_enabled: None,
            current_settings: settings,
            policy_transition: None,
            static_settings_schema: None,
        };
        assert!(
            matches!(execute_module_toggle(&db, &dispatcher, None, compensation).await,
            Err(ModuleLifecycleExecutionError::PolicyTransition(reason))
                if reason == crate::recovery::RECOVERY_EVIDENCE_UNAVAILABLE)
        );
        assert_eq!(operation_count(&db, tenant_id, compensation_key).await, 0);
        assert_eq!(inputs.lock().unwrap().len(), 1);
        assert_eq!(
            ModuleOperationJournal::find(&db, original.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            ModuleOperationStatus::CommittedHookFailed
        );

        // A committed historical retry remains a read-only idempotent replay,
        // even though creating a new retry is disabled.
        let replay_key = uuid::Uuid::new_v4();
        let replay_context = command_context(tenant_id, replay_key);
        let previous = ModuleOperationJournal::record(
            &db,
            ModuleOperationRequest {
                tenant_id,
                module_slug: "optional_test".into(),
                requested_enabled: true,
                previous_effective_enabled: plan.previous_effective_enabled,
                requested_by: Some(replay_context.actor_id.to_string()),
                trace_id: Some(replay_context.trace_id.clone()),
                correlation_id: replay_context.correlation_id.to_string(),
                idempotency_key: Some(replay_key),
                recovery_source_id: Some(original.id),
                expected_revision: Some(1),
            },
        )
        .await
        .unwrap();
        ModuleOperationJournal::mark_committed(&db, previous.id)
            .await
            .unwrap();
        let replayed = crate::recovery::retry_failed_post_hook_operation(
            &db,
            &dispatcher,
            crate::recovery::ModulePostHookRetryRequest {
                operation_id: original.id,
                context: replay_context,
                expected_revision: 1,
            },
        )
        .await
        .unwrap();
        assert_eq!(replayed.id, previous.id);
        assert_eq!(inputs.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    #[ignore = "requires RUSTOK_MODULES_TEST_DATABASE_URL and PostgreSQL temporary table privileges"]
    async fn postgres_post_hook_recovery_fails_closed_without_execution_evidence() {
        struct PgRecoveryModule(std::sync::Arc<AtomicUsize>);
        impl MigrationSource for PgRecoveryModule {
            fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
                vec![]
            }
        }
        #[async_trait]
        impl RusToKModule for PgRecoveryModule {
            fn slug(&self) -> &'static str {
                "optional_test"
            }
            fn name(&self) -> &'static str {
                "PostgreSQL recovery test"
            }
            fn description(&self) -> &'static str {
                "PostgreSQL lifecycle recovery fixture"
            }
            fn version(&self) -> &'static str {
                "0.1.0"
            }
            async fn post_enable(
                &self,
                _ctx: rustok_core::ModuleContext<'_>,
            ) -> rustok_core::Result<()> {
                if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
                    Err(rustok_core::Error::External("temporary failure".into()))
                } else {
                    Ok(())
                }
            }
        }
        let url = std::env::var("RUSTOK_MODULES_TEST_DATABASE_URL")
            .expect("RUSTOK_MODULES_TEST_DATABASE_URL is required");
        let mut options = sea_orm::ConnectOptions::new(url);
        options
            .max_connections(1)
            .min_connections(1)
            .sqlx_logging(false);
        let db = Database::connect(options).await.unwrap();
        assert_eq!(db.get_database_backend(), DbBackend::Postgres);
        for sql in [
            "CREATE TEMP TABLE module_operations (id UUID PRIMARY KEY, tenant_id UUID NOT NULL, module_slug TEXT NOT NULL, requested_enabled BOOLEAN NOT NULL, previous_effective_enabled BOOLEAN NOT NULL, status TEXT NOT NULL, requested_by TEXT, trace_id TEXT, correlation_id TEXT, idempotency_key UUID, expected_revision BIGINT, error_message TEXT, result_snapshot TEXT, created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW())",
            "CREATE UNIQUE INDEX ON module_operations (tenant_id, idempotency_key) WHERE idempotency_key IS NOT NULL",
            "CREATE TEMP TABLE module_operation_override_states (operation_id UUID PRIMARY KEY, previous_override_enabled BOOLEAN, requested_override_enabled BOOLEAN)",
            "CREATE TEMP TABLE tenant_modules (id UUID PRIMARY KEY, tenant_id UUID NOT NULL, module_slug TEXT NOT NULL, enabled BOOLEAN, settings JSONB NOT NULL DEFAULT '{}'::jsonb, created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), UNIQUE (tenant_id, module_slug))",
            "CREATE TEMP TABLE module_static_tenant_lifecycle (tenant_id UUID NOT NULL, module_slug TEXT NOT NULL, revision BIGINT NOT NULL, active_idempotency_key UUID, active_claimed_at TIMESTAMPTZ, claim_owner TEXT NOT NULL DEFAULT 'journal', created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), PRIMARY KEY (tenant_id, module_slug))",
            "CREATE TEMP TABLE module_transition_checkpoints (operation_id UUID PRIMARY KEY, revision BIGINT NOT NULL, module_slug TEXT NOT NULL, tenant_id UUID, predecessor_digest TEXT, candidate_digest TEXT NOT NULL, state JSONB NOT NULL, security_epoch BIGINT NOT NULL, fences JSONB NOT NULL, recovery_attempt_count INTEGER NOT NULL, created_at TIMESTAMPTZ NOT NULL, updated_at TIMESTAMPTZ NOT NULL)",
            "CREATE TEMP TABLE module_artifact_activation_locks (scope_kind TEXT NOT NULL, scope_tenant_key TEXT NOT NULL, slug TEXT NOT NULL, PRIMARY KEY (scope_kind, scope_tenant_key, slug))",
        ] {
            db.execute_unprepared(sql).await.unwrap();
        }
        let migration = crate::migrations::migrations()
            .into_iter()
            .find(|migration| migration.name() == "m20261030_000089_lifecycle_hook_settings")
            .expect("recovery evidence migration is registered");
        migration
            .up(&sea_orm_migration::SchemaManager::new(&db))
            .await
            .unwrap();
        let calls = std::sync::Arc::new(AtomicUsize::new(0));
        let registry = ModuleRegistry::new().register(PgRecoveryModule(calls.clone()));
        let catalog = ModuleDefinitionCatalog::from_static_registry(&registry).unwrap();
        let dispatcher = ModuleExecutionDispatcher::new(&catalog, &registry);
        let tenant_id = uuid::Uuid::new_v4();
        let settings = serde_json::json!({"limit": 1});
        TenantModuleStateStore::persist_settings(
            &db,
            TenantModuleSettingsRequest {
                tenant_id,
                module_slug: "optional_test".into(),
                settings: settings.clone(),
            },
        )
        .await
        .unwrap();
        let toggle_key = uuid::Uuid::new_v4();
        let toggle = ModuleLifecycleToggleRequest {
            tenant_id,
            module_slug: "optional_test".into(),
            enabled: true,
            context: command_context(tenant_id, toggle_key),
            expected_revision: 0,
            recovery_source_id: None,
            effective_enabled_modules: HashSet::new(),
            ordering_enabled_modules: HashSet::new(),
            previous_override_enabled: None,
            requested_override_enabled: Some(true),
            current_settings: settings.clone(),
            policy_transition: None,
            static_settings_schema: None,
        };
        let initial = execute_module_toggle(&db, &dispatcher, None, toggle).await;
        assert!(
            matches!(initial, Err(ModuleLifecycleExecutionError::PostHook(_))),
            "unexpected initial result: {initial:?}"
        );
        let original =
            ModuleOperationJournal::failed_for_tenant(&db, tenant_id, Some("optional_test"))
                .await
                .unwrap()
                .remove(0);
        assert_eq!(
            StaticTenantLifecycleStore::snapshot(&db, tenant_id, "optional_test")
                .await
                .unwrap()
                .active_idempotency_key,
            Some(toggle_key)
        );
        let retry = crate::recovery::ModulePostHookRetryRequest {
            operation_id: original.id,
            context: command_context(tenant_id, uuid::Uuid::new_v4()),
            expected_revision: 1,
        };
        for remove_claim in [false, true] {
            if remove_claim {
                // Fixture only: absence of a claim is not evidence of quiescence.
                StaticTenantLifecycleStore::release(&db, tenant_id, "optional_test", toggle_key)
                    .await
                    .unwrap();
            }
            assert!(
                matches!(crate::recovery::retry_failed_post_hook_operation(&db, &dispatcher, retry.clone()).await,
                Err(crate::ModuleOperationRecoveryError::NotRetryable(reason))
                    if reason == crate::recovery::RECOVERY_EVIDENCE_UNAVAILABLE)
            );
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            ModuleOperationJournal::find(&db, original.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            ModuleOperationStatus::CommittedHookFailed
        );
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
        seed_tenant(&database, tenant_id).await;

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
                recovery_source_id: None,
                effective_enabled_modules: HashSet::new(),
                ordering_enabled_modules: HashSet::new(),
                previous_override_enabled: None,
                requested_override_enabled: Some(true),
                current_settings: serde_json::json!({}),
                policy_transition: None,
                static_settings_schema: None,
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
        seed_tenant(&database, tenant_id).await;
        let holder_key = uuid::Uuid::new_v4();
        StaticTenantLifecycleStore::claim(&database, tenant_id, "optional_test", 0, holder_key)
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
                recovery_source_id: None,
                effective_enabled_modules: HashSet::new(),
                ordering_enabled_modules: HashSet::new(),
                previous_override_enabled: None,
                requested_override_enabled: Some(true),
                current_settings: serde_json::json!({}),
                policy_transition: None,
                static_settings_schema: None,
            },
        )
        .await;
        assert!(matches!(
            contended,
            Err(ModuleLifecycleExecutionError::OperationInProgress { .. })
        ));
        assert_eq!(
            operation_count(&database, tenant_id, contender_key).await,
            0
        );

        StaticTenantLifecycleStore::release(&database, tenant_id, "optional_test", holder_key)
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
                recovery_source_id: None,
                effective_enabled_modules: HashSet::new(),
                ordering_enabled_modules: HashSet::new(),
                previous_override_enabled: None,
                requested_override_enabled: Some(true),
                current_settings: serde_json::json!({}),
                policy_transition: None,
                static_settings_schema: None,
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
        seed_tenant(&database, tenant_id).await;
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
                recovery_source_id: None,
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
                recovery_source_id: None,
                effective_enabled_modules: HashSet::new(),
                ordering_enabled_modules: HashSet::new(),
                previous_override_enabled: None,
                requested_override_enabled: Some(true),
                current_settings: serde_json::json!({}),
                policy_transition: None,
                static_settings_schema: None,
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
        seed_tenant(&database, tenant_id).await;
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
                recovery_source_id: None,
                effective_enabled_modules: HashSet::from(["optional_test".to_string()]),
                ordering_enabled_modules: HashSet::new(),
                previous_override_enabled: Some(true),
                requested_override_enabled: Some(true),
                current_settings: serde_json::json!({ "stale": true }),
                policy_transition: None,
                static_settings_schema: None,
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
        seed_tenant(&database, tenant_id).await;
        let wrong_schema_key = uuid::Uuid::new_v4();
        let wrong_schema = execute_module_toggle(
            &database,
            &dispatcher,
            None,
            ModuleLifecycleToggleRequest {
                tenant_id,
                module_slug: "artifact_test".into(),
                enabled: true,
                context: command_context(tenant_id, wrong_schema_key),
                expected_revision: 0,
                recovery_source_id: None,
                effective_enabled_modules: HashSet::new(),
                ordering_enabled_modules: HashSet::new(),
                previous_override_enabled: None,
                requested_override_enabled: Some(true),
                current_settings: serde_json::json!({}),
                policy_transition: None,
                static_settings_schema: Some(Ok(HashMap::new())),
            },
        )
        .await;
        assert!(matches!(
            wrong_schema,
            Err(ModuleLifecycleExecutionError::Persistence(_))
        ));
        assert_eq!(
            operation_count(&database, tenant_id, wrong_schema_key).await,
            0
        );
        let holder_key = uuid::Uuid::new_v4();
        StaticTenantLifecycleStore::claim(&database, tenant_id, "artifact_test", 0, holder_key)
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
                recovery_source_id: None,
                effective_enabled_modules: HashSet::new(),
                ordering_enabled_modules: HashSet::new(),
                previous_override_enabled: None,
                requested_override_enabled: Some(true),
                current_settings: serde_json::json!({}),
                policy_transition: None,
                static_settings_schema: None,
            },
        )
        .await;
        assert!(matches!(
            contended,
            Err(ModuleLifecycleExecutionError::OperationInProgress { .. })
        ));
        assert_eq!(artifact_executor.0.load(Ordering::SeqCst), 0);
        assert_eq!(
            operation_count(&database, tenant_id, contender_key).await,
            0
        );

        StaticTenantLifecycleStore::release(&database, tenant_id, "artifact_test", holder_key)
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
                recovery_source_id: None,
                effective_enabled_modules: HashSet::new(),
                ordering_enabled_modules: HashSet::new(),
                previous_override_enabled: None,
                requested_override_enabled: Some(true),
                current_settings: serde_json::json!({}),
                policy_transition: None,
                static_settings_schema: None,
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
