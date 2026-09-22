//! N/N+1 Settings Compatibility Guard.
//!
//! Enforces that concurrent settings writes while a module rollback window is open
//! must fall strictly within the validated intersection of predecessor (N) and
//! candidate (N+1) schemas.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

use crate::settings::{
    ModuleSettingSpec, ModuleSettingsValidationError, normalize_module_settings,
};

/// State of an N/N+1 Settings Compatibility Guard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "details")]
pub enum SettingsGuardState {
    /// Active guard: all settings writes must pass validation against both
    /// predecessor and candidate schemas.
    Active,
    /// Closed guard: rollback window has been explicitly finalized; candidate
    /// schema is now the sole authority.
    Closed,
    /// Maintenance overridden: an explicit operator maintenance command fenced
    /// writers, closed rollback eligibility, and permitted a one-sided N+1 value.
    MaintenanceOverridden {
        operator_reason: String,
        overridden_at: DateTime<Utc>,
    },
}

/// Durable settings compatibility guard binding predecessor and candidate schema digests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsCompatibilityGuard {
    pub id: Uuid,
    pub tenant_id: Option<Uuid>,
    pub module_slug: String,
    pub predecessor_schema_digest: String,
    pub candidate_schema_digest: String,
    pub rollback_window_id: Uuid,
    pub installed_at: DateTime<Utc>,
    pub state: SettingsGuardState,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SettingsGuardError {
    #[error("Settings value is incompatible with predecessor (N) schema at '{key}': {reason}")]
    PredecessorIncompatible { key: String, reason: String },
    #[error("Settings value is incompatible with candidate (N+1) schema at '{key}': {reason}")]
    CandidateIncompatible { key: String, reason: String },
    #[error("Settings guard is not active for module '{module_slug}'")]
    GuardNotActive { module_slug: String },
    #[error("Schema digest mismatch: expected {expected}, actual {actual}")]
    SchemaDigestMismatch { expected: String, actual: String },
}

impl SettingsCompatibilityGuard {
    /// Creates a new active compatibility guard for a rollout operation.
    pub fn new(
        tenant_id: Option<Uuid>,
        module_slug: String,
        predecessor_schema_digest: String,
        candidate_schema_digest: String,
        rollback_window_id: Uuid,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            tenant_id,
            module_slug,
            predecessor_schema_digest,
            candidate_schema_digest,
            rollback_window_id,
            installed_at: Utc::now(),
            state: SettingsGuardState::Active,
        }
    }

    /// Constructs an active compatibility guard from a checkpoint if it is in Observing state.
    pub fn from_observing_checkpoint(
        checkpoint: &crate::ModuleTransitionCheckpoint,
        predecessor_schema_digest: String,
        candidate_schema_digest: String,
    ) -> Option<Self> {
        match checkpoint.state {
            crate::ModuleTransitionState::Observing { .. } => Some(Self {
                id: checkpoint.operation_id,
                tenant_id: checkpoint.tenant_id,
                module_slug: checkpoint.module_slug.clone(),
                predecessor_schema_digest,
                candidate_schema_digest,
                rollback_window_id: checkpoint.operation_id,
                installed_at: checkpoint.updated_at,
                state: SettingsGuardState::Active,
            }),
            _ => None,
        }
    }

    /// Validates proposed settings against the guard.
    ///
    /// When `Active`, verifies that the settings value satisfies BOTH the
    /// predecessor (N) and candidate (N+1) schemas.
    pub fn validate_and_normalize_write(
        &self,
        predecessor_schema: &HashMap<String, ModuleSettingSpec>,
        candidate_schema: &HashMap<String, ModuleSettingSpec>,
        settings: serde_json::Value,
    ) -> Result<serde_json::Value, SettingsGuardError> {
        match &self.state {
            SettingsGuardState::Active => validate_settings_intersection(
                &self.module_slug,
                predecessor_schema,
                candidate_schema,
                settings,
            ),
            SettingsGuardState::MaintenanceOverridden { .. } | SettingsGuardState::Closed => {
                // When overridden or closed, validate only against the candidate schema.
                normalize_module_settings(&self.module_slug, candidate_schema, settings).map_err(
                    |e| match e {
                        ModuleSettingsValidationError::InvalidValue { key, reason, .. }
                        | ModuleSettingsValidationError::InvalidSchema { key, reason, .. } => {
                            SettingsGuardError::CandidateIncompatible { key, reason }
                        }
                        ModuleSettingsValidationError::InvalidKey { key, .. } => {
                            SettingsGuardError::CandidateIncompatible {
                                key,
                                reason: "invalid key".to_string(),
                            }
                        }
                    },
                )
            }
        }
    }

    /// Closes the guard permanently upon rollback window expiration or finalization.
    pub fn close(&mut self) {
        self.state = SettingsGuardState::Closed;
    }

    /// Bypasses the guard via explicit operator confirmation, closing rollback eligibility.
    pub fn override_maintenance(&mut self, reason: String) {
        self.state = SettingsGuardState::MaintenanceOverridden {
            operator_reason: reason,
            overridden_at: Utc::now(),
        };
    }
}

