use crate::{
    validate_binding_definitions, validate_dynamic_definitions, ProjectDocument,
    ValidationDiagnostic, ValidationReport,
};

pub fn validate_runtime_extensions(
    document: &ProjectDocument,
) -> Vec<ValidationDiagnostic> {
    let mut diagnostics = validate_binding_definitions(document);
    diagnostics.extend(validate_dynamic_definitions(document));
    diagnostics
}

pub fn extend_with_runtime_validation(
    document: &ProjectDocument,
    mut report: ValidationReport,
) -> ValidationReport {
    report
        .diagnostics
        .extend(validate_runtime_extensions(document));
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GrapesJsV1Codec;
    use serde_json::json;

    #[test]
    fn runtime_validation_combines_binding_and_dynamic_diagnostics() {
        let document = GrapesJsV1Codec::decode_value(json!({
            "pages": [{
                "component": { "id": "root", "type": "wrapper" }
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
        .expect("document");
        let diagnostics = validate_runtime_extensions(&document);
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "runtime_binding_target_missing"));
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "runtime_repeater_targets_page_root"));
    }
}
