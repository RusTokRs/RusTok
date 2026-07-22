use std::collections::{BTreeMap, BTreeSet, HashSet};

use rustok_api::manifest_hash::hash_manifest;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::promotion::valid_digest;
use crate::{
    ArtifactPayloadKind, ModuleArtifactRegistryReleaseStatus, ModuleArtifactSecuritySnapshot,
    ModuleArtifactSecurityStatus, ModuleDefinitionCatalog, ModuleDefinitionKind,
    ModuleDefinitionSource, ModuleInstallationScope,
};

/// A persisted tenant-level module enablement override.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TenantModuleOverride {
    pub module_slug: String,
    pub enabled: bool,
}

/// Canonical channel state supplied by the channel owner at a policy boundary.
/// The modules crate consumes this snapshot but does not resolve channels or
/// query channel-owned tables itself.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleEffectivePolicyChannelInput {
    pub tenant_id: uuid::Uuid,
    pub channel_id: uuid::Uuid,
    pub surface: String,
    pub channel_revision: String,
    pub active: bool,
    pub bindings: Vec<ModuleEffectivePolicyChannelBinding>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleEffectivePolicyChannelBinding {
    pub module_slug: String,
    pub enabled: bool,
}

/// Canonical maintenance state supplied by the operational owner. `None` in
/// `affected_modules` means that active maintenance applies to every selected
/// module; an explicit list scopes the block to those module slugs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleEffectivePolicyMaintenanceInput {
    pub maintenance_revision: String,
    pub active: bool,
    pub reason_code: String,
    pub affected_modules: Option<Vec<String>>,
}

/// Node-owned readiness evidence for the effective-policy boundary. The
/// observed policy revision refers to the base policy (before this readiness
/// snapshot is included in the final policy revision), which avoids a
/// self-referential readiness check.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleEffectivePolicyNodeReadinessInput {
    pub node_id: uuid::Uuid,
    pub readiness_revision: String,
    pub observed_policy_revision: String,
    pub ready: bool,
    pub required_core_ready: bool,
    pub artifact_graph_revision: Option<u64>,
    pub cas_available: bool,
    pub executor_abi: Option<String>,
    pub affected_modules: Option<Vec<String>>,
}

/// One typed input that contributed to a module availability decision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ModuleEffectivePolicyFact {
    Definition {
        version: String,
        module_kind: ModuleDefinitionKind,
        source: ModuleDefinitionSource,
    },
    PlatformDefault {
        enabled: bool,
    },
    TenantOverride {
        enabled: bool,
    },
    Dependency {
        module_slug: String,
        enabled: bool,
    },
    ArtifactInstallation {
        installation_id: uuid::Uuid,
        scope: ModuleInstallationScope,
        release_digest: String,
        dependency_graph_revision: u64,
        dependency_graph_digest: String,
        capability_grant_revision: u64,
    },
    CapabilityPolicy {
        capability_grant_revision: u64,
    },
    Executor {
        payload_kind: ArtifactPayloadKind,
        available: bool,
    },
    RegistryRelease {
        status: ModuleArtifactRegistryReleaseStatus,
    },
    ArtifactSecurity {
        revision: u64,
        status: ModuleArtifactSecurityStatus,
        policy_revision: Option<String>,
        reason_code: Option<String>,
    },
    ChannelBinding {
        channel_id: uuid::Uuid,
        surface: String,
        channel_revision: String,
        enabled: bool,
    },
    Maintenance {
        maintenance_revision: String,
        active: bool,
        reason_code: String,
        affected_modules: Option<Vec<String>>,
    },
    NodeReadiness {
        node_id: uuid::Uuid,
        readiness_revision: String,
        observed_policy_revision: String,
        ready: bool,
        required_core_ready: bool,
        artifact_graph_revision: Option<u64>,
        cas_available: bool,
        executor_abi: Option<String>,
    },
}

/// Stable owner taxonomy for explaining why a module is unavailable.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ModuleEffectivePolicyDenialReason {
    UnknownModule,
    NotSelected,
    TenantDisabled,
    ArtifactInstallationUnavailable,
    CapabilityPolicyUnavailable,
    ExecutorUnavailable,
    DependencyUnavailable { module_slug: String },
    RegistryReleaseUnavailable,
    SecurityStateUnavailable,
    Quarantined,
    Revoked,
    ChannelInactive,
    ChannelBindingUnavailable,
    ChannelDisabled,
    MaintenanceActive,
    NodeReadinessUnavailable,
}

/// Explainable availability result for one module under one policy revision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleEffectivePolicyDecision {
    pub module_slug: String,
    pub enabled: bool,
    pub policy_revision: String,
    pub facts: Vec<ModuleEffectivePolicyFact>,
    pub denial_reasons: Vec<ModuleEffectivePolicyDenialReason>,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ModuleEffectivePolicyError {
    #[error("effective module policy revision could not be encoded: {0}")]
    RevisionEncoding(String),
    #[error("effective module policy channel input is invalid: {0}")]
    InvalidChannelInput(String),
    #[error("effective module policy maintenance input is invalid: {0}")]
    InvalidMaintenanceInput(String),
    #[error("effective module policy node readiness input is invalid: {0}")]
    InvalidNodeReadinessInput(String),
}

