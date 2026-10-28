use super::*;
use crate::{
    BindingCatalog, BindingCommand, BindingTarget, BindingTransform, ComponentNode,
    ConditionOperator, ContextCommand, ContextFieldDefinition, ContextSchemaCatalog,
    ContextValueKind, DynamicCatalog, DynamicCommand, FlyError, GrapesJsCodec, RegistrySet,
    RuntimeBinding, RuntimeCondition, SnapshotCatalog, TranslationCatalog, TranslationCommand,
    TranslationEntry, FLY_RUNTIME_CONDITIONS_FIELD,
};
use serde_json::{json, Map, Value};
use std::sync::Arc;

fn editor() -> FlyEditor {
    let document = GrapesJsCodec::decode_value(json!({
        "assets": [],
        "styles": [],
        "pages": [{
            "id": "home",
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "hero",
                    "type": "section",
                    "style": { "color": "red", "padding": "24px" }
                }]
            }
        }]
    }))
    .expect("document");
    FlyEditor::new(document, RegistrySet::with_builtins())
}

#[test]
fn style_patch_merges_and_can_remove_individual_properties() {
    let mut editor = editor();
    editor
        .apply(EditorCommand::Patch {
            component_id: "hero".to_string(),
            patch: ComponentPatch::default()
                .merge_style(json!({ "width": "320px" }))
                .remove_style_property("color"),
        })
        .expect("patch");
    let style = editor
        .document()
        .component("hero")
        .and_then(|component| component.style.as_ref())
        .expect("style");
    assert_eq!(style["padding"], "24px");
    assert_eq!(style["width"], "320px");
    assert!(style.get("color").is_none());
}

#[test]
fn dynamic_commands_participate_in_history() {
    let mut editor = editor();
    editor
        .apply(EditorCommand::Dynamic {
            command: DynamicCommand::UpsertCondition {
                condition: RuntimeCondition {
                    id: "show-hero".to_string(),
                    component_id: "hero".to_string(),
                    path: "flags.hero".to_string(),
                    operator: ConditionOperator::Truthy,
                    expected: None,
                    invert: false,
                    extensions: Map::new(),
                },
            },
        })
        .expect("dynamic command");
    assert_eq!(
        DynamicCatalog::from_document(editor.document())
            .conditions
            .len(),
        1
    );
    assert!(editor
        .document()
        .project
        .extensions
        .contains_key(FLY_RUNTIME_CONDITIONS_FIELD));
    editor.undo().expect("undo dynamic command");
    assert!(DynamicCatalog::from_document(editor.document())
        .conditions
        .is_empty());
}

#[test]
fn binding_commands_participate_in_history() {
    let mut editor = editor();
    editor
        .apply(EditorCommand::Binding {
            command: BindingCommand::Upsert {
                binding: Box::new(RuntimeBinding {
                    id: "hero-content".to_string(),
                    component_id: "hero".to_string(),
                    path: "page.hero".to_string(),
                    target: BindingTarget::Field {
                        name: "content".to_string(),
                    },
                    fallback: None,
                    transform: BindingTransform::Identity,
                    extensions: Map::new(),
                }),
            },
        })
        .expect("binding command");
    assert_eq!(
        BindingCatalog::from_document(editor.document())
            .bindings
            .len(),
        1
    );
    editor.undo().expect("undo binding command");
    assert!(BindingCatalog::from_document(editor.document())
        .bindings
        .is_empty());
}

#[test]
fn context_commands_participate_in_history() {
    let mut editor = editor();
    editor
        .apply(EditorCommand::Context {
            command: ContextCommand::UpsertField {
                field: ContextFieldDefinition {
                    id: "title".to_string(),
                    path: "page.title".to_string(),
                    kind: ContextValueKind::String,
                    required: true,
                    default: Some(json!("Untitled")),
                    item_kind: None,
                    extensions: Map::new(),
                },
            },
        })
        .expect("context command");
    assert_eq!(
        ContextSchemaCatalog::from_document(editor.document())
            .fields
            .len(),
        1
    );
    editor.undo().expect("undo context command");
    assert!(ContextSchemaCatalog::from_document(editor.document())
        .fields
        .is_empty());
}

