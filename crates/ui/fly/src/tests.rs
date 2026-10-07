use crate::*;
use proptest::prelude::*;
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn baseline() -> ProjectDocument {
    GrapesJsCodec::decode_str(include_str!("../fixtures/grapesjs/baseline.json"))
        .expect("baseline fixture must decode")
}

#[test]
fn grapesjs_round_trip_preserves_unknown_fields() {
    let input: Value =
        serde_json::from_str(include_str!("../fixtures/grapesjs/unknown-provider.json"))
            .expect("fixture json");
    let document = GrapesJsCodec::decode_value(input.clone()).expect("decode");
    let output = GrapesJsCodec::encode_value(&document).expect("encode");
    assert_eq!(output, input);
    assert_eq!(
        output["pages"][0]["component"]["components"][0]["futureField"],
        json!({"nested": [1, 2, 3]})
    );
}

#[test]
fn grapesjs_browser_capture_round_trip_is_exact() {
    let input: Value =
        serde_json::from_str(include_str!("../fixtures/grapesjs/browser-current.json"))
            .expect("browser capture json");
    let document = GrapesJsCodec::decode_value(input.clone()).expect("decode browser capture");
    let output = GrapesJsCodec::encode_value(&document).expect("encode browser capture");
    assert_eq!(output, input);
}

#[test]
fn commands_and_history_are_transactional() {
    let mut editor = FlyEditor::new(baseline(), RegistrySet::with_builtins());
    let original_hash = editor.document().hash();
    editor
        .apply(EditorCommand::Insert {
            parent_id: Some("root".to_string()),
            index: 1,
            component: ComponentNode::Object(Box::new(ComponentObject {
                id: Some("new-section".to_string()),
                component_type: Some("section".to_string()),
                ..ComponentObject::default()
            })),
        })
        .expect("insert");
    assert!(editor.document().contains_component("new-section"));
    assert_ne!(editor.document().hash(), original_hash);
    assert_eq!(editor.history().undo_len(), 1);

    editor.undo().expect("undo");
    assert!(!editor.document().contains_component("new-section"));
    assert_eq!(editor.document().hash(), original_hash);

    editor.redo().expect("redo");
    assert!(editor.document().contains_component("new-section"));
}

#[test]
fn validation_preserves_missing_provider_nodes() {
    let document =
        GrapesJsCodec::decode_str(include_str!("../fixtures/grapesjs/unknown-provider.json"))
            .expect("decode");
    let report = validate_project(
        &document,
        &RegistrySet::with_builtins(),
        ValidationLimits::default(),
    );
    assert!(report.is_valid());
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "missing_component_provider")
    );
    assert!(document.contains_component("widget-1"));
}

#[test]
fn clipboard_remaps_internal_references() {
    let document = baseline();
    let mut fragment = ProjectFragment::from_component(&document, "hero").expect("fragment");
    let mut generator = SequentialIdGenerator::new("copy");
    let mapping = fragment.remap_ids(&mut generator);
    assert_eq!(mapping.get("hero"), Some(&"copy-paste-1".to_string()));
    assert_eq!(fragment.components[0].id(), Some("copy-paste-1"));
}

#[test]
fn revision_acknowledgement_detects_conflicts() {
    let document = baseline();
    let mut revision = RevisionState::new(&document);
    let expected = revision.project_hash;
    revision.acknowledge(expected, "rev-1").expect("ack");
    assert!(!revision.dirty);

    revision.project_hash = ProjectHash(expected.0.wrapping_add(1));
    let error = revision
        .acknowledge(expected, "rev-2")
        .expect_err("conflict");
    assert!(matches!(error, FlyError::RevisionConflict { .. }));
}

#[test]
fn stable_id_assignment_avoids_existing_ids() {
    let mut document = baseline();
    let root = document.component_mut("root").expect("root");
    root.children_mut()
        .expect("root children")
        .push(ComponentNode::object("section"));
    root.children_mut()
        .expect("root children")
        .push(ComponentNode::Object(Box::new(ComponentObject {
            id: Some("fly-section-1".to_string()),
            component_type: Some("section".to_string()),
            ..ComponentObject::default()
        })));
    let mut generator = SequentialIdGenerator::default();
    document.ensure_stable_ids(&mut generator);
    let ids = document
        .project
        .pages
        .iter()
        .filter_map(|page| page.component.as_ref())
        .flat_map(|root| {
            let mut ids = Vec::new();
            root.collect_ids(&mut ids);
            ids
        })
        .collect::<BTreeSet<_>>();
    assert!(ids.contains("fly-section-1"));
    assert!(ids.contains("fly-section-2"));
}