/// One owner transition delivered to a revision-aware outbox consumer. Hashes
/// are identities, not sortable clocks; consumers must apply transitions only
/// when the predecessor matches their durable cursor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModulePolicyRevisionTransition {
    pub previous_revision: Option<String>,
    pub next_revision: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModulePolicyRevisionApplyOutcome {
    Applied,
    Duplicate,
    Stale,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModulePolicyRevisionGate {
    current_revision: Option<String>,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ModulePolicyRevisionGateError {
    #[error("policy revision transition predecessor is invalid")]
    InvalidPreviousRevision,
    #[error("policy revision transition successor is invalid")]
    InvalidNextRevision,
    #[error("policy revision transition is a no-op")]
    NoopTransition,
}

impl ModulePolicyRevisionGate {
    pub fn new(current_revision: Option<String>) -> Result<Self, ModulePolicyRevisionGateError> {
        if current_revision
            .as_deref()
            .is_some_and(|revision| !valid_digest(revision))
        {
            return Err(ModulePolicyRevisionGateError::InvalidPreviousRevision);
        }
        Ok(Self { current_revision })
    }

    pub fn current_revision(&self) -> Option<&str> {
        self.current_revision.as_deref()
    }

    pub fn apply(
        &mut self,
        transition: &ModulePolicyRevisionTransition,
    ) -> Result<ModulePolicyRevisionApplyOutcome, ModulePolicyRevisionGateError> {
        if transition
            .previous_revision
            .as_deref()
            .is_some_and(|revision| !valid_digest(revision))
        {
            return Err(ModulePolicyRevisionGateError::InvalidPreviousRevision);
        }
        if !valid_digest(&transition.next_revision) {
            return Err(ModulePolicyRevisionGateError::InvalidNextRevision);
        }
        if transition.previous_revision.as_deref() == Some(transition.next_revision.as_str()) {
            return Err(ModulePolicyRevisionGateError::NoopTransition);
        }
        if self.current_revision.as_deref() == Some(transition.next_revision.as_str()) {
            return Ok(ModulePolicyRevisionApplyOutcome::Duplicate);
        }
        if self.current_revision.as_deref() != transition.previous_revision.as_deref() {
            return Ok(ModulePolicyRevisionApplyOutcome::Stale);
        }
        self.current_revision = Some(transition.next_revision.clone());
        Ok(ModulePolicyRevisionApplyOutcome::Applied)
    }
}

/// Exact owner-resolved runtime inputs for one artifact definition. Absence of
/// installation or capability-policy evidence is a denial, never an implicit
/// grant or registry fallback.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ModuleEffectivePolicyRuntimeInput {
    pub module_slug: String,
    pub installation: Option<ModuleEffectivePolicyInstallationFact>,
    pub capability_policy_revision: Option<u64>,
    pub executor_available: bool,
    pub security: Option<ModuleArtifactSecuritySnapshot>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ModuleEffectivePolicyInstallationFact {
    pub installation_id: uuid::Uuid,
    pub scope: ModuleInstallationScope,
    pub release_digest: String,
    pub payload_kind: ArtifactPayloadKind,
    pub dependency_graph_revision: u64,
    pub dependency_graph_digest: String,
    pub capability_grant_revision: u64,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ModuleToggleValidationError {
    #[error("unknown module")]
    UnknownModule,
    #[error("module `{0}` is a core platform module and cannot be disabled")]
    CoreModuleCannotBeDisabled(String),
    #[error("missing module dependencies: {0:?}")]
    MissingDependencies(Vec<String>),
    #[error("module has enabled dependents: {0:?}")]
    HasDependents(Vec<String>),
}

/// Validates a requested module enablement change against the effective module
/// set and definition topology. Persistence, operation journaling and lifecycle
/// hooks are intentionally outside this owner policy function.
pub fn validate_module_toggle(
    catalog: &ModuleDefinitionCatalog,
    enabled_modules: &HashSet<String>,
    module_slug: &str,
    enabled: bool,
) -> Result<(), ModuleToggleValidationError> {
    let Some(module) = catalog.get(module_slug) else {
        return Err(ModuleToggleValidationError::UnknownModule);
    };

    if !enabled && module.kind == ModuleDefinitionKind::Core {
        return Err(ModuleToggleValidationError::CoreModuleCannotBeDisabled(
            module_slug.to_string(),
        ));
    }

    if enabled {
        let missing = module
            .dependencies
            .iter()
            .filter(|dependency| !enabled_modules.contains(&dependency.slug))
            .map(|dependency| dependency.slug.clone())
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(ModuleToggleValidationError::MissingDependencies(missing));
        }
    } else {
        let dependents = catalog
            .definitions()
            .filter(|candidate| enabled_modules.contains(&candidate.slug))
            .filter(|candidate| {
                candidate
                    .dependencies
                    .iter()
                    .any(|dependency| dependency.slug == module_slug)
            })
            .map(|candidate| candidate.slug.clone())
            .collect::<Vec<_>>();
        if !dependents.is_empty() {
            return Err(ModuleToggleValidationError::HasDependents(dependents));
        }
    }

    Ok(())
}

/// Owner-owned effective-availability query. Host adapters supply their
/// distribution defaults and persisted tenant overrides; this query applies the
/// canonical catalog semantics equally to static and artifact definitions.
pub(crate) struct ModuleEffectivePolicyQuery<'a> {
    catalog: &'a ModuleDefinitionCatalog,
    default_enabled: Vec<String>,
    tenant_overrides: Vec<TenantModuleOverride>,
    runtime_inputs: Vec<ModuleEffectivePolicyRuntimeInput>,
    channel: Option<ModuleEffectivePolicyChannelInput>,
    maintenance: Option<ModuleEffectivePolicyMaintenanceInput>,
    node_readiness: Option<ModuleEffectivePolicyNodeReadinessInput>,
}

/// The resolved module set used by lifecycle, routing, and installer adapters.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleEffectivePolicy {
    policy_revision: String,
    enabled_modules: BTreeSet<String>,
    decisions: BTreeMap<String, ModuleEffectivePolicyDecision>,
}

impl<'a> ModuleEffectivePolicyQuery<'a> {
    #[cfg(test)]
    pub(crate) fn new(
        catalog: &'a ModuleDefinitionCatalog,
        default_enabled: impl IntoIterator<Item = String>,
        tenant_overrides: impl IntoIterator<Item = TenantModuleOverride>,
        runtime_inputs: impl IntoIterator<Item = ModuleEffectivePolicyRuntimeInput>,
    ) -> Self {
        Self::new_with_channel(
            catalog,
            default_enabled,
            tenant_overrides,
            runtime_inputs,
            None,
        )
    }

    #[cfg(test)]
    pub(crate) fn new_with_channel(
        catalog: &'a ModuleDefinitionCatalog,
        default_enabled: impl IntoIterator<Item = String>,
        tenant_overrides: impl IntoIterator<Item = TenantModuleOverride>,
        runtime_inputs: impl IntoIterator<Item = ModuleEffectivePolicyRuntimeInput>,
        channel: Option<ModuleEffectivePolicyChannelInput>,
    ) -> Self {
        Self::new_with_inputs(
            catalog,
            default_enabled,
            tenant_overrides,
            runtime_inputs,
            channel,
            None,
        )
    }

    #[cfg(test)]
    pub(crate) fn new_with_inputs(
        catalog: &'a ModuleDefinitionCatalog,
        default_enabled: impl IntoIterator<Item = String>,
        tenant_overrides: impl IntoIterator<Item = TenantModuleOverride>,
        runtime_inputs: impl IntoIterator<Item = ModuleEffectivePolicyRuntimeInput>,
        channel: Option<ModuleEffectivePolicyChannelInput>,
        maintenance: Option<ModuleEffectivePolicyMaintenanceInput>,
    ) -> Self {
        Self::new_with_context(
            catalog,
            default_enabled,
            tenant_overrides,
            runtime_inputs,
            channel,
            maintenance,
            None,
        )
    }