#[test]
fn translation_commands_participate_in_history() {
    let mut editor = editor();
    editor
        .apply(EditorCommand::Translation {
            command: TranslationCommand::Upsert {
                entry: Box::new(TranslationEntry {
                    id: "hero_title".to_string(),
                    values: serde_json::from_value(json!({
                        "en": "Welcome",
                        "ru": "Добро пожаловать"
                    }))
                    .expect("translation values"),
                    fallback_locale: Some("en".to_string()),
                    extensions: Map::new(),
                }),
            },
        })
        .expect("translation command");
    assert_eq!(
        TranslationCatalog::from_document(editor.document())
            .entries
            .len(),
        1
    );
    editor.undo().expect("undo translation command");
    assert!(TranslationCatalog::from_document(editor.document())
        .entries
        .is_empty());
    editor.redo().expect("redo translation command");
    assert_eq!(
        TranslationCatalog::from_document(editor.document())
            .entries
            .len(),
        1
    );
}

#[test]
fn invalid_runtime_definitions_block_transaction() {
    let mut editor = editor();
    let error = editor
        .apply(EditorCommand::Dynamic {
            command: DynamicCommand::UpsertRepeater {
                repeater: crate::RuntimeRepeater {
                    id: "root-repeat".to_string(),
                    component_id: "root".to_string(),
                    path: "items".to_string(),
                    item_alias: "item".to_string(),
                    index_alias: "index".to_string(),
                    limit: None,
                    empty_behavior: crate::EmptyRepeaterBehavior::Hide,
                    extensions: Map::new(),
                },
            },
        })
        .expect_err("root repeater should fail validation");
    assert!(matches!(error, FlyError::Validation(_)));
    assert!(DynamicCatalog::from_document(editor.document())
        .repeaters
        .is_empty());
}

#[test]
fn batch_is_atomic_and_creates_one_history_entry() {
    let mut editor = editor();
    editor
        .apply(EditorCommand::batch([
            EditorCommand::Patch {
                component_id: "hero".to_string(),
                patch: ComponentPatch::default()
                    .set_field("content", Value::String("Updated".to_string())),
            },
            EditorCommand::Asset {
                command: AssetCommand::Upsert {
                    asset: json!({ "id": "hero-image", "src": "/hero.webp" }),
                },
            },
        ]))
        .expect("batch");
    assert_eq!(editor.history().undo_len(), 1);
    assert_eq!(editor.document().project.assets.len(), 1);
    editor.undo().expect("undo batch");
    assert!(editor.document().project.assets.is_empty());
    assert!(editor
        .document()
        .component("hero")
        .and_then(|component| component.extensions.get("content"))
        .is_none());
}

#[test]
fn failed_batch_does_not_change_document_or_history() {
    let mut editor = editor();
    let before = editor.document().hash();
    let error = editor
        .apply(EditorCommand::batch([
            EditorCommand::Asset {
                command: AssetCommand::Upsert {
                    asset: json!({ "id": "hero-image", "src": "/hero.webp" }),
                },
            },
            EditorCommand::Remove {
                component_id: "missing".to_string(),
            },
        ]))
        .expect_err("batch should fail");
    assert!(matches!(error, FlyError::ComponentNotFound(_)));
    assert_eq!(editor.document().hash(), before);
    assert_eq!(editor.history().undo_len(), 0);
}

