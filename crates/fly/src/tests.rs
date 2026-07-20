use crate::*;
use proptest::prelude::*;
use serde_json::{json, Value};
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
    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "missing_component_provider"));
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