    pub(crate) fn new_with_context(
        catalog: &'a ModuleDefinitionCatalog,
        default_enabled: impl IntoIterator<Item = String>,
        tenant_overrides: impl IntoIterator<Item = TenantModuleOverride>,
        runtime_inputs: impl IntoIterator<Item = ModuleEffectivePolicyRuntimeInput>,
        channel: Option<ModuleEffectivePolicyChannelInput>,
        maintenance: Option<ModuleEffectivePolicyMaintenanceInput>,
        node_readiness: Option<ModuleEffectivePolicyNodeReadinessInput>,
    ) -> Self {
        Self {
            catalog,
            default_enabled: default_enabled.into_iter().collect(),
            tenant_overrides: tenant_overrides.into_iter().collect(),
            runtime_inputs: runtime_inputs.into_iter().collect(),
            channel,
            maintenance,
            node_readiness,
        }
    }

    /// Resolves the immutable core set, selected optional defaults, and tenant
    /// intent. Unknown and legacy overrides are ignored rather than becoming
    /// active definitions.
    pub fn execute(self) -> Result<ModuleEffectivePolicy, ModuleEffectivePolicyError> {
        validate_channel_input(self.channel.as_ref())?;
        validate_maintenance_input(self.maintenance.as_ref())?;
        validate_node_readiness_input(self.node_readiness.as_ref(), None)?;
        let channel = self.channel;
        let mut maintenance = self.maintenance;
        let mut node_readiness = self.node_readiness;
        if let Some(affected_modules) = maintenance
            .as_mut()
            .and_then(|input| input.affected_modules.as_mut())
        {
            affected_modules.sort();
        }
        if let Some(affected_modules) = node_readiness
            .as_mut()
            .and_then(|input| input.affected_modules.as_mut())
        {
            affected_modules.sort();
        }
        let mut default_enabled = self.default_enabled;
        default_enabled.sort();
        default_enabled.dedup();
        let mut tenant_overrides = self.tenant_overrides;
        tenant_overrides.sort_by(|left, right| left.module_slug.cmp(&right.module_slug));
        let mut runtime_inputs = self.runtime_inputs;
        runtime_inputs.sort_by(|left, right| left.module_slug.cmp(&right.module_slug));
        let base_policy_revision = effective_policy_revision(
            self.catalog,
            &default_enabled,
            &tenant_overrides,
            &runtime_inputs,
            channel.as_ref(),
            maintenance.as_ref(),
            None,
        )?;
        validate_node_readiness_input(node_readiness.as_ref(), Some(&base_policy_revision))?;
        let policy_revision = effective_policy_revision(
            self.catalog,
            &default_enabled,
            &tenant_overrides,
            &runtime_inputs,
            channel.as_ref(),
            maintenance.as_ref(),
            node_readiness.as_ref(),
        )?;
        let tenant_override_by_slug = tenant_overrides
            .iter()
            .map(|item| (item.module_slug.as_str(), item.enabled))
            .collect::<BTreeMap<_, _>>();
        let runtime_input_by_slug = runtime_inputs
            .iter()
            .map(|input| (input.module_slug.as_str(), input))
            .collect::<BTreeMap<_, _>>();
        let mut enabled = self
            .catalog
            .definitions()
            .filter(|definition| definition.kind == ModuleDefinitionKind::Core)
            .map(|definition| definition.slug.clone())
            .collect::<BTreeSet<_>>();

        for slug in &default_enabled {
            if self
                .catalog
                .get(slug)
                .is_some_and(|definition| definition.kind == ModuleDefinitionKind::Optional)
            {
                enabled.insert(slug.clone());
            }
        }

        for module in &tenant_overrides {
            let Some(definition) = self.catalog.get(&module.module_slug) else {
                continue;
            };
            if definition.kind == ModuleDefinitionKind::Core {
                continue;
            }
            if module.enabled {
                enabled.insert(module.module_slug.clone());
            } else {
                enabled.remove(&module.module_slug);
            }
        }

        let mut denial_reasons = BTreeMap::<String, Vec<ModuleEffectivePolicyDenialReason>>::new();
        let channel_bindings = channel
            .as_ref()
            .map(|channel| {
                channel
                    .bindings
                    .iter()
                    .map(|binding| (binding.module_slug.as_str(), binding.enabled))
                    .collect::<BTreeMap<_, _>>()
            })
            .unwrap_or_default();
        if let Some(channel) = channel.as_ref() {
            if !channel.active {
                for definition in self.catalog.definitions() {
                    if enabled.remove(&definition.slug) {
                        denial_reasons
                            .entry(definition.slug.clone())
                            .or_default()
                            .push(ModuleEffectivePolicyDenialReason::ChannelInactive);
                    }
                }
            } else {
                for definition in self.catalog.definitions() {
                    if definition.kind == ModuleDefinitionKind::Core {
                        continue;
                    }
                    match channel_bindings.get(definition.slug.as_str()) {
                        None => {
                            if enabled.remove(&definition.slug) {
                                denial_reasons
                                    .entry(definition.slug.clone())
                                    .or_default()
                                    .push(
                                    ModuleEffectivePolicyDenialReason::ChannelBindingUnavailable,
                                );
                            }
                        }
                        Some(false) => {
                            if enabled.remove(&definition.slug) {
                                denial_reasons
                                    .entry(definition.slug.clone())
                                    .or_default()
                                    .push(ModuleEffectivePolicyDenialReason::ChannelDisabled);
                            }
                        }
                        Some(true) => {}
                    }
                }
            }
        }

        if let Some(maintenance) = maintenance.as_ref().filter(|input| input.active) {
            for definition in self.catalog.definitions() {
                let affected = maintenance
                    .affected_modules
                    .as_ref()
                    .is_none_or(|modules| modules.iter().any(|slug| slug == &definition.slug));
                if affected && enabled.remove(&definition.slug) {
                    denial_reasons
                        .entry(definition.slug.clone())
                        .or_default()
                        .push(ModuleEffectivePolicyDenialReason::MaintenanceActive);
                }
            }
        }

        if let Some(readiness) = node_readiness.as_ref().filter(|input| !input.ready) {
            for definition in self.catalog.definitions() {
                let affected = readiness
                    .affected_modules
                    .as_ref()
                    .is_none_or(|modules| modules.iter().any(|slug| slug == &definition.slug));
                if affected && enabled.remove(&definition.slug) {
                    denial_reasons
                        .entry(definition.slug.clone())
                        .or_default()
                        .push(ModuleEffectivePolicyDenialReason::NodeReadinessUnavailable);
                }
            }
        }

        for definition in self.catalog.definitions() {
            if !enabled.contains(&definition.slug) {
                denial_reasons
                    .entry(definition.slug.clone())
                    .or_insert_with(|| {
                        if tenant_override_by_slug.get(definition.slug.as_str()) == Some(&false) {
                            vec![ModuleEffectivePolicyDenialReason::TenantDisabled]
                        } else {
                            vec![ModuleEffectivePolicyDenialReason::NotSelected]
                        }
                    });
                continue;
            }
            if !matches!(&definition.source, ModuleDefinitionSource::Artifact { .. }) {
                continue;
            }
            let Some(runtime) = runtime_input_by_slug.get(definition.slug.as_str()) else {
                enabled.remove(&definition.slug);
                denial_reasons
                    .entry(definition.slug.clone())
                    .or_default()
                    .push(ModuleEffectivePolicyDenialReason::ArtifactInstallationUnavailable);
                continue;
            };
            let Some(installation) = runtime.installation.as_ref() else {
                enabled.remove(&definition.slug);
                denial_reasons
                    .entry(definition.slug.clone())
                    .or_default()
                    .push(ModuleEffectivePolicyDenialReason::ArtifactInstallationUnavailable);
                continue;
            };
            let ModuleDefinitionSource::Artifact { release } = &definition.source else {
                unreachable!("runtime policy inputs are evaluated only for artifact definitions");
            };
            if installation.installation_id.is_nil()
                || installation.release_digest != release.digest
                || installation.dependency_graph_revision == 0
                || !valid_digest(&installation.dependency_graph_digest)
                || installation.capability_grant_revision == 0
            {
                enabled.remove(&definition.slug);
                denial_reasons
                    .entry(definition.slug.clone())
                    .or_default()
                    .push(ModuleEffectivePolicyDenialReason::ArtifactInstallationUnavailable);
                continue;
            }
            if runtime.capability_policy_revision != Some(installation.capability_grant_revision) {
                enabled.remove(&definition.slug);
                denial_reasons
                    .entry(definition.slug.clone())
                    .or_default()
                    .push(ModuleEffectivePolicyDenialReason::CapabilityPolicyUnavailable);
            }
            if !runtime.executor_available {
                enabled.remove(&definition.slug);
                denial_reasons
                    .entry(definition.slug.clone())
                    .or_default()
                    .push(ModuleEffectivePolicyDenialReason::ExecutorUnavailable);
            }
            let Some(security) = runtime.security.as_ref() else {
                enabled.remove(&definition.slug);
                denial_reasons
                    .entry(definition.slug.clone())
                    .or_default()
                    .push(ModuleEffectivePolicyDenialReason::SecurityStateUnavailable);
                continue;
            };
            if security.release != *release {
                enabled.remove(&definition.slug);
                denial_reasons
                    .entry(definition.slug.clone())
                    .or_default()
                    .push(ModuleEffectivePolicyDenialReason::SecurityStateUnavailable);
                continue;
            }
            if security.registry_status == ModuleArtifactRegistryReleaseStatus::Unavailable {
                enabled.remove(&definition.slug);
                denial_reasons
                    .entry(definition.slug.clone())
                    .or_default()
                    .push(ModuleEffectivePolicyDenialReason::RegistryReleaseUnavailable);
            }
            match security.status {
                ModuleArtifactSecurityStatus::Clear => {}
                ModuleArtifactSecurityStatus::Quarantined => {
                    enabled.remove(&definition.slug);
                    denial_reasons
                        .entry(definition.slug.clone())
                        .or_default()
                        .push(ModuleEffectivePolicyDenialReason::Quarantined);
                }
                ModuleArtifactSecurityStatus::Revoked => {
                    enabled.remove(&definition.slug);
                    denial_reasons
                        .entry(definition.slug.clone())
                        .or_default()
                        .push(ModuleEffectivePolicyDenialReason::Revoked);
                }
            }
        }

        loop {
            let unavailable_dependencies = self
                .catalog
                .definitions()
                .filter(|definition| enabled.contains(&definition.slug))
                .filter_map(|definition| {
                    let missing = definition
                        .dependencies
                        .iter()
                        .filter(|dependency| !enabled.contains(&dependency.slug))
                        .map(|dependency| dependency.slug.clone())
                        .collect::<Vec<_>>();
                    (!missing.is_empty()).then(|| (definition.slug.clone(), missing))
                })
                .collect::<Vec<_>>();
            if unavailable_dependencies.is_empty() {
                break;
            }
            for (module_slug, dependencies) in unavailable_dependencies {
                enabled.remove(&module_slug);
                denial_reasons.entry(module_slug).or_default().extend(
                    dependencies.into_iter().map(|module_slug| {
                        ModuleEffectivePolicyDenialReason::DependencyUnavailable { module_slug }
                    }),
                );
            }
        }

        let default_enabled = default_enabled.into_iter().collect::<HashSet<_>>();
        let decisions = self
            .catalog
            .definitions()
            .map(|definition| {
                let is_enabled = enabled.contains(&definition.slug);
                let mut facts = vec![ModuleEffectivePolicyFact::Definition {
                    version: definition.version.clone(),
                    module_kind: definition.kind,
                    source: definition.source.clone(),
                }];
                facts.push(ModuleEffectivePolicyFact::PlatformDefault {
                    enabled: default_enabled.contains(&definition.slug),
                });
                if let Some(override_enabled) =
                    tenant_override_by_slug.get(definition.slug.as_str())
                {
                    facts.push(ModuleEffectivePolicyFact::TenantOverride {
                        enabled: *override_enabled,
                    });
                }
                if let Some(runtime) = runtime_input_by_slug.get(definition.slug.as_str()) {
                    if let Some(installation) = &runtime.installation {
                        facts.push(ModuleEffectivePolicyFact::ArtifactInstallation {
                            installation_id: installation.installation_id,
                            scope: installation.scope.clone(),
                            release_digest: installation.release_digest.clone(),
                            dependency_graph_revision: installation.dependency_graph_revision,
                            dependency_graph_digest: installation.dependency_graph_digest.clone(),
                            capability_grant_revision: installation.capability_grant_revision,
                        });
                        if runtime.capability_policy_revision
                            == Some(installation.capability_grant_revision)
                        {
                            facts.push(ModuleEffectivePolicyFact::CapabilityPolicy {
                                capability_grant_revision: installation.capability_grant_revision,
                            });
                        }
                        facts.push(ModuleEffectivePolicyFact::Executor {
                            payload_kind: installation.payload_kind,
                            available: runtime.executor_available,
                        });
                    }
                    if let Some(security) = &runtime.security {
                        facts.push(ModuleEffectivePolicyFact::RegistryRelease {
                            status: security.registry_status,
                        });
                        facts.push(ModuleEffectivePolicyFact::ArtifactSecurity {
                            revision: security.revision,
                            status: security.status,
                            policy_revision: security.policy_revision.clone(),
                            reason_code: security.reason_code.clone(),
                        });
                    }
                }
                if let Some(channel) = channel.as_ref() {
                    if let Some(enabled) = channel_bindings.get(definition.slug.as_str()) {
                        facts.push(ModuleEffectivePolicyFact::ChannelBinding {
                            channel_id: channel.channel_id,
                            surface: channel.surface.clone(),
                            channel_revision: channel.channel_revision.clone(),
                            enabled: *enabled,
                        });
                    }
                }
                if let Some(maintenance) = maintenance.as_ref() {
                    facts.push(ModuleEffectivePolicyFact::Maintenance {
                        maintenance_revision: maintenance.maintenance_revision.clone(),
                        active: maintenance.active,
                        reason_code: maintenance.reason_code.clone(),
                        affected_modules: maintenance.affected_modules.clone(),
                    });
                }
                if let Some(readiness) = node_readiness.as_ref() {
                    facts.push(ModuleEffectivePolicyFact::NodeReadiness {
                        node_id: readiness.node_id,
                        readiness_revision: readiness.readiness_revision.clone(),
                        observed_policy_revision: readiness.observed_policy_revision.clone(),
                        ready: readiness.ready,
                        required_core_ready: readiness.required_core_ready,
                        artifact_graph_revision: readiness.artifact_graph_revision,
                        cas_available: readiness.cas_available,
                        executor_abi: readiness.executor_abi.clone(),
                    });
                }
                let mut dependencies = definition.dependencies.iter().collect::<Vec<_>>();
                dependencies.sort_by(|left, right| left.slug.cmp(&right.slug));
                facts.extend(dependencies.into_iter().map(|dependency| {
                    ModuleEffectivePolicyFact::Dependency {
                        module_slug: dependency.slug.clone(),
                        enabled: enabled.contains(&dependency.slug),
                    }
                }));
                let module_denial_reasons = denial_reasons
                    .get(&definition.slug)
                    .cloned()
                    .unwrap_or_else(|| {
                        if is_enabled {
                            Vec::new()
                        } else {
                            vec![ModuleEffectivePolicyDenialReason::NotSelected]
                        }
                    });
                (
                    definition.slug.clone(),
                    ModuleEffectivePolicyDecision {
                        module_slug: definition.slug.clone(),
                        enabled: is_enabled,
                        policy_revision: policy_revision.clone(),
                        facts,
                        denial_reasons: module_denial_reasons,
                    },
                )
            })
            .collect();

        Ok(ModuleEffectivePolicy {
            policy_revision,
            enabled_modules: enabled,
            decisions,
        })
    }
}

