use crate::context_contract::{deduplicate_diagnostics, promote_context_diagnostic};
use crate::context_schema::validate_context_fields;
use crate::{
    ContextMaterialization, ContextSchemaCatalog, ProjectDocument,
    RuntimeContextCompatibilityPolicy, RuntimeContextCompatibilityReport,
    RuntimeContextContractChangeKind, RuntimeContextPreflightPolicy, ValidationDiagnostic,
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
    /// When disabled, validate the supplied field values without running computed producers.
    /// Field validation is mandatory in both modes.
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
    /// No blocking compatibility or payload diagnostic. Computed producers are evaluated
    /// only when the migration policy requests materialization.
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
        let catalog = ContextSchemaCatalog::from_document(target);
        let (type_mismatches, diagnostics) = validate_context_fields(&catalog, &migrated);
        ContextMaterialization {
            context: migrated,
            diagnostics,
            defaults_applied: 0,
            computed_applied: 0,
            computed_fallbacks: 0,
            unresolved_computed: 0,
            type_mismatches,
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
    // Compatibility tolerance applies only to contract changes, never to invalid payloads.
    diagnostics.extend(
        materialization
            .diagnostics
            .iter()
            .cloned()
            .map(|diagnostic| {
                promote_context_diagnostic(diagnostic, RuntimeContextPreflightPolicy::default())
            }),
    );
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

    deduplicate_diagnostics(&mut diagnostics);
    RuntimeContextMigration {
        context: materialization.context,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GrapesJsCodec;
    use serde_json::json;

    fn field_document() -> ProjectDocument {
        GrapesJsCodec::decode_value(json!({
            "pages": [{ "component": { "id": "root", "type": "wrapper" } }],
            "flyRuntimeContextSchema": [
                { "id": "count", "path": "count", "kind": "number", "required": true },
                { "id": "items", "path": "items", "kind": "array", "item_kind": "string" },
                { "id": "currency", "path": "currency", "kind": "string", "default": "EUR" }
            ]
        }))
        .expect("document")
    }

    #[test]
    fn migration_validates_fields_with_and_without_materialization() {
        let document = field_document();
        let input = json!({ "count": "wrong", "items": [1, 2, "valid"] });
        for apply_target_defaults in [true, false] {
            let migration = migrate_runtime_context(
                &document,
                &document,
                &input,
                RuntimeContextMigrationPolicy {
                    apply_target_defaults,
                    tolerate_breaking_changes: true,
                    ..Default::default()
                },
            );
            assert!(!migration.is_accepted());
            assert_eq!(migration.type_mismatches, 3);
            assert_eq!(
                migration.defaults_applied,
                if apply_target_defaults { 1 } else { 0 }
            );
            if !apply_target_defaults {
                assert_eq!(migration.context, input);
            }
            assert!(migration.diagnostics.iter().any(|diagnostic| {
                diagnostic.code == "runtime_context_array_item_type_mismatch"
                    && diagnostic.severity == ValidationSeverity::Error
            }));
        }
    }

    #[test]
    fn missing_required_field_is_not_accepted_even_without_contract_changes() {
        let document = field_document();
        for apply_target_defaults in [true, false] {
            let migration = migrate_runtime_context(
                &document,
                &document,
                &json!({}),
                RuntimeContextMigrationPolicy {
                    apply_target_defaults,
                    ..Default::default()
                },
            );
            assert!(!migration.is_accepted());
            assert!(migration.diagnostics.iter().any(|diagnostic| {
                diagnostic.code == "runtime_context_required_missing"
                    && diagnostic.severity == ValidationSeverity::Error
            }));
        }
    }

    #[test]
    fn valid_unmaterialized_payload_is_preserved() {
        let document = field_document();
        let input = json!({ "count": 3, "items": ["valid"] });
        let migration = migrate_runtime_context(
            &document,
            &document,
            &input,
            RuntimeContextMigrationPolicy {
                apply_target_defaults: false,
                ..Default::default()
            },
        );
        assert!(migration.is_accepted());
        assert_eq!(migration.context, input);
        assert_eq!(migration.defaults_applied, 0);
    }

    #[test]
    fn unresolved_computed_producer_blocks_materialized_migration() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{ "component": { "id": "root", "type": "wrapper" } }],
            "flyRuntimeComputed": [{
                "id": "label", "path": "label",
                "expression": { "op": "path", "path": "missing" }
            }]
        }))
        .expect("document");
        let migration = migrate_runtime_context(
            &document,
            &document,
            &json!({}),
            RuntimeContextMigrationPolicy::default(),
        );
        assert!(!migration.is_accepted());
        assert_eq!(migration.unresolved_computed, 1);
    }
}
