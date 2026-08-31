use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use thiserror::Error;
use uuid::Uuid;

use crate::policy::{
    ModuleEffectivePolicyChannelInput, ModuleEffectivePolicyCoRequisite,
    ModuleEffectivePolicyInstallationFact, ModuleEffectivePolicyMaintenanceInput,
    ModuleEffectivePolicyQuery, ModuleEffectivePolicyRuntimeInput,
};
use crate::recovery::{
    failed_module_operation_recovery_plans, module_operation_recovery_plan,
    retry_failed_post_hook_operation,
};
use crate::{
    ArtifactInstallationResolver, ArtifactLifecycleExecutor, ArtifactSandboxPolicyResolver,
    ControlPlaneInfrastructure, ModuleCommandContext, ModuleDefinitionCatalog,
    ModuleDefinitionError, ModuleDefinitionKind, ModuleDefinitionSource, ModuleEffectivePolicy,
    ModuleEffectivePolicyError, ModuleEffectivePolicyTransitionCoordinator,
    ModuleExecutionDispatcher, ModuleLifecycleExecutionError, ModuleLifecycleToggleRequest,
    ModuleOperationIssue, ModuleOperationJournal, ModuleOperationRecoveryError,
    ModuleOperationRecoveryPlan, ModuleOperationRequest, ModuleOperationStoreError,
    ModulePolicyRevisionTransition, ModulePostHookRetryRequest, SeaOrmArtifactInstallationStore,
    SeaOrmArtifactSandboxPolicyResolver, SeaOrmModuleArtifactSecurityResolver,
    SeaOrmModulePolicyRevisionConsumer, StaticTenantLifecycleSnapshot, StaticTenantLifecycleStore,
    StaticTenantLifecycleStoreError, TenantModuleOverride, TenantModuleSettingsRecord,
    TenantModuleSettingsRequest, TenantModuleStateStore,
    artifact_schema::ArtifactSchemaValidatorCache,
    artifact_settings::{self, ArtifactSettingsStoreError},
    execute_module_toggle,
};
use rustok_api::PortError;
use rustok_core::ModuleRegistry;
use rustok_outbox::idempotency::{self, Admission};

/// Database-backed adapter for module lifecycle execution in a host composition.
///
/// The caller supplies the selected distribution registry and its declared
/// defaults; this adapter owns the durable override read and lifecycle write.
pub struct ModuleLifecycleDbWriter<'a> {
    db: DatabaseConnection,
    infrastructure: ControlPlaneInfrastructure,
    catalog: Option<ModuleDefinitionCatalog>,
    static_registry: Option<&'a ModuleRegistry>,
    artifact_executor: Option<&'a dyn ArtifactLifecycleExecutor>,
    default_enabled_modules: Vec<String>,
    co_requisites: Vec<ModuleEffectivePolicyCoRequisite>,
    settings_schema_validators: ArtifactSchemaValidatorCache,
}

/// Owner-owned view of one explicit tenant module override. Effective
/// availability remains a separate `ModuleEffectivePolicy` decision.
#[derive(Clone, Debug, PartialEq)]
pub struct TenantModuleOverrideSnapshot {
    pub module_slug: String,
    pub enabled: bool,
    pub settings: serde_json::Value,
}

/// Authenticated, replayable command for one platform-native tenant lifecycle
/// transition. Hosts derive the complete tenant-matched context from the
/// authenticated request; callers never supply separate actor, trace,
/// correlation, or idempotency values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleLifecycleToggleCommand {
    pub tenant_id: Uuid,
    pub module_slug: String,
    pub enabled: bool,
    pub context: ModuleCommandContext,
    pub expected_revision: u64,
}

/// Authenticated, replayable command for a post-hook retry or compensation.
/// Hosts derive the tenant-matched command context; the owner binds its
/// persisted audit evidence to that context and never accepts caller-provided
/// display text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleLifecycleRecoveryCommand {
    pub tenant_id: Uuid,
    pub operation_id: Uuid,
    pub context: ModuleCommandContext,
    pub expected_revision: u64,
}

/// Authenticated static-module settings command. The normalized settings and
/// every concurrency identity participate in its durable receipt fingerprint.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleLifecycleSettingsCommand {
    pub tenant_id: Uuid,
    pub module_slug: String,
    pub settings: serde_json::Value,
    pub context: ModuleCommandContext,
    pub expected_revision: u64,
    /// Reviewed automation supplies the exact prior snapshot. The ordinary
    /// editor leaves both values absent and relies on the aggregate revision.
    pub expected_enabled: Option<bool>,
    pub expected_settings: Option<serde_json::Value>,
}

/// Immutable response retained in the owner-operation receipt ledger.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleLifecycleSettingsResult {
    pub module_slug: String,
    pub enabled: bool,
    pub settings: serde_json::Value,
    pub revision: u64,
}

#[derive(Serialize)]
struct ModuleLifecycleSettingsReceiptRequest<'a> {
    context: &'a ModuleCommandContext,
    expected_revision: u64,
    module_slug: &'a str,
    settings: &'a serde_json::Value,
    expected_enabled: Option<bool>,
    expected_settings: Option<&'a serde_json::Value>,
}

const STATIC_LIFECYCLE_OWNER_SLUG: &str = "modules.static_lifecycle";
const STATIC_LIFECYCLE_SETTINGS_OPERATION: &str = "settings";

struct OverrideOperationRequest<'a> {
    tenant_id: Uuid,
    module_slug: &'a str,
    enabled: bool,
    requested_override_enabled: Option<bool>,
    requested_by: Option<String>,
    trace_id: Option<String>,
    correlation_id: Option<String>,
    idempotency_key: Option<Uuid>,
    expected_revision: Option<u64>,
}

impl<'a> ModuleLifecycleDbWriter<'a> {
    pub fn new(
        db: DatabaseConnection,
        registry: &'a ModuleRegistry,
        default_enabled_modules: Vec<String>,
    ) -> Self {
        Self::with_infrastructure(
            db,
            registry,
            default_enabled_modules,
            ControlPlaneInfrastructure::default(),
        )
    }

    pub fn with_infrastructure(
        db: DatabaseConnection,
        registry: &'a ModuleRegistry,
        default_enabled_modules: Vec<String>,
        infrastructure: ControlPlaneInfrastructure,
    ) -> Self {
        Self {
            db,
            infrastructure,
            catalog: None,
            static_registry: Some(registry),
            artifact_executor: None,
            default_enabled_modules,
            co_requisites: Vec::new(),
            settings_schema_validators: ArtifactSchemaValidatorCache::default(),
        }
    }

    /// Creates the lifecycle owner for a verified native distribution. The
    /// supplied catalog carries the exact promoted release identities while
    /// the compiled registry remains the only implementation handle source.
    pub fn static_distribution_with_infrastructure(
        db: DatabaseConnection,
        catalog: ModuleDefinitionCatalog,
        registry: &'a ModuleRegistry,
        default_enabled_modules: Vec<String>,
        infrastructure: ControlPlaneInfrastructure,
    ) -> Self {
        Self {
            db,
            infrastructure,
            catalog: Some(catalog),
            static_registry: Some(registry),
            artifact_executor: None,
            default_enabled_modules,
            co_requisites: Vec::new(),
            settings_schema_validators: ArtifactSchemaValidatorCache::default(),
        }
    }