impl ModuleEffectivePolicy {
    pub fn policy_revision(&self) -> &str {
        &self.policy_revision
    }

    pub fn contains(&self, module_slug: &str) -> bool {
        self.enabled_modules.contains(module_slug)
    }

    pub fn decision(&self, module_slug: &str) -> ModuleEffectivePolicyDecision {
        self.decisions
            .get(module_slug)
            .cloned()
            .unwrap_or_else(|| ModuleEffectivePolicyDecision {
                module_slug: module_slug.to_string(),
                enabled: false,
                policy_revision: self.policy_revision.clone(),
                facts: Vec::new(),
                denial_reasons: vec![ModuleEffectivePolicyDenialReason::UnknownModule],
            })
    }

    pub fn decisions(&self) -> impl Iterator<Item = &ModuleEffectivePolicyDecision> {
        self.decisions.values()
    }

    pub fn into_enabled_modules(self) -> HashSet<String> {
        self.enabled_modules.into_iter().collect()
    }
}

#[derive(Serialize)]
struct EffectivePolicyRevisionInput<'a> {
    contract: &'static str,
    catalog: &'a ModuleDefinitionCatalog,
    default_enabled: &'a [String],
    tenant_overrides: &'a [TenantModuleOverride],
    runtime_inputs: &'a [ModuleEffectivePolicyRuntimeInput],
    channel: Option<&'a ModuleEffectivePolicyChannelInput>,
    maintenance: Option<&'a ModuleEffectivePolicyMaintenanceInput>,
    node_readiness: Option<&'a ModuleEffectivePolicyNodeReadinessInput>,
}

