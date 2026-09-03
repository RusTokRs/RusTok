use crate::entities::module::model::{
    RegistryFollowUpGateLifecycle, RegistryGovernanceActionLifecycle,
    RegistryValidationStageLifecycle,
};
use crate::entities::module::{
    BuildJob, InstalledModule, MarketplaceModule, ModuleCompositionSnapshot, ModuleInfo,
    ModuleOperationRecoveryPlan, TenantModule, ToggleModuleResult,
};
use serde::{Deserialize, Serialize};

pub const ENABLED_MODULES_QUERY: &str = "query EnabledModules { enabledModules }";

pub const MODULE_REGISTRY_QUERY: &str = "query ModuleRegistry { moduleRegistry { moduleSlug name description version kind dependencies enabled lifecycleRevision ownership trustLevel recommendedAdminSurfaces showcaseAdminSurfaces } }";

pub const INSTALLED_MODULES_QUERY: &str = "query InstalledModules { installedModules { slug source crateName version required dependencies } }";

pub const MODULE_COMPOSITION_SNAPSHOT_QUERY: &str =
    "query ModuleCompositionSnapshot { moduleCompositionSnapshot { revision } }";

pub const TENANT_MODULES_QUERY: &str =
    "query TenantModules { tenantModules { moduleSlug enabled settings revision } }";

pub const MARKETPLACE_QUERY: &str = "query Marketplace($search: String, $category: String, $tag: String, $source: String, $trustLevel: String, $onlyCompatible: Boolean, $installedOnly: Boolean) { marketplace(search: $search, category: $category, tag: $tag, source: $source, trustLevel: $trustLevel, onlyCompatible: $onlyCompatible, installedOnly: $installedOnly) { slug name latestVersion description source kind category tags iconUrl bannerUrl screenshots crateName dependencies ownership trustLevel rustokMinVersion rustokMaxVersion publisher checksumSha256 signaturePresent versions { version changelog yanked publishedAt checksumSha256 signaturePresent } compatible recommendedAdminSurfaces showcaseAdminSurfaces settingsSchema { key type required defaultValue description min max options objectKeys itemType shape } installed installedVersion updateAvailable } }";

pub const MARKETPLACE_MODULE_QUERY: &str = "query MarketplaceModule($slug: String!) { marketplaceModule(slug: $slug) { slug name latestVersion description source kind category tags iconUrl bannerUrl screenshots crateName dependencies ownership trustLevel rustokMinVersion rustokMaxVersion publisher checksumSha256 signaturePresent versions { version changelog yanked publishedAt checksumSha256 signaturePresent } registryLifecycle { ownerBinding { owner { displayLabel } boundBy { displayLabel } boundAt updatedAt } latestRequest { id revision status requestedBy { displayLabel } publisher { displayLabel } approvedBy { displayLabel } rejectedBy { displayLabel } rejectionReason changesRequestedBy { displayLabel } changesRequestedReason changesRequestedReasonCode changesRequestedAt heldBy { displayLabel } heldReason heldReasonCode heldAt heldFromStatus warnings errors createdAt updatedAt publishedAt } latestRelease { version status publisher { displayLabel } checksumSha256 publishedAt yankedReason yankedBy { displayLabel } yankedAt } recentEvents { id eventType actor { displayLabel } publisher { displayLabel } payload { reason reasonCode detail version stageKey attemptNumber warnings errors mode ownerTransition { previousOwner { displayLabel } newOwner { displayLabel } boundBy { displayLabel } } } createdAt } followUpGates { key status detail updatedAt } validationStages { key status detail attemptNumber updatedAt startedAt finishedAt } governanceActions { key reasonRequired reasonCodeRequired reasonCodes destructive } } compatible recommendedAdminSurfaces showcaseAdminSurfaces settingsSchema { key type required defaultValue description min max options objectKeys itemType shape } installed installedVersion updateAvailable } }";

pub const MARKETPLACE_REGISTRY_FRESHNESS_QUERY: &str = "query MarketplaceRegistryFreshness { marketplaceRegistryFreshness { registryId status lastSuccessUnixMs consecutiveFailures } }";

pub const ACTIVE_BUILD_QUERY: &str = "query ActiveBuild { activeBuild { id status stage progress profile manifestRef manifestHash manifestRevision modulesDelta requestedBy reason logsUrl errorMessage startedAt createdAt updatedAt finishedAt } }";

pub const BUILD_HISTORY_QUERY: &str = "query BuildHistory($limit: Int!, $offset: Int!) { buildHistory(limit: $limit, offset: $offset) { id status stage progress profile manifestRef manifestHash manifestRevision modulesDelta requestedBy reason logsUrl errorMessage startedAt createdAt updatedAt finishedAt } }";

