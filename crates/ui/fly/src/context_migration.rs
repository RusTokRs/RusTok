use crate::{
    ContextMaterialization, ProjectDocument, RuntimeContextCompatibilityPolicy,
    RuntimeContextCompatibilityReport, RuntimeContextContractChangeKind, ValidationDiagnostic,
    ValidationSeverity, diff_runtime_context_contracts_with_policy, materialize_context,
    remove_context_path,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeContextMigrationPolicy {
    /// Remove values whose paths the target contract no longer declares.
    ///
    /// Defaults to `false`: dropping caller data is destructive and cannot be undone by the
    /// target contract, so it is an explicit choice. Dropped paths are reported either way.
    pub drop_removed_paths: bool,
    /// Apply the target contract's defaults and computed values to the migrated payload.
    pub apply_target_defaults: bool,
    /// Keep migrating when the target contract breaks compatibility with the source contract.
    pub tolerate_breaking_changes: bool,
    pub compatibility_policy: RuntimeContextCompatibilityPolicy,
}

impl Default for RuntimeContextMigrationPolicy {
    fn default() -> Self {
        Self {
            drop_removed_paths: false,
            apply_target_defaults: true,
            tolerate_breaking_changes: false,
            compatibility_policy: RuntimeContextCompatibilityPolicy::default(),
        }
    }
}

/// Result of moving a runtime context payload from one contract to another.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeContextMigration {
    pub context: Value,
    pub compatibility: RuntimeContextCompatibilityReport,
    pub dropped_paths: Vec<String>,
    pub defaults_applied: usize,
    pub computed_applied: usize,
    pub computed_fallbacks: usize,
    pub unresolved_computed: usize,
    pub type_mismatches: usize,
    pub diagnostics: Vec<ValidationDiagnostic>,
}

impl RuntimeContextMigration {
    /// No error-severity diagnostic: the migrated payload is valid under the target contract.
    pub fn is_accepted(&self) -> bool {
        !self
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == ValidationSeverity::Error)
    }
}

/// Migrate a runtime context payload from the source document's contract to the target's.
///
/// The source document is the contract the payload was authored against (for example the
/// published revision) and the target is the candidate contract. The payload is never repaired
/// silently: values the target contract cannot accept surface as error diagnostics, and paths
/// dropped under `drop_removed_paths` surface as warnings.
pub fn migrate_runtime_context(
    source: &ProjectDocument,
    target: &ProjectDocument,
    context: &Value,
    policy: RuntimeContextMigrationPolicy,
) -> RuntimeContextMigration {
    let compatibility =
        diff_runtime_context_contracts_with_policy(source, target, policy.compatibility_policy);

    let mut migrated = context.clone();
    let mut dropped_paths = Vec::new();
    if policy.drop_removed_paths {
        for change in compatibility.changes.iter().filter(|change| {
            matches!(
                change.kind,
                RuntimeContextContractChangeKind::FieldRemoved
                    | RuntimeContextContractChangeKind::ComputedRemoved
            )
        }) {
            if remove_context_path(&mut migrated, &change.path) {
                dropped_paths.push(change.path.clone());
            }
        }
        dropped_paths.sort();
        dropped_paths.dedup();
    }

    let materialization = if policy.apply_target_defaults {
        materialize_context(target, &migrated)
    } else {
        ContextMaterialization {
            context: migrated,
            diagnostics: Vec::new(),
            defaults_applied: 0,
            computed_applied: 0,
            computed_fallbacks: 0,
            unresolved_computed: 0,
            type_mismatches: 0,
        }
    };

    let mut diagnostics = compatibility.diagnostics.clone();
    if policy.tolerate_breaking_changes {
        for diagnostic in &mut diagnostics {
            if diagnostic.code == "runtime_context_contract_breaking_change" {
                diagnostic.code = "runtime_context_contract_breaking_change_tolerated".to_string();
                diagnostic.severity = ValidationSeverity::Warning;
            }
        }
    }
    diagnostics.extend(materialization.diagnostics.iter().cloned());
    for path in &dropped_paths {
        diagnostics.push(migration_diagnostic(
            ValidationSeverity::Warning,
            "runtime_context_migration_path_dropped",
            path,
            format!(
                "value at `{path}` was dropped because the target contract no longer declares it"
            ),
        ));
    }

    RuntimeContextMigration {
        context: materialization.context.clone(),
        compatibility,
        dropped_paths,
        defaults_applied: materialization.defaults_applied,
        computed_applied: materialization.computed_applied,
        computed_fallbacks: materialization.computed_fallbacks,
        unresolved_computed: materialization.unresolved_computed,
        type_mismatches: materialization.type_mismatches,
        diagnostics,
    }
}

fn migration_diagnostic(
    severity: ValidationSeverity,
    code: &str,
    path: &str,
    message: impl Into<String>,
) -> ValidationDiagnostic {
    ValidationDiagnostic {
        severity,
        code: code.to_string(),
        path: path.to_string(),
        message: message.into(),
    }
}