fn effective_policy_revision(
    catalog: &ModuleDefinitionCatalog,
    default_enabled: &[String],
    tenant_overrides: &[TenantModuleOverride],
    runtime_inputs: &[ModuleEffectivePolicyRuntimeInput],
    channel: Option<&ModuleEffectivePolicyChannelInput>,
    maintenance: Option<&ModuleEffectivePolicyMaintenanceInput>,
    node_readiness: Option<&ModuleEffectivePolicyNodeReadinessInput>,
) -> Result<String, ModuleEffectivePolicyError> {
    let digest = hash_manifest(&EffectivePolicyRevisionInput {
        contract: "rustok.module_effective_policy",
        catalog,
        default_enabled,
        tenant_overrides,
        runtime_inputs,
        channel,
        maintenance,
        node_readiness,
    })
    .map_err(|error| ModuleEffectivePolicyError::RevisionEncoding(error.to_string()))?;
    Ok(format!("sha256:{digest}"))
}

fn validate_channel_input(
    channel: Option<&ModuleEffectivePolicyChannelInput>,
) -> Result<(), ModuleEffectivePolicyError> {
    let Some(channel) = channel else {
        return Ok(());
    };
    if channel.tenant_id.is_nil() || channel.channel_id.is_nil() {
        return Err(ModuleEffectivePolicyError::InvalidChannelInput(
            "tenant_id and channel_id must be non-nil UUIDs".to_string(),
        ));
    }
    if !valid_text(&channel.surface, 64) {
        return Err(ModuleEffectivePolicyError::InvalidChannelInput(
            "surface must be non-empty and at most 64 characters".to_string(),
        ));
    }
    if !valid_digest(&channel.channel_revision) {
        return Err(ModuleEffectivePolicyError::InvalidChannelInput(
            "channel_revision must be a sha256 digest".to_string(),
        ));
    }
    let mut seen = BTreeSet::new();
    for binding in &channel.bindings {
        if !valid_text(&binding.module_slug, 128) {
            return Err(ModuleEffectivePolicyError::InvalidChannelInput(
                "channel module slugs must be non-empty and at most 128 characters".to_string(),
            ));
        }
        if !seen.insert(binding.module_slug.as_str()) {
            return Err(ModuleEffectivePolicyError::InvalidChannelInput(
                "channel module bindings must not contain duplicate module slugs".to_string(),
            ));
        }
    }
    Ok(())
}