    /// Creates a lifecycle writer for an artifact-only composition. It has no
    /// compiled registry fallback: hooks dispatch through the admitted runtime
    /// executor supplied by the host composition.
    pub fn artifact_only(
        db: DatabaseConnection,
        catalog: ModuleDefinitionCatalog,
        artifact_executor: &'a dyn ArtifactLifecycleExecutor,
        default_enabled_modules: Vec<String>,
    ) -> Self {
        Self::artifact_only_with_infrastructure(
            db,
            catalog,
            artifact_executor,
            default_enabled_modules,
            ControlPlaneInfrastructure::default(),
        )
    }

    pub fn artifact_only_with_infrastructure(
        db: DatabaseConnection,
        catalog: ModuleDefinitionCatalog,
        artifact_executor: &'a dyn ArtifactLifecycleExecutor,
        default_enabled_modules: Vec<String>,
        infrastructure: ControlPlaneInfrastructure,
    ) -> Self {
        Self {
            db,
            infrastructure,
            catalog: Some(catalog),
            static_registry: None,
            artifact_executor: Some(artifact_executor),
            default_enabled_modules,
            co_requisites: Vec::new(),
            settings_schema_validators: ArtifactSchemaValidatorCache::default(),
        }
    }

    /// Adds deployment-admitted availability constraints to every effective
    /// policy resolved by this lifecycle owner. The host-facing shape is only
    /// a normalized selection map; the owner converts it to its typed policy
    /// input without creating dependency or migration-order edges.
    pub fn with_corequisites(
        mut self,
        co_requisites: BTreeMap<String, BTreeMap<String, String>>,
    ) -> Self {
        self.co_requisites = co_requisites
            .into_iter()
            .flat_map(|(module_slug, required_modules)| {
                required_modules.into_iter().map(
                    move |(required_module_slug, version_requirement)| {
                        ModuleEffectivePolicyCoRequisite {
                            module_slug: module_slug.clone(),
                            required_module_slug,
                            version_requirement,
                        }
                    },
                )
            })
            .collect();
        self
    }

    pub async fn toggle(
        &self,
        command: ModuleLifecycleToggleCommand,
    ) -> Result<crate::ModuleLifecycleToggleResult, ModuleLifecycleDbWriterError> {
        if command.tenant_id.is_nil() || command.context.tenant_id != Some(command.tenant_id) {
            return Err(ModuleLifecycleDbWriterError::Lifecycle(
                ModuleLifecycleExecutionError::InvalidCommandIdentity,
            ));
        }
        if command.context.validate().is_err() {
            return Err(ModuleLifecycleDbWriterError::Lifecycle(
                ModuleLifecycleExecutionError::InvalidIdempotencyKey,
            ));
        }
        self.apply_override_with_operation_context(OverrideOperationRequest {
            tenant_id: command.tenant_id,
            module_slug: &command.module_slug,
            enabled: command.enabled,
            requested_override_enabled: Some(command.enabled),
            requested_by: Some(command.context.actor_id.to_string()),
            trace_id: Some(command.context.trace_id.clone()),
            correlation_id: Some(command.context.correlation_id.to_string()),
            idempotency_key: Some(command.context.idempotency_key),
            expected_revision: Some(command.expected_revision),
        })
        .await
    }

    async fn apply_override_with_operation_context(
        &self,
        request: OverrideOperationRequest<'_>,
    ) -> Result<crate::ModuleLifecycleToggleResult, ModuleLifecycleDbWriterError> {
        let (
            catalog,
            effective_enabled_modules,
            ordering_enabled_modules,
            previous_override_enabled,
            current_settings,
            policy_transition,
        ) = self
            .override_execution_context(
                request.tenant_id,
                request.module_slug,
                request.requested_override_enabled,
            )
            .await?;
        let dispatcher = match (self.static_registry, self.artifact_executor) {
            (Some(registry), Some(executor)) => {
                ModuleExecutionDispatcher::new(&catalog, registry).with_artifact_executor(executor)
            }
            (Some(registry), None) => ModuleExecutionDispatcher::new(&catalog, registry),
            (None, Some(executor)) => ModuleExecutionDispatcher::artifact_only(&catalog, executor),
            (None, None) => {
                return Err(ModuleLifecycleDbWriterError::Configuration(
                    "artifact lifecycle writer has no runtime executor".into(),
                ));
            }
        };
        let static_lifecycle = catalog.get(request.module_slug).is_some_and(|definition| {
            matches!(
                &definition.source,
                ModuleDefinitionSource::PlatformNative { .. }
                    | ModuleDefinitionSource::PromotedNative { .. }
            )
        });
        execute_module_toggle(
            &self.infrastructure,
            &self.db,
            &dispatcher,
            Some(ModuleEffectivePolicyTransitionCoordinator::new(
                self.infrastructure.clone(),
                SeaOrmModulePolicyRevisionConsumer::new(self.db.clone()),
            )),
            ModuleLifecycleToggleRequest {
                tenant_id: request.tenant_id,
                module_slug: request.module_slug.to_string(),
                enabled: request.enabled,
                requested_by: request.requested_by,
                trace_id: request.trace_id,
                correlation_id: request.correlation_id,
                idempotency_key: request.idempotency_key,
                expected_revision: request.expected_revision,
                static_lifecycle,
                effective_enabled_modules,
                ordering_enabled_modules,
                previous_override_enabled,
                requested_override_enabled: request.requested_override_enabled,
                current_settings,
                policy_transition,
            },
        )
        .await
        .map_err(ModuleLifecycleDbWriterError::Lifecycle)
    }

    /// Returns one recovery plan only when it belongs to the authenticated
    /// tenant. Hosts must not load a plan globally and filter it after reading
    /// owner state.
    pub async fn recovery_plan(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
    ) -> Result<ModuleOperationRecoveryPlan, ModuleLifecycleDbWriterError> {
        let plan = module_operation_recovery_plan(&self.db, operation_id)
            .await
            .map_err(ModuleLifecycleDbWriterError::Recovery)?;
        if plan.tenant_id != tenant_id {
            return Err(ModuleLifecycleDbWriterError::Recovery(
                ModuleOperationRecoveryError::OperationNotFound,
            ));
        }
        Ok(plan)
    }

    /// Returns failed recovery plans for the authenticated tenant from the
    /// lifecycle owner journal.
    pub async fn failed_recovery_plans(
        &self,
        tenant_id: Uuid,
        module_slug: Option<&str>,
    ) -> Result<Vec<ModuleOperationRecoveryPlan>, ModuleLifecycleDbWriterError> {
        failed_module_operation_recovery_plans(&self.db, tenant_id, module_slug)
            .await
            .map_err(ModuleLifecycleDbWriterError::Recovery)
    }