pub const BUILD_PROGRESS_SUBSCRIPTION: &str =
    "subscription BuildProgress { buildProgress { buildId status stage progress errorMessage } }";

pub const TOGGLE_MODULE_MUTATION: &str = "mutation ToggleModule($moduleSlug: String!, $enabled: Boolean!, $expectedRevision: Int!, $idempotencyKey: UUID!) { toggleModule(moduleSlug: $moduleSlug, enabled: $enabled, expectedRevision: $expectedRevision, idempotencyKey: $idempotencyKey) { moduleSlug enabled settings revision } }";

pub const MODULE_OPERATION_RECOVERY_PLAN_QUERY: &str = "query ModuleOperationRecoveryPlan($operationId: UUID!) { moduleOperationRecoveryPlan(operationId: $operationId) { operationId tenantId moduleSlug requestedEnabled previousEffectiveEnabled status issue retryable recommendedAction correlationId requestedBy errorMessage } }";

pub const FAILED_MODULE_OPERATION_RECOVERY_PLANS_QUERY: &str = "query FailedModuleOperationRecoveryPlans($moduleSlug: String, $limit: Int) { failedModuleOperationRecoveryPlans(moduleSlug: $moduleSlug, limit: $limit) { operationId tenantId moduleSlug requestedEnabled previousEffectiveEnabled status issue retryable recommendedAction correlationId requestedBy errorMessage } }";

pub const RETRY_FAILED_MODULE_OPERATION_POST_HOOK_MUTATION: &str = "mutation RetryFailedModuleOperationPostHook($operationId: UUID!, $idempotencyKey: UUID!, $expectedRevision: Int!) { retryFailedModuleOperationPostHook(operationId: $operationId, idempotencyKey: $idempotencyKey, expectedRevision: $expectedRevision) { operationId tenantId moduleSlug requestedEnabled previousEffectiveEnabled status issue retryable recommendedAction correlationId requestedBy errorMessage } }";

pub const COMPENSATE_FAILED_MODULE_OPERATION_MUTATION: &str = "mutation CompensateFailedModuleOperation($operationId: UUID!, $idempotencyKey: UUID!, $expectedRevision: Int!) { compensateFailedModuleOperation(operationId: $operationId, idempotencyKey: $idempotencyKey, expectedRevision: $expectedRevision) { moduleSlug enabled settings revision } }";

pub const UPDATE_MODULE_SETTINGS_MUTATION: &str = "mutation UpdateModuleSettings($moduleSlug: String!, $settings: String!, $expectedRevision: Int!, $idempotencyKey: UUID!) { updateModuleSettings(moduleSlug: $moduleSlug, settings: $settings, expectedRevision: $expectedRevision, idempotencyKey: $idempotencyKey) { moduleSlug enabled settings revision } }";

pub const INSTALL_MODULE_MUTATION: &str = "mutation InstallModule($slug: String!, $version: String!, $expectedRevision: Int!, $idempotencyKey: UUID!) { installModule(slug: $slug, version: $version, expectedRevision: $expectedRevision, idempotencyKey: $idempotencyKey) { id status stage progress profile manifestRef manifestHash manifestRevision modulesDelta requestedBy reason logsUrl errorMessage startedAt createdAt updatedAt finishedAt } }";

#[cfg(feature = "ssr")]
pub const REGISTRY_OWNER_TRANSFER_REASON_CODES: &[&str] = &[
    "maintenance_handoff",
    "team_restructure",
    "publisher_rotation",
    "security_emergency",
    "governance_override",
    "other",
];

#[cfg(feature = "ssr")]
pub const REGISTRY_YANK_REASON_CODES: &[&str] = &[
    "security",
    "legal",
    "malware",
    "critical_regression",
    "rollback",
    "other",
];

pub const UNINSTALL_MODULE_MUTATION: &str = "mutation UninstallModule($slug: String!, $expectedRevision: Int!, $idempotencyKey: UUID!) { uninstallModule(slug: $slug, expectedRevision: $expectedRevision, idempotencyKey: $idempotencyKey) { id status stage progress profile manifestRef manifestHash manifestRevision modulesDelta requestedBy reason logsUrl errorMessage startedAt createdAt updatedAt finishedAt } }";

pub const UPGRADE_MODULE_MUTATION: &str = "mutation UpgradeModule($slug: String!, $version: String!, $expectedRevision: Int!, $idempotencyKey: UUID!) { upgradeModule(slug: $slug, version: $version, expectedRevision: $expectedRevision, idempotencyKey: $idempotencyKey) { id status stage progress profile manifestRef manifestHash manifestRevision modulesDelta requestedBy reason logsUrl errorMessage startedAt createdAt updatedAt finishedAt } }";