fn validate_maintenance_input(
    maintenance: Option<&ModuleEffectivePolicyMaintenanceInput>,
) -> Result<(), ModuleEffectivePolicyError> {
    let Some(maintenance) = maintenance else {
        return Ok(());
    };
    if !valid_digest(&maintenance.maintenance_revision) {
        return Err(ModuleEffectivePolicyError::InvalidMaintenanceInput(
            "maintenance_revision must be a sha256 digest".to_string(),
        ));
    }
    if !valid_text(&maintenance.reason_code, 128) {
        return Err(ModuleEffectivePolicyError::InvalidMaintenanceInput(
            "reason_code must be non-empty and at most 128 characters".to_string(),
        ));
    }
    if let Some(affected_modules) = &maintenance.affected_modules {
        let mut seen = BTreeSet::new();
        for module_slug in affected_modules {
            if !valid_text(module_slug, 128) {
                return Err(ModuleEffectivePolicyError::InvalidMaintenanceInput(
                    "affected module slugs must be non-empty and at most 128 characters"
                        .to_string(),
                ));
            }
            if !seen.insert(module_slug.as_str()) {
                return Err(ModuleEffectivePolicyError::InvalidMaintenanceInput(
                    "affected module slugs must not contain duplicates".to_string(),
                ));
            }
        }
    }
    Ok(())
}

fn validate_node_readiness_input(
    readiness: Option<&ModuleEffectivePolicyNodeReadinessInput>,
    expected_base_policy_revision: Option<&str>,
) -> Result<(), ModuleEffectivePolicyError> {
    let Some(readiness) = readiness else {
        return Ok(());
    };
    if readiness.node_id.is_nil() {
        return Err(ModuleEffectivePolicyError::InvalidNodeReadinessInput(
            "node_id must be a non-nil UUID".to_string(),
        ));
    }
    if !valid_digest(&readiness.readiness_revision) {
        return Err(ModuleEffectivePolicyError::InvalidNodeReadinessInput(
            "readiness_revision must be a sha256 digest".to_string(),
        ));
    }
    if !valid_digest(&readiness.observed_policy_revision) {
        return Err(ModuleEffectivePolicyError::InvalidNodeReadinessInput(
            "observed_policy_revision must be a sha256 digest".to_string(),
        ));
    }
    if let Some(expected) = expected_base_policy_revision {
        if readiness.observed_policy_revision != expected {
            return Err(ModuleEffectivePolicyError::InvalidNodeReadinessInput(
                "observed_policy_revision does not match the base policy revision".to_string(),
            ));
        }
    }
    if readiness
        .artifact_graph_revision
        .is_some_and(|revision| revision == 0)
    {
        return Err(ModuleEffectivePolicyError::InvalidNodeReadinessInput(
            "artifact_graph_revision must be positive when present".to_string(),
        ));
    }
    if let Some(executor_abi) = &readiness.executor_abi {
        if !valid_text(executor_abi, 128) {
            return Err(ModuleEffectivePolicyError::InvalidNodeReadinessInput(
                "executor_abi must be non-empty and at most 128 characters".to_string(),
            ));
        }
    }
    if readiness.ready
        && (!readiness.required_core_ready
            || !readiness.cas_available
            || readiness.executor_abi.is_none())
    {
        return Err(ModuleEffectivePolicyError::InvalidNodeReadinessInput(
            "ready snapshots must include core, CAS, and executor ABI evidence".to_string(),
        ));
    }
    if let Some(affected_modules) = &readiness.affected_modules {
        let mut seen = BTreeSet::new();
        for module_slug in affected_modules {
            if !valid_text(module_slug, 128) {
                return Err(ModuleEffectivePolicyError::InvalidNodeReadinessInput(
                    "affected module slugs must be non-empty and at most 128 characters"
                        .to_string(),
                ));
            }
            if !seen.insert(module_slug.as_str()) {
                return Err(ModuleEffectivePolicyError::InvalidNodeReadinessInput(
                    "affected module slugs must not contain duplicates".to_string(),
                ));
            }
        }
    }
    Ok(())
}

fn valid_text(value: &str, max_chars: usize) -> bool {
    !value.trim().is_empty() && value.chars().count() <= max_chars
}

#[cfg(test)]
mod tests {
    use super::{
        ModuleEffectivePolicyChannelBinding, ModuleEffectivePolicyChannelInput,
        ModuleEffectivePolicyDenialReason, ModuleEffectivePolicyFact,
        ModuleEffectivePolicyInstallationFact, ModuleEffectivePolicyMaintenanceInput,
        ModuleEffectivePolicyNodeReadinessInput, ModuleEffectivePolicyQuery,
        ModuleEffectivePolicyRuntimeInput, ModulePolicyRevisionApplyOutcome,
        ModulePolicyRevisionGate, ModulePolicyRevisionGateError, ModulePolicyRevisionTransition,
        TenantModuleOverride,
    };
    use crate::{
        ArtifactPayloadKind, ArtifactReleaseRef, ModuleArtifactRegistryReleaseStatus,
        ModuleArtifactSecuritySnapshot, ModuleArtifactSecurityStatus, ModuleDefinition,
        ModuleDefinitionCatalog, ModuleDefinitionKind, ModuleDefinitionSource,
        ModuleInstallationScope, ModulesModule,
    };
    use rustok_core::ModuleRegistry;
    use uuid::Uuid;

    #[test]
    fn channel_snapshot_is_part_of_revision_and_can_disable_the_projection() {
        let catalog = ModuleDefinitionCatalog::from_static_registry(
            &ModuleRegistry::new().register(ModulesModule),
        )
        .expect("catalog");
        let channel = ModuleEffectivePolicyChannelInput {
            tenant_id: Uuid::new_v4(),
            channel_id: Uuid::new_v4(),
            surface: "http".to_string(),
            channel_revision: digest('c'),
            active: false,
            bindings: vec![ModuleEffectivePolicyChannelBinding {
                module_slug: "modules".to_string(),
                enabled: true,
            }],
        };
        let policy = ModuleEffectivePolicyQuery::new_with_channel(
            &catalog,
            ["modules".to_string()],
            [],
            [],
            Some(channel.clone()),
        )
        .execute()
        .expect("policy");

        assert!(!policy.contains("modules"));
        assert_eq!(
            policy.decision("modules").denial_reasons,
            vec![ModuleEffectivePolicyDenialReason::ChannelInactive]
        );

        let without_channel =
            ModuleEffectivePolicyQuery::new(&catalog, ["modules".to_string()], [], [])
                .execute()
                .expect("policy");
        assert_ne!(policy.policy_revision(), without_channel.policy_revision());
        assert!(policy.decision("modules").facts.iter().any(|fact| matches!(
            fact,
            ModuleEffectivePolicyFact::ChannelBinding { enabled: true, .. }
        )));
    }

