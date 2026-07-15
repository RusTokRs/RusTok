use crate::{
    extract_runtime_context_contract, materialize_bindings, materialize_context,
    materialize_runtime, BindingMaterialization, ContextMaterialization, ProjectDocument,
    RuntimeMaterialization, ValidationDiagnostic,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeProjectMaterialization {
    pub document: ProjectDocument,
    pub effective_context: Value,
    pub diagnostics: Vec<ValidationDiagnostic>,
    pub defaults_applied: usize,
    pub computed_applied: usize,
    pub computed_fallbacks: usize,
    pub unresolved_computed: usize,
    pub context_type_mismatches: usize,
    pub applied_bindings: usize,
    pub fallback_bindings: usize,
    pub unresolved_bindings: usize,
    pub evaluated_conditions: usize,
    pub hidden_components: usize,
    pub repeated_nodes: usize,
}

pub fn materialize_project_with_runtime_context(
    document: &ProjectDocument,
    input_context: &Value,
) -> RuntimeProjectMaterialization {
    let contract = extract_runtime_context_contract(document);
    let contract_is_valid = contract.is_valid();
    let mut diagnostics = contract.definition_diagnostics;
    let (
        effective_context,
        defaults_applied,
        computed_applied,
        computed_fallbacks,
        unresolved_computed,
        context_type_mismatches,
    ) = if contract_is_valid {
        let ContextMaterialization {
            context,
            diagnostics: context_diagnostics,
            defaults_applied,
            computed_applied,
            computed_fallbacks,
            unresolved_computed,
            type_mismatches,
        } = materialize_context(document, input_context);
        diagnostics.extend(context_diagnostics);
        (
            context,
            defaults_applied,
            computed_applied,
            computed_fallbacks,
            unresolved_computed,
            type_mismatches,
        )
    } else {
        (input_context.clone(), 0, 0, 0, 0, 0)
    };

    let BindingMaterialization {
        document,
        diagnostics: binding_diagnostics,
        applied_bindings,
        fallback_bindings,
        unresolved_bindings,
    } = materialize_bindings(document, &effective_context);
    diagnostics.extend(binding_diagnostics);
    let RuntimeMaterialization {
        document,
        diagnostics: dynamic_diagnostics,
        evaluated_conditions,
        hidden_components,
        repeated_nodes,
    } = materialize_runtime(&document, &effective_context);
    diagnostics.extend(dynamic_diagnostics);

    RuntimeProjectMaterialization {
        document,
        effective_context,
        diagnostics,
        defaults_applied,
        computed_applied,
        computed_fallbacks,
        unresolved_computed,
        context_type_mismatches,
        applied_bindings,
        fallback_bindings,
        unresolved_bindings,
        evaluated_conditions,
        hidden_components,
        repeated_nodes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GrapesJsV1Codec;
    use serde_json::json;

    #[test]
    fn pipeline_exposes_effective_context_and_materialized_document() {
        let document = GrapesJsV1Codec::decode_value(json!({
            "pages": [{
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "title",
                        "type": "text",
                        "content": "Static"
                    }]
                }
            }],
            "flyRuntimeContextSchema": [{
                "id": "prefix",
                "path": "page.prefix",
                "kind": "string",
                "default": "Hello"
            }],
            "flyRuntimeComputed": [{
                "id": "title",
                "path": "page.title",
                "expression": {
                    "op": "format",
                    "template": "{{page.prefix}} world"
                }
            }],
            "flyRuntimeBindings": [{
                "id": "title-content",
                "component_id": "title",
                "path": "page.title",
                "target": "field",
                "name": "content"
            }]
        }))
        .expect("document");
        let materialized = materialize_project_with_runtime_context(&document, &json!({}));
        assert_eq!(
            materialized.effective_context["page"]["title"],
            "Hello world"
        );
        assert_eq!(materialized.defaults_applied, 1);
        assert_eq!(materialized.computed_applied, 1);
        assert_eq!(materialized.applied_bindings, 1);
        assert_eq!(
            materialized
                .document
                .component("title")
                .and_then(|component| component.extensions.get("content"))
                .and_then(Value::as_str),
            Some("Hello world")
        );
    }

    #[test]
    fn invalid_context_contract_does_not_replace_root_context() {
        let document = GrapesJsV1Codec::decode_value(json!({
            "pages": [{
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "title",
                        "type": "text",
                        "content": "Static"
                    }]
                }
            }],
            "flyRuntimeContextSchema": [{
                "id": "invalid-root",
                "path": "",
                "kind": "object",
                "default": { "replaced": true }
            }]
        }))
        .expect("document");
        let input = json!({ "safe": true });
        let materialized = materialize_project_with_runtime_context(&document, &input);
        assert_eq!(materialized.effective_context, input);
        assert_eq!(materialized.defaults_applied, 0);
        assert!(materialized
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "runtime_context_field_path_invalid"));
    }
}