    /// Retries only a post-hook failure using the exact persisted tenant
    /// override state committed by the original operation. Serving availability
    /// may differ while Product co-requisites are staged and is not a retry gate.
    pub async fn retry_post_hook(
        &self,
        command: ModuleLifecycleRecoveryCommand,
    ) -> Result<ModuleOperationRecoveryPlan, ModuleLifecycleDbWriterError> {
        validate_recovery_command(&command)?;
        let plan = self
            .recovery_plan(command.tenant_id, command.operation_id)
            .await?;
        let (catalog, current_override_enabled, current_settings) = self
            .recovery_execution_context(plan.tenant_id, &plan.module_slug)
            .await?;
        let static_lifecycle = catalog.get(&plan.module_slug).is_some_and(|definition| {
            matches!(
                &definition.source,
                ModuleDefinitionSource::PlatformNative { .. }
                    | ModuleDefinitionSource::PromotedNative { .. }
            )
        });
        let replay_request = ModuleOperationRequest {
            tenant_id: plan.tenant_id,
            module_slug: plan.module_slug.clone(),
            requested_enabled: plan.requested_enabled,
            previous_effective_enabled: plan.previous_effective_enabled,
            requested_by: Some(command.context.actor_id.to_string()),
            trace_id: Some(command.context.trace_id.clone()),
            correlation_id: command.context.correlation_id.to_string(),
            idempotency_key: Some(command.context.idempotency_key),
            expected_revision: static_lifecycle.then_some(command.expected_revision),
        };
        if let Some(operation) =
            ModuleOperationJournal::replay_idempotent(&self.db, &replay_request)
                .await
                .map_err(map_idempotency_command_error)?
        {
            return module_operation_recovery_plan(&self.db, operation.id)
                .await
                .map_err(ModuleLifecycleDbWriterError::Recovery);
        }
        let dispatcher = match (self.static_registry, self.artifact_executor) {
            (Some(registry), Some(executor)) => {
                ModuleExecutionDispatcher::new(&catalog, registry).with_artifact_executor(executor)
            }
            (Some(registry), None) => ModuleExecutionDispatcher::new(&catalog, registry),
            (None, Some(executor)) => ModuleExecutionDispatcher::artifact_only(&catalog, executor),
            (None, None) => {
                return Err(ModuleLifecycleDbWriterError::Configuration(
                    "artifact lifecycle writer has no runtime executor".into(),
                ));
            }
        };
        if static_lifecycle {
            StaticTenantLifecycleStore::claim(
                &self.db,
                command.tenant_id,
                &plan.module_slug,
                command.expected_revision,
                command.context.idempotency_key,
            )
            .await
            .map_err(map_static_lifecycle_recovery_error)?;
        }
        let retry_result = retry_failed_post_hook_operation(
            &self.db,
            &dispatcher,
            ModulePostHookRetryRequest {
                operation_id: command.operation_id,
                requested_by: Some(command.context.actor_id.to_string()),
                trace_id: Some(command.context.trace_id.clone()),
                idempotency_key: command.context.idempotency_key,
                expected_revision: static_lifecycle.then_some(command.expected_revision),
                current_override_enabled,
                current_settings,
            },
        )
        .await;
        let release_result = if static_lifecycle {
            StaticTenantLifecycleStore::release(
                &self.db,
                command.tenant_id,
                &plan.module_slug,
                command.context.idempotency_key,
            )
            .await
            .map_err(map_static_lifecycle_recovery_error)
        } else {
            Ok(())
        };
        release_result?;
        let operation = retry_result.map_err(ModuleLifecycleDbWriterError::Recovery)?;
        module_operation_recovery_plan(&self.db, operation.id)
            .await
            .map_err(ModuleLifecycleDbWriterError::Recovery)
    }

    /// Compensates a post-hook failure only while the exact committed tenant
    /// override still matches the original requested intent. The reverse
    /// transition restores the original explicit override, including removing
    /// the row when the predecessor was inherited/default selection.
    pub async fn compensate_failed_operation(
        &self,
        command: ModuleLifecycleRecoveryCommand,
    ) -> Result<crate::ModuleLifecycleToggleResult, ModuleLifecycleDbWriterError> {
        validate_recovery_command(&command)?;
        let plan = self
            .recovery_plan(command.tenant_id, command.operation_id)
            .await?;
        if plan.issue != ModuleOperationIssue::PostHookFailed {
            return Err(ModuleLifecycleDbWriterError::Recovery(
                ModuleOperationRecoveryError::NotRetryable(plan.issue.as_str().to_string()),
            ));
        }
        if !plan.override_state_recorded {
            return Err(ModuleLifecycleDbWriterError::Recovery(
                ModuleOperationRecoveryError::NotRetryable(
                    "selected_intent_state_unavailable".to_string(),
                ),
            ));
        }
        let (_, current_override_enabled, effective_enabled_modules, _) = self
            .recovery_policy_context(plan.tenant_id, &plan.module_slug)
            .await?;
        let current_effective_enabled = effective_enabled_modules.contains(&plan.module_slug);
        let reverse_enabled = !plan.requested_enabled;
        let requested_by = Some(command.context.actor_id.to_string());
        let replay_request = ModuleOperationRequest {
            tenant_id: plan.tenant_id,
            module_slug: plan.module_slug.clone(),
            requested_enabled: reverse_enabled,
            previous_effective_enabled: current_effective_enabled,
            requested_by: requested_by.clone(),
            trace_id: Some(command.context.trace_id.clone()),
            correlation_id: command.context.correlation_id.to_string(),
            idempotency_key: Some(command.context.idempotency_key),
            expected_revision: Some(command.expected_revision),
        };
        if ModuleOperationJournal::replay_idempotent_command(&self.db, &replay_request)
            .await
            .map_err(map_idempotency_command_error)?
            .is_some()
        {
            return self
                .apply_override_with_operation_context(OverrideOperationRequest {
                    tenant_id: plan.tenant_id,
                    module_slug: &plan.module_slug,
                    enabled: reverse_enabled,
                    requested_override_enabled: plan.previous_override_enabled,
                    requested_by,
                    trace_id: Some(command.context.trace_id.clone()),
                    correlation_id: Some(command.context.correlation_id.to_string()),
                    idempotency_key: Some(command.context.idempotency_key),
                    expected_revision: Some(command.expected_revision),
                })
                .await;
        }
        if current_override_enabled != plan.requested_override_enabled {
            return Err(ModuleLifecycleDbWriterError::Recovery(
                ModuleOperationRecoveryError::StateMismatch {
                    requested_override_enabled: plan.requested_override_enabled,
                    current_override_enabled,
                },
            ));
        }
        self.apply_override_with_operation_context(OverrideOperationRequest {
            tenant_id: plan.tenant_id,
            module_slug: &plan.module_slug,
            enabled: reverse_enabled,
            requested_override_enabled: plan.previous_override_enabled,
            requested_by,
            trace_id: Some(command.context.trace_id.clone()),
            correlation_id: Some(command.context.correlation_id.to_string()),
            idempotency_key: Some(command.context.idempotency_key),
            expected_revision: Some(command.expected_revision),
        })
        .await
    }