/// Validates that `settings` is accepted by both `predecessor_schema` and `candidate_schema`.
pub fn validate_settings_intersection(
    module_slug: &str,
    predecessor_schema: &HashMap<String, ModuleSettingSpec>,
    candidate_schema: &HashMap<String, ModuleSettingSpec>,
    settings: serde_json::Value,
) -> Result<serde_json::Value, SettingsGuardError> {
    // 1. Validate against predecessor schema (N)
    let normalized_predecessor =
        normalize_module_settings(module_slug, predecessor_schema, settings.clone()).map_err(
            |e| match e {
                ModuleSettingsValidationError::InvalidValue { key, reason, .. }
                | ModuleSettingsValidationError::InvalidSchema { key, reason, .. } => {
                    SettingsGuardError::PredecessorIncompatible { key, reason }
                }
                ModuleSettingsValidationError::InvalidKey { key, .. } => {
                    SettingsGuardError::PredecessorIncompatible {
                        key,
                        reason: "unknown key in predecessor schema".to_string(),
                    }
                }
            },
        )?;

    // 2. Validate against candidate schema (N+1)
    let normalized_candidate = normalize_module_settings(module_slug, candidate_schema, settings)
        .map_err(|e| match e {
        ModuleSettingsValidationError::InvalidValue { key, reason, .. }
        | ModuleSettingsValidationError::InvalidSchema { key, reason, .. } => {
            SettingsGuardError::CandidateIncompatible { key, reason }
        }
        ModuleSettingsValidationError::InvalidKey { key, .. } => {
            SettingsGuardError::CandidateIncompatible {
                key,
                reason: "unknown key in candidate schema".to_string(),
            }
        }
    })?;

    // Return the candidate-normalized value since N+1 is the active target,
    // but having proven N also accepts it without error.
    let _ = normalized_predecessor;
    Ok(normalized_candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_schema_n() -> HashMap<String, ModuleSettingSpec> {
        let mut map = HashMap::new();
        map.insert(
            "timeout".to_string(),
            ModuleSettingSpec {
                value_type: "number".to_string(),
                required: true,
                default: Some(serde_json::json!(30)),
                min: Some(1.0),
                max: Some(100.0),
                ..Default::default()
            },
        );
        map.insert(
            "mode".to_string(),
            ModuleSettingSpec {
                value_type: "string".to_string(),
                required: true,
                options: vec![serde_json::json!("sync"), serde_json::json!("async")],
                ..Default::default()
            },
        );
        map
    }

    fn make_schema_n_plus_1() -> HashMap<String, ModuleSettingSpec> {
        let mut map = make_schema_n();
        // N+1 adds an option "batch" to mode
        map.get_mut("mode")
            .unwrap()
            .options
            .push(serde_json::json!("batch"));
        // N+1 adds an optional setting "retries" with a default
        map.insert(
            "retries".to_string(),
            ModuleSettingSpec {
                value_type: "number".to_string(),
                required: false,
                default: Some(serde_json::json!(3)),
                min: Some(0.0),
                max: Some(10.0),
                ..Default::default()
            },
        );
        map
    }

    #[test]
    fn test_valid_intersection_value_passes_guard() {
        let schema_n = make_schema_n();
        let schema_n1 = make_schema_n_plus_1();
        let guard = SettingsCompatibilityGuard::new(
            Some(Uuid::new_v4()),
            "email".to_string(),
            "sha256:predecessor".to_string(),
            "sha256:candidate".to_string(),
            Uuid::new_v4(),
        );

        let input = serde_json::json!({
            "timeout": 45,
            "mode": "sync"
        });

        let result = guard.validate_and_normalize_write(&schema_n, &schema_n1, input);
        assert!(result.is_ok());
        let normalized = result.unwrap();
        assert_eq!(normalized["timeout"], 45);
        assert_eq!(normalized["mode"], "sync");
        // candidate default "retries: 3" is applied by candidate normalization
        assert_eq!(normalized["retries"], 3);
    }

    #[test]
    fn test_candidate_only_value_rejected_by_active_guard() {
        let schema_n = make_schema_n();
        let schema_n1 = make_schema_n_plus_1();
        let guard = SettingsCompatibilityGuard::new(
            Some(Uuid::new_v4()),
            "email".to_string(),
            "sha256:predecessor".to_string(),
            "sha256:candidate".to_string(),
            Uuid::new_v4(),
        );

        // "mode: batch" is valid in N+1 but unknown/rejected in N
        let input = serde_json::json!({
            "timeout": 45,
            "mode": "batch"
        });

        let result = guard.validate_and_normalize_write(&schema_n, &schema_n1, input);
        assert!(matches!(
            result,
            Err(SettingsGuardError::PredecessorIncompatible { .. })
        ));
    }

    #[test]
    fn test_maintenance_override_allows_candidate_only_value() {
        let schema_n = make_schema_n();
        let schema_n1 = make_schema_n_plus_1();
        let mut guard = SettingsCompatibilityGuard::new(
            Some(Uuid::new_v4()),
            "email".to_string(),
            "sha256:predecessor".to_string(),
            "sha256:candidate".to_string(),
            Uuid::new_v4(),
        );

        guard.override_maintenance("Operator approved batch mode switch".to_string());

        let input = serde_json::json!({
            "timeout": 45,
            "mode": "batch"
        });

        let result = guard.validate_and_normalize_write(&schema_n, &schema_n1, input);
        assert!(result.is_ok());
        assert_eq!(result.unwrap()["mode"], "batch");
    }
}
