use serde::{Deserialize, Serialize};
fn deserialize_registry_principal_label<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    registry_principal_label_from_value(&value)
        .ok_or_else(|| serde::de::Error::custom("expected registry principal object or label"))
}

fn deserialize_optional_registry_principal_label<'de, D>(
    deserializer: D,
) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(value
        .as_ref()
        .and_then(registry_principal_label_from_value))
}

pub fn registry_principal_label_from_value(value: &serde_json::Value) -> Option<String> {
    value
        .as_str()
        .map(ToString::to_string)
        .or_else(|| {
            value
                .get("displayLabel")
                .or_else(|| value.get("display_label"))
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string)
        })
        .or_else(|| {
            value
                .get("subject")
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string)
        })
        .or_else(|| {
            value
                .get("legacyLabel")
                .or_else(|| value.get("legacy_label"))
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string)
        })
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RegistryOwnerTransitionLifecycle {
    #[serde(rename = "previousOwner")]
    #[serde(default, deserialize_with = "deserialize_optional_registry_principal_label")]
    pub previous_owner: Option<String>,
    #[serde(rename = "newOwner")]
    #[serde(default, deserialize_with = "deserialize_optional_registry_principal_label")]
    pub new_owner: Option<String>,
    #[serde(rename = "boundBy")]
    #[serde(default, deserialize_with = "deserialize_optional_registry_principal_label")]
    pub bound_by: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct RegistryGovernanceEventPayloadLifecycle {
    pub reason: Option<String>,
    #[serde(rename = "reasonCode")]
    pub reason_code: Option<String>,
    pub detail: Option<String>,
    pub version: Option<String>,
    #[serde(rename = "stageKey")]
    pub stage_key: Option<String>,
    #[serde(rename = "attemptNumber")]
    pub attempt_number: Option<i32>,
    #[serde(rename = "ownerTransition")]
    pub owner_transition: Option<RegistryOwnerTransitionLifecycle>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub errors: Vec<String>,
    pub mode: Option<String>,
}

impl RegistryGovernanceEventPayloadLifecycle {
    pub fn from_details(details: &serde_json::Value) -> Self {
        let warnings = details
            .get("warnings")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|value| value.as_str().map(ToString::to_string))
            .collect();
        let errors = details
            .get("errors")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|value| value.as_str().map(ToString::to_string))
            .collect();
        let attempt_number = details
            .get("attempt_number")
            .and_then(serde_json::Value::as_i64)
            .map(|value| value as i32);
        let owner_transition = if details.get("previous_owner_actor").is_some()
            || details.get("new_owner_actor").is_some()
            || details.get("owner_actor").is_some()
            || details.get("bound_by").is_some()
        {
            Some(RegistryOwnerTransitionLifecycle {
                previous_owner: details
                    .get("previous_owner_actor")
                    .and_then(registry_principal_label_from_value),
                new_owner: details
                    .get("new_owner_actor")
                    .or_else(|| details.get("owner_actor"))
                    .and_then(registry_principal_label_from_value),
                bound_by: details
                    .get("bound_by")
                    .and_then(registry_principal_label_from_value),
            })
        } else {
            None
        };

        Self {
            reason: details
                .get("reason")
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string),
            reason_code: details
                .get("reason_code")
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string),
            detail: details
                .get("detail")
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string),
            version: details
                .get("version")
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string),
            stage_key: details
                .get("stage")
                .or_else(|| details.get("gate"))
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string),
            attempt_number,
            owner_transition,
            warnings,
            errors,
            mode: details
                .get("mode")
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ModuleInfo {
    #[serde(rename = "moduleSlug")]
    pub module_slug: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub kind: String,
    pub dependencies: Vec<String>,
    pub enabled: bool,
    pub ownership: String,
    #[serde(rename = "trustLevel")]
    pub trust_level: String,
    #[serde(default, rename = "hasAdminUi")]
    pub has_admin_ui: bool,
    #[serde(default, rename = "hasStorefrontUi")]
    pub has_storefront_ui: bool,
    #[serde(default, rename = "uiClassification")]
    pub ui_classification: String,
    #[serde(rename = "recommendedAdminSurfaces")]
    pub recommended_admin_surfaces: Vec<String>,
    #[serde(rename = "showcaseAdminSurfaces")]
    pub showcase_admin_surfaces: Vec<String>,
}

