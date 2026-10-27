use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    sync::Arc,
};

use async_trait::async_trait;
use rustok_api::{
    ModuleEffectivePolicyDecisionView, ModuleEffectivePolicyDenialReasonView,
    ModuleEffectivePolicyView, is_valid_module_slug, manifest_hash::canonical_json_bytes,
};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::artifact::parse_module_version_requirement;
use crate::installation::valid_digest;
use crate::{
    ArtifactExecutionAdmission, ArtifactInstallationTarget, ArtifactPayloadKind,
    ArtifactReleaseRef, ModuleArtifactRegistryReleaseStatus, ModuleArtifactSecuritySnapshot,
    ModuleArtifactSecurityStatus, ModuleDefinitionCatalog, ModuleDefinitionKind,
    ModuleDefinitionSource, ModuleDependencyConstraint, ModuleInstallationScope,
};

/// A persisted tenant-level module enablement override.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TenantModuleOverride {
    pub module_slug: String,
    pub enabled: bool,
}

/// Deployment-admitted availability requirement evaluated by effective policy.
///
/// Co-requisites deliberately do not participate in dependency or migration
/// ordering. Hosts supply this normalized owner input separately from the
/// definition catalog's ordinary `dependencies` edges.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleEffectivePolicyCoRequisite {
    pub module_slug: String,
    pub required_module_slug: String,
    pub version_requirement: String,
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
    CoRequisite {
        module_slug: String,
        version_requirement: String,
        version: String,
        enabled: bool,
        compatible: bool,
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
    DependencyVersionMismatch { module_slug: String },
    CoRequisiteUnavailable { module_slug: String },
    CoRequisiteVersionMismatch { module_slug: String },
    RegistryReleaseUnavailable,
    SecurityStateUnavailable,
    Quarantined,
    Revoked,
    ChannelInactive,
    ChannelBindingUnavailable,
    ChannelDisabled,
    MaintenanceActive,
}

impl From<ModuleEffectivePolicyDenialReason> for ModuleEffectivePolicyDenialReasonView {
    fn from(reason: ModuleEffectivePolicyDenialReason) -> Self {
        match reason {
            ModuleEffectivePolicyDenialReason::UnknownModule => Self::UnknownModule,
            ModuleEffectivePolicyDenialReason::NotSelected => Self::NotSelected,
            ModuleEffectivePolicyDenialReason::TenantDisabled => Self::TenantDisabled,
            ModuleEffectivePolicyDenialReason::ArtifactInstallationUnavailable => {
                Self::ArtifactInstallationUnavailable
            }
            ModuleEffectivePolicyDenialReason::CapabilityPolicyUnavailable => {
                Self::CapabilityPolicyUnavailable
            }
            ModuleEffectivePolicyDenialReason::ExecutorUnavailable => Self::ExecutorUnavailable,
            ModuleEffectivePolicyDenialReason::DependencyUnavailable { module_slug } => {
                Self::DependencyUnavailable { module_slug }
            }
            ModuleEffectivePolicyDenialReason::DependencyVersionMismatch { module_slug } => {
                Self::DependencyVersionMismatch { module_slug }
            }
            ModuleEffectivePolicyDenialReason::CoRequisiteUnavailable { module_slug } => {
                Self::CoRequisiteUnavailable { module_slug }
            }
            ModuleEffectivePolicyDenialReason::CoRequisiteVersionMismatch { module_slug } => {
                Self::CoRequisiteVersionMismatch { module_slug }
            }
            ModuleEffectivePolicyDenialReason::RegistryReleaseUnavailable => {
                Self::RegistryReleaseUnavailable
            }
            ModuleEffectivePolicyDenialReason::SecurityStateUnavailable => {
                Self::SecurityStateUnavailable
            }
            ModuleEffectivePolicyDenialReason::Quarantined => Self::Quarantined,
            ModuleEffectivePolicyDenialReason::Revoked => Self::Revoked,
            ModuleEffectivePolicyDenialReason::ChannelInactive => Self::ChannelInactive,
            ModuleEffectivePolicyDenialReason::ChannelBindingUnavailable => {
                Self::ChannelBindingUnavailable
            }
            ModuleEffectivePolicyDenialReason::ChannelDisabled => Self::ChannelDisabled,
            ModuleEffectivePolicyDenialReason::MaintenanceActive => Self::MaintenanceActive,
        }
    }
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

/// Tenant-scoped content identity for a resolved effective-policy cache entry.
///
/// A TTL, process generation, or tenant id alone is never sufficient for a
/// cache hit. Consumers must match both fields against the current owner
/// decision before using cached policy facts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectivePolicyCacheIdentity {
    tenant_id: uuid::Uuid,
    policy_revision: String,
}

impl EffectivePolicyCacheIdentity {
    fn new(
        tenant_id: uuid::Uuid,
        policy_revision: String,
    ) -> Result<Self, ModuleEffectivePolicyError> {
        let identity = Self {
            tenant_id,
            policy_revision,
        };
        identity.validate()?;
        Ok(identity)
    }

    pub fn tenant_id(&self) -> uuid::Uuid {
        self.tenant_id
    }

    pub fn policy_revision(&self) -> &str {
        &self.policy_revision
    }

    pub fn validate(&self) -> Result<(), ModuleEffectivePolicyError> {
        if self.tenant_id.is_nil() {
            return Err(ModuleEffectivePolicyError::InvalidCacheIdentity(
                "tenant_id must be a non-nil UUID".to_string(),
            ));
        }
        if !valid_digest(&self.policy_revision) {
            return Err(ModuleEffectivePolicyError::InvalidCacheIdentity(
                "policy_revision must be a sha256 digest".to_string(),
            ));
        }
        Ok(())
    }

