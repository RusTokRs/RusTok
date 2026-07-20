#[allow(unused_imports)]
use crate::entities::module::model::{
    MarketplaceModuleVersion, RegistryFollowUpGateLifecycle, RegistryGovernanceActionLifecycle,
    RegistryGovernanceEventLifecycle, RegistryGovernanceEventPayloadLifecycle,
    RegistryModuleLifecycle, RegistryOwnerLifecycle, RegistryPublishRequestLifecycle,
    RegistryReleaseLifecycle, RegistryValidationStageLifecycle,
    registry_principal_label_from_value,
};
use crate::entities::module::{
    BuildJob, InstalledModule, MarketplaceModule, ModuleInfo, ModuleOperationRecoveryPlan,
    ReleaseInfo, TenantModule, ToggleModuleResult,
};
use serde::{Deserialize, Serialize};

pub const ENABLED_MODULES_QUERY: &str = "query EnabledModules { enabledModules }";

pub const MODULE_REGISTRY_QUERY: &str = "query ModuleRegistry { moduleRegistry { moduleSlug name description version kind dependencies enabled ownership trustLevel recommendedAdminSurfaces showcaseAdminSurfaces } }";

pub const INSTALLED_MODULES_QUERY: &str = "query InstalledModules { installedModules { slug source crateName version required dependencies } }";

pub const TENANT_MODULES_QUERY: &str =
    "query TenantModules { tenantModules { moduleSlug enabled settings } }";

pub const MARKETPLACE_QUERY: &str = "query Marketplace($search: String, $category: String, $tag: String, $source: String, $trustLevel: String, $onlyCompatible: Boolean, $installedOnly: Boolean) { marketplace(search: $search, category: $category, tag: $tag, source: $source, trustLevel: $trustLevel, onlyCompatible: $onlyCompatible, installedOnly: $installedOnly) { slug name latestVersion description source kind category tags iconUrl bannerUrl screenshots crateName dependencies ownership trustLevel rustokMinVersion rustokMaxVersion publisher checksumSha256 signaturePresent versions { version changelog yanked publishedAt checksumSha256 signaturePresent } compatible recommendedAdminSurfaces showcaseAdminSurfaces settingsSchema { key type required defaultValue description min max options objectKeys itemType shape } installed installedVersion updateAvailable } }";

pub const MARKETPLACE_MODULE_QUERY: &str = "query MarketplaceModule($slug: String!) { marketplaceModule(slug: $slug) { slug name latestVersion description source kind category tags iconUrl bannerUrl screenshots crateName dependencies ownership trustLevel rustokMinVersion rustokMaxVersion publisher checksumSha256 signaturePresent versions { version changelog yanked publishedAt checksumSha256 signaturePresent } registryLifecycle { ownerBinding { owner { displayLabel } boundBy { displayLabel } boundAt updatedAt } latestRequest { id status requestedBy { displayLabel } publisher { displayLabel } approvedBy { displayLabel } rejectedBy { displayLabel } rejectionReason changesRequestedBy { displayLabel } changesRequestedReason changesRequestedReasonCode changesRequestedAt heldBy { displayLabel } heldReason heldReasonCode heldAt heldFromStatus warnings errors createdAt updatedAt publishedAt } latestRelease { version status publisher { displayLabel } checksumSha256 publishedAt yankedReason yankedBy { displayLabel } yankedAt } recentEvents { id eventType actor { displayLabel } publisher { displayLabel } payload { reason reasonCode detail version stageKey attemptNumber warnings errors mode ownerTransition { previousOwner { displayLabel } newOwner { displayLabel } boundBy { displayLabel } } } createdAt } followUpGates { key status detail updatedAt } validationStages { key status detail attemptNumber updatedAt startedAt finishedAt } governanceActions { key reasonRequired reasonCodeRequired reasonCodes destructive } } compatible recommendedAdminSurfaces showcaseAdminSurfaces settingsSchema { key type required defaultValue description min max options objectKeys itemType shape } installed installedVersion updateAvailable } }";

pub const ACTIVE_BUILD_QUERY: &str = "query ActiveBuild { activeBuild { id status stage progress profile manifestRef manifestHash manifestRevision modulesDelta requestedBy reason releaseId logsUrl errorMessage startedAt createdAt updatedAt finishedAt } }";

pub const ACTIVE_RELEASE_QUERY: &str = "query ActiveRelease { activeRelease { id buildId status environment manifestHash manifestRevision modules previousReleaseId deployedAt rolledBackAt createdAt updatedAt } }";