    /// Applies trusted, normalized static-module settings through the same
    /// tenant/module aggregate used by enablement and recovery. This owner
    /// admits the request before claiming the aggregate, advances the revision
    /// in the settings transaction, and persists its exact result for replay.
    pub async fn update_static_normalized_settings(
        &self,
        command: ModuleLifecycleSettingsCommand,
    ) -> Result<ModuleLifecycleSettingsResult, ModuleLifecycleDbWriterError> {
        validate_settings_command(&command)?;
        let catalog = self.definition_catalog()?;
        let definition = catalog.get(&command.module_slug).ok_or_else(|| {
            ModuleLifecycleDbWriterError::UnknownModule(command.module_slug.clone())
        })?;
        if !matches!(
            &definition.source,
            ModuleDefinitionSource::PlatformNative { .. }
                | ModuleDefinitionSource::PromotedNative { .. }
        ) {
            return Err(ModuleLifecycleDbWriterError::ArtifactSettings {
                module_slug: command.module_slug,
                reason: "artifact settings must use owner-resolved admitted schema validation",
            });
        }
        let receipt_request = ModuleLifecycleSettingsReceiptRequest {
            context: &command.context,
            expected_revision: command.expected_revision,
            module_slug: &command.module_slug,
            settings: &command.settings,
            expected_enabled: command.expected_enabled,
            expected_settings: command.expected_settings.as_ref(),
        };
        let lease = match idempotency::admit(
            &self.db,
            idempotency::OwnerOperationScope::Tenant(command.tenant_id),
            STATIC_LIFECYCLE_OWNER_SLUG,
            &command.context.idempotency_key.to_string(),
            STATIC_LIFECYCLE_SETTINGS_OPERATION,
            &receipt_request,
        )
        .await
        .map_err(ModuleLifecycleDbWriterError::OperationReceipt)?
        {
            Admission::Replay(value) => {
                return serde_json::from_value(value).map_err(|error| {
                    ModuleLifecycleDbWriterError::OperationReceipt(PortError::invariant_violation(
                        "modules.static_lifecycle_settings_receipt_corrupt",
                        error.to_string(),
                    ))
                });
            }
            Admission::ReplayError(error) => {
                return Err(ModuleLifecycleDbWriterError::OperationReceipt(error));
            }
            Admission::Run(lease) => {
                let claim = StaticTenantLifecycleStore::claim(
                    &self.db,
                    command.tenant_id,
                    &command.module_slug,
                    command.expected_revision,
                    command.context.idempotency_key,
                )
                .await
                .map_err(map_static_lifecycle_settings_error);
                if let Err(error) = claim {
                    self.fail_static_settings_operation(lease, &error).await?;
                    return Err(error);
                }
                lease
            }
        };

        let current_override = match self.overrides(command.tenant_id).await {
            Ok(overrides) => overrides
                .into_iter()
                .find(|override_state| override_state.module_slug == command.module_slug),
            Err(error) => {
                self.abandon_static_settings_operation(
                    lease,
                    command.tenant_id,
                    &command.module_slug,
                    command.context.idempotency_key,
                    &error,
                )
                .await?;
                return Err(error);
            }
        };
        let current_enabled = current_override.as_ref().map(|state| state.enabled);
        let current_settings = match self.settings(command.tenant_id, &command.module_slug).await {
            Ok(settings) => settings,
            Err(error) => {
                self.abandon_static_settings_operation(
                    lease,
                    command.tenant_id,
                    &command.module_slug,
                    command.context.idempotency_key,
                    &error,
                )
                .await?;
                return Err(error);
            }
        };
        if let (Some(expected_enabled), Some(expected_settings)) =
            (command.expected_enabled, command.expected_settings.as_ref())
            && (current_enabled != Some(expected_enabled) || current_settings != *expected_settings)
        {
            let error = ModuleLifecycleDbWriterError::SettingsSnapshotConflict;
            self.abandon_static_settings_operation(
                lease,
                command.tenant_id,
                &command.module_slug,
                command.context.idempotency_key,
                &error,
            )
            .await?;
            return Err(error);
        }

        let effective_enabled_modules =
            match self.effective_enabled_modules(command.tenant_id).await {
                Ok(enabled_modules) => enabled_modules,
                Err(error) => {
                    self.abandon_static_settings_operation(
                        lease,
                        command.tenant_id,
                        &command.module_slug,
                        command.context.idempotency_key,
                        &error,
                    )
                    .await?;
                    return Err(error);
                }
            };
        if definition.kind != ModuleDefinitionKind::Core
            && !effective_enabled_modules.contains(&definition.slug)
        {
            let error = ModuleLifecycleDbWriterError::Settings(
                ModuleOperationStoreError::ModuleNotEnabled(definition.slug.clone()),
            );
            self.abandon_static_settings_operation(
                lease,
                command.tenant_id,
                &command.module_slug,
                command.context.idempotency_key,
                &error,
            )
            .await?;
            return Err(error);
        }

        let transaction = match self.db.begin().await {
            Ok(transaction) => transaction,
            Err(error) => {
                let error = database_error(error);
                self.abandon_static_settings_operation(
                    lease,
                    command.tenant_id,
                    &command.module_slug,
                    command.context.idempotency_key,
                    &error,
                )
                .await?;
                return Err(error);
            }
        };
        let result = async {
            let state = TenantModuleStateStore::persist_settings(
                &transaction,
                TenantModuleSettingsRequest {
                    tenant_id: command.tenant_id,
                    module_slug: definition.slug.clone(),
                    settings: command.settings.clone(),
                    is_core: definition.kind == ModuleDefinitionKind::Core,
                    is_effectively_enabled: effective_enabled_modules.contains(&definition.slug),
                },
            )
            .await
            .map_err(ModuleLifecycleDbWriterError::Settings)?;
            let revision = StaticTenantLifecycleStore::advance(
                &transaction,
                command.tenant_id,
                &command.module_slug,
                command.expected_revision,
                command.context.idempotency_key,
            )
            .await
            .map_err(map_static_lifecycle_settings_error)?;
            StaticTenantLifecycleStore::release(
                &transaction,
                command.tenant_id,
                &command.module_slug,
                command.context.idempotency_key,
            )
            .await
            .map_err(map_static_lifecycle_settings_error)?;
            let result = ModuleLifecycleSettingsResult {
                module_slug: state.module_slug,
                enabled: state.enabled,
                settings: state.settings,
                revision,
            };
            idempotency::complete(&transaction, lease, &result)
                .await
                .map_err(ModuleLifecycleDbWriterError::OperationReceipt)?;
            Ok::<_, ModuleLifecycleDbWriterError>(result)
        }
        .await;
        match result {
            Ok(result) => {
                transaction.commit().await.map_err(database_error)?;
                Ok(result)
            }
            Err(error) => {
                let _ = transaction.rollback().await;
                let release_result = StaticTenantLifecycleStore::release(
                    &self.db,
                    command.tenant_id,
                    &command.module_slug,
                    command.context.idempotency_key,
                )
                .await
                .map_err(map_static_lifecycle_settings_error);
                let receipt_result = self.fail_static_settings_operation(lease, &error).await;
                release_result?;
                receipt_result?;
                Err(error)
            }
        }
    }