    pub fn matches(&self, tenant_id: uuid::Uuid, policy_revision: &str) -> bool {
        self.validate().is_ok()
            && self.tenant_id == tenant_id
            && self.policy_revision == policy_revision
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ModuleEffectivePolicyError {
    #[error("effective module policy revision could not be encoded: {0}")]
    RevisionEncoding(String),
    #[error("effective module policy cache identity is invalid: {0}")]
    InvalidCacheIdentity(String),
    #[error("effective module policy snapshot is invalid: {0}")]
    InvalidPolicySnapshot(String),
    #[error("effective module policy tenant overrides contain a duplicate entry for module `{0}`")]
    DuplicateTenantOverride(String),
    #[error("effective module policy runtime inputs contain a duplicate entry for module `{0}`")]
    DuplicateRuntimeInput(String),
    #[error("effective module policy co-requisite input is invalid: {0}")]
    InvalidCoRequisiteInput(String),
    #[error("effective module policy channel input is invalid: {0}")]
    InvalidChannelInput(String),
    #[error("effective module policy maintenance input is invalid: {0}")]
    InvalidMaintenanceInput(String),
    #[error("effective module policy cache lock is poisoned")]
    CacheLockPoisoned,
}

/// One owner transition delivered to a revision-aware outbox consumer. Hashes
/// are identities, not sortable clocks; consumers apply a transition
/// incrementally only when the predecessor matches their durable cursor. A
/// gap reports `ModulePolicyRevisionApplyOutcome::ResyncRequired` without
/// moving the cursor: the consumer must explicitly resync from a full policy
/// snapshot before any later incremental transition can be trusted.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModulePolicyRevisionTransition {
    pub previous_revision: Option<String>,
    pub next_revision: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModulePolicyRevisionApplyOutcome {
    /// The predecessor matched the cursor; the transition was applied
    /// incrementally.
    Applied,
    /// The successor is already the cursor; nothing changed.
    Duplicate,
    /// The predecessor did not match the cursor (at least one transition was
    /// missed). The cursor did not move. The consumer must reload the full
    /// policy snapshot and explicitly resync the cursor to that snapshot
    /// instead of trusting incremental state.
    ResyncRequired,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModulePolicyRevisionGate {
    current_revision: Option<String>,
    /// Durable cursor generation when the caller loaded one. Revision-only
    /// validation contexts do not invent a sequence for an existing digest.
    #[serde(default)]
    current_sequence: Option<u64>,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ModulePolicyRevisionGateError {
    #[error("policy revision transition predecessor is invalid")]
    InvalidPreviousRevision,
    #[error("policy revision transition successor is invalid")]
    InvalidNextRevision,
    #[error("policy revision transition is a no-op")]
    NoopTransition,
    #[error("policy revision cursor sequence does not match its revision state")]
    InvalidSequenceState,
    #[error("policy revision cursor sequence is exhausted")]
    SequenceExhausted,
}

impl ModulePolicyRevisionGate {
    pub fn new(current_revision: Option<String>) -> Result<Self, ModulePolicyRevisionGateError> {
        if current_revision
            .as_deref()
            .is_some_and(|revision| !valid_digest(revision))
        {
            return Err(ModulePolicyRevisionGateError::InvalidPreviousRevision);
        }
        let current_sequence = if current_revision.is_none() {
            Some(0)
        } else {
            None
        };
        Ok(Self {
            current_revision,
            current_sequence,
        })
    }

    /// Builds a gate from one durable cursor state. An empty cursor is the
    /// sequence-zero state; a populated pre-sequencing cursor may also carry
    /// sequence zero and receives sequence one on its next durable advance.
    pub fn with_sequence(
        current_revision: Option<String>,
        current_sequence: u64,
    ) -> Result<Self, ModulePolicyRevisionGateError> {
        if current_revision
            .as_deref()
            .is_some_and(|revision| !valid_digest(revision))
        {
            return Err(ModulePolicyRevisionGateError::InvalidPreviousRevision);
        }
        if current_revision.is_none() && current_sequence != 0 {
            return Err(ModulePolicyRevisionGateError::InvalidSequenceState);
        }
        Ok(Self {
            current_revision,
            current_sequence: Some(current_sequence),
        })
    }

    pub fn current_revision(&self) -> Option<&str> {
        self.current_revision.as_deref()
    }

    pub const fn current_sequence(&self) -> Option<u64> {
        self.current_sequence
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
            // A gap proves this gate did not observe the owner's immediately
            // preceding state. Do not fabricate continuity by jumping to the
            // successor: leave both revision and sequence untouched until the
            // caller explicitly resyncs from a full snapshot.
            return Ok(ModulePolicyRevisionApplyOutcome::ResyncRequired);
        }
        self.advance_to(transition.next_revision.clone())?;
        Ok(ModulePolicyRevisionApplyOutcome::Applied)
    }

    /// Explicitly replaces incremental state with a full-snapshot revision.
    /// Returns `true` only when the revision and known sequence advanced.
    pub fn resync_to(
        &mut self,
        snapshot_revision: &str,
    ) -> Result<bool, ModulePolicyRevisionGateError> {
        if !valid_digest(snapshot_revision) {
            return Err(ModulePolicyRevisionGateError::InvalidNextRevision);
        }
        if self.current_revision.as_deref() == Some(snapshot_revision) {
            return Ok(false);
        }
        self.advance_to(snapshot_revision.to_string())?;
        Ok(true)
    }

    fn advance_to(&mut self, next_revision: String) -> Result<(), ModulePolicyRevisionGateError> {
        let next_sequence = match self.current_sequence {
            Some(current_sequence) => Some(
                current_sequence
                    .checked_add(1)
                    .ok_or(ModulePolicyRevisionGateError::SequenceExhausted)?,
            ),
            None => None,
        };
        self.current_revision = Some(next_revision);
        self.current_sequence = next_sequence;
        Ok(())
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
    #[error("invalid module slug `{0}`")]
    InvalidModuleSlug(String),
    #[error("unknown module")]
    UnknownModule,
    #[error("module `{0}` is a core platform module and cannot be disabled")]
    CoreModuleCannotBeDisabled(String),
    #[error("missing module dependencies: {0:?}")]
    MissingDependencies(Vec<String>),
    #[error("module dependencies with incompatible versions: {0:?}")]
    IncompatibleDependencies(Vec<String>),
    #[error("module has enabled dependents: {0:?}")]
    HasDependents(Vec<String>),
}

/// Validates a requested module enablement change against the effective module
/// set and definition topology, including declared dependency version
/// requirements. Persistence, operation journaling and lifecycle hooks are
/// intentionally outside this owner policy function.
///
/// Layering contract: this validates *tenant intent admission* against the
/// catalog topology. Co-requisites deliberately do not participate — they are
/// availability constraints without ordering edges, evaluated by
/// `ModuleEffectivePolicyQuery` — and neither do channel/maintenance state or
/// runtime evidence (installation, executor, security), which are effective
/// availability inputs. Recording intent for a temporarily unavailable module
/// is therefore valid; the effective policy decides availability and explains
/// denials.
pub fn validate_module_toggle(
    catalog: &ModuleDefinitionCatalog,
    enabled_modules: &HashSet<String>,
    module_slug: &str,
    enabled: bool,
) -> Result<(), ModuleToggleValidationError> {
    if !is_valid_module_slug(module_slug) {
        return Err(ModuleToggleValidationError::InvalidModuleSlug(
            module_slug.to_string(),
        ));
    }
    let Some(module) = catalog.get(module_slug) else {
        return Err(ModuleToggleValidationError::UnknownModule);
    };

    if !enabled && module.kind == ModuleDefinitionKind::Core {
        return Err(ModuleToggleValidationError::CoreModuleCannotBeDisabled(
            module_slug.to_string(),
        ));
    }

    if enabled {
        let mut missing = Vec::new();
        let mut incompatible = Vec::new();
        for dependency in &module.dependencies {
            if !enabled_modules.contains(&dependency.slug) {
                missing.push(dependency.slug.clone());
            } else if !dependency_version_satisfied(catalog, dependency) {
                incompatible.push(dependency.slug.clone());
            }
        }
        if !missing.is_empty() {
            return Err(ModuleToggleValidationError::MissingDependencies(missing));
        }
        if !incompatible.is_empty() {
            return Err(ModuleToggleValidationError::IncompatibleDependencies(
                incompatible,
            ));
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
    co_requisites: Vec<ModuleEffectivePolicyCoRequisite>,
    channel: Option<ModuleEffectivePolicyChannelInput>,
    maintenance: Option<ModuleEffectivePolicyMaintenanceInput>,
}

/// The resolved module set used by lifecycle, routing, and installer adapters.
///
/// Deserialization re-establishes the aggregate's internal invariants: the
/// revision is a digest, every decision is keyed by and bound to that same
/// revision, enabled decisions carry no denial reasons, disabled decisions
/// explain themselves, and `enabled_modules` is exactly the enabled
/// decision set. Catalog membership and revision provenance remain owner
/// concerns: consumers that need provenance must resolve through the policy
/// owner or compare against its current revision identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "ModuleEffectivePolicyWire")]
pub struct ModuleEffectivePolicy {
    policy_revision: String,
    enabled_modules: BTreeSet<String>,
    decisions: BTreeMap<String, ModuleEffectivePolicyDecision>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ModuleEffectivePolicyWire {
    policy_revision: String,
    enabled_modules: BTreeSet<String>,
    decisions: BTreeMap<String, ModuleEffectivePolicyDecision>,
}

impl TryFrom<ModuleEffectivePolicyWire> for ModuleEffectivePolicy {
    type Error = String;

    fn try_from(wire: ModuleEffectivePolicyWire) -> Result<Self, Self::Error> {
        let policy = Self {
            policy_revision: wire.policy_revision,
            enabled_modules: wire.enabled_modules,
            decisions: wire.decisions,
        };
        policy
            .validate_invariants()
            .map_err(|error| error.to_string())?;
        Ok(policy)
    }
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
        )
    }

    pub(crate) fn new_with_context(
        catalog: &'a ModuleDefinitionCatalog,
        default_enabled: impl IntoIterator<Item = String>,
        tenant_overrides: impl IntoIterator<Item = TenantModuleOverride>,
        runtime_inputs: impl IntoIterator<Item = ModuleEffectivePolicyRuntimeInput>,
        channel: Option<ModuleEffectivePolicyChannelInput>,
        maintenance: Option<ModuleEffectivePolicyMaintenanceInput>,
    ) -> Self {
        Self {
            catalog,
            default_enabled: default_enabled.into_iter().collect(),
            tenant_overrides: tenant_overrides.into_iter().collect(),
            runtime_inputs: runtime_inputs.into_iter().collect(),
            co_requisites: Vec::new(),
            channel,
            maintenance,
        }
    }

    pub(crate) fn with_corequisites(
        mut self,
        co_requisites: impl IntoIterator<Item = ModuleEffectivePolicyCoRequisite>,
    ) -> Self {
        self.co_requisites = co_requisites.into_iter().collect();
        self
    }

    /// Resolves the immutable core set, selected optional defaults, and tenant
    /// intent. Unknown and legacy overrides are ignored rather than becoming
    /// active definitions.
    pub fn execute(self) -> Result<ModuleEffectivePolicy, ModuleEffectivePolicyError> {
        validate_channel_input(self.channel.as_ref())?;
        validate_maintenance_input(self.maintenance.as_ref())?;
        let co_requisites = normalize_corequisites(self.catalog, self.co_requisites)?;
        let channel = self.channel;
        let mut maintenance = self.maintenance;
        if let Some(affected_modules) = maintenance
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
        if let Some(duplicate) = tenant_overrides
            .windows(2)
            .find(|pair| pair[0].module_slug == pair[1].module_slug)
        {
            return Err(ModuleEffectivePolicyError::DuplicateTenantOverride(
                duplicate[0].module_slug.clone(),
            ));
        }
        let mut runtime_inputs = self.runtime_inputs;
        runtime_inputs.sort_by(|left, right| left.module_slug.cmp(&right.module_slug));
        if let Some(duplicate) = runtime_inputs
            .windows(2)
            .find(|pair| pair[0].module_slug == pair[1].module_slug)
        {
            return Err(ModuleEffectivePolicyError::DuplicateRuntimeInput(
                duplicate[0].module_slug.clone(),
            ));
        }
        // The revision identifies durable policy intent and durable runtime
        // evidence. Exclude the volatile executor-liveness flag from the hash
        // so worker flapping does not churn the revision; the flag still
        // drives decisions below.
        let hashed_runtime_inputs = runtime_inputs
            .iter()
            .map(|input| {
                let mut projection = input.clone();
                projection.executor_available = true;
                projection
            })
            .collect::<Vec<_>>();
        let policy_revision = effective_policy_revision(&EffectivePolicyRevisionInput {
            contract: "rustok.module_effective_policy.v3",
            catalog: self.catalog,
            default_enabled: &default_enabled,
            tenant_overrides: &tenant_overrides,
            runtime_inputs: &hashed_runtime_inputs,
            co_requisites: &co_requisites,
            channel: channel.as_ref(),
            maintenance: maintenance.as_ref(),
        })?;
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
                    let failures = definition
                        .dependencies
                        .iter()
                        .filter_map(|dependency| {
                            if !enabled.contains(&dependency.slug) {
                                Some((dependency.slug.clone(), true))
                            } else if !dependency_version_satisfied(self.catalog, dependency) {
                                Some((dependency.slug.clone(), false))
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>();
                    (!failures.is_empty()).then(|| (definition.slug.clone(), failures))
                })
                .collect::<Vec<_>>();
            let unavailable_corequisites = co_requisites
                .iter()
                .filter(|co_requisite| enabled.contains(&co_requisite.module_slug))
                .filter_map(|co_requisite| {
                    let provider_enabled = enabled.contains(&co_requisite.required_module_slug);
                    let compatible = corequisite_version_compatible(self.catalog, co_requisite);
                    (!provider_enabled || !compatible).then(|| {
                        (
                            co_requisite.module_slug.clone(),
                            co_requisite.required_module_slug.clone(),
                            provider_enabled,
                            compatible,
                        )
                    })
                })
                .collect::<Vec<_>>();
            if unavailable_dependencies.is_empty() && unavailable_corequisites.is_empty() {
                break;
            }
            for (module_slug, failures) in unavailable_dependencies {
                if enabled.remove(&module_slug) {
                    denial_reasons.entry(module_slug).or_default().extend(
                        failures.into_iter().map(|(dependency_slug, missing)| {
                            if missing {
                                ModuleEffectivePolicyDenialReason::DependencyUnavailable {
                                    module_slug: dependency_slug,
                                }
                            } else {
                                ModuleEffectivePolicyDenialReason::DependencyVersionMismatch {
                                    module_slug: dependency_slug,
                                }
                            }
                        }),
                    );
                }
            }
            for (module_slug, required_module_slug, provider_enabled, compatible) in
                unavailable_corequisites
            {
                if !enabled.remove(&module_slug) {
                    continue;
                }
                let reason = if !provider_enabled {
                    ModuleEffectivePolicyDenialReason::CoRequisiteUnavailable {
                        module_slug: required_module_slug,
                    }
                } else if !compatible {
                    ModuleEffectivePolicyDenialReason::CoRequisiteVersionMismatch {
                        module_slug: required_module_slug,
                    }
                } else {
                    unreachable!("unavailable co-requisite must have a denial cause")
                };
                denial_reasons.entry(module_slug).or_default().push(reason);
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
                // Core definitions ignore tenant overrides entirely, so an
                // override fact would falsely claim tenant intent influenced
                // the decision.
                if definition.kind != ModuleDefinitionKind::Core
                    && let Some(override_enabled) =
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
                if let Some(channel) = channel.as_ref()
                    && let Some(enabled) = channel_bindings.get(definition.slug.as_str())
                {
                    facts.push(ModuleEffectivePolicyFact::ChannelBinding {
                        channel_id: channel.channel_id,
                        surface: channel.surface.clone(),
                        channel_revision: channel.channel_revision.clone(),
                        enabled: *enabled,
                    });
                }
                if let Some(maintenance) = maintenance.as_ref() {
                    facts.push(ModuleEffectivePolicyFact::Maintenance {
                        maintenance_revision: maintenance.maintenance_revision.clone(),
                        active: maintenance.active,
                        reason_code: maintenance.reason_code.clone(),
                        affected_modules: maintenance.affected_modules.clone(),
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
                facts.extend(
                    co_requisites
                        .iter()
                        .filter(|co_requisite| co_requisite.module_slug == definition.slug)
                        .filter_map(|co_requisite| {
                            // Normalization proved the provider exists; a
                            // missing provider omits the explanatory fact
                            // while compatibility still fails closed.
                            let provider = self.catalog.get(&co_requisite.required_module_slug)?;
                            Some(ModuleEffectivePolicyFact::CoRequisite {
                                module_slug: co_requisite.required_module_slug.clone(),
                                version_requirement: co_requisite.version_requirement.clone(),
                                version: provider.version.clone(),
                                enabled: enabled.contains(&co_requisite.required_module_slug),
                                compatible: corequisite_version_compatible(
                                    self.catalog,
                                    co_requisite,
                                ),
                            })
                        }),
                );
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

        let policy = ModuleEffectivePolicy {
            policy_revision,
            enabled_modules: enabled,
            decisions,
        };
        policy.validate_invariants()?;
        Ok(policy)
    }
}

impl ModuleEffectivePolicy {
    pub fn policy_revision(&self) -> &str {
        &self.policy_revision
    }

    pub fn validate_invariants(&self) -> Result<(), ModuleEffectivePolicyError> {
        let invalid =
            |message: &str| ModuleEffectivePolicyError::InvalidPolicySnapshot(message.to_string());
        if !valid_digest(&self.policy_revision) {
            return Err(invalid("policy_revision must be a sha256 digest"));
        }

        let mut enabled_decisions = BTreeSet::new();
        for (slug, decision) in &self.decisions {
            if !is_valid_module_slug(slug) || decision.module_slug != *slug {
                return Err(invalid(
                    "every decision key must be a valid module slug matching its decision",
                ));
            }
            if decision.policy_revision != self.policy_revision {
                return Err(invalid(
                    "every decision must be bound to the aggregate policy revision",
                ));
            }
            if decision.facts.is_empty() {
                return Err(invalid("every decision must carry policy facts"));
            }
            if decision.enabled {
                if !decision.denial_reasons.is_empty() {
                    return Err(invalid("enabled decisions must not carry denial reasons"));
                }
                enabled_decisions.insert(slug.clone());
            } else if decision.denial_reasons.is_empty() {
                return Err(invalid("disabled decisions must carry a denial reason"));
            }
            for reason in &decision.denial_reasons {
                let referenced_slug = match reason {
                    ModuleEffectivePolicyDenialReason::DependencyUnavailable { module_slug }
                    | ModuleEffectivePolicyDenialReason::DependencyVersionMismatch {
                        module_slug,
                    }
                    | ModuleEffectivePolicyDenialReason::CoRequisiteUnavailable { module_slug }
                    | ModuleEffectivePolicyDenialReason::CoRequisiteVersionMismatch {
                        module_slug,
                    } => Some(module_slug),
                    _ => None,
                };
                if referenced_slug.is_some_and(|module_slug| !is_valid_module_slug(module_slug)) {
                    return Err(invalid("denial reasons must reference valid module slugs"));
                }
            }
        }
        if enabled_decisions != self.enabled_modules {
            return Err(invalid(
                "enabled_modules must exactly match the enabled decision set",
            ));
        }
        Ok(())
    }

    pub fn cache_identity(
        &self,
        tenant_id: uuid::Uuid,
    ) -> Result<EffectivePolicyCacheIdentity, ModuleEffectivePolicyError> {
        self.validate_invariants()?;
        EffectivePolicyCacheIdentity::new(tenant_id, self.policy_revision.clone())
    }

    pub fn contains(&self, module_slug: &str) -> bool {
        self.enabled_modules.contains(module_slug)
    }

    /// Mints the opaque execution admission required by artifact binding
    /// dispatch, or returns `None` when this policy does not admit the
    /// exact release/target for the tenant.
    ///
    /// When the decision carries artifact-installation facts (the
    /// artifact-aware catalog path), the token is minted only for the
    /// installation and release digest those facts selected. When the
    /// decision came from a static-only catalog (no artifact facts), the
    /// token binds the requested release/target to the enabled slug
    /// decision; the production executor still resolves the exact or
    /// current installation and re-checks the *current* policy before
    /// executing, so this token never substitutes for installation
    /// admission or a fresh policy decision.
    pub fn admit_artifact_execution(
        &self,
        tenant_id: uuid::Uuid,
        release: &ArtifactReleaseRef,
        target: ArtifactInstallationTarget,
    ) -> Option<ArtifactExecutionAdmission> {
        if tenant_id.is_nil() {
            return None;
        }
        let decision = self.decisions.get(&release.slug)?;
        if !decision.enabled || !decision.denial_reasons.is_empty() {
            return None;
        }
        let mut saw_installation_fact = false;
        let mut installation_fact_matches = false;
        for fact in &decision.facts {
            if let ModuleEffectivePolicyFact::ArtifactInstallation {
                installation_id,
                scope,
                release_digest,
                ..
            } = fact
            {
                saw_installation_fact = true;
                let scope_matches = match scope {
                    ModuleInstallationScope::Platform => true,
                    ModuleInstallationScope::Tenant {
                        tenant_id: scoped_tenant_id,
                    } => *scoped_tenant_id == tenant_id,
                };
                let target_matches = match target {
                    ArtifactInstallationTarget::CurrentRelease => true,
                    ArtifactInstallationTarget::ExactInstallation {
                        installation_id: requested_id,
                    } => requested_id == *installation_id,
                };
                if scope_matches && target_matches && release_digest == &release.digest {
                    installation_fact_matches = true;
                }
            }
        }
        if saw_installation_fact && !installation_fact_matches {
            return None;
        }
        Some(ArtifactExecutionAdmission::effective_policy(
            tenant_id,
            release,
            target,
            self.policy_revision.clone(),
        ))
    }

    /// Returns the stored decision for a catalog module, or `None` when the
    /// slug was not part of the resolved catalog. Prefer this accessor when
    /// "absent from catalog" must stay distinguishable from "denied".
    pub fn get_decision(&self, module_slug: &str) -> Option<&ModuleEffectivePolicyDecision> {
        self.decisions.get(module_slug)
    }

    /// Total decision lookup for view projection: slugs absent from the
    /// resolved catalog yield a synthesized disabled decision whose only
    /// denial reason is `UnknownModule` and whose facts are empty.
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

impl From<ModuleEffectivePolicyDecision> for ModuleEffectivePolicyDecisionView {
    fn from(decision: ModuleEffectivePolicyDecision) -> Self {
        Self {
            module_slug: decision.module_slug,
            enabled: decision.enabled,
            policy_revision: decision.policy_revision,
            denial_reasons: decision
                .denial_reasons
                .into_iter()
                .map(Into::into)
                .collect(),
        }
    }
}

impl From<ModuleEffectivePolicy> for ModuleEffectivePolicyView {
    fn from(policy: ModuleEffectivePolicy) -> Self {
        Self {
            policy_revision: policy.policy_revision,
            decisions: policy.decisions.into_values().map(Into::into).collect(),
        }
    }
}

/// Host-composed read boundary for the browser-safe effective-policy decision.
///
/// The module owner retains policy semantics and private facts. Hosts supply
/// their active composition inputs through one implementation of this port;
/// operator transports never rebuild effective availability from tenant rows.
#[async_trait]
pub trait ModuleEffectivePolicyReader: Send + Sync {
    async fn resolve(
        &self,
        tenant_id: uuid::Uuid,
    ) -> Result<ModuleEffectivePolicyView, ModuleEffectivePolicyReaderError>;
}

/// Stable host-port failure for an unavailable effective-policy projection.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum ModuleEffectivePolicyReaderError {
    #[error("effective module policy is unavailable")]
    Unavailable,
}

/// A host-composed effective-policy reader shared with internal transports.
#[derive(Clone)]
pub struct SharedModuleEffectivePolicyReader(pub Arc<dyn ModuleEffectivePolicyReader>);

#[derive(Serialize)]
struct EffectivePolicyRevisionInput<'a> {
    contract: &'static str,
    catalog: &'a ModuleDefinitionCatalog,
    default_enabled: &'a [String],
    tenant_overrides: &'a [TenantModuleOverride],
    runtime_inputs: &'a [ModuleEffectivePolicyRuntimeInput],
    co_requisites: &'a [ModuleEffectivePolicyCoRequisite],
    channel: Option<&'a ModuleEffectivePolicyChannelInput>,
    maintenance: Option<&'a ModuleEffectivePolicyMaintenanceInput>,
}

fn effective_policy_revision(
    input: &EffectivePolicyRevisionInput<'_>,
) -> Result<String, ModuleEffectivePolicyError> {
    // Byte-for-byte equivalent to `hash_manifest(input)`: the canonical
    // JSON form is one compact object whose keys are sorted recursively.
    // The catalog segment is spliced from the catalog's cached canonical
    // bytes instead of re-serializing every admitted schema document on
    // each resolve; a standalone canonical object spliced as a field
    // value is identical to the same value canonicalized in place,
    // because key sorting is purely structural. The remaining segments
    // are small per-call values and are canonicalized here. Outer keys
    // below are in the canonical (lexicographic) order; `None` segments
    // canonicalize to `null`, exactly as inside the whole-input form.
    let encode =
        |error: serde_json::Error| ModuleEffectivePolicyError::RevisionEncoding(error.to_string());
    let catalog_bytes = input
        .catalog
        .canonical_json_bytes_cached()
        .map_err(encode)?;
    let channel_bytes = canonical_json_bytes(&input.channel).map_err(encode)?;
    let co_requisites_bytes = canonical_json_bytes(&input.co_requisites).map_err(encode)?;
    let contract_bytes = canonical_json_bytes(&input.contract).map_err(encode)?;
    let default_enabled_bytes = canonical_json_bytes(&input.default_enabled).map_err(encode)?;
    let maintenance_bytes = canonical_json_bytes(&input.maintenance).map_err(encode)?;
    let runtime_inputs_bytes = canonical_json_bytes(&input.runtime_inputs).map_err(encode)?;
    let tenant_overrides_bytes = canonical_json_bytes(&input.tenant_overrides).map_err(encode)?;
    let segments: [(&str, &[u8]); 8] = [
        ("catalog", catalog_bytes.as_slice()),
        ("channel", channel_bytes.as_slice()),
        ("co_requisites", co_requisites_bytes.as_slice()),
        ("contract", contract_bytes.as_slice()),
        ("default_enabled", default_enabled_bytes.as_slice()),
        ("maintenance", maintenance_bytes.as_slice()),
        ("runtime_inputs", runtime_inputs_bytes.as_slice()),
        ("tenant_overrides", tenant_overrides_bytes.as_slice()),
    ];
    let mut canonical = Vec::new();
    canonical.push(b'{');
    for (index, (key, segment)) in segments.iter().enumerate() {
        if index > 0 {
            canonical.push(b',');
        }
        canonical.extend(serde_json::to_vec(key).map_err(encode)?);
        canonical.push(b':');
        canonical.extend_from_slice(segment);
    }
    canonical.push(b'}');
    Ok(format!(
        "sha256:{}",
        hex::encode(Sha256::digest(&canonical))
    ))
}

fn normalize_corequisites(
    catalog: &ModuleDefinitionCatalog,
    co_requisites: Vec<ModuleEffectivePolicyCoRequisite>,
) -> Result<Vec<ModuleEffectivePolicyCoRequisite>, ModuleEffectivePolicyError> {
    let mut normalized = BTreeMap::<(String, String), String>::new();
    for co_requisite in co_requisites {
        let module_slug = co_requisite.module_slug.trim().to_string();
        let required_module_slug = co_requisite.required_module_slug.trim().to_string();
        let version_requirement = co_requisite.version_requirement.trim().to_string();
        if !is_valid_module_slug(&module_slug) || !is_valid_module_slug(&required_module_slug) {
            return Err(ModuleEffectivePolicyError::InvalidCoRequisiteInput(
                "module slugs must use the canonical module slug grammar".to_string(),
            ));
        }
        if module_slug == required_module_slug {
            return Err(ModuleEffectivePolicyError::InvalidCoRequisiteInput(
                format!("module '{module_slug}' cannot require itself as a co-requisite"),
            ));
        }
        let module = catalog.get(&module_slug).ok_or_else(|| {
            ModuleEffectivePolicyError::InvalidCoRequisiteInput(format!(
                "co-requisite consumer '{module_slug}' is absent from the active definition catalog"
            ))
        })?;
        let provider = catalog.get(&required_module_slug).ok_or_else(|| {
            ModuleEffectivePolicyError::InvalidCoRequisiteInput(format!(
                "co-requisite provider '{required_module_slug}' is absent from the active definition catalog"
            ))
        })?;
        if module
            .dependencies
            .iter()
            .any(|dependency| dependency.slug == required_module_slug)
        {
            return Err(ModuleEffectivePolicyError::InvalidCoRequisiteInput(
                format!(
                    "module '{module_slug}' declares '{required_module_slug}' as both an ordinary dependency and a co-requisite"
                ),
            ));
        }
        parse_module_version_requirement(&version_requirement).map_err(|_| {
            ModuleEffectivePolicyError::InvalidCoRequisiteInput(format!(
                "module '{module_slug}' co-requisite '{required_module_slug}' has invalid version requirement '{version_requirement}'"
            ))
        })?;
        Version::parse(&provider.version).map_err(|_| {
            ModuleEffectivePolicyError::InvalidCoRequisiteInput(format!(
                "co-requisite provider '{required_module_slug}' has invalid module version '{}'",
                provider.version
            ))
        })?;
        let key = (module_slug.clone(), required_module_slug.clone());
        if let Some(existing) = normalized.get(&key) {
            if existing != &version_requirement {
                return Err(ModuleEffectivePolicyError::InvalidCoRequisiteInput(
                    format!(
                        "module '{module_slug}' has conflicting co-requisite requirements for '{required_module_slug}'"
                    ),
                ));
            }
            continue;
        }
        normalized.insert(key, version_requirement);
    }
    Ok(normalized
        .into_iter()
        .map(
            |((module_slug, required_module_slug), version_requirement)| {
                ModuleEffectivePolicyCoRequisite {
                    module_slug,
                    required_module_slug,
                    version_requirement,
                }
            },
        )
        .collect())
}

fn corequisite_version_compatible(
    catalog: &ModuleDefinitionCatalog,
    co_requisite: &ModuleEffectivePolicyCoRequisite,
) -> bool {
    // `normalize_corequisites` proves provider presence and parseability
    // before evaluation; anything else fails closed as incompatible
    // instead of panicking on a non-local invariant.
    let Some(provider) = catalog.get(&co_requisite.required_module_slug) else {
        return false;
    };
    let Ok(requirement) = parse_module_version_requirement(&co_requisite.version_requirement)
    else {
        return false;
    };
    let Ok(version) = Version::parse(&provider.version) else {
        return false;
    };
    requirement.matches(&version)
}

/// Returns whether the catalog definition of an ordinary dependency satisfies
/// the declared version requirement. `*` accepts any version; empty or
/// unparsable requirements and versions fail closed. The catalog validates
/// requirements at construction, so the parse here is defense in depth.
fn dependency_version_satisfied(
    catalog: &ModuleDefinitionCatalog,
    dependency: &ModuleDependencyConstraint,
) -> bool {
    let Some(provider) = catalog.get(&dependency.slug) else {
        return false;
    };
    let (Ok(requirement), Ok(version)) = (
        parse_module_version_requirement(&dependency.version_requirement),
        Version::parse(&provider.version),
    ) else {
        return false;
    };
    requirement.matches(&version)
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
        if !is_valid_module_slug(&binding.module_slug) {
            return Err(ModuleEffectivePolicyError::InvalidChannelInput(
                "channel module slugs must use the canonical module slug grammar".to_string(),
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
            if !is_valid_module_slug(module_slug) {
                return Err(ModuleEffectivePolicyError::InvalidMaintenanceInput(
                    "affected module slugs must use the canonical module slug grammar".to_string(),
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

fn valid_text(value: &str, max_chars: usize) -> bool {
    !value.trim().is_empty() && value.chars().count() <= max_chars
}

#[cfg(test)]
mod tests {
    use super::{
        ModuleEffectivePolicyChannelBinding, ModuleEffectivePolicyChannelInput,
        ModuleEffectivePolicyCoRequisite, ModuleEffectivePolicyDenialReason,
        ModuleEffectivePolicyError, ModuleEffectivePolicyFact,
        ModuleEffectivePolicyInstallationFact, ModuleEffectivePolicyMaintenanceInput,
        ModuleEffectivePolicyQuery, ModuleEffectivePolicyRuntimeInput,
        ModulePolicyRevisionApplyOutcome, ModulePolicyRevisionGate, ModulePolicyRevisionGateError,
        ModulePolicyRevisionTransition, ModuleToggleValidationError, TenantModuleOverride,
        validate_module_toggle,
    };
    use crate::{
        ArtifactPayloadKind, ArtifactReleaseRef, ModuleArtifactRegistryReleaseStatus,
        ModuleArtifactSecuritySnapshot, ModuleArtifactSecurityStatus, ModuleDefinition,
        ModuleDefinitionCatalog, ModuleDefinitionKind, ModuleDefinitionSource,
        ModuleDependencyConstraint, ModuleInstallationScope, ModulesModule,
    };
    use rustok_core::ModuleRegistry;
    use std::collections::HashSet;
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

        let view: rustok_api::ModuleEffectivePolicyView = policy.into();
        assert!(view.enabled_module_slugs().is_empty());
        assert!(
            view.decisions
                .iter()
                .any(|decision| decision.module_slug == "modules" && !decision.enabled)
        );
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
    fn revision_gate_applies_matching_predecessors_resyncs_gaps_and_replays_duplicates() {
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
        assert_eq!(gate.current_sequence(), Some(1));
        assert_eq!(
            gate.apply(&ModulePolicyRevisionTransition {
                previous_revision: None,
                next_revision: first.clone(),
            }),
            Ok(ModulePolicyRevisionApplyOutcome::Duplicate)
        );
        assert_eq!(gate.current_sequence(), Some(1));
        // A transition whose predecessor was never seen is a gap. Neither the
        // revision nor the sequence may move until a full snapshot is
        // explicitly resynced.
        assert_eq!(
            gate.apply(&ModulePolicyRevisionTransition {
                previous_revision: Some(third.clone()),
                next_revision: second.clone(),
            }),
            Ok(ModulePolicyRevisionApplyOutcome::ResyncRequired)
        );
        assert_eq!(gate.current_revision(), Some(first.as_str()));
        assert_eq!(gate.current_sequence(), Some(1));
        assert_eq!(gate.resync_to(&second), Ok(true));
        assert_eq!(gate.current_revision(), Some(second.as_str()));
        assert_eq!(gate.current_sequence(), Some(2));
        assert_eq!(gate.resync_to(&second), Ok(false));
        assert_eq!(gate.current_sequence(), Some(2));
        // The skipped link now names the explicitly resynced successor.
        assert_eq!(
            gate.apply(&ModulePolicyRevisionTransition {
                previous_revision: Some(first),
                next_revision: second.clone(),
            }),
            Ok(ModulePolicyRevisionApplyOutcome::Duplicate)
        );
        assert_eq!(gate.current_revision(), Some(second.as_str()));
        assert_eq!(gate.current_sequence(), Some(2));
        assert_eq!(
            gate.apply(&ModulePolicyRevisionTransition {
                previous_revision: Some(second.clone()),
                next_revision: third,
            }),
            Ok(ModulePolicyRevisionApplyOutcome::Applied)
        );
        assert_eq!(gate.current_sequence(), Some(3));

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
        // Core definitions ignore tenant overrides entirely, so the recorded
        // disable override must not surface as an honored tenant-intent fact.
        assert!(
            !core
                .facts
                .iter()
                .any(|fact| matches!(fact, ModuleEffectivePolicyFact::TenantOverride { .. }))
        );

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
    fn corequisites_are_revisioned_explainable_availability_not_dependency_edges() {
        let catalog = corequisite_catalog();
        let co_requisites = vec![
            ModuleEffectivePolicyCoRequisite {
                module_slug: "product".to_string(),
                required_module_slug: "inventory".to_string(),
                version_requirement: ">=0.1.0".to_string(),
            },
            ModuleEffectivePolicyCoRequisite {
                module_slug: "product".to_string(),
                required_module_slug: "pricing".to_string(),
                version_requirement: ">=0.1.0".to_string(),
            },
        ];
        let staged = ModuleEffectivePolicyQuery::new(
            &catalog,
            ["taxonomy".to_string(), "product".to_string()],
            [],
            [],
        )
        .with_corequisites(co_requisites.clone())
        .execute()
        .expect("staged policy");
        let product = staged.decision("product");
        assert!(!product.enabled);
        assert!(product.denial_reasons.iter().any(|reason| matches!(
            reason,
            ModuleEffectivePolicyDenialReason::CoRequisiteUnavailable { module_slug }
                if module_slug == "inventory"
        )));
        assert!(product.facts.iter().any(|fact| matches!(
            fact,
            ModuleEffectivePolicyFact::CoRequisite {
                module_slug,
                enabled: false,
                compatible: true,
                ..
            } if module_slug == "inventory"
        )));

        let complete = ModuleEffectivePolicyQuery::new(
            &catalog,
            [
                "taxonomy".to_string(),
                "product".to_string(),
                "inventory".to_string(),
                "pricing".to_string(),
            ],
            [],
            [],
        )
        .with_corequisites(co_requisites)
        .execute()
        .expect("complete policy");
        assert!(complete.contains("product"));
        assert!(complete.contains("inventory"));
        assert!(complete.contains("pricing"));
        assert_ne!(staged.policy_revision(), complete.policy_revision());
        assert_eq!(
            catalog
                .get("product")
                .expect("product definition")
                .dependencies
                .iter()
                .map(|dependency| dependency.slug.as_str())
                .collect::<Vec<_>>(),
            vec!["taxonomy"]
        );
    }

    #[test]
    fn owner_projects_a_redacted_effective_policy_view() {
        let policy = ModuleEffectivePolicyQuery::new(
            &corequisite_catalog(),
            ["taxonomy".to_string(), "product".to_string()],
            [],
            [],
        )
        .with_corequisites([ModuleEffectivePolicyCoRequisite {
            module_slug: "product".to_string(),
            required_module_slug: "inventory".to_string(),
            version_requirement: ">=0.1.0".to_string(),
        }])
        .execute()
        .expect("policy");

        let view: rustok_api::ModuleEffectivePolicyView = policy.into();
        let product = view
            .decisions
            .iter()
            .find(|decision| decision.module_slug == "product")
            .expect("product decision");

        assert!(!product.enabled);
        assert_eq!(product.policy_revision, view.policy_revision);
        assert!(product.denial_reasons.iter().any(|reason| matches!(
            reason,
            rustok_api::ModuleEffectivePolicyDenialReasonView::CoRequisiteUnavailable {
                module_slug
            } if module_slug == "inventory"
        )));
        assert_eq!(view.enabled_module_slugs(), vec!["taxonomy"]);
    }

    #[test]
    fn policy_deserialization_rejects_inconsistent_decisions() {
        let catalog = ModuleDefinitionCatalog::from_static_registry(
            &ModuleRegistry::new().register(ModulesModule),
        )
        .expect("catalog");
        let policy = ModuleEffectivePolicyQuery::new(&catalog, ["modules".to_string()], [], [])
            .execute()
            .expect("policy");
        let encoded = serde_json::to_value(&policy).expect("encode policy");
        let decoded: crate::ModuleEffectivePolicy =
            serde_json::from_value(encoded.clone()).expect("decode owner policy");
        assert_eq!(decoded, policy);

        let mut inconsistent = encoded;
        inconsistent["enabled_modules"] = serde_json::json!([]);
        assert!(
            serde_json::from_value::<crate::ModuleEffectivePolicy>(inconsistent).is_err(),
            "enabled_modules must match enabled decisions"
        );
    }

    #[test]
    fn cache_identity_requires_exact_tenant_and_policy_revision() {
        let catalog = ModuleDefinitionCatalog::from_static_registry(
            &ModuleRegistry::new().register(ModulesModule),
        )
        .expect("catalog");
        let tenant_id = Uuid::new_v4();
        let enabled = ModuleEffectivePolicyQuery::new(&catalog, ["modules".to_string()], [], [])
            .execute()
            .expect("enabled policy");
        let identity = enabled.cache_identity(tenant_id).expect("cache identity");

        assert!(identity.matches(tenant_id, enabled.policy_revision()));
        assert!(!identity.matches(Uuid::new_v4(), enabled.policy_revision()));

        let disabled = ModuleEffectivePolicyQuery::new(&catalog, [], [], [])
            .execute()
            .expect("disabled policy");
        assert_ne!(disabled.policy_revision(), enabled.policy_revision());
        assert!(!identity.matches(tenant_id, disabled.policy_revision()));
        assert!(matches!(
            enabled.cache_identity(Uuid::nil()),
            Err(super::ModuleEffectivePolicyError::InvalidCacheIdentity(_))
        ));
    }

    #[test]
    fn selected_artifact_requires_exact_runtime_evidence() {
        let catalog = artifact_catalog();

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
            [artifact_runtime(
                ModuleArtifactSecurityStatus::Clear,
                ModuleArtifactRegistryReleaseStatus::Unlisted,
            )],
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

    #[test]
    fn security_enforcement_preserves_tenant_intent_and_keeps_yanking_non_emergency() {
        let catalog = artifact_catalog();
        let tenant_intent = [TenantModuleOverride {
            module_slug: "artifact".to_string(),
            enabled: true,
        }];

        for (status, expected_denial) in [
            (
                ModuleArtifactSecurityStatus::Quarantined,
                ModuleEffectivePolicyDenialReason::Quarantined,
            ),
            (
                ModuleArtifactSecurityStatus::Revoked,
                ModuleEffectivePolicyDenialReason::Revoked,
            ),
        ] {
            let policy = ModuleEffectivePolicyQuery::new(
                &catalog,
                [],
                tenant_intent.clone(),
                [artifact_runtime(
                    status,
                    ModuleArtifactRegistryReleaseStatus::Active,
                )],
            )
            .execute()
            .expect("security-enforced policy");
            let decision = policy.decision("artifact");

            assert!(!decision.enabled);
            assert_eq!(decision.denial_reasons, vec![expected_denial]);
            assert!(decision.facts.iter().any(|fact| matches!(
                fact,
                ModuleEffectivePolicyFact::TenantOverride { enabled: true }
            )));
        }

        let yanked = ModuleEffectivePolicyQuery::new(
            &catalog,
            [],
            tenant_intent,
            [artifact_runtime(
                ModuleArtifactSecurityStatus::Clear,
                ModuleArtifactRegistryReleaseStatus::Yanked,
            )],
        )
        .execute()
        .expect("yanked release policy");
        let yanked_decision = yanked.decision("artifact");

        assert!(yanked_decision.enabled);
        assert!(yanked_decision.denial_reasons.is_empty());
        assert!(yanked_decision.facts.iter().any(|fact| matches!(
            fact,
            ModuleEffectivePolicyFact::RegistryRelease {
                status: ModuleArtifactRegistryReleaseStatus::Yanked
            }
        )));
    }

    #[test]
    fn duplicate_tenant_overrides_and_runtime_inputs_are_rejected() {
        let catalog = artifact_catalog();

        let duplicate_override = ModuleEffectivePolicyQuery::new(
            &catalog,
            [],
            [
                TenantModuleOverride {
                    module_slug: "artifact".to_string(),
                    enabled: true,
                },
                TenantModuleOverride {
                    module_slug: "artifact".to_string(),
                    enabled: false,
                },
            ],
            [],
        )
        .execute()
        .expect_err("conflicting duplicate overrides must fail closed");
        assert_eq!(
            duplicate_override,
            ModuleEffectivePolicyError::DuplicateTenantOverride("artifact".to_string())
        );

        let duplicate_runtime = ModuleEffectivePolicyQuery::new(
            &catalog,
            [],
            [],
            [
                artifact_runtime(
                    ModuleArtifactSecurityStatus::Clear,
                    ModuleArtifactRegistryReleaseStatus::Active,
                ),
                artifact_runtime(
                    ModuleArtifactSecurityStatus::Quarantined,
                    ModuleArtifactRegistryReleaseStatus::Active,
                ),
            ],
        )
        .execute()
        .expect_err("duplicate runtime inputs must fail closed");
        assert_eq!(
            duplicate_runtime,
            ModuleEffectivePolicyError::DuplicateRuntimeInput("artifact".to_string())
        );
    }

    #[test]
    fn executor_availability_flap_does_not_change_policy_revision() {
        let catalog = artifact_catalog();
        let intent = [TenantModuleOverride {
            module_slug: "artifact".to_string(),
            enabled: true,
        }];
        let available = artifact_runtime(
            ModuleArtifactSecurityStatus::Clear,
            ModuleArtifactRegistryReleaseStatus::Active,
        );
        let mut unavailable = available.clone();
        unavailable.executor_available = false;

        let up = ModuleEffectivePolicyQuery::new(&catalog, [], intent.clone(), [available])
            .execute()
            .expect("executor-up policy");
        let down = ModuleEffectivePolicyQuery::new(&catalog, [], intent, [unavailable])
            .execute()
            .expect("executor-down policy");

        // Executor liveness is volatile: it gates availability but must not
        // churn the durable policy revision as workers flap.
        assert_eq!(up.policy_revision(), down.policy_revision());
        assert!(up.contains("artifact"));
        let decision = down.decision("artifact");
        assert!(!decision.enabled);
        assert_eq!(
            decision.denial_reasons,
            vec![ModuleEffectivePolicyDenialReason::ExecutorUnavailable]
        );
    }

    #[test]
    fn dependency_version_mismatch_is_distinct_from_missing_dependency() {
        let catalog = versioned_dependency_catalog();
        let policy = ModuleEffectivePolicyQuery::new(
            &catalog,
            ["provider".to_string(), "consumer".to_string()],
            [],
            [],
        )
        .execute()
        .expect("policy over versioned dependency");

        assert!(policy.contains("provider"));
        let consumer = policy.decision("consumer");
        assert!(!consumer.enabled);
        assert_eq!(
            consumer.denial_reasons,
            vec![
                ModuleEffectivePolicyDenialReason::DependencyVersionMismatch {
                    module_slug: "provider".to_string()
                }
            ]
        );
    }

    #[test]
    fn toggle_validation_rejects_malformed_slugs_before_catalog_lookup() {
        let catalog = versioned_dependency_catalog();

        for malformed in ["", " consumer", "Consumer", "bad slug", "bad/slug"] {
            assert_eq!(
                validate_module_toggle(&catalog, &HashSet::new(), malformed, true),
                Err(ModuleToggleValidationError::InvalidModuleSlug(
                    malformed.to_string()
                )),
                "slug {malformed:?} must be rejected as malformed, not unknown"
            );
        }
        assert_eq!(
            validate_module_toggle(&catalog, &HashSet::new(), "absent_module", true),
            Err(ModuleToggleValidationError::UnknownModule)
        );
    }

    #[test]
    fn toggle_validation_rejects_incompatible_dependency_versions() {
        let catalog = versioned_dependency_catalog();

        let enabled = HashSet::from(["provider".to_string()]);
        let incompatible = validate_module_toggle(&catalog, &enabled, "consumer", true)
            .expect_err("incompatible dependency version must reject the toggle");
        assert_eq!(
            incompatible,
            ModuleToggleValidationError::IncompatibleDependencies(vec!["provider".to_string()])
        );

        let missing = validate_module_toggle(&catalog, &HashSet::new(), "consumer", true)
            .expect_err("missing dependency must reject the toggle");
        assert_eq!(
            missing,
            ModuleToggleValidationError::MissingDependencies(vec!["provider".to_string()])
        );
    }

    fn versioned_dependency_catalog() -> ModuleDefinitionCatalog {
        let mut catalog = ModuleDefinitionCatalog::default();
        catalog
            .insert(ModuleDefinition {
                slug: "provider".to_string(),
                version: "1.0.0".to_string(),
                kind: ModuleDefinitionKind::Optional,
                source: ModuleDefinitionSource::PlatformNative {
                    distribution_version: "1.0.0".to_string(),
                },
                dependencies: Vec::new(),
                permissions: Vec::new(),
                settings_schema_digest: None,
                schema_documents: Vec::new(),
                bindings: Vec::new(),
                ui: Vec::new(),
                capabilities: Vec::new(),
            })
            .expect("provider definition");
        catalog
            .insert(ModuleDefinition {
                slug: "consumer".to_string(),
                version: "0.1.0".to_string(),
                kind: ModuleDefinitionKind::Optional,
                source: ModuleDefinitionSource::PlatformNative {
                    distribution_version: "0.1.0".to_string(),
                },
                dependencies: vec![ModuleDependencyConstraint {
                    slug: "provider".to_string(),
                    version_requirement: ">=2.0.0".to_string(),
                }],
                permissions: Vec::new(),
                settings_schema_digest: None,
                schema_documents: Vec::new(),
                bindings: Vec::new(),
                ui: Vec::new(),
                capabilities: Vec::new(),
            })
            .expect("consumer definition");
        catalog
    }

    fn corequisite_catalog() -> ModuleDefinitionCatalog {
        let mut catalog = ModuleDefinitionCatalog::default();
        for (slug, dependencies) in [
            ("taxonomy", Vec::<&str>::new()),
            ("product", vec!["taxonomy"]),
            ("inventory", vec!["product"]),
            ("pricing", vec!["product"]),
        ] {
            catalog
                .insert(ModuleDefinition {
                    slug: slug.to_string(),
                    version: "0.1.0".to_string(),
                    kind: ModuleDefinitionKind::Optional,
                    source: ModuleDefinitionSource::PlatformNative {
                        distribution_version: "0.1.0".to_string(),
                    },
                    dependencies: dependencies
                        .into_iter()
                        .map(|dependency| ModuleDependencyConstraint {
                            slug: dependency.to_string(),
                            version_requirement: "*".to_string(),
                        })
                        .collect(),
                    permissions: Vec::new(),
                    settings_schema_digest: None,
                    schema_documents: Vec::new(),
                    bindings: Vec::new(),
                    ui: Vec::new(),
                    capabilities: Vec::new(),
                })
                .expect("corequisite definition");
        }
        catalog
    }

    fn artifact_catalog() -> ModuleDefinitionCatalog {
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
        catalog
    }

    fn artifact_runtime(
        status: ModuleArtifactSecurityStatus,
        registry_status: ModuleArtifactRegistryReleaseStatus,
    ) -> ModuleEffectivePolicyRuntimeInput {
        ModuleEffectivePolicyRuntimeInput {
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
                revision: u64::from(status != ModuleArtifactSecurityStatus::Clear),
                status,
                registry_status,
                policy_revision: (status != ModuleArtifactSecurityStatus::Clear)
                    .then(|| "artifact-security-policy".to_string()),
                reason_code: (status != ModuleArtifactSecurityStatus::Clear)
                    .then(|| "security_hold".to_string()),
                reason_detail: None,
            }),
        }
    }

    fn digest(character: char) -> String {
        format!("sha256:{}", character.to_string().repeat(64))
    }
}
