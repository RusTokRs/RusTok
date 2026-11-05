/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use serde::{Deserialize, Serialize};

/// Current operator-visible state for one configured federated module registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarketplaceRegistryStatus {
    #[serde(alias = "UNKNOWN")]
    Unknown,
    #[serde(alias = "READY")]
    Ready,
    #[serde(alias = "DEGRADED")]
    Degraded,
}

/// Bounded freshness evidence for one stable logical registry identity.
///
/// Endpoint URLs and provider errors are deliberately excluded so this DTO can
/// cross native, GraphQL, and headless transports without disclosing deployment
/// topology or untrusted remote response content.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketplaceRegistryFreshness {
    #[serde(alias = "registryId")]
    pub registry_id: String,
    pub status: MarketplaceRegistryStatus,
    #[serde(alias = "lastSuccessUnixMs")]
    pub last_success_unix_ms: Option<u64>,
    #[serde(alias = "consecutiveFailures")]
    pub consecutive_failures: u64,
}

/// Browser-safe marketplace catalog projection shared by native Admin and
/// GraphQL transports.
///
/// The server host adapts owner snapshots, settings schemas, and registry
/// principal references before returning this view. Consumers must not rebuild
/// it from manifest metadata, catalog persistence, or raw governance payloads.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceModule {
    pub slug: String,
    pub name: String,
    pub latest_version: String,
    pub description: String,
    pub source: String,
    pub kind: String,
    pub category: String,
    pub tags: Vec<String>,
    pub icon_url: Option<String>,
    pub banner_url: Option<String>,
    pub screenshots: Vec<String>,
    pub crate_name: String,
    pub dependencies: Vec<String>,
    pub ownership: String,
    pub trust_level: String,
    pub rustok_min_version: Option<String>,
    pub rustok_max_version: Option<String>,
    pub publisher: Option<String>,
    pub checksum_sha256: Option<String>,
    pub signature_present: bool,
    pub versions: Vec<MarketplaceModuleVersion>,
    pub has_admin_ui: bool,
    pub has_storefront_ui: bool,
    pub ui_classification: String,
    pub registry_lifecycle: Option<RegistryModuleLifecycle>,
    pub compatible: bool,
    pub recommended_admin_surfaces: Vec<String>,
    pub showcase_admin_surfaces: Vec<String>,
    pub settings_schema: Vec<ModuleSettingField>,
    pub installed: bool,
    pub installed_version: Option<String>,
    pub update_available: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceModuleVersion {
    pub version: String,
    pub changelog: Option<String>,
    pub yanked: bool,
    pub published_at: Option<String>,
    pub checksum_sha256: Option<String>,
    pub signature_present: bool,
}

