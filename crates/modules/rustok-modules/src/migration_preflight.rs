//! Migration preflight evaluation and update mode classifier.
//!
//! Evaluates whether a proposed module update qualifies for `Automatic` update
//! and rollback mode or fails closed to `Maintenance` mode ("Database update required").

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Operational update mode derived from preflight analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateMode {
    /// Safe for automated rollout and single-attempt rollback without DB reversal.
    /// Requires non-destructive additive-only schema evolution and N/N+1 compatibility.
    Automatic,
    /// Requires scheduled maintenance window and manual operator confirmation.
    /// Automatic rollback is prohibited due to locking, destructive, or irreversible changes.
    Maintenance,
}

/// Immutable receipt of a migration preflight evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationPreflightReceipt {
    pub operation_id: Uuid,
    pub module_slug: String,
    pub mode: UpdateMode,
    pub source_schema_digest: String,
    pub target_schema_digest: String,
    pub migration_plan_digest: String,
    pub is_additive_safe: bool,
    pub settings_guard_installed: bool,
    pub denial_reasons: Vec<String>,
    pub evaluated_at: DateTime<Utc>,
}

/// Input parameters for migration preflight evaluation.
#[derive(Debug, Clone)]
pub struct MigrationPreflightInput {
    pub operation_id: Uuid,
    pub module_slug: String,
    pub source_schema_digest: String,
    pub target_schema_digest: String,
    pub migration_plan_digest: String,
    pub is_additive_safe: bool,
    pub migration_reasons: Vec<String>,
    pub settings_guard_installed: bool,
    pub has_irreversible_external_effects: bool,
    pub requires_cross_revision_data_copy: bool,
}

/// Evaluates migration safety and computes the canonical update mode.
pub fn evaluate_migration_preflight(input: MigrationPreflightInput) -> MigrationPreflightReceipt {
    let mut denial_reasons = Vec::new();

    if !input.is_additive_safe {
        denial_reasons.extend(input.migration_reasons);
    }

    if input.has_irreversible_external_effects {
        denial_reasons.push(
            "Release contains uncompensated or irreversible external side effects".to_string(),
        );
    }

    if input.requires_cross_revision_data_copy {
        denial_reasons.push(
            "Cross-revision dynamic data-contract evolution is classified as maintenance-only"
                .to_string(),
        );
    }

    let mode = if denial_reasons.is_empty() {
        UpdateMode::Automatic
    } else {
        UpdateMode::Maintenance
    };

    MigrationPreflightReceipt {
        operation_id: input.operation_id,
        module_slug: input.module_slug,
        mode,
        source_schema_digest: input.source_schema_digest,
        target_schema_digest: input.target_schema_digest,
        migration_plan_digest: input.migration_plan_digest,
        is_additive_safe: input.is_additive_safe,
        settings_guard_installed: input.settings_guard_installed,
        denial_reasons,
        evaluated_at: Utc::now(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_additive_safe_yields_automatic_mode() {
        let input = MigrationPreflightInput {
            operation_id: Uuid::new_v4(),
            module_slug: "product".to_string(),
            source_schema_digest: "sha256:source".to_string(),
            target_schema_digest: "sha256:target".to_string(),
            migration_plan_digest: "sha256:plan".to_string(),
            is_additive_safe: true,
            migration_reasons: vec![],
            settings_guard_installed: true,
            has_irreversible_external_effects: false,
            requires_cross_revision_data_copy: false,
        };

        let receipt = evaluate_migration_preflight(input);
        assert_eq!(receipt.mode, UpdateMode::Automatic);
        assert!(receipt.denial_reasons.is_empty());
    }

    #[test]
    fn test_destructive_migration_fails_to_maintenance() {
        let input = MigrationPreflightInput {
            operation_id: Uuid::new_v4(),
            module_slug: "product".to_string(),
            source_schema_digest: "sha256:source".to_string(),
            target_schema_digest: "sha256:target".to_string(),
            migration_plan_digest: "sha256:plan".to_string(),
            is_additive_safe: false,
            migration_reasons: vec!["Dropped column legacy_code".to_string()],
            settings_guard_installed: true,
            has_irreversible_external_effects: false,
            requires_cross_revision_data_copy: false,
        };

        let receipt = evaluate_migration_preflight(input);
        assert_eq!(receipt.mode, UpdateMode::Maintenance);
        assert_eq!(receipt.denial_reasons.len(), 1);
        assert!(receipt.denial_reasons[0].contains("Dropped column"));
    }

    #[test]
    fn test_irreversible_effects_fails_to_maintenance() {
        let input = MigrationPreflightInput {
            operation_id: Uuid::new_v4(),
            module_slug: "payment".to_string(),
            source_schema_digest: "sha256:source".to_string(),
            target_schema_digest: "sha256:target".to_string(),
            migration_plan_digest: "sha256:plan".to_string(),
            is_additive_safe: true,
            migration_reasons: vec![],
            settings_guard_installed: true,
            has_irreversible_external_effects: true,
            requires_cross_revision_data_copy: false,
        };

        let receipt = evaluate_migration_preflight(input);
        assert_eq!(receipt.mode, UpdateMode::Maintenance);
        assert!(
            receipt
                .denial_reasons
                .iter()
                .any(|r| r.contains("irreversible"))
        );
    }

    #[test]
    fn test_cross_revision_data_copy_fails_to_maintenance() {
        let input = MigrationPreflightInput {
            operation_id: Uuid::new_v4(),
            module_slug: "order".to_string(),
            source_schema_digest: "sha256:source".to_string(),
            target_schema_digest: "sha256:target".to_string(),
            migration_plan_digest: "sha256:plan".to_string(),
            is_additive_safe: true,
            migration_reasons: vec![],
            settings_guard_installed: true,
            has_irreversible_external_effects: false,
            requires_cross_revision_data_copy: true,
        };

        let receipt = evaluate_migration_preflight(input);
        assert_eq!(receipt.mode, UpdateMode::Maintenance);
        assert!(
            receipt
                .denial_reasons
                .iter()
                .any(|r| r.contains("Cross-revision"))
        );
    }
}
