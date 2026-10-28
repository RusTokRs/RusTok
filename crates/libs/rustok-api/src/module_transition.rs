//! Browser-safe contracts for the module transition control plane.
//!
//! The module owner projects durable transition records into these values before
//! a GraphQL, native Leptos, or headless client adapter serializes them.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum ModuleTransitionStateView {
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
    #[serde(rename = "RECOVERED_TO_PREDECESSOR")]
    RecoveredToPredecessor,
    #[serde(rename = "CONVERGED")]
    Converged,
    #[serde(rename = "INTERVENTION_REQUIRED")]
    InterventionRequired,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ModuleTransitionCheckpointView {
    #[serde(rename = "operationId")]
    pub operation_id: String,
    pub revision: i64,
    #[serde(rename = "moduleSlug")]
    pub module_slug: String,
    #[serde(rename = "tenantId")]
    pub tenant_id: Option<String>,
    #[serde(rename = "predecessorDigest")]
    pub predecessor_digest: Option<String>,
    #[serde(rename = "candidateDigest")]
    pub candidate_digest: String,
    pub state: ModuleTransitionStateView,
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

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ModuleRetentionHoldView {
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

/// Browser-safe direct predecessor rollback receipt projection.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ArtifactRollbackView {
    #[serde(rename = "operationId")]
    pub operation_id: String,
    #[serde(rename = "sourceInstallationId")]
    pub source_installation_id: String,
    #[serde(rename = "targetInstallationId")]
    pub target_installation_id: String,
    #[serde(rename = "sourceRevision")]
    pub source_revision: i64,
    #[serde(rename = "targetRevision")]
    pub target_revision: i64,
}

#[cfg(test)]
mod tests {
    use super::{ArtifactRollbackView, ModuleTransitionCheckpointView, ModuleTransitionStateView};

    #[test]
    fn rollback_view_uses_the_graphql_field_contract() {
        let rollback = ArtifactRollbackView {
            operation_id: "11111111-1111-1111-1111-111111111111".to_string(),
            source_installation_id: "22222222-2222-2222-2222-222222222222".to_string(),
            target_installation_id: "33333333-3333-3333-3333-333333333333".to_string(),
            source_revision: 2,
            target_revision: 3,
        };

        let encoded = serde_json::to_value(rollback).expect("rollback view serializes");
        assert_eq!(
            encoded["operationId"],
            "11111111-1111-1111-1111-111111111111"
        );
        assert_eq!(
            encoded["sourceInstallationId"],
            "22222222-2222-2222-2222-222222222222"
        );
        assert_eq!(
            encoded["targetInstallationId"],
            "33333333-3333-3333-3333-333333333333"
        );
        assert_eq!(encoded["sourceRevision"], 2);
        assert_eq!(encoded["targetRevision"], 3);
    }

    #[test]
    fn transition_view_uses_the_graphql_field_and_state_contract() {
        let checkpoint = ModuleTransitionCheckpointView {
            operation_id: "11111111-1111-1111-1111-111111111111".to_string(),
            revision: 3,
            module_slug: "forum".to_string(),
            tenant_id: Some("22222222-2222-2222-2222-222222222222".to_string()),
            predecessor_digest: Some("sha256:previous".to_string()),
            candidate_digest: "sha256:candidate".to_string(),
            state: ModuleTransitionStateView::PointOfNoReturn,
            state_details: Some("Committed".to_string()),
            security_epoch: 7,
            recovery_attempt_count: 1,
            created_at: "2026-09-06T00:00:00Z".to_string(),
            updated_at: "2026-09-06T00:01:00Z".to_string(),
        };

        let encoded = serde_json::to_value(checkpoint).expect("transition view serializes");
        assert_eq!(
            encoded["operationId"],
            "11111111-1111-1111-1111-111111111111"
        );
        assert_eq!(encoded["state"], "POINT_OF_NO_RETURN");
        assert_eq!(encoded["securityEpoch"], 7);
    }
}