#[test]
fn snapshot_restore_is_hash_verified_and_participates_in_history() {
    let mut editor = editor();
    let mut snapshots = SnapshotCatalog::default();
    let snapshot = snapshots
        .capture("Initial", editor.document(), Map::new())
        .expect("snapshot")
        .clone();

    editor
        .apply(EditorCommand::Patch {
            component_id: "hero".to_string(),
            patch: ComponentPatch::default().set_field("content", json!("Updated")),
        })
        .expect("update");
    assert_eq!(
        editor.document().component("hero").unwrap().extensions["content"],
        "Updated"
    );

    editor
        .restore_snapshot(&snapshot)
        .expect("restore snapshot");
    assert!(editor
        .document()
        .component("hero")
        .unwrap()
        .extensions
        .get("content")
        .is_none());
    assert_eq!(editor.history().undo_len(), 2);

    editor.undo().expect("undo restore");
    assert_eq!(
        editor.document().component("hero").unwrap().extensions["content"],
        "Updated"
    );
    editor.redo().expect("redo restore");
    assert!(editor
        .document()
        .component("hero")
        .unwrap()
        .extensions
        .get("content")
        .is_none());
}

#[test]
fn tampered_snapshot_does_not_change_document_or_history() {
    let mut editor = editor();
    let mut snapshots = SnapshotCatalog::default();
    let snapshot = snapshots
        .capture("Initial", editor.document(), Map::new())
        .expect("snapshot")
        .clone();
    let before = editor.document().hash();

    // Payload tampering is caught by the content digest, which `restore` checks first.
    let mut payload_tampered = snapshot.clone();
    payload_tampered.project_data["pages"][0]["id"] = json!("tampered");
    let error = editor
        .restore_snapshot(&payload_tampered)
        .expect_err("tampered payload");
    assert!(
        matches!(error, FlyError::SnapshotDigestMismatch { .. }),
        "{error:?}"
    );

    // Tampering with the declared hash alone reaches the second check.
    let mut hash_tampered = snapshot.clone();
    hash_tampered.project_hash = "0".repeat(16);
    let error = editor
        .restore_snapshot(&hash_tampered)
        .expect_err("tampered hash");
    assert!(
        matches!(error, FlyError::SnapshotHashMismatch { .. }),
        "{error:?}"
    );

    // Neither attempt may leave a trace in the document or the history.
    assert_eq!(editor.document().hash(), before);
    assert_eq!(editor.history().undo_len(), 0);
}

#[test]
fn history_evicts_oldest_entries_once_the_entry_limit_is_reached() {
    let mut editor = editor().with_history_limit(2);
    for index in 0..4 {
        editor
            .apply(EditorCommand::Patch {
                component_id: "hero".to_string(),
                patch: ComponentPatch {
                    attributes: Map::from_iter([(
                        "data-step".to_string(),
                        json!(index.to_string()),
                    )]),
                    ..ComponentPatch::default()
                },
            })
            .expect("patch");
    }

    assert_eq!(editor.history().undo_len(), 2);
    assert!(editor.history().retained_bytes() > 0);
}

#[test]
fn history_respects_its_memory_budget_but_always_keeps_one_entry() {
    let mut editor = editor().with_history_memory_budget(1);

    for index in 0..5 {
        editor
            .apply(EditorCommand::Patch {
                component_id: "hero".to_string(),
                patch: ComponentPatch {
                    attributes: Map::from_iter([(
                        "data-step".to_string(),
                        json!(index.to_string()),
                    )]),
                    ..ComponentPatch::default()
                },
            })
            .expect("patch");
    }

    // The budget is deliberately unreachable, but undo must never be emptied by eviction.
    assert_eq!(editor.history().undo_len(), 1);
    assert!(editor.undo().is_ok());
}

#[test]
fn retained_bytes_returns_to_zero_after_undoing_everything() {
    let mut editor = editor();
    editor
        .apply(EditorCommand::Patch {
            component_id: "hero".to_string(),
            patch: ComponentPatch {
                attributes: Map::from_iter([("data-step".to_string(), json!("1"))]),
                ..ComponentPatch::default()
            },
        })
        .expect("patch");

    assert!(editor.history().retained_bytes() > 0);
    editor.undo().expect("undo");
    assert_eq!(editor.history().retained_bytes(), 0);
    assert_eq!(editor.history().undo_len(), 0);
}