#[cfg(feature = "ssr")]
pub const REGISTRY_MUTATION_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EnabledModulesResponse {
    #[serde(rename = "enabledModules")]
    pub enabled_modules: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModuleRegistryResponse {
    #[serde(rename = "moduleRegistry")]
    pub module_registry: Vec<ModuleInfo>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InstalledModulesResponse {
    #[serde(rename = "installedModules")]
    pub installed_modules: Vec<InstalledModule>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModuleCompositionSnapshotResponse {
    #[serde(rename = "moduleCompositionSnapshot")]
    pub module_composition_snapshot: ModuleCompositionSnapshot,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TenantModulesResponse {
    #[serde(rename = "tenantModules")]
    pub tenant_modules: Vec<TenantModule>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MarketplaceResponse {
    pub marketplace: Vec<MarketplaceModule>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MarketplaceModuleResponse {
    #[serde(rename = "marketplaceModule")]
    pub marketplace_module: Option<MarketplaceModule>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MarketplaceRegistryFreshnessResponse {
    #[serde(rename = "marketplaceRegistryFreshness")]
    pub marketplace_registry_freshness: Vec<rustok_api::MarketplaceRegistryFreshness>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ActiveBuildResponse {
    #[serde(rename = "activeBuild")]
    pub active_build: Option<BuildJob>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BuildHistoryResponse {
    #[serde(rename = "buildHistory")]
    pub build_history: Vec<BuildJob>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct BuildProgressEvent {
    #[serde(rename = "buildId")]
    pub build_id: String,
    pub status: String,
    pub stage: String,
    pub progress: i32,
    #[serde(rename = "errorMessage")]
    pub error_message: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ToggleModuleResponse {
    #[serde(rename = "toggleModule")]
    pub toggle_module: ToggleModuleResult,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModuleOperationRecoveryPlanResponse {
    #[serde(rename = "moduleOperationRecoveryPlan")]
    pub module_operation_recovery_plan: Option<ModuleOperationRecoveryPlan>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FailedModuleOperationRecoveryPlansResponse {
    #[serde(rename = "failedModuleOperationRecoveryPlans")]
    pub failed_module_operation_recovery_plans: Vec<ModuleOperationRecoveryPlan>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RetryFailedModuleOperationPostHookResponse {
    #[serde(rename = "retryFailedModuleOperationPostHook")]
    pub retry_failed_module_operation_post_hook: ModuleOperationRecoveryPlan,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CompensateFailedModuleOperationResponse {
    #[serde(rename = "compensateFailedModuleOperation")]
    pub compensate_failed_module_operation: TenantModule,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UpdateModuleSettingsResponse {
    #[serde(rename = "updateModuleSettings")]
    pub update_module_settings: TenantModule,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InstallModuleResponse {
    #[serde(rename = "installModule")]
    pub install_module: BuildJob,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UninstallModuleResponse {
    #[serde(rename = "uninstallModule")]
    pub uninstall_module: BuildJob,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UpgradeModuleResponse {
    #[serde(rename = "upgradeModule")]
    pub upgrade_module: BuildJob,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RegistryMutationResult {
    pub schema_version: u32,
    pub action: String,
    pub dry_run: bool,
    pub accepted: bool,
    pub request_id: Option<String>,
    pub status: Option<String>,
    pub slug: String,
    pub version: String,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub errors: Vec<String>,
    pub next_step: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RegistryPublishStatusContract {
    pub schema_version: u32,
    pub request_id: String,
    pub slug: String,
    pub version: String,
    pub status: String,
    pub accepted: bool,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub errors: Vec<String>,
    #[serde(default, rename = "followUpGates")]
    pub follow_up_gates: Vec<RegistryFollowUpGateLifecycle>,
    #[serde(default, rename = "validationStages")]
    pub validation_stages: Vec<RegistryValidationStageLifecycle>,
    #[serde(default, rename = "approvalOverrideRequired")]
    pub approval_override_required: bool,
    #[serde(default, rename = "approvalOverrideReasonCodes")]
    pub approval_override_reason_codes: Vec<String>,
    #[serde(default, rename = "governanceActions")]
    pub governance_actions: Vec<RegistryGovernanceActionLifecycle>,
    pub next_step: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ToggleModuleVariables {
    #[serde(rename = "moduleSlug")]
    pub module_slug: String,
    pub enabled: bool,
    #[serde(rename = "expectedRevision")]
    pub expected_revision: i64,
    #[serde(rename = "idempotencyKey")]
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ModuleOperationRecoveryPlanVariables {
    #[serde(rename = "operationId")]
    pub operation_id: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ModuleOperationRecoveryMutationVariables {
    #[serde(rename = "operationId")]
    pub operation_id: String,
    #[serde(rename = "idempotencyKey")]
    pub idempotency_key: String,
    #[serde(rename = "expectedRevision")]
    pub expected_revision: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct FailedModuleOperationRecoveryPlansVariables {
    #[serde(rename = "moduleSlug")]
    pub module_slug: Option<String>,
    pub limit: Option<i32>,
}

#[derive(Clone, Debug, Serialize)]
pub struct UpdateModuleSettingsVariables {
    #[serde(rename = "moduleSlug")]
    pub module_slug: String,
    pub settings: String,
    #[serde(rename = "expectedRevision")]
    pub expected_revision: i64,
    #[serde(rename = "idempotencyKey")]
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct BuildHistoryVariables {
    pub limit: i32,
    pub offset: i32,
}

#[derive(Clone, Debug, Serialize)]
pub struct MarketplaceVariables {
    pub search: Option<String>,
    pub category: Option<String>,
    pub tag: Option<String>,
    pub source: Option<String>,
    #[serde(rename = "trustLevel")]
    pub trust_level: Option<String>,
    #[serde(rename = "onlyCompatible")]
    pub only_compatible: Option<bool>,
    #[serde(rename = "installedOnly")]
    pub installed_only: Option<bool>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MarketplaceModuleVariables {
    pub slug: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct InstallModuleVariables {
    pub slug: String,
    pub version: String,
    #[serde(rename = "expectedRevision")]
    pub expected_revision: i64,
    #[serde(rename = "idempotencyKey")]
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct UninstallModuleVariables {
    pub slug: String,
    #[serde(rename = "expectedRevision")]
    pub expected_revision: i64,
    #[serde(rename = "idempotencyKey")]
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct UpgradeModuleVariables {
    pub slug: String,
    pub version: String,
    #[serde(rename = "expectedRevision")]
    pub expected_revision: i64,
    #[serde(rename = "idempotencyKey")]
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum ModuleTransitionState {
    #[serde(rename = "PREFLIGHTING")]
    Preflighting,
    #[serde(rename = "FENCED")]
    Fenced,
    #[serde(rename = "PRESTAGING")]
    Prestaging,
    #[serde(rename = "ACTIVATING")]
    Activating,
    #[serde(rename = "OBSERVING")]
    Observing,
    #[serde(rename = "POINT_OF_NO_RETURN")]
    PointOfNoReturn,
    #[serde(rename = "ROLLBACK_TRIGGERED")]
    RollbackTriggered,
    #[serde(rename = "RECOVERED_TO_PREDECESSOR")]
    RecoveredToPredecessor,
    #[serde(rename = "CONVERGED")]
    Converged,
    #[serde(rename = "FAILED_CLOSED")]
    FailedClosed,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModuleTransitionCheckpoint {
    #[serde(rename = "operationId")]
    pub operation_id: String,
    #[serde(rename = "moduleSlug")]
    pub module_slug: String,
    #[serde(rename = "tenantId")]
    pub tenant_id: Option<String>,
    #[serde(rename = "predecessorDigest")]
    pub predecessor_digest: Option<String>,
    #[serde(rename = "candidateDigest")]
    pub candidate_digest: String,
    pub state: ModuleTransitionState,
    #[serde(rename = "stateDetails")]
    pub state_details: Option<String>,
    #[serde(rename = "securityEpoch")]
    pub security_epoch: i64,
    #[serde(rename = "recoveryAttemptCount")]
    pub recovery_attempt_count: i32,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RetentionHold {
    #[serde(rename = "holdId")]
    pub hold_id: String,
    #[serde(rename = "targetType")]
    pub target_type: String,
    #[serde(rename = "targetIdentity")]
    pub target_identity: String,
    pub kind: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

#[derive(Deserialize)]
pub struct ModuleTransitionCheckpointResponse {
    #[serde(rename = "moduleTransitionCheckpoint")]
    pub checkpoint: Option<ModuleTransitionCheckpoint>,
}

#[derive(Deserialize)]
pub struct ModuleRetentionHoldsResponse {
    #[serde(rename = "moduleRetentionHolds")]
    pub holds: Vec<RetentionHold>,
}

#[derive(Deserialize)]
pub struct TriggerModuleRecoveryResponse {
    #[serde(rename = "triggerModuleRecovery")]
    pub checkpoint: ModuleTransitionCheckpoint,
}

#[derive(Deserialize)]
pub struct FinalizeModuleTransitionResponse {
    #[serde(rename = "finalizeModuleTransition")]
    pub checkpoint: ModuleTransitionCheckpoint,
}

#[derive(Deserialize)]
pub struct ActiveModuleTransitionsResponse {
    #[serde(rename = "activeModuleTransitions")]
    pub active_transitions: Vec<ModuleTransitionCheckpoint>,
}