    /// Validates and persists artifact settings against the exact immutable
    /// schema selected by the admitted definition. Callers cannot supply a
    /// schema or bypass this owner boundary with a pre-normalized payload.
    pub async fn persist_artifact_settings(
        &self,
        tenant_id: Uuid,
        module_slug: &str,
        settings: serde_json::Value,
    ) -> Result<TenantModuleSettingsRecord, ModuleLifecycleDbWriterError> {
        let catalog = self.definition_catalog()?;
        let definition = catalog
            .get(module_slug)
            .ok_or_else(|| ModuleLifecycleDbWriterError::UnknownModule(module_slug.to_string()))?;
        if !matches!(&definition.source, ModuleDefinitionSource::Artifact { .. }) {
            return Err(ModuleLifecycleDbWriterError::ArtifactSettings {
                module_slug: module_slug.to_string(),
                reason: "static settings require trusted host-manifest normalization",
            });
        }
        artifact_settings::persist(
            &self.db,
            &self.settings_schema_validators,
            tenant_id,
            module_slug,
            settings,
        )
        .await
        .map_err(|error| map_artifact_settings_error(module_slug, error))
    }

    async fn fail_static_settings_operation(
        &self,
        lease: idempotency::Lease,
        error: &ModuleLifecycleDbWriterError,
    ) -> Result<(), ModuleLifecycleDbWriterError> {
        let receipt_error = match error {
            ModuleLifecycleDbWriterError::SettingsSnapshotConflict => PortError::conflict(
                "modules.static_lifecycle_settings_snapshot_conflict",
                error.to_string(),
            ),
            ModuleLifecycleDbWriterError::Settings(
                ModuleOperationStoreError::ModuleNotEnabled(_),
            ) => PortError::validation(
                "modules.static_lifecycle_settings_module_disabled",
                error.to_string(),
            ),
            ModuleLifecycleDbWriterError::Lifecycle(
                ModuleLifecycleExecutionError::RevisionConflict { .. },
            ) => PortError::conflict(
                "modules.static_lifecycle_revision_conflict",
                error.to_string(),
            ),
            _ => PortError::invariant_violation(
                "modules.static_lifecycle_settings_failed",
                error.to_string(),
            ),
        };
        idempotency::fail(&self.db, lease, &receipt_error)
            .await
            .map_err(ModuleLifecycleDbWriterError::OperationReceipt)
    }

    async fn abandon_static_settings_operation(
        &self,
        lease: idempotency::Lease,
        tenant_id: Uuid,
        module_slug: &str,
        idempotency_key: Uuid,
        error: &ModuleLifecycleDbWriterError,
    ) -> Result<(), ModuleLifecycleDbWriterError> {
        let release_result =
            StaticTenantLifecycleStore::release(&self.db, tenant_id, module_slug, idempotency_key)
                .await
                .map_err(map_static_lifecycle_settings_error);
        let receipt_result = self.fail_static_settings_operation(lease, error).await;
        release_result?;
        receipt_result
    }

    /// Confirms that the active owner catalog contains a module before a host
    /// adapter resolves its static-only settings schema.
    pub fn require_module_definition(
        &self,
        module_slug: &str,
    ) -> Result<(), ModuleLifecycleDbWriterError> {
        if self.definition_catalog()?.get(module_slug).is_none() {
            return Err(ModuleLifecycleDbWriterError::UnknownModule(
                module_slug.to_string(),
            ));
        }
        Ok(())
    }

    /// Returns the static lifecycle revision without materializing an override
    /// row for inherited/default state. Artifact installations expose their
    /// separate tenant-lifecycle snapshot through the installation owner.
    pub async fn static_lifecycle_snapshot(
        &self,
        tenant_id: Uuid,
        module_slug: &str,
    ) -> Result<StaticTenantLifecycleSnapshot, ModuleLifecycleDbWriterError> {
        let catalog = self.definition_catalog()?;
        let definition = catalog
            .get(module_slug)
            .ok_or_else(|| ModuleLifecycleDbWriterError::UnknownModule(module_slug.to_string()))?;
        if !matches!(
            &definition.source,
            ModuleDefinitionSource::PlatformNative { .. }
                | ModuleDefinitionSource::PromotedNative { .. }
        ) {
            return Err(ModuleLifecycleDbWriterError::ArtifactSettings {
                module_slug: module_slug.to_string(),
                reason: "artifact lifecycle uses the admitted installation aggregate",
            });
        }
        StaticTenantLifecycleStore::snapshot(&self.db, tenant_id, module_slug)
            .await
            .map_err(|error| ModuleLifecycleDbWriterError::Database(error.to_string()))
    }

    /// Loads revision snapshots for one registry-owned static module set in a
    /// single query. This read intentionally does not materialize aggregate or
    /// override rows for inherited/default state.
    pub async fn static_lifecycle_snapshots(
        &self,
        tenant_id: Uuid,
        module_slugs: impl IntoIterator<Item = String>,
    ) -> Result<BTreeMap<String, StaticTenantLifecycleSnapshot>, ModuleLifecycleDbWriterError> {
        let catalog = self.definition_catalog()?;
        let module_slugs = module_slugs.into_iter().collect::<Vec<_>>();
        for module_slug in &module_slugs {
            let definition = catalog.get(module_slug).ok_or_else(|| {
                ModuleLifecycleDbWriterError::UnknownModule(module_slug.to_string())
            })?;
            if !matches!(
                &definition.source,
                ModuleDefinitionSource::PlatformNative { .. }
                    | ModuleDefinitionSource::PromotedNative { .. }
            ) {
                return Err(ModuleLifecycleDbWriterError::ArtifactSettings {
                    module_slug: module_slug.to_string(),
                    reason: "artifact lifecycle uses the admitted installation aggregate",
                });
            }
        }
        StaticTenantLifecycleStore::snapshots(&self.db, tenant_id, module_slugs)
            .await
            .map_err(|error| ModuleLifecycleDbWriterError::Database(error.to_string()))
    }

    /// Resolves Core/default/tenant-override availability from the same owner
    /// catalog and tenant-state source used by lifecycle commands.
    pub async fn effective_enabled_modules(
        &self,
        tenant_id: Uuid,
    ) -> Result<HashSet<String>, ModuleLifecycleDbWriterError> {
        Ok(self
            .effective_policy(tenant_id)
            .await?
            .into_enabled_modules())
    }

    /// Returns explicit tenant override rows without exposing owner tables to
    /// GraphQL, native, or admin adapters.
    pub async fn tenant_override_snapshots(
        &self,
        tenant_id: Uuid,
        limit: u32,
    ) -> Result<Vec<TenantModuleOverrideSnapshot>, ModuleLifecycleDbWriterError> {
        if tenant_id.is_nil() || limit == 0 || limit > 1_000 {
            return Err(ModuleLifecycleDbWriterError::InvalidTenantOverrideQuery);
        }
        let backend = self.db.get_database_backend();
        let sql = match backend {
            DbBackend::Postgres => {
                "SELECT module_slug, enabled, settings FROM tenant_modules \
                 WHERE tenant_id = $1 ORDER BY module_slug ASC LIMIT $2"
            }
            _ => {
                "SELECT module_slug, enabled, settings FROM tenant_modules \
                 WHERE tenant_id = ?1 ORDER BY module_slug ASC LIMIT ?2"
            }
        };
        self.db
            .query_all(Statement::from_sql_and_values(
                backend,
                sql,
                vec![tenant_id.into(), i64::from(limit).into()],
            ))
            .await
            .map_err(database_error)?
            .into_iter()
            .map(|row| {
                Ok(TenantModuleOverrideSnapshot {
                    module_slug: row.try_get("", "module_slug").map_err(database_error)?,
                    enabled: row.try_get("", "enabled").map_err(database_error)?,
                    settings: row.try_get("", "settings").map_err(database_error)?,
                })
            })
            .collect()
    }