impl ModuleInfo {
    pub fn is_core(&self) -> bool {
        self.kind == "core"
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct InstalledModule {
    pub slug: String,
    pub source: String,
    #[serde(rename = "crateName")]
    pub crate_name: String,
    pub version: Option<String>,
    pub required: bool,
    pub dependencies: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct TenantModule {
    #[serde(rename = "moduleSlug")]
    pub module_slug: String,
    pub enabled: bool,
    pub settings: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct MarketplaceModule {
    pub slug: String,
    pub name: String,
    #[serde(rename = "latestVersion")]
    pub latest_version: String,
    pub description: String,
    pub source: String,
    pub kind: String,
    pub category: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default, rename = "iconUrl")]
    pub icon_url: Option<String>,
    #[serde(default, rename = "bannerUrl")]
    pub banner_url: Option<String>,
    #[serde(default)]
    pub screenshots: Vec<String>,
    #[serde(rename = "crateName")]
    pub crate_name: String,
    pub dependencies: Vec<String>,
    pub ownership: String,
    #[serde(rename = "trustLevel")]
    pub trust_level: String,
    #[serde(rename = "rustokMinVersion")]
    pub rustok_min_version: Option<String>,
    #[serde(rename = "rustokMaxVersion")]
    pub rustok_max_version: Option<String>,
    pub publisher: Option<String>,
    #[serde(rename = "checksumSha256")]
    pub checksum_sha256: Option<String>,
    #[serde(rename = "signaturePresent")]
    pub signature_present: bool,
    pub versions: Vec<MarketplaceModuleVersion>,
    #[serde(default, rename = "hasAdminUi")]
    pub has_admin_ui: bool,
    #[serde(default, rename = "hasStorefrontUi")]
    pub has_storefront_ui: bool,
    #[serde(default, rename = "uiClassification")]
    pub ui_classification: String,
    #[serde(default, rename = "registryLifecycle")]
    pub registry_lifecycle: Option<RegistryModuleLifecycle>,
    pub compatible: bool,
    #[serde(rename = "recommendedAdminSurfaces")]
    pub recommended_admin_surfaces: Vec<String>,
    #[serde(rename = "showcaseAdminSurfaces")]
    pub showcase_admin_surfaces: Vec<String>,
    #[serde(default, rename = "settingsSchema")]
    pub settings_schema: Vec<ModuleSettingField>,
    pub installed: bool,
    #[serde(rename = "installedVersion")]
    pub installed_version: Option<String>,
    #[serde(rename = "updateAvailable")]
    pub update_available: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RegistryModuleLifecycle {
    #[serde(rename = "moderationPolicy")]
    pub moderation_policy: RegistryModerationPolicyLifecycle,
    #[serde(default, rename = "ownerBinding")]
    pub owner_binding: Option<RegistryOwnerLifecycle>,
    #[serde(default, rename = "latestRequest")]
    pub latest_request: Option<RegistryPublishRequestLifecycle>,
    #[serde(default, rename = "latestRelease")]
    pub latest_release: Option<RegistryReleaseLifecycle>,
    #[serde(default, rename = "recentEvents")]
    pub recent_events: Vec<RegistryGovernanceEventLifecycle>,
    #[serde(default, rename = "followUpGates")]
    pub follow_up_gates: Vec<RegistryFollowUpGateLifecycle>,
    #[serde(default, rename = "validationStages")]
    pub validation_stages: Vec<RegistryValidationStageLifecycle>,
    #[serde(default, rename = "governanceActions")]
    pub governance_actions: Vec<RegistryGovernanceActionLifecycle>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RegistryModerationPolicyLifecycle {
    pub mode: String,
    #[serde(rename = "livePublishSupported")]
    pub live_publish_supported: bool,
    #[serde(rename = "liveGovernanceSupported")]
    pub live_governance_supported: bool,
    #[serde(rename = "manualReviewRequired")]
    pub manual_review_required: bool,
    #[serde(rename = "restrictionReasonCode")]
    pub restriction_reason_code: Option<String>,
    #[serde(rename = "restrictionReason")]
    pub restriction_reason: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RegistryGovernanceActionLifecycle {
    pub key: String,
    #[serde(default, rename = "reasonRequired")]
    pub reason_required: bool,
    #[serde(default, rename = "reasonCodeRequired")]
    pub reason_code_required: bool,
    #[serde(default, rename = "reasonCodes")]
    pub reason_codes: Vec<String>,
    #[serde(default)]
    pub destructive: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RegistryOwnerLifecycle {
    #[serde(rename = "ownerActor")]
    #[serde(deserialize_with = "deserialize_registry_principal_label")]
    pub owner_actor: String,
    #[serde(rename = "boundBy")]
    #[serde(deserialize_with = "deserialize_registry_principal_label")]
    pub bound_by: String,
    #[serde(rename = "boundAt")]
    pub bound_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RegistryGovernanceEventLifecycle {
    pub id: String,
    #[serde(rename = "eventType")]
    pub event_type: String,
    #[serde(deserialize_with = "deserialize_registry_principal_label")]
    pub actor: String,
    #[serde(default, deserialize_with = "deserialize_optional_registry_principal_label")]
    pub publisher: Option<String>,
    #[serde(rename = "payload", alias = "details")]
    pub payload: RegistryGovernanceEventPayloadLifecycle,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RegistryFollowUpGateLifecycle {
    pub key: String,
    pub status: String,
    pub detail: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RegistryValidationStageLifecycle {
    pub key: String,
    pub status: String,
    pub detail: String,
    #[serde(rename = "attemptNumber")]
    pub attempt_number: i32,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    #[serde(rename = "startedAt")]
    pub started_at: Option<String>,
    #[serde(rename = "finishedAt")]
    pub finished_at: Option<String>,
    #[serde(default, rename = "executionMode")]
    pub execution_mode: String,
    #[serde(default)]
    pub runnable: bool,
    #[serde(default, rename = "requiresManualConfirmation")]
    pub requires_manual_confirmation: bool,
    #[serde(default, rename = "allowedTerminalReasonCodes")]
    pub allowed_terminal_reason_codes: Vec<String>,
    #[serde(default, rename = "suggestedPassReasonCode")]
    pub suggested_pass_reason_code: Option<String>,
    #[serde(default, rename = "suggestedFailureReasonCode")]
    pub suggested_failure_reason_code: Option<String>,
    #[serde(default, rename = "suggestedBlockedReasonCode")]
    pub suggested_blocked_reason_code: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RegistryPublishRequestLifecycle {
    pub id: String,
    pub status: String,
    #[serde(rename = "requestedBy")]
    #[serde(deserialize_with = "deserialize_registry_principal_label")]
    pub requested_by: String,
    #[serde(rename = "publisherIdentity")]
    #[serde(default, deserialize_with = "deserialize_optional_registry_principal_label")]
    pub publisher_identity: Option<String>,
    #[serde(rename = "approvedBy")]
    #[serde(default, deserialize_with = "deserialize_optional_registry_principal_label")]
    pub approved_by: Option<String>,
    #[serde(rename = "rejectedBy")]
    #[serde(default, deserialize_with = "deserialize_optional_registry_principal_label")]
    pub rejected_by: Option<String>,
    #[serde(rename = "rejectionReason")]
    pub rejection_reason: Option<String>,
    #[serde(rename = "changesRequestedBy")]
    #[serde(default, deserialize_with = "deserialize_optional_registry_principal_label")]
    pub changes_requested_by: Option<String>,
    #[serde(rename = "changesRequestedReason")]
    pub changes_requested_reason: Option<String>,
    #[serde(rename = "changesRequestedReasonCode")]
    pub changes_requested_reason_code: Option<String>,
    #[serde(rename = "changesRequestedAt")]
    pub changes_requested_at: Option<String>,
    #[serde(rename = "heldBy")]
    #[serde(default, deserialize_with = "deserialize_optional_registry_principal_label")]
    pub held_by: Option<String>,
    #[serde(rename = "heldReason")]
    pub held_reason: Option<String>,
    #[serde(rename = "heldReasonCode")]
    pub held_reason_code: Option<String>,
    #[serde(rename = "heldAt")]
    pub held_at: Option<String>,
    #[serde(rename = "heldFromStatus")]
    pub held_from_status: Option<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub errors: Vec<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    #[serde(rename = "publishedAt")]
    pub published_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RegistryReleaseLifecycle {
    pub version: String,
    pub status: String,
    #[serde(deserialize_with = "deserialize_registry_principal_label")]
    pub publisher: String,
    #[serde(rename = "checksumSha256")]
    pub checksum_sha256: Option<String>,
    #[serde(rename = "publishedAt")]
    pub published_at: String,
    #[serde(rename = "yankedReason")]
    pub yanked_reason: Option<String>,
    #[serde(rename = "yankedBy")]
    #[serde(default, deserialize_with = "deserialize_optional_registry_principal_label")]
    pub yanked_by: Option<String>,
    #[serde(rename = "yankedAt")]
    pub yanked_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ModuleSettingField {
    pub key: String,
    #[serde(rename = "type")]
    pub value_type: String,
    pub required: bool,
    #[serde(rename = "defaultValue")]
    pub default_value: Option<serde_json::Value>,
    pub description: Option<String>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    #[serde(default)]
    pub options: Vec<serde_json::Value>,
    #[serde(default, rename = "objectKeys")]
    pub object_keys: Vec<String>,
    #[serde(rename = "itemType")]
    pub item_type: Option<String>,
    pub shape: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct MarketplaceModuleVersion {
    pub version: String,
    pub changelog: Option<String>,
    pub yanked: bool,
    #[serde(rename = "publishedAt")]
    pub published_at: Option<String>,
    #[serde(rename = "checksumSha256")]
    pub checksum_sha256: Option<String>,
    #[serde(rename = "signaturePresent")]
    pub signature_present: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct BuildJob {
    pub id: String,
    pub status: String,
    pub stage: String,
    pub progress: i32,
    pub profile: String,
    #[serde(rename = "manifestRef")]
    pub manifest_ref: String,
    #[serde(rename = "manifestHash")]
    pub manifest_hash: String,
    #[serde(rename = "modulesDelta")]
    pub modules_delta: String,
    #[serde(rename = "requestedBy")]
    pub requested_by: String,
    pub reason: Option<String>,
    #[serde(rename = "releaseId")]
    pub release_id: Option<String>,
    #[serde(rename = "logsUrl")]
    pub logs_url: Option<String>,
    #[serde(rename = "errorMessage")]
    pub error_message: Option<String>,
    #[serde(rename = "startedAt")]
    pub started_at: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    #[serde(rename = "finishedAt")]
    pub finished_at: Option<String>,
}

impl BuildJob {
    pub fn summary(&self) -> String {
        format!("{} / {} / {}%", self.status, self.stage, self.progress)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ReleaseInfo {
    pub id: String,
    #[serde(rename = "buildId")]
    pub build_id: String,
    pub status: String,
    pub environment: String,
    #[serde(rename = "manifestHash")]
    pub manifest_hash: String,
    pub modules: Vec<String>,
    #[serde(rename = "previousReleaseId")]
    pub previous_release_id: Option<String>,
    #[serde(rename = "deployedAt")]
    pub deployed_at: Option<String>,
    #[serde(rename = "rolledBackAt")]
    pub rolled_back_at: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ToggleModuleResult {
    #[serde(rename = "moduleSlug")]
    pub module_slug: String,
    pub enabled: bool,
    pub settings: String,
}