/// Registry moderation state emitted with an exact marketplace module.
///
/// Principal fields intentionally contain only display labels. Authenticated
/// actor identity, legacy evidence, and raw persisted principal envelopes stay
/// inside the server and owner layers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryModuleLifecycle {
    pub moderation_policy: RegistryModerationPolicyLifecycle,
    pub owner_binding: Option<RegistryOwnerLifecycle>,
    pub latest_request: Option<RegistryPublishRequestLifecycle>,
    pub latest_release: Option<RegistryReleaseLifecycle>,
    pub recent_events: Vec<RegistryGovernanceEventLifecycle>,
    pub follow_up_gates: Vec<RegistryFollowUpGateLifecycle>,
    pub validation_stages: Vec<RegistryValidationStageLifecycle>,
    pub governance_actions: Vec<RegistryGovernanceActionLifecycle>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryModerationPolicyLifecycle {
    pub mode: String,
    pub live_publish_supported: bool,
    pub live_governance_supported: bool,
    pub manual_review_required: bool,
    pub restriction_reason_code: Option<String>,
    pub restriction_reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryGovernanceActionLifecycle {
    pub key: String,
    pub reason_required: bool,
    pub reason_code_required: bool,
    pub reason_codes: Vec<String>,
    pub destructive: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryOwnerLifecycle {
    pub owner: String,
    pub bound_by: String,
    pub bound_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryOwnerTransitionLifecycle {
    pub previous_owner: Option<String>,
    pub new_owner: Option<String>,
    pub bound_by: Option<String>,
}

/// Browser-safe result of one owner-validated automated governance check.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryAutomatedCheckLifecycle {
    pub key: String,
    pub status: String,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryGovernanceEventPayloadLifecycle {
    pub reason: Option<String>,
    pub reason_code: Option<String>,
    pub detail: Option<String>,
    pub version: Option<String>,
    pub stage_key: Option<String>,
    pub attempt_number: Option<i32>,
    pub owner_transition: Option<RegistryOwnerTransitionLifecycle>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub mode: Option<String>,
    pub automated_checks: Vec<RegistryAutomatedCheckLifecycle>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryGovernanceEventLifecycle {
    pub id: String,
    pub event_type: String,
    pub actor: String,
    pub publisher: Option<String>,
    pub payload: RegistryGovernanceEventPayloadLifecycle,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryFollowUpGateLifecycle {
    pub key: String,
    pub status: String,
    pub detail: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryValidationStageLifecycle {
    pub key: String,
    pub status: String,
    pub detail: String,
    pub attempt_number: i32,
    pub updated_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub execution_mode: String,
    pub runnable: bool,
    pub requires_manual_confirmation: bool,
    pub allowed_terminal_reason_codes: Vec<String>,
    pub suggested_pass_reason_code: Option<String>,
    pub suggested_failure_reason_code: Option<String>,
    pub suggested_blocked_reason_code: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryPublishRequestLifecycle {
    pub id: String,
    pub revision: i64,
    pub status: String,
    pub requested_by: String,
    pub publisher: Option<String>,
    pub approved_by: Option<String>,
    pub rejected_by: Option<String>,
    pub rejection_reason: Option<String>,
    pub changes_requested_by: Option<String>,
    pub changes_requested_reason: Option<String>,
    pub changes_requested_reason_code: Option<String>,
    pub changes_requested_at: Option<String>,
    pub held_by: Option<String>,
    pub held_reason: Option<String>,
    pub held_reason_code: Option<String>,
    pub held_at: Option<String>,
    pub held_from_status: Option<String>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub published_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryReleaseLifecycle {
    pub version: String,
    pub status: String,
    pub publisher: String,
    pub checksum_sha256: Option<String>,
    pub published_at: String,
    pub yanked_reason: Option<String>,
    pub yanked_by: Option<String>,
    pub yanked_at: Option<String>,
}

/// Exact browser-visible outcome of one registry governance mutation.
///
/// The server's versioned REST boundary adapts its response into this one
/// contract before native Admin consumes it. Callers must not synthesize
/// acceptance, warnings, errors, or follow-up guidance from local state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistryMutationResult {
    pub action: String,
    pub dry_run: bool,
    pub accepted: bool,
    pub request_id: Option<String>,
    pub status: Option<String>,
    pub slug: String,
    pub version: String,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub next_step: Option<String>,
}

/// Exact browser-visible lifecycle status of one registry publish request.
///
/// The owner derives lifecycle state, gates, validation stages, and allowed
/// actions. The server adapter adds only route-specific `next_step` guidance
/// before returning this strict transport contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistryPublishStatus {
    pub request_id: String,
    pub revision: i64,
    pub slug: String,
    pub version: String,
    pub status: String,
    #[serde(rename = "artifactOrigin")]
    pub artifact_origin: String,
    pub accepted: bool,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    #[serde(rename = "followUpGates")]
    pub follow_up_gates: Vec<RegistryFollowUpGateLifecycle>,
    #[serde(rename = "validationStages")]
    pub validation_stages: Vec<RegistryValidationStageLifecycle>,
    #[serde(rename = "approvalOverrideRequired")]
    pub approval_override_required: bool,
    #[serde(rename = "approvalOverrideReasonCodes")]
    pub approval_override_reason_codes: Vec<String>,
    #[serde(rename = "governanceActions")]
    pub governance_actions: Vec<RegistryGovernanceActionLifecycle>,
    pub next_step: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleSettingField {
    pub key: String,
    #[serde(rename = "type")]
    pub value_type: String,
    pub required: bool,
    pub default_value: Option<serde_json::Value>,
    pub description: Option<String>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub options: Vec<serde_json::Value>,
    pub object_keys: Vec<String>,
    pub item_type: Option<String>,
    pub shape: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn freshness_serializes_without_endpoint_details() {
        let freshness = MarketplaceRegistryFreshness {
            registry_id: "community.eu".to_string(),
            status: MarketplaceRegistryStatus::Degraded,
            last_success_unix_ms: Some(1_725_000_000_000),
            consecutive_failures: 2,
        };

        let encoded = serde_json::to_value(&freshness).expect("registry freshness");
        assert_eq!(encoded["registry_id"], "community.eu");
        assert_eq!(encoded["status"], "degraded");
        assert!(encoded.get("url").is_none());
        assert!(encoded.get("error").is_none());
    }

    #[test]
    fn registry_publish_status_uses_the_complete_strict_stage_contract() {
        let status: RegistryPublishStatus = serde_json::from_str(
            r#"{
                "request_id": "request-1",
                "revision": 7,
                "slug": "forum",
                "version": "1.2.3",
                "status": "validating",
                "artifactOrigin": "platform_built",
                "accepted": true,
                "warnings": [],
                "errors": [],
                "followUpGates": [{
                    "key": "platform_build",
                    "status": "pending",
                    "detail": "Awaiting build",
                    "updatedAt": "2026-09-08T00:00:00Z"
                }],
                "validationStages": [{
                    "key": "targeted_tests",
                    "status": "running",
                    "detail": "Executing targeted tests",
                    "attemptNumber": 2,
                    "updatedAt": "2026-09-08T00:00:00Z",
                    "startedAt": "2026-09-08T00:00:00Z",
                    "finishedAt": null,
                    "executionMode": "remote",
                    "runnable": true,
                    "requiresManualConfirmation": false,
                    "allowedTerminalReasonCodes": ["test_failure"],
                    "suggestedPassReasonCode": null,
                    "suggestedFailureReasonCode": "test_failure",
                    "suggestedBlockedReasonCode": null
                }],
                "approvalOverrideRequired": false,
                "approvalOverrideReasonCodes": [],
                "governanceActions": [{
                    "key": "hold",
                    "reasonRequired": true,
                    "reasonCodeRequired": true,
                    "reasonCodes": ["release_window"],
                    "destructive": false
                }],
                "next_step": "Poll /v2/catalog/publish/request-1 for the latest publish lifecycle status."
            }"#,
        )
        .expect("complete registry publish status");

        assert_eq!(status.revision, 7);
        assert_eq!(status.validation_stages[0].execution_mode, "remote");
        assert_eq!(
            status.validation_stages[0]
                .suggested_failure_reason_code
                .as_deref(),
            Some("test_failure")
        );
        assert_eq!(status.governance_actions[0].key, "hold");
    }

    #[test]
    fn governance_event_payload_uses_the_typed_automated_check_contract() {
        let payload = RegistryGovernanceEventPayloadLifecycle {
            reason: None,
            reason_code: None,
            detail: None,
            version: Some("1.2.3".to_string()),
            stage_key: None,
            attempt_number: None,
            owner_transition: None,
            warnings: Vec::new(),
            errors: Vec::new(),
            mode: None,
            automated_checks: vec![RegistryAutomatedCheckLifecycle {
                key: "artifact_contract".to_string(),
                status: "passed".to_string(),
                detail: Some("Artifact contract validation passed.".to_string()),
            }],
        };

        let encoded = serde_json::to_value(payload).expect("governance event payload");
        assert_eq!(encoded["automatedChecks"][0]["key"], "artifact_contract");
        assert_eq!(encoded["automatedChecks"][0]["status"], "passed");
        assert_eq!(
            encoded["automatedChecks"][0]["detail"],
            "Artifact contract validation passed."
        );
        assert!(encoded.get("automated_checks").is_none());
    }

    #[test]
    fn marketplace_module_uses_the_browser_safe_transport_field_contract() {
        let module: MarketplaceModule = serde_json::from_str(
            r#"{
            "slug": "forum",
            "name": "Forum",
            "latestVersion": "1.2.3",
            "description": "Discussion boards",
            "source": "registry",
            "kind": "optional",
            "category": "community",
            "tags": ["discussion"],
            "iconUrl": null,
            "bannerUrl": null,
            "screenshots": [],
            "crateName": "rustok-forum",
            "dependencies": ["content"],
            "ownership": "first_party",
            "trustLevel": "verified",
            "rustokMinVersion": null,
            "rustokMaxVersion": null,
            "publisher": "RusTok",
            "checksumSha256": "sha256:abc",
            "signaturePresent": true,
            "versions": [{
                "version": "1.2.3",
                "changelog": null,
                "yanked": false,
                "publishedAt": "2026-09-08T00:00:00Z",
                "checksumSha256": "sha256:abc",
                "signaturePresent": true
            }],
            "hasAdminUi": true,
            "hasStorefrontUi": false,
            "uiClassification": "admin_only",
            "registryLifecycle": {
                "moderationPolicy": {
                    "mode": "review",
                    "livePublishSupported": true,
                    "liveGovernanceSupported": true,
                    "manualReviewRequired": false,
                    "restrictionReasonCode": null,
                    "restrictionReason": "Ready"
                },
                "ownerBinding": {
                    "owner": "user:owner",
                    "boundBy": "user:operator",
                    "boundAt": "2026-09-08T00:00:00Z",
                    "updatedAt": "2026-09-08T00:00:00Z"
                },
                "latestRequest": null,
                "latestRelease": null,
                "recentEvents": [],
                "followUpGates": [],
                "validationStages": [],
                "governanceActions": []
            },
            "compatible": true,
            "recommendedAdminSurfaces": ["forum"],
            "showcaseAdminSurfaces": [],
            "settingsSchema": [{
                "key": "visibility",
                "type": "string",
                "required": true,
                "defaultValue": "public",
                "description": null,
                "min": null,
                "max": null,
                "options": [],
                "objectKeys": [],
                "itemType": null,
                "shape": null
            }],
            "installed": true,
            "installedVersion": "1.2.3",
            "updateAvailable": false
        }"#,
        )
        .expect("marketplace module contract");

        let encoded = serde_json::to_value(module).expect("marketplace module serializes");
        assert_eq!(encoded["latestVersion"], "1.2.3");
        assert_eq!(encoded["settingsSchema"][0]["type"], "string");
        assert_eq!(
            encoded["registryLifecycle"]["ownerBinding"]["owner"],
            "user:owner"
        );
        assert!(
            encoded["registryLifecycle"]["ownerBinding"]
                .get("ownerPrincipal")
                .is_none()
        );
    }
}