    #[test]
    fn channel_snapshot_rejects_duplicate_bindings() {
        let catalog = ModuleDefinitionCatalog::from_static_registry(
            &ModuleRegistry::new().register(ModulesModule),
        )
        .expect("catalog");
        let error = ModuleEffectivePolicyQuery::new_with_channel(
            &catalog,
            [],
            [],
            [],
            Some(ModuleEffectivePolicyChannelInput {
                tenant_id: Uuid::new_v4(),
                channel_id: Uuid::new_v4(),
                surface: "http".to_string(),
                channel_revision: digest('c'),
                active: true,
                bindings: vec![
                    ModuleEffectivePolicyChannelBinding {
                        module_slug: "modules".to_string(),
                        enabled: true,
                    },
                    ModuleEffectivePolicyChannelBinding {
                        module_slug: "modules".to_string(),
                        enabled: false,
                    },
                ],
            }),
        )
        .execute()
        .expect_err("duplicate channel bindings must be rejected");
        assert!(matches!(
            error,
            super::ModuleEffectivePolicyError::InvalidChannelInput(_)
        ));
    }

    #[test]
    fn active_maintenance_blocks_selected_modules_without_changing_tenant_intent() {
        let catalog = ModuleDefinitionCatalog::from_static_registry(
            &ModuleRegistry::new().register(ModulesModule),
        )
        .expect("catalog");
        let maintenance = ModuleEffectivePolicyMaintenanceInput {
            maintenance_revision: digest('a'),
            active: true,
            reason_code: "planned_upgrade".to_string(),
            affected_modules: Some(vec!["modules".to_string()]),
        };
        let policy = ModuleEffectivePolicyQuery::new_with_inputs(
            &catalog,
            ["modules".to_string()],
            [],
            [],
            None,
            Some(maintenance),
        )
        .execute()
        .expect("policy");

        assert!(!policy.contains("modules"));
        assert_eq!(
            policy.decision("modules").denial_reasons,
            vec![ModuleEffectivePolicyDenialReason::MaintenanceActive]
        );
        assert!(policy.decision("modules").facts.iter().any(|fact| matches!(
            fact,
            ModuleEffectivePolicyFact::Maintenance {
                active: true,
                reason_code,
                ..
            } if reason_code == "planned_upgrade"
        )));
    }

    #[test]
    fn maintenance_revision_is_validated_and_order_normalized() {
        let catalog = ModuleDefinitionCatalog::from_static_registry(
            &ModuleRegistry::new().register(ModulesModule),
        )
        .expect("catalog");
        let left = ModuleEffectivePolicyQuery::new_with_inputs(
            &catalog,
            [],
            [],
            [],
            None,
            Some(ModuleEffectivePolicyMaintenanceInput {
                maintenance_revision: digest('a'),
                active: false,
                reason_code: "none".to_string(),
                affected_modules: Some(vec!["zeta".to_string(), "alpha".to_string()]),
            }),
        )
        .execute()
        .expect("policy");
        let right = ModuleEffectivePolicyQuery::new_with_inputs(
            &catalog,
            [],
            [],
            [],
            None,
            Some(ModuleEffectivePolicyMaintenanceInput {
                maintenance_revision: digest('a'),
                active: false,
                reason_code: "none".to_string(),
                affected_modules: Some(vec!["alpha".to_string(), "zeta".to_string()]),
            }),
        )
        .execute()
        .expect("policy");
        assert_eq!(left.policy_revision(), right.policy_revision());

        let error = ModuleEffectivePolicyQuery::new_with_inputs(
            &catalog,
            [],
            [],
            [],
            None,
            Some(ModuleEffectivePolicyMaintenanceInput {
                maintenance_revision: "not-a-digest".to_string(),
                active: true,
                reason_code: "planned_upgrade".to_string(),
                affected_modules: None,
            }),
        )
        .execute()
        .expect_err("invalid maintenance revision must be rejected");
        assert!(matches!(
            error,
            super::ModuleEffectivePolicyError::InvalidMaintenanceInput(_)
        ));
    }

    #[test]
    fn node_readiness_must_observe_base_revision_and_blocks_affected_modules() {
        let catalog = ModuleDefinitionCatalog::from_static_registry(
            &ModuleRegistry::new().register(ModulesModule),
        )
        .expect("catalog");
        let base = ModuleEffectivePolicyQuery::new(&catalog, ["modules".to_string()], [], [])
            .execute()
            .expect("base policy");
        let node_readiness = ModuleEffectivePolicyNodeReadinessInput {
            node_id: Uuid::new_v4(),
            readiness_revision: digest('b'),
            observed_policy_revision: base.policy_revision().to_string(),
            ready: false,
            required_core_ready: false,
            artifact_graph_revision: None,
            cas_available: false,
            executor_abi: None,
            affected_modules: Some(vec!["modules".to_string()]),
        };
        let policy = ModuleEffectivePolicyQuery::new_with_context(
            &catalog,
            ["modules".to_string()],
            [],
            [],
            None,
            None,
            Some(node_readiness),
        )
        .execute()
        .expect("policy");

        assert!(!policy.contains("modules"));
        assert_eq!(
            policy.decision("modules").denial_reasons,
            vec![ModuleEffectivePolicyDenialReason::NodeReadinessUnavailable]
        );
        assert!(policy.decision("modules").facts.iter().any(|fact| matches!(
            fact,
            ModuleEffectivePolicyFact::NodeReadiness { ready: false, .. }
        )));
        assert_ne!(policy.policy_revision(), base.policy_revision());

        let stale = ModuleEffectivePolicyQuery::new_with_context(
            &catalog,
            ["modules".to_string()],
            [],
            [],
            None,
            None,
            Some(ModuleEffectivePolicyNodeReadinessInput {
                node_id: Uuid::new_v4(),
                readiness_revision: digest('b'),
                observed_policy_revision: digest('c'),
                ready: false,
                required_core_ready: false,
                artifact_graph_revision: None,
                cas_available: false,
                executor_abi: None,
                affected_modules: None,
            }),
        )
        .execute()
        .expect_err("stale readiness must fail closed");
        assert!(matches!(
            stale,
            super::ModuleEffectivePolicyError::InvalidNodeReadinessInput(_)
        ));
    }

