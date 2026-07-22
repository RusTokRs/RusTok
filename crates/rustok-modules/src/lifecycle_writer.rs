use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use std::collections::HashSet;
use thiserror::Error;
use uuid::Uuid;

use rustok_core::ModuleRegistry;

use crate::policy::{
    ModuleEffectivePolicyChannelInput, ModuleEffectivePolicyInstallationFact,
    ModuleEffectivePolicyMaintenanceInput, ModuleEffectivePolicyNodeReadinessInput,
    ModuleEffectivePolicyQuery, ModuleEffectivePolicyRuntimeInput,
};
use crate::{
    artifact_schema::{ArtifactSchemaValidationError, ArtifactSchemaValidatorCache},
    execute_module_toggle, module_operation_recovery_plan, retry_failed_post_hook_operation,
    ArtifactInstallationResolver, ArtifactLifecycleExecutor, ArtifactSandboxPolicyResolver,
    ControlPlaneInfrastructure, ModuleDefinitionCatalog, ModuleDefinitionError,
    ModuleDefinitionKind, ModuleDefinitionSource, ModuleEffectivePolicy,
    ModuleEffectivePolicyError, ModuleEffectivePolicyTransitionCoordinator,
    ModuleExecutionDispatcher, ModuleLifecycleExecutionError, ModuleLifecycleToggleRequest,
    ModuleOperationIssue, ModuleOperationJournal, ModuleOperationRecord,
    ModuleOperationRecoveryError, ModuleOperationRequest, ModuleOperationStoreError,
    ModulePolicyRevisionTransition, ModulePostHookRetryRequest, SeaOrmArtifactInstallationStore,
    SeaOrmArtifactSandboxPolicyResolver, SeaOrmModuleArtifactSecurityResolver,
    SeaOrmModulePolicyRevisionConsumer, TenantModuleOverride, TenantModuleSettingsRecord,
    TenantModuleSettingsRequest, TenantModuleStateStore,
};

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
            settings_schema_validators: ArtifactSchemaValidatorCache::default(),
        }
    }

    pub async fn toggle(
        &self,
        tenant_id: Uuid,
        module_slug: &str,
        enabled: bool,
        requested_by: Option<String>,
    ) -> Result<crate::ModuleLifecycleToggleResult, ModuleLifecycleDbWriterError> {
        self.toggle_with_operation_context(
            tenant_id,
            module_slug,
            enabled,
            requested_by,
            None,
            None,
        )
        .await
    }

    async fn toggle_with_operation_context(
        &self,
        tenant_id: Uuid,
        module_slug: &str,
        enabled: bool,
        requested_by: Option<String>,
        correlation_id: Option<String>,
        idempotency_key: Option<Uuid>,
    ) -> Result<crate::ModuleLifecycleToggleResult, ModuleLifecycleDbWriterError> {
        let (catalog, effective_enabled_modules, current_settings, policy_transition) = self
            .toggle_execution_context(tenant_id, module_slug, enabled)
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
        execute_module_toggle(
            &self.infrastructure,
            &self.db,
            &dispatcher,
            Some(ModuleEffectivePolicyTransitionCoordinator::new(
                self.infrastructure.clone(),
                SeaOrmModulePolicyRevisionConsumer::new(self.db.clone()),
            )),
            ModuleLifecycleToggleRequest {
                tenant_id,
                module_slug: module_slug.to_string(),
                enabled,
                requested_by,
                correlation_id,
                idempotency_key,
                effective_enabled_modules,
                current_settings,
                policy_transition,
            },
        )
        .await
        .map_err(ModuleLifecycleDbWriterError::Lifecycle)
    }

    /// Retries only a post-hook failure using the same owner-owned effective
    /// policy, catalog, and dispatcher assembly as a normal lifecycle toggle.
    pub async fn retry_post_hook(
        &self,
        operation_id: Uuid,
        requested_by: Option<String>,
        idempotency_key: Uuid,
    ) -> Result<ModuleOperationRecord, ModuleLifecycleDbWriterError> {
        let plan = module_operation_recovery_plan(&self.db, operation_id)
            .await
            .map_err(ModuleLifecycleDbWriterError::Recovery)?;
        let (catalog, effective_enabled_modules, current_settings) = self
            .execution_context(plan.tenant_id, &plan.module_slug)
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
        retry_failed_post_hook_operation(
            &self.db,
            &dispatcher,
            ModulePostHookRetryRequest {
                operation_id,
                requested_by,
                idempotency_key,
                effective_enabled_modules,
                current_settings,
            },
        )
        .await
        .map_err(ModuleLifecycleDbWriterError::Recovery)
    }

    /// Compensates a committed operation only after the recovery contract
    /// confirms that it failed in its post-hook and remains at the requested
    /// effective state. The resulting reverse transition is a normal owner
    /// lifecycle operation with its own journal record.
    pub async fn compensate_failed_operation(
        &self,
        operation_id: Uuid,
        requested_by: Option<String>,
        idempotency_key: Uuid,
    ) -> Result<crate::ModuleLifecycleToggleResult, ModuleLifecycleDbWriterError> {
        if idempotency_key.is_nil() {
            return Err(ModuleLifecycleDbWriterError::Lifecycle(
                ModuleLifecycleExecutionError::InvalidIdempotencyKey,
            ));
        }
        let plan = module_operation_recovery_plan(&self.db, operation_id)
            .await
            .map_err(ModuleLifecycleDbWriterError::Recovery)?;
        if plan.issue != ModuleOperationIssue::PostHookFailed {
            return Err(ModuleLifecycleDbWriterError::Recovery(
                ModuleOperationRecoveryError::NotRetryable(plan.issue.as_str().to_string()),
            ));
        }
        let (_, effective_enabled_modules, _) = self
            .execution_context(plan.tenant_id, &plan.module_slug)
            .await?;
        let current_enabled = effective_enabled_modules.contains(&plan.module_slug);
        let replay_request = ModuleOperationRequest {
            tenant_id: plan.tenant_id,
            module_slug: plan.module_slug.clone(),
            requested_enabled: plan.previous_effective_enabled,
            previous_effective_enabled: current_enabled,
            requested_by: requested_by.clone(),
            correlation_id: plan.operation_id.to_string(),
            idempotency_key: Some(idempotency_key),
        };
        match ModuleOperationJournal::replay_idempotent_command(&self.db, &replay_request)
            .await
            .map_err(map_idempotency_command_error)?
        {
            Some(_) => {
                return self
                    .toggle_with_operation_context(
                        plan.tenant_id,
                        &plan.module_slug,
                        plan.previous_effective_enabled,
                        requested_by,
                        Some(plan.operation_id.to_string()),
                        Some(idempotency_key),
                    )
                    .await;
            }
            None => {}
        }
        if current_enabled != plan.requested_enabled {
            return Err(ModuleLifecycleDbWriterError::Recovery(
                ModuleOperationRecoveryError::StateMismatch {
                    requested_enabled: plan.requested_enabled,
                    current_enabled,
                },
            ));
        }
        self.toggle_with_operation_context(
            plan.tenant_id,
            &plan.module_slug,
            plan.previous_effective_enabled,
            requested_by,
            Some(plan.operation_id.to_string()),
            Some(idempotency_key),
        )
        .await
    }

    /// Persists a static module settings value after the host adapter has
    /// normalized it through the trusted distribution manifest schema.
    pub async fn persist_static_normalized_settings(
        &self,
        tenant_id: Uuid,
        module_slug: &str,
        settings: serde_json::Value,
    ) -> Result<TenantModuleSettingsRecord, ModuleLifecycleDbWriterError> {
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
                reason: "artifact settings must use owner-resolved admitted schema validation",
            });
        }
        self.persist_settings_value(tenant_id, definition, settings)
            .await
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
        if !settings.is_object() {
            return Err(ModuleLifecycleDbWriterError::ArtifactSettings {
                module_slug: module_slug.to_string(),
                reason: "artifact settings must be a JSON object",
            });
        }
        let schema_digest = definition
            .settings_schema_digest
            .as_deref()
            .ok_or_else(|| ModuleLifecycleDbWriterError::ArtifactSettings {
                module_slug: module_slug.to_string(),
                reason: "artifact does not declare a settings schema",
            })?;
        let schema = definition.settings_schema().ok_or_else(|| {
            ModuleLifecycleDbWriterError::ArtifactSettings {
                module_slug: module_slug.to_string(),
                reason: "artifact settings schema is absent from the admitted bundle",
            }
        })?;
        self.settings_schema_validators
            .validate(schema_digest, schema, &settings)
            .map_err(|error| ModuleLifecycleDbWriterError::ArtifactSettings {
                module_slug: module_slug.to_string(),
                reason: match error {
                    ArtifactSchemaValidationError::Compilation => {
                        "admitted artifact settings schema cannot be compiled"
                    }
                    ArtifactSchemaValidationError::Violation => {
                        "artifact settings do not satisfy the admitted schema"
                    }
                    ArtifactSchemaValidationError::CachePoisoned => {
                        "artifact settings validator cache is unavailable"
                    }
                },
            })?;
        self.persist_settings_value(tenant_id, definition, settings)
            .await
    }

    async fn persist_settings_value(
        &self,
        tenant_id: Uuid,
        definition: &crate::ModuleDefinition,
        settings: serde_json::Value,
    ) -> Result<TenantModuleSettingsRecord, ModuleLifecycleDbWriterError> {
        let effective_enabled_modules = self.effective_enabled_modules(tenant_id).await?;
        TenantModuleStateStore::persist_settings(
            &self.db,
            TenantModuleSettingsRequest {
                tenant_id,
                module_slug: definition.slug.clone(),
                settings,
                is_core: definition.kind == ModuleDefinitionKind::Core,
                is_effectively_enabled: effective_enabled_modules.contains(&definition.slug),
            },
        )
        .await
        .map_err(ModuleLifecycleDbWriterError::Settings)
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
        self.effective_policy_with_context(tenant_id, None, None, None)
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
        self.effective_policy_for_context(tenant_id, Some(channel), None, None)
            .await
    }

    /// Resolves availability from an operational maintenance snapshot. The
    /// snapshot blocks serving without rewriting tenant enablement intent.
    pub async fn effective_policy_for_maintenance(
        &self,
        tenant_id: Uuid,
        maintenance: ModuleEffectivePolicyMaintenanceInput,
    ) -> Result<ModuleEffectivePolicy, ModuleLifecycleDbWriterError> {
        self.effective_policy_for_context(tenant_id, None, Some(maintenance), None)
            .await
    }

    /// Resolves availability from node-owned readiness evidence. The node must
    /// have observed the base policy revision before the final policy revision
    /// is materialized.
    pub async fn effective_policy_for_node_readiness(
        &self,
        tenant_id: Uuid,
        node_readiness: ModuleEffectivePolicyNodeReadinessInput,
    ) -> Result<ModuleEffectivePolicy, ModuleLifecycleDbWriterError> {
        self.effective_policy_for_context(tenant_id, None, None, Some(node_readiness))
            .await
    }

    /// Resolves availability from all host-owned policy context snapshots.
    pub async fn effective_policy_for_context(
        &self,
        tenant_id: Uuid,
        channel: Option<ModuleEffectivePolicyChannelInput>,
        maintenance: Option<ModuleEffectivePolicyMaintenanceInput>,
        node_readiness: Option<ModuleEffectivePolicyNodeReadinessInput>,
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
        self.effective_policy_with_context(tenant_id, channel, maintenance, node_readiness)
            .await
    }

    async fn effective_policy_with_context(
        &self,
        tenant_id: Uuid,
        channel: Option<ModuleEffectivePolicyChannelInput>,
        maintenance: Option<ModuleEffectivePolicyMaintenanceInput>,
        node_readiness: Option<ModuleEffectivePolicyNodeReadinessInput>,
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
            node_readiness,
        )
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

    async fn execution_context(
        &self,
        tenant_id: Uuid,
        module_slug: &str,
    ) -> Result<
        (ModuleDefinitionCatalog, HashSet<String>, serde_json::Value),
        ModuleLifecycleDbWriterError,
    > {
        let catalog = self.definition_catalog()?;
        let effective_enabled_modules = self.effective_enabled_modules(tenant_id).await?;
        let current_settings = self.settings(tenant_id, module_slug).await?;
        Ok((catalog, effective_enabled_modules, current_settings))
    }

    async fn toggle_execution_context(
        &self,
        tenant_id: Uuid,
        module_slug: &str,
        requested_enabled: bool,
    ) -> Result<
        (
            ModuleDefinitionCatalog,
            HashSet<String>,
            serde_json::Value,
            Option<ModulePolicyRevisionTransition>,
        ),
        ModuleLifecycleDbWriterError,
    > {
        let catalog = self.definition_catalog()?;
        let overrides = self.overrides(tenant_id).await?;
        let current_policy = self
            .effective_policy_from_overrides(tenant_id, &catalog, overrides.clone())
            .await?;
        let mut next_overrides = overrides;
        if let Some(override_value) = next_overrides
            .iter_mut()
            .find(|value| value.module_slug == module_slug)
        {
            override_value.enabled = requested_enabled;
        } else {
            next_overrides.push(TenantModuleOverride {
                module_slug: module_slug.to_string(),
                enabled: requested_enabled,
            });
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

fn database_error(error: impl std::fmt::Display) -> ModuleLifecycleDbWriterError {
    ModuleLifecycleDbWriterError::Database(error.to_string())
}
