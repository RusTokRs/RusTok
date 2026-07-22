use crate::{
    ProjectDocument, ValidationDiagnostic, ValidationReport, analyze_runtime_context_dependencies,
    extract_runtime_context_contract, validate_binding_definitions, validate_component_actions,
    validate_dynamic_definitions, validate_internal_page_links, validate_localized_page_routes,
    validate_project_locale_policy, validate_translation_definitions,
};
use std::collections::BTreeSet;

pub fn validate_runtime_extensions(document: &ProjectDocument) -> Vec<ValidationDiagnostic> {
    let mut diagnostics = extract_runtime_context_contract(document).definition_diagnostics;
    diagnostics.extend(validate_project_locale_policy(document));
    diagnostics.extend(validate_translation_definitions(document));
    diagnostics.extend(validate_localized_page_routes(document));
    diagnostics.extend(validate_internal_page_links(document));
    diagnostics.extend(validate_component_actions(document));
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

fn diagnostic_identity(diagnostic: &ValidationDiagnostic) -> (u8, String, String, String) {
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
    use crate::{GrapesJsCodec, RegistrySet, ValidationLimits, validate_project};
    use serde_json::json;

    fn invalid_runtime_document() -> ProjectDocument {
        GrapesJsCodec::decode_value(json!({
            "pages": [{
                "component": { "id": "root", "type": "wrapper" }
            }],
            "flyTranslations": [{
                "id": "hero",
                "values": { "invalid locale": "Hello" }
            }, {
                "id": "hero",
                "values": { "en": "Hello" }
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
    fn runtime_validation_combines_locale_translation_contract_dependency_binding_and_dynamic_diagnostics()
     {
        let document = invalid_runtime_document();
        let diagnostics = validate_runtime_extensions(&document);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "translation_locale_invalid")
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "duplicate_translation_id")
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "runtime_context_default_type_mismatch")
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "runtime_context_field_path_invalid")
        );
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.code == "runtime_context_path_shadowed_by_computed"
            })
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "runtime_binding_target_missing")
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "runtime_repeater_targets_page_root")
        );
    }

    #[test]
    fn strict_project_locale_policy_promotes_missing_coverage_to_errors() {
        let document = GrapesJsCodec::decode_value(json!({
            "flyLocales": {
                "default_locale": "en",
                "supported_locales": ["en", "ru"],
                "required_locales": ["en", "ru"],
                "enforce_required_locales": true
            },
            "flyTranslations": [{
                "id": "hero",
                "values": { "en": "Hello" }
            }],
            "pages": [{
                "flyPageMeta": {
                    "title": { "$localized": { "en": "Home" } }
                },
                "component": { "id": "root", "type": "wrapper" }
            }]
        }))
        .expect("document");
        let diagnostics = validate_runtime_extensions(&document);
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "translation_required_locale_missing"
                && diagnostic.severity == crate::ValidationSeverity::Error
        }));
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "localized_metadata_required_locale_missing"
                && diagnostic.severity == crate::ValidationSeverity::Error
        }));
    }

    #[test]
    fn duplicate_localized_slugs_block_publish_validation() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "one",
                "flyPageMeta": { "slug": { "$localized": { "en": "shared" } } },
                "component": { "id": "root-one", "type": "wrapper" }
            }, {
                "id": "two",
                "flyPageMeta": { "slug": { "$localized": { "en": "shared" } } },
                "component": { "id": "root-two", "type": "wrapper" }
            }]
        }))
        .expect("document");
        let diagnostics = validate_runtime_extensions(&document);
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "duplicate_localized_page_slug"
                && diagnostic.severity == crate::ValidationSeverity::Error
        }));
    }

    #[test]
    fn missing_internal_page_link_target_blocks_publish_validation() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "flyPageMeta": { "slug": "home" },
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "missing-link",
                        "type": "link",
                        "flyPageLink": { "page_id": "missing" }
                    }]
                }
            }]
        }))
        .expect("document");
        let diagnostics = validate_runtime_extensions(&document);
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "internal_page_link_target_missing"
                && diagnostic.severity == crate::ValidationSeverity::Error
        }));
    }

    #[test]
    fn invalid_action_and_form_contracts_block_publish_validation() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "flyPageMeta": { "slug": "home" },
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "form",
                        "type": "form",
                        "flyForm": {
                            "id": "contact",
                            "action_url": "/submit",
                            "provider": "crm",
                            "action": "create"
                        }
                    }, {
                        "id": "button",
                        "type": "button",
                        "flyAction": {
                            "kind": "submit_form",
                            "form_id": "missing"
                        }
                    }]
                }
            }]
        }))
        .expect("document");
        let diagnostics = validate_runtime_extensions(&document);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "form_definition_invalid")
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "action_definition_invalid")
        );
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
