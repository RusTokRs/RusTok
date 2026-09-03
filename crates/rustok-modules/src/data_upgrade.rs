//! Derives dynamic data-upgrade phase, irreversibility, and rollback eligibility from owner evidence.
//!
//! Evaluates migration preflight receipt, live settings compatibility, and live object inventory
//! to determine whether an operation is compatible, maintenance-only, or has reached the point of no return.

use serde::{Deserialize, Serialize};

use crate::MigrationPreflightReceipt;

/// Lifecycle phase of dynamic data-contract and persistence upgrades.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataUpgradePhase {
    /// Fully reversible zero-downtime evolution: non-destructive steps, settings compatible with both N and N+1.
    Compatible,
    /// Fenced maintenance mode: structured data or object migration required; reversible prior to destructive cutover.
    MaintenancePreCutover,
    /// Irreversible phase: destructive migration step, non-transactional effect, or candidate-only settings committed.
    PointOfNoReturn,
    /// All migrations and transformations successfully converged.
    Completed,
}

/// Comprehensive owner evidence required for data upgrade and rollback decisions.
#[derive(Debug, Clone)]
pub struct DataUpgradeEvidence<'a> {
    pub preflight: &'a MigrationPreflightReceipt,
    pub settings_intersection_valid: bool,
    pub requires_cross_revision_data_copy: bool,
    pub unmigrated_live_objects_count: u64,
    pub point_of_no_return_committed: bool,
}

/// Decision derived from owner evidence for governance and safety enforcement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataUpgradeDecision {
    pub phase: DataUpgradePhase,
    pub is_irreversible: bool,
    pub rollback_allowed: bool,
    pub can_auto_converge: bool,
    pub reason: String,
}

/// Evaluates dynamic data-upgrade phase, irreversibility, and rollback eligibility from owner evidence.
pub fn evaluate_data_upgrade_decision(evidence: &DataUpgradeEvidence) -> DataUpgradeDecision {
    if evidence.point_of_no_return_committed {
        return DataUpgradeDecision {
            phase: DataUpgradePhase::PointOfNoReturn,
            is_irreversible: true,
            rollback_allowed: false,
            can_auto_converge: false,
            reason: "Point of no return has been committed; rollback is permanently closed".to_string(),
        };
    }

    if !evidence.preflight.is_additive_safe {
        return DataUpgradeDecision {
            phase: DataUpgradePhase::MaintenancePreCutover,
            is_irreversible: false,
            rollback_allowed: true,
            can_auto_converge: false,
            reason: "Destructive or non-additive migration steps require explicit point-of-no-return and fences before execution"
                .to_string(),
        };
    }

    if !evidence.settings_intersection_valid {
        return DataUpgradeDecision {
            phase: DataUpgradePhase::MaintenancePreCutover,
            is_irreversible: false,
            rollback_allowed: true,
            can_auto_converge: false,
            reason: "Live settings value does not satisfy both predecessor and candidate schemas; requires fenced settings update"
                .to_string(),
        };
    }

    if evidence.requires_cross_revision_data_copy {
        return DataUpgradeDecision {
            phase: DataUpgradePhase::MaintenancePreCutover,
            is_irreversible: false,
            rollback_allowed: true,
            can_auto_converge: false,
            reason: "Cross-revision dynamic data-contract evolution is maintenance-only".to_string(),
        };
    }

    if evidence.unmigrated_live_objects_count > 0 {
        return DataUpgradeDecision {
            phase: DataUpgradePhase::MaintenancePreCutover,
            is_irreversible: false,
            rollback_allowed: true,
            can_auto_converge: false,
            reason: format!(
                "Source contract has {} unmigrated live objects in module_artifact_data_objects",
                evidence.unmigrated_live_objects_count
            ),
        };
    }

    DataUpgradeDecision {
        phase: DataUpgradePhase::Compatible,
        is_irreversible: false,
        rollback_allowed: true,
        can_auto_converge: true,
        reason: "Zero-downtime reversible evolution; settings accepted by both schemas and no unmigrated live objects"
            .to_string(),
    }
}
