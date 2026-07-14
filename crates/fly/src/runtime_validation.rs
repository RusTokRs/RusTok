use crate::{
    analyze_runtime_context_dependencies, extract_runtime_context_contract,
    validate_binding_definitions, validate_dynamic_definitions, ProjectDocument,
    ValidationDiagnostic, ValidationReport,
};
use std::collections::BTreeSet;

pub fn validate_runtime_extensions(
    document: &ProjectDocument,
) -> Vec<ValidationDiagnostic> {
    let mut diagnostics = extract_runtime_context_contract(document).definition_diagnostics;
    diagnostics.extend(validate_binding_definitions(document));
    diagnostics.extend(validate_dynamic_definitions(document));
    diagnostics.extend(analyze_runtime_context_dependencies(document).diagnostics);
    deduplicate_diagnostics(&mut diagnostics);
    diagnostics
}

pub fn extend_with_runtime_validation(
    document: &ProjectDocument,
    mut report: ValidationReport,
) -> ValidationReport {
    let mut seen = report
        .diagnostics
        .iter()
        .map(diagnostic_identity)
        .collect::<BTreeSet<_>>();
    for diagnostic in validate_runtime_extensions(document) {
        if seen.insert(diagnostic_identity(&diagnostic)) {
            report.diagnostics.push(diagnostic);
        }
    }
    report
}

fn deduplicate_diagnostics(diagnostics: &mut Vec<ValidationDiagnostic>) {
    let mut seen = BTreeSet::new();
    diagnostics.retain(|diagnostic| seen.insert(diagnostic_identity(diagnostic)));
}

fn diagnostic_identity(
    diagnostic: &ValidationDiagnostic,
) -> (u8, String, String, String) {
    (
        diagnostic.severity as u8,
        diagnostic.code.clone(),
        diagnostic.path.clone(),
        diagnostic.message.clone(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{validate_project, GrapesJsV1Codec, RegistrySet, ValidationLimits};
    use serde_json::json;

    fn invalid_runtime_document() -> ProjectDocument {
        GrapesJsV1Codec::decode_value(json!({
            "pages": [{
                "component": { "id": "root", "type": "wrapper" }
            }],
            "flyRuntimeContextSchema": [{
                "id": "count",
                "path": "count",
                "kind": "number",
                "default": "invalid"
            }, {
                "id": "root-context",
                "path": "",
                "kind": "object"
            }, {
                "id": "title-input",
                "path": "page.title",
                "kind": "string"
            }],
            "flyRuntimeComputed": [{
                "id": "title-computed",
                "path": "page.title",
                "expression": { "op": "literal", "value": "Computed" }
            }],
            "flyRuntimeBindings": [{
                "id": "binding",
                "component_id": "missing",
                "path": "value",
                "target": "field",
                "name": "content"
            }],
            "flyRuntimeRepeaters": [{
                "id": "repeater",
                "component_id": "root",
                "path": "items"
            }]
        }))
        .expect("document")
    }

    #[test]
    fn runtime_validation_combines_contract_dependency_binding_and_dynamic_diagnostics() {
        let document = invalid_runtime_document();
        let diagnostics = validate_runtime_extensions(&document);
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "runtime_context_default_type_mismatch"));
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "runtime_context_field_path_invalid"));
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "runtime_context_path_shadowed_by_computed"
        }));
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "runtime_binding_target_missing"));
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "runtime_repeater_targets_page_root"));
    }

    #[test]
    fn extending_canonical_report_does_not_duplicate_runtime_diagnostics() {
        let document = invalid_runtime_document();
        let report = validate_project(
            &document,
            &RegistrySet::with_builtins(),
            ValidationLimits::default(),
        );
        let extended = extend_with_runtime_validation(&document, report);
        let count = extended
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "runtime_context_field_path_invalid")
            .count();
        assert_eq!(count, 1);
    }
}