pub const BUILD_HISTORY_QUERY: &str = "query BuildHistory($limit: Int!, $offset: Int!) { buildHistory(limit: $limit, offset: $offset) { id status stage progress profile manifestRef manifestHash manifestRevision modulesDelta requestedBy reason releaseId logsUrl errorMessage startedAt createdAt updatedAt finishedAt } }";

pub const BUILD_PROGRESS_SUBSCRIPTION: &str = "subscription BuildProgress { buildProgress { buildId status stage progress releaseId errorMessage } }";

pub const TOGGLE_MODULE_MUTATION: &str = "mutation ToggleModule($moduleSlug: String!, $enabled: Boolean!) { toggleModule(moduleSlug: $moduleSlug, enabled: $enabled) { moduleSlug enabled settings } }";

pub const MODULE_OPERATION_RECOVERY_PLAN_QUERY: &str = "query ModuleOperationRecoveryPlan($operationId: UUID!) { moduleOperationRecoveryPlan(operationId: $operationId) { operationId tenantId moduleSlug requestedEnabled previousEffectiveEnabled status issue retryable recommendedAction correlationId requestedBy errorMessage } }";

pub const FAILED_MODULE_OPERATION_RECOVERY_PLANS_QUERY: &str = "query FailedModuleOperationRecoveryPlans($moduleSlug: String, $limit: Int) { failedModuleOperationRecoveryPlans(moduleSlug: $moduleSlug, limit: $limit) { operationId tenantId moduleSlug requestedEnabled previousEffectiveEnabled status issue retryable recommendedAction correlationId requestedBy errorMessage } }";

pub const RETRY_FAILED_MODULE_OPERATION_POST_HOOK_MUTATION: &str = "mutation RetryFailedModuleOperationPostHook($operationId: UUID!) { retryFailedModuleOperationPostHook(operationId: $operationId) { operationId tenantId moduleSlug requestedEnabled previousEffectiveEnabled status issue retryable recommendedAction correlationId requestedBy errorMessage } }";

pub const COMPENSATE_FAILED_MODULE_OPERATION_MUTATION: &str = "mutation CompensateFailedModuleOperation($operationId: UUID!) { compensateFailedModuleOperation(operationId: $operationId) { moduleSlug enabled settings } }";

pub const UPDATE_MODULE_SETTINGS_MUTATION: &str = "mutation UpdateModuleSettings($moduleSlug: String!, $settings: String!) { updateModuleSettings(moduleSlug: $moduleSlug, settings: $settings) { moduleSlug enabled settings } }";

pub const INSTALL_MODULE_MUTATION: &str = "mutation InstallModule($slug: String!, $version: String!) { installModule(slug: $slug, version: $version) { id status stage progress profile manifestRef manifestHash manifestRevision modulesDelta requestedBy reason releaseId logsUrl errorMessage startedAt createdAt updatedAt finishedAt } }";

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

pub const UNINSTALL_MODULE_MUTATION: &str = "mutation UninstallModule($slug: String!) { uninstallModule(slug: $slug) { id status stage progress profile manifestRef manifestHash manifestRevision modulesDelta requestedBy reason releaseId logsUrl errorMessage startedAt createdAt updatedAt finishedAt } }";

pub const UPGRADE_MODULE_MUTATION: &str = "mutation UpgradeModule($slug: String!, $version: String!) { upgradeModule(slug: $slug, version: $version) { id status stage progress profile manifestRef manifestHash manifestRevision modulesDelta requestedBy reason releaseId logsUrl errorMessage startedAt createdAt updatedAt finishedAt } }";

pub const ROLLBACK_BUILD_MUTATION: &str = "mutation RollbackBuild($buildId: String!) { rollbackBuild(buildId: $buildId) { id status stage progress profile manifestRef manifestHash manifestRevision modulesDelta requestedBy reason releaseId logsUrl errorMessage startedAt createdAt updatedAt finishedAt } }";

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
pub struct ActiveBuildResponse {
    #[serde(rename = "activeBuild")]
    pub active_build: Option<BuildJob>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ActiveReleaseResponse {
    #[serde(rename = "activeRelease")]
    pub active_release: Option<ReleaseInfo>,
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
    #[serde(rename = "releaseId")]
    pub release_id: Option<String>,
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RollbackBuildResponse {
    #[serde(rename = "rollbackBuild")]
    pub rollback_build: BuildJob,
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
}

#[derive(Clone, Debug, Serialize)]
pub struct ModuleOperationRecoveryPlanVariables {
    #[serde(rename = "operationId")]
    pub operation_id: String,
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
}

#[derive(Clone, Debug, Serialize)]
pub struct UninstallModuleVariables {
    pub slug: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct UpgradeModuleVariables {
    pub slug: String,
    pub version: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct RollbackBuildVariables {
    #[serde(rename = "buildId")]
    pub build_id: String,
}