    /// Resolves the explainable, revisioned availability policy from the exact
    /// owner catalog, platform defaults, tenant overrides, and artifact runtime
    /// evidence used by writes.
    pub async fn effective_policy(
        &self,
        tenant_id: Uuid,
    ) -> Result<ModuleEffectivePolicy, ModuleLifecycleDbWriterError> {
        self.effective_policy_with_context(tenant_id, None, None)
            .await
    }

    /// Resolves availability using a channel-owner snapshot. Channel lookup
    /// and channel-table access remain outside this module owner; only the
    /// canonical tenant-safe input is evaluated here.
    pub async fn effective_policy_for_channel(
        &self,
        tenant_id: Uuid,
        channel: ModuleEffectivePolicyChannelInput,
    ) -> Result<ModuleEffectivePolicy, ModuleLifecycleDbWriterError> {
        self.effective_policy_for_context(tenant_id, Some(channel), None)
            .await
    }

    /// Resolves availability from an operational maintenance snapshot. The
    /// snapshot blocks serving without rewriting tenant enablement intent.
    pub async fn effective_policy_for_maintenance(
        &self,
        tenant_id: Uuid,
        maintenance: ModuleEffectivePolicyMaintenanceInput,
    ) -> Result<ModuleEffectivePolicy, ModuleLifecycleDbWriterError> {
        self.effective_policy_for_context(tenant_id, None, Some(maintenance))
            .await
    }

    /// Resolves availability from the channel and maintenance owner inputs.
    pub async fn effective_policy_for_context(
        &self,
        tenant_id: Uuid,
        channel: Option<ModuleEffectivePolicyChannelInput>,
        maintenance: Option<ModuleEffectivePolicyMaintenanceInput>,
    ) -> Result<ModuleEffectivePolicy, ModuleLifecycleDbWriterError> {
        if channel
            .as_ref()
            .is_some_and(|channel| channel.tenant_id != tenant_id)
        {
            return Err(ModuleLifecycleDbWriterError::Policy(
                ModuleEffectivePolicyError::InvalidChannelInput(
                    "channel tenant_id does not match the policy tenant".to_string(),
                ),
            ));
        }
        self.effective_policy_with_context(tenant_id, channel, maintenance)
            .await
    }

    async fn effective_policy_with_context(
        &self,
        tenant_id: Uuid,
        channel: Option<ModuleEffectivePolicyChannelInput>,
        maintenance: Option<ModuleEffectivePolicyMaintenanceInput>,
    ) -> Result<ModuleEffectivePolicy, ModuleLifecycleDbWriterError> {
        let catalog = self.definition_catalog()?;
        let runtime_inputs = self.runtime_policy_inputs(&catalog, tenant_id).await;
        ModuleEffectivePolicyQuery::new_with_context(
            &catalog,
            self.default_enabled_modules.iter().cloned(),
            self.overrides(tenant_id).await?,
            runtime_inputs,
            channel,
            maintenance,
        )
        .with_corequisites(self.co_requisites.iter().cloned())
        .execute()
        .map_err(ModuleLifecycleDbWriterError::Policy)
    }

    async fn effective_policy_from_overrides(
        &self,
        tenant_id: Uuid,
        catalog: &ModuleDefinitionCatalog,
        overrides: Vec<TenantModuleOverride>,
    ) -> Result<ModuleEffectivePolicy, ModuleLifecycleDbWriterError> {
        let runtime_inputs = self.runtime_policy_inputs(catalog, tenant_id).await;
        ModuleEffectivePolicyQuery::new_with_context(
            catalog,
            self.default_enabled_modules.iter().cloned(),
            overrides,
            runtime_inputs,
            None,
            None,
        )
        .with_corequisites(self.co_requisites.iter().cloned())
        .execute()
        .map_err(ModuleLifecycleDbWriterError::Policy)
    }

    async fn ordering_policy_from_overrides(
        &self,
        tenant_id: Uuid,
        catalog: &ModuleDefinitionCatalog,
        overrides: Vec<TenantModuleOverride>,
    ) -> Result<ModuleEffectivePolicy, ModuleLifecycleDbWriterError> {
        let runtime_inputs = self.runtime_policy_inputs(catalog, tenant_id).await;
        ModuleEffectivePolicyQuery::new_with_context(
            catalog,
            self.default_enabled_modules.iter().cloned(),
            overrides,
            runtime_inputs,
            None,
            None,
        )
        .execute()
        .map_err(ModuleLifecycleDbWriterError::Policy)
    }

    async fn runtime_policy_inputs(
        &self,
        catalog: &ModuleDefinitionCatalog,
        tenant_id: Uuid,
    ) -> Vec<ModuleEffectivePolicyRuntimeInput> {
        let installations = SeaOrmArtifactInstallationStore::with_infrastructure(
            self.db.clone(),
            self.infrastructure.clone(),
        );
        let policies = SeaOrmArtifactSandboxPolicyResolver::new(self.db.clone());
        let security = SeaOrmModuleArtifactSecurityResolver::new(self.db.clone());
        let mut inputs = Vec::new();
        for definition in catalog.definitions() {
            let ModuleDefinitionSource::Artifact { release } = &definition.source else {
                continue;
            };
            let artifact =
                ArtifactInstallationResolver::resolve(&installations, release, tenant_id).await;
            let Ok(artifact) = artifact else {
                inputs.push(ModuleEffectivePolicyRuntimeInput {
                    module_slug: definition.slug.clone(),
                    installation: None,
                    capability_policy_revision: None,
                    executor_available: false,
                    security: None,
                });
                continue;
            };
            let capability_policy_revision =
                ArtifactSandboxPolicyResolver::resolve(&policies, &artifact, tenant_id)
                    .await
                    .ok()
                    .map(|_| artifact.capability_grant_revision);
            let executor_available = self.artifact_executor.is_some_and(|executor| {
                executor.supports_payload_kind(artifact.descriptor.payload_kind)
            });
            let security = security.resolve(release).await.ok();
            inputs.push(ModuleEffectivePolicyRuntimeInput {
                module_slug: definition.slug.clone(),
                installation: Some(ModuleEffectivePolicyInstallationFact {
                    installation_id: artifact.installation_id,
                    scope: artifact.scope,
                    release_digest: artifact.release.digest,
                    payload_kind: artifact.descriptor.payload_kind,
                    dependency_graph_revision: artifact.dependency_lock.graph_revision,
                    dependency_graph_digest: artifact.dependency_lock.graph_digest,
                    capability_grant_revision: artifact.capability_grant_revision,
                }),
                capability_policy_revision,
                executor_available,
                security,
            });
        }
        inputs
    }