    #[test]
    fn revision_gate_applies_only_matching_predecessors_and_replays_duplicates() {
        let first = digest('a');
        let second = digest('b');
        let third = digest('c');
        let mut gate = ModulePolicyRevisionGate::new(None).expect("empty gate");

        assert_eq!(
            gate.apply(&ModulePolicyRevisionTransition {
                previous_revision: None,
                next_revision: first.clone(),
            }),
            Ok(ModulePolicyRevisionApplyOutcome::Applied)
        );
        assert_eq!(gate.current_revision(), Some(first.as_str()));
        assert_eq!(
            gate.apply(&ModulePolicyRevisionTransition {
                previous_revision: None,
                next_revision: first.clone(),
            }),
            Ok(ModulePolicyRevisionApplyOutcome::Duplicate)
        );
        assert_eq!(
            gate.apply(&ModulePolicyRevisionTransition {
                previous_revision: Some(third.clone()),
                next_revision: second.clone(),
            }),
            Ok(ModulePolicyRevisionApplyOutcome::Stale)
        );
        assert_eq!(gate.current_revision(), Some(first.as_str()));
        assert_eq!(
            gate.apply(&ModulePolicyRevisionTransition {
                previous_revision: Some(first),
                next_revision: second.clone(),
            }),
            Ok(ModulePolicyRevisionApplyOutcome::Applied)
        );
        assert_eq!(gate.current_revision(), Some(second.as_str()));

        let error = gate
            .apply(&ModulePolicyRevisionTransition {
                previous_revision: Some(second),
                next_revision: "invalid".to_string(),
            })
            .expect_err("invalid successor must fail closed");
        assert_eq!(error, ModulePolicyRevisionGateError::InvalidNextRevision);
    }

    #[test]
    fn core_is_immutable_and_overrides_require_registered_optional_modules() {
        let catalog = ModuleDefinitionCatalog::from_static_registry(
            &ModuleRegistry::new().register(ModulesModule),
        )
        .expect("catalog");
        let policy = ModuleEffectivePolicyQuery::new(
            &catalog,
            ["modules".to_string(), "missing".to_string()],
            [
                TenantModuleOverride {
                    module_slug: "modules".to_string(),
                    enabled: false,
                },
                TenantModuleOverride {
                    module_slug: "persisted-legacy-override".to_string(),
                    enabled: true,
                },
            ],
            [],
        )
        .execute()
        .expect("policy");

        assert!(policy.contains("modules"));
        assert!(!policy.contains("missing"));
        assert!(!policy.contains("persisted-legacy-override"));
        assert!(policy.policy_revision().starts_with("sha256:"));
        assert_eq!(policy.policy_revision().len(), 71);

        let core = policy.decision("modules");
        assert!(core.enabled);
        assert!(core.denial_reasons.is_empty());
        assert!(core.facts.iter().any(|fact| matches!(
            fact,
            ModuleEffectivePolicyFact::TenantOverride { enabled: false }
        )));

        let unknown = policy.decision("missing");
        assert!(!unknown.enabled);
        assert_eq!(
            unknown.denial_reasons,
            vec![ModuleEffectivePolicyDenialReason::UnknownModule]
        );
    }

    #[test]
    fn policy_revision_is_independent_of_input_order() {
        let catalog = ModuleDefinitionCatalog::from_static_registry(
            &ModuleRegistry::new().register(ModulesModule),
        )
        .expect("catalog");
        let left = ModuleEffectivePolicyQuery::new(
            &catalog,
            ["z".to_string(), "modules".to_string()],
            [
                TenantModuleOverride {
                    module_slug: "z".to_string(),
                    enabled: true,
                },
                TenantModuleOverride {
                    module_slug: "modules".to_string(),
                    enabled: false,
                },
            ],
            [],
        )
        .execute()
        .expect("left policy");
        let right = ModuleEffectivePolicyQuery::new(
            &catalog,
            ["modules".to_string(), "z".to_string()],
            [
                TenantModuleOverride {
                    module_slug: "modules".to_string(),
                    enabled: false,
                },
                TenantModuleOverride {
                    module_slug: "z".to_string(),
                    enabled: true,
                },
            ],
            [],
        )
        .execute()
        .expect("right policy");

        assert_eq!(left.policy_revision(), right.policy_revision());
    }

    #[test]
    fn selected_artifact_requires_exact_runtime_evidence() {
        let mut catalog = ModuleDefinitionCatalog::default();
        catalog
            .insert(ModuleDefinition {
                slug: "artifact".to_string(),
                version: "1.2.3".to_string(),
                kind: ModuleDefinitionKind::Optional,
                source: ModuleDefinitionSource::Artifact {
                    release: ArtifactReleaseRef {
                        slug: "artifact".to_string(),
                        version: "1.2.3".to_string(),
                        digest: digest('a'),
                    },
                },
                dependencies: Vec::new(),
                permissions: Vec::new(),
                settings_schema_digest: None,
                schema_documents: Vec::new(),
                bindings: Vec::new(),
                ui: Vec::new(),
                capabilities: Vec::new(),
            })
            .expect("artifact definition");

        let unavailable =
            ModuleEffectivePolicyQuery::new(&catalog, ["artifact".to_string()], [], [])
                .execute()
                .expect("unavailable policy");
        assert_eq!(
            unavailable.decision("artifact").denial_reasons,
            vec![ModuleEffectivePolicyDenialReason::ArtifactInstallationUnavailable]
        );

        let available = ModuleEffectivePolicyQuery::new(
            &catalog,
            ["artifact".to_string()],
            [],
            [ModuleEffectivePolicyRuntimeInput {
                module_slug: "artifact".to_string(),
                installation: Some(ModuleEffectivePolicyInstallationFact {
                    installation_id: uuid::Uuid::from_u128(1),
                    scope: ModuleInstallationScope::Platform,
                    release_digest: digest('a'),
                    payload_kind: ArtifactPayloadKind::Rhai,
                    dependency_graph_revision: 4,
                    dependency_graph_digest: digest('b'),
                    capability_grant_revision: 7,
                }),
                capability_policy_revision: Some(7),
                executor_available: true,
                security: Some(ModuleArtifactSecuritySnapshot {
                    release: ArtifactReleaseRef {
                        slug: "artifact".to_string(),
                        version: "1.2.3".to_string(),
                        digest: digest('a'),
                    },
                    revision: 0,
                    status: ModuleArtifactSecurityStatus::Clear,
                    registry_status: ModuleArtifactRegistryReleaseStatus::Unlisted,
                    policy_revision: None,
                    reason_code: None,
                    reason_detail: None,
                }),
            }],
        )
        .execute()
        .expect("available policy");
        let decision = available.decision("artifact");
        assert!(decision.enabled);
        assert!(decision.denial_reasons.is_empty());
        assert!(decision.facts.iter().any(|fact| matches!(
            fact,
            ModuleEffectivePolicyFact::CapabilityPolicy {
                capability_grant_revision: 7
            }
        )));
    }

    fn digest(character: char) -> String {
        format!("sha256:{}", character.to_string().repeat(64))
    }
}