#[test]
fn adjacent_history_entries_share_one_document_allocation() {
    // The point of holding `Arc<ProjectDocument>`: the document an entry records as `after` is
    // the very same allocation the next entry records as `before`. If this regresses to
    // independent clones, history memory doubles and every command pays an extra deep copy —
    // silently, because every behavioural test still passes.
    let mut editor = editor();
    editor
        .apply(EditorCommand::Patch {
            component_id: "hero".to_string(),
            patch: ComponentPatch::default().merge_style(json!({ "width": "320px" })),
        })
        .expect("first command");
    editor
        .apply(EditorCommand::Patch {
            component_id: "hero".to_string(),
            patch: ComponentPatch::default().merge_style(json!({ "height": "64px" })),
        })
        .expect("second command");

    let entries = editor.history().undo_entries().collect::<Vec<_>>();
    assert_eq!(entries.len(), 2);
    assert!(
        Arc::ptr_eq(&entries[0].after, &entries[1].before),
        "adjacent entries hold separate copies of the same document"
    );
}

#[test]
fn undo_and_redo_rebind_instead_of_copying() {
    let mut editor = editor();
    editor
        .apply(EditorCommand::Patch {
            component_id: "hero".to_string(),
            patch: ComponentPatch::default().merge_style(json!({ "width": "320px" })),
        })
        .expect("command");

    let entry_before = Arc::clone(
        &editor
            .history()
            .undo_entries()
            .next()
            .expect("entry")
            .before,
    );

    editor.undo().expect("undo");
    assert_eq!(
        editor.document(),
        &*entry_before,
        "undo restored the wrong state"
    );
    assert!(
        editor
            .document()
            .component("hero")
            .and_then(|component| component.style.as_ref())
            .and_then(|style| style.get("width"))
            .is_none(),
        "undo left the applied style behind"
    );

    editor.redo().expect("redo");
    assert_eq!(
        editor
            .document()
            .component("hero")
            .and_then(|component| component.style.as_ref())
            .map(|style| style["width"].clone()),
        Some(json!("320px")),
        "redo did not reapply the command"
    );
}

#[test]
fn the_editor_refuses_a_snapshot_with_no_integrity_digest() {
    // `ProjectSnapshot::restore` tolerates a missing digest so that snapshots captured before
    // digests existed stay readable. The editor must not: `EditorCommand` is `Deserialize`, so
    // tolerating an absent digest here would let the first API that accepts a command downgrade
    // integrity to the forgeable `project_hash` fingerprint.
    let mut editor = editor();
    let mut snapshots = SnapshotCatalog::default();
    let mut snapshot = snapshots
        .capture("Initial", editor.document(), Map::new())
        .expect("snapshot")
        .clone();
    snapshot.content_digest = None;

    let before = editor.document().hash();
    let error = editor
        .restore_snapshot(&snapshot)
        .expect_err("a digestless snapshot must be refused");
    assert!(
        matches!(error, FlyError::SnapshotDigestMissing(_)),
        "{error:?}"
    );
    assert_eq!(editor.document().hash(), before);
    assert_eq!(editor.history().undo_len(), 0);
}

#[test]
fn selection_returns_the_cached_report_without_changing_validity() {
    let mut editor = editor();
    let first = editor
        .apply(EditorCommand::Select {
            component_id: Some("hero".to_string()),
        })
        .expect("select");
    // Patch changes nothing about validity either, but refreshes the cache through the
    // mutating path; a following selection must report the same document state.
    editor
        .apply(EditorCommand::Patch {
            component_id: "hero".to_string(),
            patch: ComponentPatch::default().merge_style(json!({ "width": "10px" })),
        })
        .expect("patch");
    let second = editor
        .apply(EditorCommand::Select {
            component_id: Some("root".to_string()),
        })
        .expect("select after patch");
    assert_eq!(first.errors().count(), second.errors().count());
    assert_eq!(editor.selection(), Some("root"));
    // Undo clears the cache; selection still works and revalidates on demand.
    editor.undo().expect("undo");
    let third = editor
        .apply(EditorCommand::Select {
            component_id: Some("hero".to_string()),
        })
        .expect("select after undo");
    assert_eq!(third.errors().count(), 0);
}