    async fn recovery_execution_context(
        &self,
        tenant_id: Uuid,
        module_slug: &str,
    ) -> Result<
        (ModuleDefinitionCatalog, Option<bool>, serde_json::Value),
        ModuleLifecycleDbWriterError,
    > {
        let catalog = self.definition_catalog()?;
        let overrides = self.overrides(tenant_id).await?;
        let current_override_enabled = override_enabled(&overrides, module_slug);
        let current_settings = self.settings(tenant_id, module_slug).await?;
        Ok((catalog, current_override_enabled, current_settings))
    }

    async fn recovery_policy_context(
        &self,
        tenant_id: Uuid,
        module_slug: &str,
    ) -> Result<
        (
            ModuleDefinitionCatalog,
            Option<bool>,
            HashSet<String>,
            serde_json::Value,
        ),
        ModuleLifecycleDbWriterError,
    > {
        let catalog = self.definition_catalog()?;
        let overrides = self.overrides(tenant_id).await?;
        let current_override_enabled = override_enabled(&overrides, module_slug);
        let current_policy = self
            .effective_policy_from_overrides(tenant_id, &catalog, overrides)
            .await?;
        let current_settings = self.settings(tenant_id, module_slug).await?;
        Ok((
            catalog,
            current_override_enabled,
            current_policy.into_enabled_modules(),
            current_settings,
        ))
    }

    async fn override_execution_context(
        &self,
        tenant_id: Uuid,
        module_slug: &str,
        requested_override_enabled: Option<bool>,
    ) -> Result<
        (
            ModuleDefinitionCatalog,
            HashSet<String>,
            HashSet<String>,
            Option<bool>,
            serde_json::Value,
            Option<ModulePolicyRevisionTransition>,
        ),
        ModuleLifecycleDbWriterError,
    > {
        let catalog = self.definition_catalog()?;
        let overrides = self.overrides(tenant_id).await?;
        let previous_override_enabled = override_enabled(&overrides, module_slug);
        let current_policy = self
            .effective_policy_from_overrides(tenant_id, &catalog, overrides.clone())
            .await?;
        let ordering_policy = self
            .ordering_policy_from_overrides(tenant_id, &catalog, overrides.clone())
            .await?;
        let mut next_overrides = overrides;
        match requested_override_enabled {
            Some(enabled) => {
                if let Some(override_value) = next_overrides
                    .iter_mut()
                    .find(|value| value.module_slug == module_slug)
                {
                    override_value.enabled = enabled;
                } else {
                    next_overrides.push(TenantModuleOverride {
                        module_slug: module_slug.to_string(),
                        enabled,
                    });
                }
            }
            None => next_overrides.retain(|value| value.module_slug != module_slug),
        }
        let next_policy = self
            .effective_policy_from_overrides(tenant_id, &catalog, next_overrides)
            .await?;
        let policy_transition = if current_policy.policy_revision() != next_policy.policy_revision()
        {
            let consumer = SeaOrmModulePolicyRevisionConsumer::new(self.db.clone());
            Some(ModulePolicyRevisionTransition {
                previous_revision: consumer
                    .current_revision(tenant_id, "module.lifecycle")
                    .await
                    .map_err(|error| {
                        ModuleLifecycleDbWriterError::PolicyTransition(error.to_string())
                    })?,
                next_revision: next_policy.policy_revision().to_string(),
            })
        } else {
            None
        };
        let current_settings = self.settings(tenant_id, module_slug).await?;
        Ok((
            catalog,
            current_policy.into_enabled_modules(),
            ordering_policy.into_enabled_modules(),
            previous_override_enabled,
            current_settings,
            policy_transition,
        ))
    }

    fn definition_catalog(&self) -> Result<ModuleDefinitionCatalog, ModuleLifecycleDbWriterError> {
        match &self.catalog {
            Some(catalog) => Ok(catalog.clone()),
            None => Ok(ModuleDefinitionCatalog::from_static_registry(
                self.static_registry.ok_or_else(|| {
                    ModuleLifecycleDbWriterError::Configuration(
                        "static lifecycle writer has no module registry".into(),
                    )
                })?,
            )
            .map_err(ModuleLifecycleDbWriterError::Definition)?),
        }
    }

    async fn overrides(
        &self,
        tenant_id: Uuid,
    ) -> Result<Vec<TenantModuleOverride>, ModuleLifecycleDbWriterError> {
        let backend = self.db.get_database_backend();
        let sql = match backend {
            DbBackend::Postgres => {
                "SELECT module_slug, enabled FROM tenant_modules WHERE tenant_id = $1"
            }
            _ => "SELECT module_slug, enabled FROM tenant_modules WHERE tenant_id = ?1",
        };
        self.db
            .query_all(Statement::from_sql_and_values(
                backend,
                sql,
                vec![tenant_id.into()],
            ))
            .await
            .map_err(database_error)?
            .into_iter()
            .map(|row| {
                Ok(TenantModuleOverride {
                    module_slug: row.try_get("", "module_slug").map_err(database_error)?,
                    enabled: row.try_get("", "enabled").map_err(database_error)?,
                })
            })
            .collect()
    }

    async fn settings(
        &self,
        tenant_id: Uuid,
        module_slug: &str,
    ) -> Result<serde_json::Value, ModuleLifecycleDbWriterError> {
        let catalog = self.definition_catalog()?;
        let definition = catalog
            .get(module_slug)
            .ok_or_else(|| ModuleLifecycleDbWriterError::UnknownModule(module_slug.to_string()))?;
        if matches!(&definition.source, ModuleDefinitionSource::Artifact { .. }) {
            return artifact_settings::load(&self.db, tenant_id, module_slug)
                .await
                .map_err(|error| map_artifact_settings_error(module_slug, error));
        }
        let backend = self.db.get_database_backend();
        let sql = match backend {
            DbBackend::Postgres => {
                "SELECT settings FROM tenant_modules WHERE tenant_id = $1 AND module_slug = $2 LIMIT 1"
            }
            _ => {
                "SELECT settings FROM tenant_modules WHERE tenant_id = ?1 AND module_slug = ?2 LIMIT 1"
            }
        };
        self.db
            .query_one(Statement::from_sql_and_values(
                backend,
                sql,
                vec![tenant_id.into(), module_slug.into()],
            ))
            .await
            .map_err(database_error)?
            .map(|row| row.try_get("", "settings").map_err(database_error))
            .transpose()
            .map(|settings| settings.unwrap_or_else(|| serde_json::json!({})))
    }
}

fn validate_recovery_command(
    command: &ModuleLifecycleRecoveryCommand,
) -> Result<(), ModuleLifecycleDbWriterError> {
    if command.tenant_id.is_nil()
        || command.operation_id.is_nil()
        || command.context.tenant_id != Some(command.tenant_id)
    {
        return Err(ModuleLifecycleDbWriterError::Recovery(
            ModuleOperationRecoveryError::InvalidCommandIdentity,
        ));
    }
    if command.context.validate().is_err() {
        return Err(ModuleLifecycleDbWriterError::Recovery(
            ModuleOperationRecoveryError::InvalidIdempotencyKey,
        ));
    }
    Ok(())
}

fn override_enabled(overrides: &[TenantModuleOverride], module_slug: &str) -> Option<bool> {
    overrides
        .iter()
        .find(|value| value.module_slug == module_slug)
        .map(|value| value.enabled)
}

