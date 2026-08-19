use fly::{
    FLY_RUNTIME_BINDINGS_FIELD, GrapesJsCodec, materialize_runtime,
    validate_binding_definitions,
};
use serde_json::{Value, json};

#[test]
fn binding_validation_reports_runtime_binding_target_missing() {
    let mut document = GrapesJsCodec::decode_value(json!({
        "pages": [{
            "id": "home",
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": []
            }
        }]
    }))
    .expect("document");

    document.project.extensions.insert(
        FLY_RUNTIME_BINDINGS_FIELD.to_string(),
        json!([{
            "id": "missing-target",
            "component_id": "missing-component",
            "path": "page.title",
            "target": "attribute",
            "name": "data-title"
        }]),
    );

    let diagnostics = validate_binding_definitions(&document);
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "runtime_binding_target_missing"
            && diagnostic.message.contains("missing-component")
    }));
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != "runtime_binding_component_missing")
    );
}

#[test]
fn nested_template_markers_are_not_exact_runtime_paths() {
    let document = GrapesJsCodec::decode_value(json!({
        "pages": [{
            "id": "home",
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "card",
                    "type": "section",
                    "content": "{{item.{{key}}}}"
                }]
            }
        }],
        "flyRuntimeRepeaters": [{
            "id": "cards",
            "component_id": "card",
            "path": "items",
            "item_alias": "item",
            "index_alias": "index"
        }]
    }))
    .expect("document");

    let materialized = materialize_runtime(
        &document,
        &json!({
            "items": [{
                "{{key}}": "must-not-resolve-as-an-exact-path"
            }]
        }),
    );

    let repeated = materialized
        .document
        .component("card--cards-0")
        .expect("repeated card");
    let content = repeated
        .extensions
        .get("content")
        .and_then(Value::as_str)
        .expect("content string");

    assert_ne!(content, "must-not-resolve-as-an-exact-path");
}