#[test]
fn stable_ids_fast_path_leaves_valid_documents_untouched() {
    let mut editor = editor();
    let before = GrapesJsCodec::encode_value(editor.document()).expect("encode");
    editor
        .apply(EditorCommand::Patch {
            component_id: "hero".to_string(),
            patch: ComponentPatch::default().merge_style(json!({ "width": "10px" })),
        })
        .expect("patch");
    let hero = editor.document().component("hero").expect("hero");
    assert_eq!(hero.id(), Some("hero"));
    let root = editor.document().component("root").expect("root");
    assert_eq!(root.id(), Some("root"));
    assert!(before.is_object());
}

#[test]
fn history_entry_records_exact_inverse_command() {
    let mut editor = editor();
    editor
        .apply(EditorCommand::Patch {
            component_id: "hero".to_string(),
            patch: ComponentPatch::default()
                .merge_style(json!({ "width": "500px" }))
                .set_attribute("data-custom", json!("test-val")),
        })
        .expect("patch applied");

    let entry = editor
        .history()
        .undo_entries()
        .next()
        .expect("history entry");
    let inverse = entry.inverse().expect("inverse command must be recorded");

    match inverse {
        EditorCommand::Patch {
            component_id,
            patch,
        } => {
            assert_eq!(component_id, "hero");
            assert!(patch.remove_attributes.contains(&"data-custom".to_string()));
            if let Some(style) = &patch.style {
                assert!(style.get("width").is_none());
            }
        }
        _ => panic!("expected Patch inverse command"),
    }
}

#[test]
fn invert_insert_and_remove_are_reciprocal() {
    let mut editor = editor();
    let mut new_node = ComponentNode::object("section");
    new_node.as_object_mut().unwrap().id = Some("section-new".to_string());
    let insert_cmd = EditorCommand::Insert {
        parent_id: Some("root".to_string()),
        index: 1,
        component: new_node,
    };
    let inverse_insert = invert_command(editor.document(), &insert_cmd).expect("inverse of insert");
    assert_eq!(
        inverse_insert,
        EditorCommand::Remove {
            component_id: "section-new".to_string()
        }
    );

    editor.apply(insert_cmd).expect("insert");
    let inserted_id = editor.document().component("root").unwrap().children()[1]
        .id()
        .unwrap()
        .to_string();

    let remove_cmd = EditorCommand::Remove {
        component_id: inserted_id.clone(),
    };
    let inverse_remove = invert_command(editor.document(), &remove_cmd).expect("inverse of remove");
    match inverse_remove {
        EditorCommand::Insert {
            parent_id,
            index,
            component,
        } => {
            assert_eq!(parent_id.as_deref(), Some("root"));
            assert_eq!(index, 1);
            assert_eq!(component.id(), Some(inserted_id.as_str()));
        }
        _ => panic!("expected Insert inverse"),
    }
}

#[test]
fn incremental_patch_validation_produces_identical_diagnostics_to_full_validation() {
    let mut editor = editor();
    let incremental_report = editor
        .apply(EditorCommand::Patch {
            component_id: "hero".to_string(),
            patch: ComponentPatch::default().merge_style(json!({ "margin": "16px" })),
        })
        .expect("patch");
    let full_report = editor.validate();

    assert_eq!(incremental_report.is_valid(), full_report.is_valid());
    assert_eq!(incremental_report.node_count, full_report.node_count);
    assert_eq!(incremental_report.diagnostics, full_report.diagnostics);
}