#[test]
fn stable_id_assignment_repairs_duplicate_component_ids() {
    let mut document = GrapesJsCodec::decode_value(json!({
        "pages": [{
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [
                    { "id": "duplicate", "type": "section" },
                    { "id": "duplicate", "type": "section" }
                ]
            }
        }]
    }))
    .expect("document");
    let mut generator = SequentialIdGenerator::new("repair");
    document.ensure_stable_ids(&mut generator);
    let mut ids = Vec::new();
    document.project.pages[0]
        .component
        .as_ref()
        .expect("root")
        .collect_ids(&mut ids);
    assert_eq!(ids.iter().filter(|id| *id == "duplicate").count(), 1);
    assert!(ids.iter().any(|id| id == "repair-section-1"));
}

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

proptest! {
    #[test]
    fn opaque_top_level_fields_round_trip(key in "[a-zA-Z][a-zA-Z0-9_]{0,16}", value in any::<i64>()) {
        prop_assume!(!matches!(key.as_str(), "assets" | "styles" | "pages"));
        let mut object = serde_json::Map::new();
        object.insert("assets".to_string(), json!([]));
        object.insert("styles".to_string(), json!([]));
        object.insert("pages".to_string(), json!([]));
        object.insert(key.clone(), json!(value));
        let input = Value::Object(object);
        let document = GrapesJsCodec::decode_value(input.clone()).expect("decode");
        let output = GrapesJsCodec::encode_value(&document).expect("encode");
        prop_assert_eq!(output, input);
    }
}

// ---------------------------------------------------------------------------------------------
// Property-based coverage
//
// The suite previously contained a single property (round-tripping one opaque scalar field),
// which left the invariants that actually matter — escaping, URL policy agreement, id remapping,
// history accounting — covered only by hand-picked examples. Each property below states an
// invariant that example tests can only sample.
// ---------------------------------------------------------------------------------------------

/// Strings that exercise escaping: markup, quotes, CSS terminators, and benign text.
fn hostile_text() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("</style><script>alert(1)</script>".to_string()),
        Just("</script><svg onload=alert(1)>".to_string()),
        Just("\"><img src=x onerror=alert(1)>".to_string()),
        Just("5 < 10 && 3 > 2".to_string()),
        Just("--></style>".to_string()),
        "[ -~]{0,40}".prop_map(|value| value.to_string()),
        "[<>\"'&;{}()\\\\/*]{0,24}".prop_map(|value| value.to_string()),
    ]
}

/// A component subtree of bounded size, with author-controlled ids, text and attributes.
fn component_tree() -> impl Strategy<Value = Value> {
    let leaf = ("[a-z][a-z0-9-]{0,8}", hostile_text(), hostile_text()).prop_map(
        |(id, content, attribute)| {
            json!({
                "id": id,
                "type": "text",
                "content": content,
                "components": [],
                "attributes": { "title": attribute, "data-x": "static" }
            })
        },
    );

    leaf.prop_recursive(4, 24, 3, |inner| {
        ("[a-z][a-z0-9-]{0,8}", prop::collection::vec(inner, 0..3)).prop_map(|(id, children)| {
            json!({
                "id": id,
                "type": "section",
                "components": children,
                "style": { "margin-top": "4px" }
            })
        })
    })
}

/// Build a complete project around a generated subtree.
///
/// `assets` and `styles` are spelled out because `GrapesProject` serializes them unconditionally;
/// omitting them would make the round-trip property fail on a generator artefact rather than on a
/// real defect.
fn project_from(component: Value) -> Value {
    json!({
        "assets": [],
        "styles": [],
        "pages": [{
            "id": "home",
            "component": { "id": "root", "type": "wrapper", "components": [component] }
        }]
    })
}

