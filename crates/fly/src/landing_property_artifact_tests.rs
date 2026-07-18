use crate::{
    build_static_landing_artifact, FlyEditor, GrapesJsCodec, LandingPropertySnapshot,
    LandingPropertyValidationReport, LandingReadinessPolicy, LandingSectionKind, ProjectDocument,
    RegistrySet, RenderPolicy,
};
use serde_json::{json, Value};

fn landing_project(kind: LandingSectionKind) -> ProjectDocument {
    let registries = RegistrySet::with_builtins();
    let block = registries
        .blocks
        .get(kind.block_id())
        .expect("built-in landing block");
    let mut section = serde_json::to_value(&block.component).expect("section JSON");
    assign_stable_ids(&mut section, kind.as_str());

    GrapesJsCodec::decode_value(json!({
        "pages": [{
            "id": "home",
            "flyPageMeta": {
                "title": "Home",
                "description": "A stable landing page",
                "slug": "home"
            },
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [section]
            }
        }]
    }))
    .expect("landing project")
}

fn assign_stable_ids(value: &mut Value, prefix: &str) {
    fn walk(value: &mut Value, prefix: &str, sequence: &mut usize) {
        let Some(object) = value.as_object_mut() else {
            return;
        };
        object.insert(
            "id".to_string(),
            Value::String(format!("{prefix}-{sequence}")),
        );
        *sequence += 1;
        if let Some(children) = object
            .get_mut("components")
            .and_then(Value::as_array_mut)
        {
            for child in children {
                walk(child, prefix, sequence);
            }
        }
    }

    walk(value, prefix, &mut 0);
}

fn property(document: &ProjectDocument, id_suffix: &str) -> LandingPropertySnapshot {
    let report = LandingPropertyValidationReport::for_document(document);
    assert!(report.valid, "landing property issues: {:?}", report.issues);
    report.sections[0]
        .properties
        .iter()
        .find(|property| property.schema.id.ends_with(id_suffix))
        .cloned()
        .unwrap_or_else(|| panic!("missing landing property ending with `{id_suffix}`"))
}

#[test]
fn typed_property_edit_rebuilds_a_deterministic_static_artifact() {
    let project = landing_project(LandingSectionKind::Hero);
    let headline = property(&project, ".headline");
    let command = headline
        .command_from_text("Production-ready landing headline")
        .expect("typed headline command");
    let mut editor = FlyEditor::new(project, RegistrySet::with_builtins());
    let registries = RegistrySet::with_builtins();

    let before = build_static_landing_artifact(
        editor.document(),
        &registries,
        LandingReadinessPolicy::default(),
        &RenderPolicy::default(),
    )
    .expect("artifact before edit");
    assert!(before.ready);
    let before = before.artifact.expect("artifact before edit");

    editor.apply(command).expect("apply typed landing edit");

    let first = build_static_landing_artifact(
        editor.document(),
        &registries,
        LandingReadinessPolicy::default(),
        &RenderPolicy::default(),
    )
    .expect("first artifact after edit");
    let second = build_static_landing_artifact(
        editor.document(),
        &registries,
        LandingReadinessPolicy::default(),
        &RenderPolicy::default(),
    )
    .expect("second artifact after edit");

    assert!(first.ready);
    assert!(first.landing_properties.valid);
    let first = first.artifact.expect("first artifact after edit");
    let second = second.artifact.expect("second artifact after edit");

    assert_ne!(before.source_hash, first.source_hash);
    assert_ne!(before.artifact_hash, first.artifact_hash);
    assert_ne!(before.pages[0].content_hash, first.pages[0].content_hash);
    assert_eq!(first.artifact_hash, second.artifact_hash);
    assert_eq!(first.pages[0].content_hash, second.pages[0].content_hash);
    assert!(first.pages[0]
        .html
        .contains("Production-ready landing headline"));
}

#[test]
fn invalid_typed_values_are_rejected_before_editor_dispatch() {
    let contact = landing_project(LandingSectionKind::ContactForm);
    let method = property(&contact, ".form.method");
    let action = property(&contact, ".form.action");

    assert!(method.command_from_text("delete").is_err());
    assert!(action.command_from_text("javascript:alert(1)").is_err());
}