#[derive(Debug, Error)]
pub enum ModuleLifecycleDbWriterError {
    #[error("module lifecycle persistence failed: {0}")]
    Database(String),
    #[error("module lifecycle writer configuration is invalid: {0}")]
    Configuration(String),
    #[error(transparent)]
    Lifecycle(#[from] ModuleLifecycleExecutionError),
    #[error(transparent)]
    Definition(#[from] ModuleDefinitionError),
    #[error(transparent)]
    Policy(#[from] ModuleEffectivePolicyError),
    #[error("module effective-policy transition could not be prepared: {0}")]
    PolicyTransition(String),
    #[error("module settings changed since the reviewed snapshot")]
    SettingsSnapshotConflict,
    #[error("tenant module override query requires a tenant and a limit between 1 and 1000")]
    InvalidTenantOverrideQuery,
    #[error(transparent)]
    Recovery(#[from] ModuleOperationRecoveryError),
    #[error("module `{0}` is not part of the active definition catalog")]
    UnknownModule(String),
    #[error("artifact settings for module `{module_slug}` are invalid: {reason}")]
    ArtifactSettings {
        module_slug: String,
        reason: &'static str,
    },
    #[error(transparent)]
    Settings(#[from] ModuleOperationStoreError),
    #[error(transparent)]
    OperationReceipt(PortError),
}

fn map_idempotency_command_error(error: ModuleOperationStoreError) -> ModuleLifecycleDbWriterError {
    match error {
        ModuleOperationStoreError::IdempotencyConflict => ModuleLifecycleDbWriterError::Lifecycle(
            ModuleLifecycleExecutionError::IdempotencyConflict,
        ),
        error => ModuleLifecycleDbWriterError::Lifecycle(
            ModuleLifecycleExecutionError::Persistence(error.to_string()),
        ),
    }
}

fn map_static_lifecycle_recovery_error(
    error: StaticTenantLifecycleStoreError,
) -> ModuleLifecycleDbWriterError {
    let error = match error {
        StaticTenantLifecycleStoreError::RevisionConflict {
            expected, current, ..
        } => ModuleOperationRecoveryError::RevisionConflict { expected, current },
        StaticTenantLifecycleStoreError::OperationInProgress { .. } => {
            ModuleOperationRecoveryError::OperationInProgress
        }
        error => ModuleOperationRecoveryError::Persistence(error.to_string()),
    };
    ModuleLifecycleDbWriterError::Recovery(error)
}

fn map_static_lifecycle_settings_error(
    error: StaticTenantLifecycleStoreError,
) -> ModuleLifecycleDbWriterError {
    match error {
        StaticTenantLifecycleStoreError::RevisionConflict {
            module_slug,
            expected,
            current,
        } => ModuleLifecycleDbWriterError::Lifecycle(
            ModuleLifecycleExecutionError::RevisionConflict {
                module_slug,
                expected,
                current,
            },
        ),
        StaticTenantLifecycleStoreError::OperationInProgress { module_slug } => {
            ModuleLifecycleDbWriterError::Lifecycle(
                ModuleLifecycleExecutionError::OperationInProgress { module_slug },
            )
        }
        error => ModuleLifecycleDbWriterError::Database(error.to_string()),
    }
}

fn validate_settings_command(
    command: &ModuleLifecycleSettingsCommand,
) -> Result<(), ModuleLifecycleDbWriterError> {
    if command.tenant_id.is_nil()
        || command.context.tenant_id != Some(command.tenant_id)
        || command.module_slug.is_empty()
    {
        return Err(ModuleLifecycleDbWriterError::Lifecycle(
            ModuleLifecycleExecutionError::InvalidCommandIdentity,
        ));
    }
    if command.context.validate().is_err() {
        return Err(ModuleLifecycleDbWriterError::Lifecycle(
            ModuleLifecycleExecutionError::InvalidIdempotencyKey,
        ));
    }
    if command.settings.is_object()
        && command
            .expected_settings
            .as_ref()
            .is_none_or(serde_json::Value::is_object)
        && command.expected_enabled.is_some() == command.expected_settings.is_some()
    {
        return Ok(());
    }
    Err(ModuleLifecycleDbWriterError::Settings(
        ModuleOperationStoreError::Database(
            "static lifecycle settings command requires object settings and a complete reviewed snapshot"
                .to_string(),
        ),
    ))
}

fn database_error(error: impl std::fmt::Display) -> ModuleLifecycleDbWriterError {
    ModuleLifecycleDbWriterError::Database(error.to_string())
}

fn map_artifact_settings_error(
    module_slug: &str,
    error: ArtifactSettingsStoreError,
) -> ModuleLifecycleDbWriterError {
    match error {
        ArtifactSettingsStoreError::Database(error) => {
            ModuleLifecycleDbWriterError::Database(error)
        }
        ArtifactSettingsStoreError::InvalidIdentity => {
            ModuleLifecycleDbWriterError::ArtifactSettings {
                module_slug: module_slug.to_string(),
                reason: "artifact settings require a non-nil tenant and canonical module slug",
            }
        }
        ArtifactSettingsStoreError::InvalidValue => {
            ModuleLifecycleDbWriterError::ArtifactSettings {
                module_slug: module_slug.to_string(),
                reason: "artifact settings must be a JSON object",
            }
        }
        ArtifactSettingsStoreError::InstallationUnavailable => {
            ModuleLifecycleDbWriterError::ArtifactSettings {
                module_slug: module_slug.to_string(),
                reason: "no active admitted artifact installation is available",
            }
        }
        ArtifactSettingsStoreError::AmbiguousInstallation => {
            ModuleLifecycleDbWriterError::ArtifactSettings {
                module_slug: module_slug.to_string(),
                reason: "active admitted artifact installation is ambiguous",
            }
        }
        ArtifactSettingsStoreError::InvalidInstallation => {
            ModuleLifecycleDbWriterError::ArtifactSettings {
                module_slug: module_slug.to_string(),
                reason: "active admitted artifact installation metadata is invalid",
            }
        }
        ArtifactSettingsStoreError::MissingSchema => {
            ModuleLifecycleDbWriterError::ArtifactSettings {
                module_slug: module_slug.to_string(),
                reason: "active artifact does not declare a settings schema",
            }
        }
        ArtifactSettingsStoreError::SchemaViolation => {
            ModuleLifecycleDbWriterError::ArtifactSettings {
                module_slug: module_slug.to_string(),
                reason: "artifact settings do not satisfy the admitted schema",
            }
        }
        ArtifactSettingsStoreError::ValidatorUnavailable => {
            ModuleLifecycleDbWriterError::ArtifactSettings {
                module_slug: module_slug.to_string(),
                reason: "artifact settings validator is unavailable",
            }
        }
        ArtifactSettingsStoreError::SchemaMismatch => {
            ModuleLifecycleDbWriterError::ArtifactSettings {
                module_slug: module_slug.to_string(),
                reason: "artifact settings instance schema does not match the active installation",
            }
        }
        ArtifactSettingsStoreError::Tombstoned => ModuleLifecycleDbWriterError::ArtifactSettings {
            module_slug: module_slug.to_string(),
            reason: "artifact settings instance was purged and requires an explicit recovery restore",
        },
    }
}