proptest! {
    /// Rendering must never let authored content escape into markup, whatever it contains.
    ///
    /// Stated as a property because the escaping bug this guards against (`component.id` landing
    /// unescaped in a `<style>` selector) was invisible to every example test in the suite.
    #[test]
    fn rendering_never_emits_executable_markup_from_authored_content(tree in component_tree()) {
        let document = match GrapesJsCodec::decode_value(project_from(tree)) {
            Ok(document) => document,
            Err(_) => return Ok(()),
        };
        let rendered = match render_page(&document, &PageSelection::First, &RenderPolicy::default())
        {
            Ok(rendered) => rendered,
            Err(_) => return Ok(()),
        };
        let html = rendered.document_html();
        let lowered = html.to_ascii_lowercase();

        prop_assert!(!lowered.contains("<script"), "script element in output: {html}");
        prop_assert!(!lowered.contains("<svg"), "raw svg in output: {html}");
        prop_assert!(!lowered.contains("<img"), "raw img in output: {html}");
        // Exactly the one real closing tag; a second means the stylesheet broke out.
        prop_assert_eq!(lowered.matches("</style>").count(), 1, "style breakout: {}", html);
        prop_assert!(!rendered.css.contains('<'), "raw `<` in css: {}", rendered.css);
    }

    /// Decoding then encoding must reproduce the input byte for byte.
    #[test]
    fn codec_round_trips_arbitrary_component_trees(tree in component_tree()) {
        let input = project_from(tree);
        let document = match GrapesJsCodec::decode_value(input.clone()) {
            Ok(document) => document,
            Err(_) => return Ok(()),
        };
        let output = GrapesJsCodec::encode_value(&document).expect("encode");
        prop_assert_eq!(output, input);
    }

    /// Ids accepted by validation must be safe to interpolate into a CSS selector unchanged.
    ///
    /// This ties the two halves of the id defence together: if `validate_identifier` ever admits
    /// a character that `escape_css_attribute` has to escape, the two have drifted apart.
    #[test]
    fn accepted_identifiers_need_no_css_escaping(id in "[A-Za-z0-9_.:-]{1,64}") {
        prop_assert!(validate_identifier(&id).is_ok(), "rejected {id}");
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{ "id": "home", "component": {
                "id": "root", "type": "wrapper",
                "components": [{ "id": id.clone(), "type": "section",
                                 "style": { "margin-top": "1px" } }]
            }}]
        }))
        .expect("decode");
        let rendered = render_page(&document, &PageSelection::First, &RenderPolicy::default())
            .expect("render");
        // The id appears verbatim in the selector: nothing needed escaping.
        prop_assert!(
            rendered.css.contains(&id),
            "accepted id was escaped in css: {} / {}",
            id,
            rendered.css
        );
    }

    /// Anything rejected as an identifier must never reach the stylesheet unescaped.
    #[test]
    fn rejected_identifiers_are_always_escaped(id in "[^\\x00-\\x1f]{1,24}") {
        prop_assume!(validate_identifier(&id).is_err());
        let document = match GrapesJsCodec::decode_value(json!({
            "pages": [{ "id": "home", "component": {
                "id": "root", "type": "wrapper",
                "components": [{ "id": id, "type": "section",
                                 "style": { "margin-top": "1px" } }]
            }}]
        })) {
            Ok(document) => document,
            Err(_) => return Ok(()),
        };
        let rendered = match render_page(&document, &PageSelection::First, &RenderPolicy::default())
        {
            Ok(rendered) => rendered,
            Err(_) => return Ok(()),
        };
        // An id is rejected here only because it contains a character outside the allow-list
        // (the generated length is far below the limit), so it must not survive verbatim.
        prop_assert!(!rendered.css.contains('<'));
        let unescaped_selector = format!("[data-fly-style-id=\"{id}\"]");
        prop_assert!(
            !rendered.css.contains(&unescaped_selector),
            "rejected id reached the stylesheet unescaped: {id} / {}",
            rendered.css
        );
    }

    /// Validation must account for every component, including ones it cannot type.
    ///
    /// The opaque bypass was precisely a violation of this: nodes existed but were not counted.
    #[test]
    fn every_component_is_counted_exactly_once(tree in component_tree()) {
        let input = project_from(tree);
        let expected = count_components(&input["pages"][0]["component"]);
        let document = match GrapesJsCodec::decode_value(input) {
            Ok(document) => document,
            Err(_) => return Ok(()),
        };
        let report = validate_project(
            &document,
            &RegistrySet::with_builtins(),
            ValidationLimits::default(),
        );
        prop_assert_eq!(report.node_count, expected);
    }
}

/// Count component-shaped nodes in raw project JSON, independently of the typed model.
fn count_components(value: &Value) -> usize {
    let mut total = 1;
    if let Some(children) = value.get("components").and_then(Value::as_array) {
        for child in children {
            total += count_components(child);
        }
    }
    total
}
