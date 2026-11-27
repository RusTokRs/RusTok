use super::*;
use crate::{
    BindingCatalog, BindingCommand, BindingTarget, BindingTransform, ComponentNode,
    ConditionOperator, ContextCommand, ContextFieldDefinition, ContextSchemaCatalog,
    ContextValueKind, DynamicCatalog, DynamicCommand, FLY_RUNTIME_CONDITIONS_FIELD, FlyError,
    GrapesJsCodec, RegistrySet, RuntimeBinding, RuntimeCondition, SnapshotCatalog,
    TranslationCatalog, TranslationCommand, TranslationEntry,
};
use serde_json::{Map, Value, json};
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
fn idempotent_and_empty_commands_do_not_consume_history_or_revision() {
    let mut editor = editor();
    let before = editor.clone();
    let report = editor
        .apply(EditorCommand::Batch { commands: vec![] })
        .expect("empty batch is a verified no-op");
    assert!(report.is_valid());
    assert_eq!(editor.document(), before.document());
    assert_eq!(editor.history(), before.history());
    assert_eq!(editor.revision(), before.revision());

    editor
        .apply(EditorCommand::Patch {
            component_id: "hero".to_string(),
            patch: ComponentPatch::default().merge_style(json!({"color": "red"})),
        })
        .expect("same value");
    assert_eq!(editor.document(), before.document());
    assert_eq!(editor.history(), before.history());
    assert_eq!(editor.revision(), before.revision());

    // Once a meaningful edit exists, an idempotent follow-up must not clear redo
    // or create a second history entry.
    editor
        .apply(EditorCommand::Patch {
            component_id: "hero".to_string(),
            patch: ComponentPatch::default().merge_style(json!({"color": "blue"})),
        })
        .expect("edit");
    editor.undo().expect("undo");
    let revision_before = editor.revision().clone();
    let redo_before = editor.history().redo_len();
    editor
        .apply(EditorCommand::Batch { commands: vec![] })
        .expect("no-op");
    assert_eq!(editor.revision(), &revision_before);
    assert_eq!(editor.history().redo_len(), redo_before);
    editor.redo().expect("redo remains available");
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
fn serialized_patch_conflicts_remove_values_and_round_trip_through_history() {
    let mut editor = editor();
    editor
        .apply(EditorCommand::Patch {
            component_id: "hero".to_string(),
            patch: ComponentPatch::default()
                .set_attribute("title", json!("Original"))
                .set_field("content", json!("Original")),
        })
        .expect("initial values");
    let before = editor.document().clone();
    let sequence = editor.revision().command_sequence;
    let patch: ComponentPatch = serde_json::from_value(json!({
        "attributes": { "title": "Replacement", "aria-label": "Keep" },
        "remove_attributes": ["title"],
        "fields": { "content": "Replacement" },
        "remove_fields": ["content"],
        "style": { "color": "blue" },
        "remove_style_properties": ["color"]
    }))
    .expect("deserialize patch");
    editor
        .apply(EditorCommand::Patch {
            component_id: "hero".to_string(),
            patch,
        })
        .expect("apply");
    let after = editor.document().clone();
    let component = after.component("hero").expect("component");
    assert!(!component.attributes.contains_key("title"));
    assert_eq!(component.attributes["aria-label"], "Keep");
    assert!(!component.extensions.contains_key("content"));
    assert!(component.style.as_ref().unwrap().get("color").is_none());
    assert_eq!(editor.revision().command_sequence, sequence + 1);

    let inverse = editor
        .history()
        .undo_entries()
        .last()
        .and_then(|entry| entry.inverse())
        .expect("local inverse")
        .clone();
    let mut replay = FlyEditor::new(after.clone(), editor.registries().clone());
    replay.apply(inverse).expect("apply inverse");
    assert_eq!(replay.document(), &before);

    editor.undo().expect("undo");
    assert_eq!(editor.document(), &before);
    editor.redo().expect("redo");
    assert_eq!(editor.document(), &after);
}

#[test]
fn composed_style_replacement_is_atomic_and_reversible() {
    let mut editor = editor();
    let before = editor.document().clone();
    let initial_sequence = editor.revision().command_sequence;
    editor
        .apply(EditorCommand::Patch {
            component_id: "hero".to_string(),
            patch: ComponentPatch::default()
                .merge_style(json!("color: blue"))
                .merge_style(json!({"width": "320px"})),
        })
        .expect("composed style replacement");
    let after = editor.document().clone();
    assert_eq!(
        after.component("hero").unwrap().style,
        Some(json!({"width": "320px"}))
    );
    assert_eq!(editor.history().undo_len(), 1);
    assert_eq!(editor.revision().command_sequence, initial_sequence + 1);
    editor.undo().expect("undo style replacement");
    assert_eq!(editor.document(), &before);
    editor.redo().expect("redo style replacement");
    assert_eq!(editor.document(), &after);
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
    assert!(
        editor
            .document()
            .project()
            .extensions
            .contains_key(FLY_RUNTIME_CONDITIONS_FIELD)
    );
    editor.undo().expect("undo dynamic command");
    assert!(
        DynamicCatalog::from_document(editor.document())
            .conditions
            .is_empty()
    );
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
    assert!(
        BindingCatalog::from_document(editor.document())
            .bindings
            .is_empty()
    );
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
    assert!(
        ContextSchemaCatalog::from_document(editor.document())
            .fields
            .is_empty()
    );
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
    assert!(
        TranslationCatalog::from_document(editor.document())
            .entries
            .is_empty()
    );
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
    assert!(
        DynamicCatalog::from_document(editor.document())
            .repeaters
            .is_empty()
    );
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
    assert_eq!(editor.document().project().assets.len(), 1);
    editor.undo().expect("undo batch");
    assert!(editor.document().project().assets.is_empty());
    assert!(
        editor
            .document()
            .component("hero")
            .and_then(|component| component.extensions.get("content"))
            .is_none()
    );
}

#[test]
fn dependent_batch_uses_snapshot_history_not_a_false_local_inverse() {
    let mut editor = editor();
    let before = editor.document().clone();
    let batch = EditorCommand::batch([
        EditorCommand::Asset {
            command: AssetCommand::Upsert {
                asset: json!({"id": "temporary", "src": "/temporary.png"}),
            },
        },
        EditorCommand::Asset {
            command: AssetCommand::Remove {
                asset_id: "temporary".to_string(),
            },
        },
        EditorCommand::Patch {
            component_id: "hero".to_string(),
            patch: ComponentPatch::default().set_attribute("title", json!("changed")),
        },
    ]);
    assert!(invert_command(&before, &batch).is_none());
    editor.apply(batch).expect("transaction");
    let after = editor.document().clone();
    assert!(
        editor
            .history()
            .undo_entries()
            .last()
            .expect("entry")
            .inverse()
            .is_none()
    );
    editor.undo().expect("undo");
    assert_eq!(editor.document(), &before);
    editor.redo().expect("redo");
    assert_eq!(editor.document(), &after);
}

#[test]
fn style_inverse_restores_absent_and_replaced_style_slots() {
    for original in [None, Some(json!({"color": "red", "padding": "24px"}))] {
        let mut document = editor().document().clone();
        document.component_mut("hero").expect("hero").style = original.clone();
        let mut editor = FlyEditor::new(document, RegistrySet::with_builtins());
        let before = editor.document().clone();
        let command = EditorCommand::Patch {
            component_id: "hero".to_string(),
            patch: ComponentPatch::default().merge_style(json!({"width": "20px"})),
        };
        let inverse = invert_command(&before, &command).expect("style inverse");
        editor.apply(command).expect("apply");
        editor.apply(inverse).expect("restore original style");
        assert_eq!(editor.document(), &before);
    }
}

#[test]
fn asset_removal_only_has_local_inverse_when_it_preserves_order() {
    let mut document = editor().document().clone();
    document.project_mut().assets = vec![
        json!({ "id": "first", "src": "/first.png" }),
        json!({ "id": "last", "src": "/last.png" }),
    ];
    let mut editor = FlyEditor::new(document, RegistrySet::with_builtins());
    let before = editor.document().clone();
    let remove = |id: &str| EditorCommand::Asset {
        command: AssetCommand::Remove {
            asset_id: id.to_string(),
        },
    };
    assert!(invert_command(&before, &remove("first")).is_none());
    editor.apply(remove("first")).expect("remove first");
    assert!(
        editor
            .history()
            .undo_entries()
            .last()
            .unwrap()
            .inverse()
            .is_none()
    );
    editor.undo().expect("restore exact order");
    assert_eq!(editor.document(), &before);
    let inverse = invert_command(&before, &remove("last")).expect("tail inverse");
    editor.apply(remove("last")).expect("remove last");
    editor.apply(inverse).expect("append last");
    assert_eq!(editor.document(), &before);
}

#[test]
fn theme_inverse_does_not_claim_to_restore_normalized_or_rewritten_data() {
    let mut document = editor().document().clone();
    document.project_mut().extensions.insert(
        crate::FLY_THEME_FIELD.to_string(),
        json!({ "tokens": { "brand": " red " }, "future": true }),
    );
    let update = EditorCommand::Theme {
        command: crate::ThemeCommand::Upsert {
            token: crate::DesignToken {
                name: "brand".into(),
                value: "blue".into(),
            },
        },
    };
    assert!(invert_command(&document, &update).is_none());

    document.project_mut().extensions.insert(
        crate::FLY_THEME_FIELD.to_string(),
        json!({ "tokens": { "brand": "red" } }),
    );
    let rename = EditorCommand::Theme {
        command: crate::ThemeCommand::Rename {
            old_name: "brand".into(),
            new_name: "primary".into(),
        },
    };
    assert!(invert_command(&document, &rename).is_none());
    assert!(invert_command(&document, &update).is_some());
    let mut editor = FlyEditor::new(document, RegistrySet::with_builtins());
    let before = editor.document().clone();
    editor.apply(rename).expect("rename");
    editor.undo().expect("snapshot undo");
    assert_eq!(editor.document(), &before);
}

#[test]
fn content_patch_that_removes_text_children_has_no_local_inverse() {
    let document = GrapesJsCodec::decode_value(json!({
        "pages": [{"id": "home", "component": {"id": "root", "type": "wrapper",
            "components": [{"id": "hero", "type": "text", "components": ["Original"]}]
        }}]
    }))
    .expect("document");
    let command = EditorCommand::Patch {
        component_id: "hero".to_string(),
        patch: ComponentPatch::default().set_field("content", json!("Updated")),
    };
    assert!(invert_command(&document, &command).is_none());
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
    assert!(
        editor
            .document()
            .component("hero")
            .unwrap()
            .extensions
            .get("content")
            .is_none()
    );
    assert_eq!(editor.history().undo_len(), 2);

    editor.undo().expect("undo restore");
    assert_eq!(
        editor.document().component("hero").unwrap().extensions["content"],
        "Updated"
    );
    editor.redo().expect("redo restore");
    assert!(
        editor
            .document()
            .component("hero")
            .unwrap()
            .extensions
            .get("content")
            .is_none()
    );
}

fn deep_snapshot_value() -> Value {
    let mut value = Value::Null;
    for _ in 0..50_000 {
        value = Value::Array(vec![value]);
    }
    value
}

#[test]
fn borrowed_snapshot_restore_checks_depth_before_cloning() {
    let mut editor = editor();
    let before = editor.clone();
    let mut snapshots = SnapshotCatalog::default();
    let mut snapshot = snapshots
        .capture("Initial", editor.document(), Map::new())
        .expect("snapshot")
        .clone();

    snapshot
        .metadata
        .insert("deep".into(), deep_snapshot_value());
    let result = editor.restore_snapshot(&snapshot);
    crate::drop_value_iteratively(Value::Object(std::mem::take(&mut snapshot.metadata)));
    assert!(matches!(result, Err(FlyError::MaximumDepthExceeded { .. })));
    snapshot.project_data = deep_snapshot_value();
    let result = editor.restore_snapshot(&snapshot);
    crate::drop_value_iteratively(std::mem::take(&mut snapshot.project_data));
    assert!(matches!(result, Err(FlyError::MaximumDepthExceeded { .. })));
    assert_eq!(editor, before);
}

#[test]
fn owned_batch_rejects_deep_snapshot_payloads_even_after_an_earlier_invalid_command() {
    let mut editor = editor();
    let before = editor.clone();
    let mut snapshots = SnapshotCatalog::default();
    let snapshot = snapshots
        .capture("Initial", editor.document(), Map::new())
        .expect("snapshot")
        .clone();
    let mut deep_metadata = snapshot.clone();
    deep_metadata
        .metadata
        .insert("deep".into(), deep_snapshot_value());
    let mut deep_payload = snapshot;
    deep_payload.project_data = deep_snapshot_value();

    let result = editor.apply(EditorCommand::batch([
        EditorCommand::Remove {
            component_id: "missing".into(),
        },
        EditorCommand::batch([
            EditorCommand::restore_snapshot(deep_metadata),
            EditorCommand::restore_snapshot(deep_payload),
        ]),
    ]));
    assert!(matches!(result, Err(FlyError::MaximumDepthExceeded { .. })));
    assert_eq!(editor, before);
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
fn changing_history_limit_preserves_budget_and_nearest_transitions() {
    let mut editor = editor().with_history_memory_budget(1_000_000);
    for step in 1..=3 {
        editor
            .apply(EditorCommand::Patch {
                component_id: "hero".to_string(),
                patch: ComponentPatch::default().set_attribute("data-step", json!(step)),
            })
            .expect("edit");
    }
    editor.undo().expect("undo third edit");
    let before = editor.document().clone();
    let revision = editor.revision().clone();
    let mut editor = editor.with_history_limit(2);
    assert_eq!(editor.history().memory_budget_bytes(), 1_000_000);
    assert_eq!(editor.history().undo_len(), 1);
    assert_eq!(editor.history().redo_len(), 1);
    assert_eq!(editor.document(), &before);
    assert_eq!(editor.revision(), &revision);
    editor.undo().expect("nearest undo");
    assert_eq!(
        editor.document().component("hero").unwrap().attributes["data-step"],
        1
    );
    editor.redo().expect("nearest redo");
    assert_eq!(editor.document(), &before);
    editor.redo().expect("third edit remains");
    assert_eq!(
        editor.document().component("hero").unwrap().attributes["data-step"],
        3
    );
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
fn retained_bytes_include_redo_after_undoing_everything() {
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

    let retained = editor.history().retained_bytes();
    assert!(retained > 0);
    editor.undo().expect("undo");
    assert_eq!(editor.history().retained_bytes(), retained);
    assert_eq!(editor.history().undo_len(), 0);
    assert_eq!(editor.history().redo_len(), 1);
    editor.redo().expect("redo");
    assert_eq!(editor.history().retained_bytes(), retained);
}

#[test]
fn history_budget_counts_snapshots_even_for_a_small_invertible_patch() {
    let mut document = editor().document().clone();
    document
        .project_mut()
        .extensions
        .insert("large".to_string(), json!("x".repeat(50_000)));
    let serialized_size = serde_json::to_vec(&document).expect("document json").len();
    let mut editor = FlyEditor::new(document, RegistrySet::with_builtins())
        .with_history_memory_budget(serialized_size * 5);
    for index in 0..5 {
        editor
            .apply(EditorCommand::Patch {
                component_id: "hero".to_string(),
                patch: ComponentPatch {
                    attributes: Map::from_iter([("data-step".to_string(), json!(index))]),
                    ..ComponentPatch::default()
                },
            })
            .expect("patch");
    }
    // Five tiny deltas would fit, but five before/after document pairs cannot.
    assert_eq!(editor.history().undo_len(), 2);
    assert!(editor.history().retained_bytes() >= serialized_size * 4);
    assert!(editor.history().retained_bytes() <= editor.history().memory_budget_bytes());
    assert!(
        editor
            .history()
            .undo_entries()
            .all(|entry| entry.inverse().is_some())
    );
    let retained = editor.history().retained_bytes();
    editor.undo().expect("undo");
    editor.undo().expect("undo");
    assert_eq!(editor.history().retained_bytes(), retained);
    assert_eq!(editor.history().redo_len(), 2);
    editor.redo().expect("redo");
    assert_eq!(editor.history().retained_bytes(), retained);
}

#[test]
fn loading_legacy_history_recalculates_untrusted_sizes_and_keeps_a_contiguous_redo() {
    let mut editor = editor();
    for index in 0..4 {
        editor
            .apply(EditorCommand::Patch {
                component_id: "hero".to_string(),
                patch: ComponentPatch {
                    attributes: Map::from_iter([("data-step".to_string(), json!(index))]),
                    ..ComponentPatch::default()
                },
            })
            .expect("patch");
    }
    for _ in 0..4 {
        editor.undo().expect("undo");
    }
    let mut stored = serde_json::to_value(&editor).expect("serialize editor");
    stored["history"]["retained_bytes"] = json!(1);
    stored["history"]["undo_sizes"] = json!([]);
    stored["history"]["redo_sizes"] = json!([1, 1, 1, 1]);
    stored["history"]["memory_budget_bytes"] = json!(1);
    let mut restored: FlyEditor = serde_json::from_value(stored).expect("restore editor");
    // Always preserve the nearest available entry, even above a tiny budget.
    assert_eq!(restored.history().undo_len(), 0);
    assert_eq!(restored.history().redo_len(), 1);
    assert!(restored.history().retained_bytes() > 1);
    restored.redo().expect("nearest redo survives");
    assert_eq!(
        restored.document().component("hero").unwrap().attributes["data-step"],
        json!(0)
    );
    assert!(!restored.history().can_redo());
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
fn move_to_implicit_root_uses_the_same_insertion_slots_as_explicit_root() {
    let document = GrapesJsCodec::decode_value(json!({
        "pages": [{"id": "home", "component": {"id": "root", "type": "wrapper",
            "components": [
                {"id": "a", "type": "section"},
                {"id": "b", "type": "section"},
                {"id": "c", "type": "section"}
            ]
        }}]
    }))
    .expect("document");
    for parent in [None, Some("root".to_string())] {
        let mut editor = FlyEditor::new(document.clone(), RegistrySet::with_builtins());
        editor
            .apply(EditorCommand::Move {
                component_id: "a".to_string(),
                new_parent_id: parent,
                index: 3,
            })
            .expect("move to end");
        assert_eq!(
            editor.document().component_location("a").expect("a").index,
            2
        );
        assert_eq!(
            editor.document().component_location("b").expect("b").index,
            0
        );
    }
}

#[test]
fn move_inverse_restores_same_parent_order_in_both_directions() {
    let document = GrapesJsCodec::decode_value(json!({
        "pages": [{"id": "home", "component": {"id": "root", "type": "wrapper",
            "components": [
                {"id": "a", "type": "section"},
                {"id": "b", "type": "section"},
                {"id": "c", "type": "section"}
            ]
        }}]
    }))
    .expect("document");
    for (id, index) in [("a", 3), ("c", 0), ("b", 1), ("b", 2)] {
        for parent in [None, Some("root".to_string())] {
            let mut editor = FlyEditor::new(document.clone(), RegistrySet::with_builtins());
            let before = editor.document().clone();
            let command = EditorCommand::Move {
                component_id: id.to_string(),
                new_parent_id: parent,
                index,
            };
            let inverse = invert_command(&before, &command).expect("move inverse");
            editor.apply(command).expect("forward");
            editor.apply(inverse).expect("inverse");
            assert_eq!(editor.document(), &before, "move {id} to slot {index}");
        }
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
fn cached_patch_validation_produces_identical_diagnostics_to_full_validation() {
    let mut editor = editor();
    editor
        .apply(EditorCommand::Select {
            component_id: Some("hero".to_string()),
        })
        .expect("warm validation cache");
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

#[test]
fn edit_after_failed_save_keeps_the_failure_signal() {
    let mut editor = editor();
    let patch = |margin: &str| EditorCommand::Patch {
        component_id: "hero".to_string(),
        patch: ComponentPatch::default().merge_style(json!({ "margin": margin })),
    };
    editor.apply(patch("8px")).expect("first edit");
    editor.revision_mut().begin_save().expect("start save");
    editor.revision_mut().fail_save();
    assert!(editor.revision().save_failed);

    // The rejected changes are still not on the server; another edit, an undo and a redo
    // must not make the failure look resolved.
    editor.apply(patch("16px")).expect("edit after failed save");
    assert!(editor.revision().dirty);
    assert!(editor.revision().save_failed);
    editor.undo().expect("undo");
    assert!(editor.revision().save_failed);
    editor.redo().expect("redo");
    assert!(editor.revision().save_failed);

    // Only a new save attempt or a successful acknowledge clears it.
    editor.revision_mut().begin_save().expect("retry save");
    assert!(!editor.revision().save_failed);
    editor.revision_mut().fail_save();
    let hash = editor.revision().project_hash;
    editor.revision_mut().begin_save().expect("last retry");
    editor
        .revision_mut()
        .acknowledge(hash, "rev-2")
        .expect("acknowledge");
    assert!(!editor.revision().save_failed);
    assert!(!editor.revision().dirty);
}

#[test]
fn restoring_editor_does_not_resurrect_orphaned_save_requests() {
    let mut editor = editor();
    editor.revision_mut().begin_save().expect("begin");
    let restored: FlyEditor =
        serde_json::from_value(serde_json::to_value(&editor).expect("serialize"))
            .expect("restore editor");
    assert!(!restored.revision().save_in_progress);
    assert!(!restored.revision().dirty);
    let mut restored = restored;
    restored
        .revision_mut()
        .begin_save()
        .expect("new process can save");
}

#[test]
fn an_identical_retry_has_a_new_attempt_identity() {
    let mut editor = editor();
    let same_hash = editor.revision().project_hash;
    let first = editor
        .revision_mut()
        .begin_save_with_id()
        .expect("first save");
    editor.revision_mut().fail_save();
    let second = editor.revision_mut().begin_save_with_id().expect("retry");
    assert_ne!(first, second);
    assert!(matches!(
        editor
            .revision_mut()
            .acknowledge_submitted_attempt(same_hash, first, "stale"),
        Err(FlyError::InvalidSaveState(_))
    ));
    assert_eq!(editor.revision().active_save_attempt_id(), Some(second));
    assert_eq!(editor.revision().last_acknowledged_revision, None);
    editor
        .revision_mut()
        .acknowledge_submitted_attempt(same_hash, second, "rev-new")
        .expect("current attempt");
    assert_eq!(
        editor.revision().last_acknowledged_revision.as_deref(),
        Some("rev-new")
    );
}

#[test]
fn empty_revision_cannot_acknowledge_an_active_save() {
    let mut editor = editor();
    let hash = editor.revision().project_hash;
    let attempt = editor.revision_mut().begin_save_with_id().expect("start");
    let before = editor.revision().clone();
    for revision in ["", " \t"] {
        assert!(
            editor
                .revision_mut()
                .acknowledge_submitted_attempt(hash, attempt, revision)
                .is_err()
        );
        assert_eq!(editor.revision(), &before);
    }
    editor
        .revision_mut()
        .acknowledge_submitted_attempt(hash, attempt, "rev-2")
        .expect("valid acknowledgement");
    assert_eq!(
        editor.revision().last_acknowledged_revision.as_deref(),
        Some("rev-2")
    );
}

#[test]
fn successful_save_of_an_earlier_snapshot_retains_newer_local_edits() {
    let mut editor = editor();
    editor
        .apply(EditorCommand::Patch {
            component_id: "hero".to_string(),
            patch: ComponentPatch::default().merge_style(json!({"color": "blue"})),
        })
        .expect("first edit");
    let submitted_hash = editor.revision().project_hash;
    editor.revision_mut().begin_save().expect("begin save");
    assert!(editor.revision_mut().begin_save().is_err());
    editor
        .apply(EditorCommand::Patch {
            component_id: "hero".to_string(),
            patch: ComponentPatch::default().merge_style(json!({"padding": "7px"})),
        })
        .expect("new edit");
    let working_hash = editor.revision().project_hash;
    assert_ne!(submitted_hash, working_hash);
    editor
        .revision_mut()
        .acknowledge_submitted(submitted_hash, "rev-2")
        .expect("old snapshot persisted successfully");
    assert_eq!(
        editor.revision().last_acknowledged_revision.as_deref(),
        Some("rev-2")
    );
    assert!(editor.revision().dirty);
    assert_eq!(editor.revision().project_hash, working_hash);
    assert!(!editor.revision().save_in_progress);
    assert!(matches!(
        editor
            .revision_mut()
            .acknowledge_submitted(submitted_hash, "rev-3"),
        Err(FlyError::InvalidSaveState(_))
    ));
    editor.revision_mut().begin_save().expect("save latest");
    assert!(matches!(
        editor
            .revision_mut()
            .acknowledge_submitted(submitted_hash, "rev-3"),
        Err(FlyError::InvalidSaveState(_))
    ));
    editor
        .revision_mut()
        .acknowledge_submitted(working_hash, "rev-3")
        .expect("save latest");
    assert!(!editor.revision().dirty);
}

#[test]
fn edit_after_save_conflict_keeps_the_conflict_and_failure_signals() {
    let mut editor = editor();
    editor.revision_mut().mark_save_conflict();
    editor
        .apply(EditorCommand::Patch {
            component_id: "hero".to_string(),
            patch: ComponentPatch::default().merge_style(json!({ "margin": "4px" })),
        })
        .expect("edit after conflict");
    assert!(editor.revision().save_conflict);
    assert!(editor.revision().save_failed);
}

#[test]
fn repeated_builtin_interactive_blocks_have_disjoint_html_ids_and_local_actions() {
    for block_id in ["tabs", "accordion", "slider"] {
        let mut editor = editor();
        let template = editor
            .registries()
            .blocks
            .get(block_id)
            .expect("built-in")
            .component
            .clone();
        for index in 1..=2 {
            editor
                .apply(EditorCommand::Insert {
                    parent_id: Some("root".to_string()),
                    index,
                    component: template.clone(),
                })
                .expect("insert");
        }
        let root = editor.document().component("root").expect("root");
        let mut sets = Vec::new();
        for block in &root.children()[1..=2] {
            let mut ids = Vec::new();
            block.collect_dom_ids(&mut ids);
            let ids = ids.into_iter().collect::<std::collections::BTreeSet<_>>();
            block.visit(0, "block", &mut |node, _, _| {
                let Some(action) = node.extensions.get(crate::FLY_ACTION_FIELD) else {
                    return;
                };
                match action["kind"].as_str() {
                    Some("switch_tab") => {
                        assert!(ids.contains(action["panel_id"].as_str().unwrap()));
                        assert_eq!(node.attributes["data-fly-tab-target"], action["panel_id"]);
                    }
                    Some("toggle_accordion") => {
                        assert!(ids.contains(action["target_id"].as_str().unwrap()))
                    }
                    Some("switch_slide") => assert_eq!(action["slider_id"], "self"),
                    _ => {}
                }
            });
            sets.push(ids);
        }
        assert!(!sets[0].is_empty(), "{block_id}");
        assert!(sets[0].is_disjoint(&sets[1]), "{block_id}: {sets:?}");
        let snapshot = editor.document().clone();
        editor.undo().expect("undo insertion");
        editor.redo().expect("redo insertion");
        assert_eq!(editor.document(), &snapshot);
    }
}

#[test]
fn direct_insert_reidentifies_a_colliding_subtree_and_its_local_wiring() {
    let mut editor = editor();
    let node: ComponentNode = serde_json::from_value(json!({
        "id": "group", "type": "container", "components": [
            {"id": "trigger", "type": "button", "attributes": {"aria-controls": "panel-dom", "placeholder": "panel"},
             "flyAction": {"kind": "toggle_visibility", "target_id": "panel"}},
            {"id": "panel", "type": "section", "attributes": {"id": "panel-dom", "href": "#panel-dom"}, "content": "panel"},
            {"id": "external", "type": "button", "flyAction": {"kind": "scroll_to", "target_id": "hero"}}
        ]
    })).expect("node");
    for index in 1..=2 {
        editor
            .apply(EditorCommand::Insert {
                parent_id: Some("root".to_string()),
                index,
                component: node.clone(),
            })
            .expect("insert");
    }
    let root = editor.document().component("root").unwrap();
    let first = root.children()[1].as_object().unwrap();
    let second = root.children()[2].as_object().unwrap();
    let second_id = second.id.as_ref().unwrap().clone();
    assert_eq!(first.id.as_deref(), Some("group"));
    assert_ne!(second.id.as_deref(), Some("group"));
    let trigger = second.children()[0].as_object().unwrap();
    let panel = second.children()[1].as_object().unwrap();
    let dom_id = panel.attributes["id"].as_str().unwrap();
    assert_ne!(dom_id, "panel-dom");
    assert_eq!(trigger.attributes["aria-controls"], dom_id);
    assert_eq!(panel.attributes["href"], format!("#{dom_id}"));
    assert_eq!(
        trigger.extensions["flyAction"]["target_id"],
        panel.id.as_deref().unwrap()
    );
    assert_eq!(trigger.attributes["placeholder"], "panel");
    assert_eq!(panel.extensions["content"], "panel");
    assert_eq!(
        second.children()[2].as_object().unwrap().extensions["flyAction"]["target_id"],
        "hero"
    );
    let entry = editor.history().undo_entries().last().unwrap();
    assert_eq!(
        entry.inverse(),
        Some(&EditorCommand::Remove {
            component_id: second_id.clone()
        })
    );
    editor.undo().expect("undo");
    assert!(!editor.document().contains_component(&second_id));
    editor.redo().expect("redo");
    assert!(editor.document().contains_component(&second_id));
}

#[test]
fn ambiguous_component_collision_inside_batch_is_rejected_atomically() {
    let mut editor = editor();
    let original_editor = editor.clone();
    let original = editor.document().clone();
    let duplicate = ComponentNode::Object(Box::new(original.component("hero").unwrap().clone()));
    let result = editor.apply(EditorCommand::batch([
        EditorCommand::Insert {
            parent_id: Some("root".to_string()),
            index: 1,
            component: duplicate,
        },
        EditorCommand::Patch {
            component_id: "hero".to_string(),
            patch: ComponentPatch::default(),
        },
    ]));
    assert!(result.is_err());
    assert_eq!(editor.document(), &original);
    assert_eq!(editor, original_editor);
    assert_eq!(editor.history().undo_len(), 0);
}

#[test]
fn batch_does_not_silently_rewire_a_colliding_html_id() {
    let mut editor = editor();
    let before = editor.clone();
    let first: ComponentNode =
        serde_json::from_value(json!({"id": "a", "type": "text", "attributes": {"id": "shared"}}))
            .unwrap();
    let second: ComponentNode =
        serde_json::from_value(json!({"id": "b", "type": "text", "attributes": {"id": "shared"}}))
            .unwrap();
    let error = editor
        .apply(EditorCommand::batch([
            EditorCommand::Insert {
                parent_id: Some("root".to_string()),
                index: 1,
                component: first,
            },
            EditorCommand::Insert {
                parent_id: Some("root".to_string()),
                index: 2,
                component: second,
            },
        ]))
        .expect_err("ambiguous batch");
    assert!(error.to_string().contains("HTML id `shared`"));
    assert_eq!(editor, before);
}

#[test]
fn inserted_anonymous_root_is_recorded_with_its_undo_identity() {
    let mut editor = editor();
    editor
        .apply(EditorCommand::Insert {
            parent_id: Some("root".to_string()),
            index: 1,
            component: ComponentNode::object("section"),
        })
        .expect("insert");
    let inserted_id = editor.document().component("root").unwrap().children()[1]
        .id()
        .unwrap()
        .to_string();
    assert_eq!(
        editor.history().undo_entries().last().unwrap().inverse(),
        Some(&EditorCommand::Remove {
            component_id: inserted_id.clone()
        })
    );
    editor.undo().expect("undo");
    assert!(!editor.document().contains_component(&inserted_id));
    editor.redo().expect("redo");
    assert!(editor.document().contains_component(&inserted_id));
}

#[test]
fn form_and_another_nodes_html_identity_are_rejected_atomically() {
    let mut editor = editor();
    let before = editor.clone();
    let node: ComponentNode = serde_json::from_value(json!({
        "id": "group", "type": "container", "components": [
            {"id": "form-node", "type": "form", "flyForm": {"id": "contact", "provider": "forms", "action": "submit"}},
            {"id": "other", "type": "section", "attributes": {"id": "contact"}}
        ]
    })).unwrap();
    let error = editor
        .apply(EditorCommand::Insert {
            parent_id: Some("root".into()),
            index: 1,
            component: node,
        })
        .expect_err("a form and an unrelated HTML element cannot share an id");
    assert!(error.to_string().contains("contact"), "{error}");
    assert_eq!(editor, before);

    // An authored HTML id on the form itself is not a second owner.
    let form: ComponentNode = serde_json::from_value(json!({
        "id": "form-node", "type": "form", "attributes": {"id": "contact"},
        "flyForm": {"id": "contact", "provider": "forms", "action": "submit"}
    }))
    .unwrap();
    editor
        .apply(EditorCommand::Insert {
            parent_id: Some("root".into()),
            index: 1,
            component: form,
        })
        .expect("form owns both identity slots");
}

#[test]
fn form_insert_distinguishes_component_ids_from_html_and_form_ids() {
    let document = GrapesJsCodec::decode_value(json!({
        "pages": [{"component": {"id": "root", "type": "wrapper", "components": [
            {"id": "contact", "type": "section"}
        ]}}]
    }))
    .unwrap();
    let mut editor = FlyEditor::new(document, RegistrySet::with_builtins());
    let form: ComponentNode = serde_json::from_value(json!({
        "id": "incoming", "type": "form", "flyForm": {"id": "contact", "action_url": "/contact"}
    }))
    .unwrap();
    editor
        .apply(EditorCommand::Insert {
            parent_id: Some("root".into()),
            index: 1,
            component: form,
        })
        .expect("a different component id is not a DOM/form collision");
    let result = crate::materialize_project_with_runtime_context(editor.document(), &json!({}));
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.severity != crate::ValidationSeverity::Error),
        "{:?}",
        result.diagnostics
    );
    let html = crate::render_page(
        &result.document,
        &crate::PageSelection::First,
        &crate::RenderPolicy::default(),
    )
    .unwrap()
    .html;
    assert_eq!(html.matches(" id=\"contact\"").count(), 1, "{html}");
    editor.undo().expect("undo");
    editor.redo().expect("redo");
    assert!(editor.document().contains_component("incoming"));
}

#[test]
fn form_insert_checks_dom_collisions_on_the_destination_page() {
    let make_document = |same_page: bool| {
        GrapesJsCodec::decode_value(json!({
            "pages": [
                {"component": {"id": "first-root", "type": "wrapper", "components": if same_page {
                    json!([{"id": "own", "type": "section", "attributes": {"id": "contact"}}])
                } else { json!([]) }}},
                {"component": {"id": "second-root", "type": "wrapper", "components": [
                    {"id": "other", "type": "section", "attributes": {"id": "contact"}}
                ]}}
            ]
        }))
        .unwrap()
    };
    let form: ComponentNode = serde_json::from_value(json!({
        "id": "incoming", "type": "form", "flyForm": {"id": "contact", "action_url": "/contact"}
    }))
    .unwrap();
    let insert = |form: ComponentNode| EditorCommand::Insert {
        parent_id: Some("first-root".into()),
        index: 0,
        component: form,
    };
    let mut separate = FlyEditor::new(make_document(false), RegistrySet::with_builtins());
    separate
        .apply(insert(form.clone()))
        .expect("HTML ids on separate pages are independent");
    let mut same = FlyEditor::new(make_document(true), RegistrySet::with_builtins());
    let before = same.clone();
    let error = same
        .apply(insert(form))
        .expect_err("same-page HTML conflict");
    assert!(error.to_string().contains("form id `contact`"), "{error}");
    assert_eq!(same, before);
}

#[test]
fn duplicate_form_identity_is_refused_without_redirecting_provider_submission() {
    let mut editor = editor();
    let form: ComponentNode = serde_json::from_value(json!({
        "type": "form", "flyForm": {"id": "contact", "method": "post", "provider": "forms", "action": "submit"}
    })).unwrap();
    editor
        .apply(EditorCommand::Insert {
            parent_id: Some("root".to_string()),
            index: 1,
            component: form.clone(),
        })
        .expect("first form");
    let before = editor.clone();
    let error = editor
        .apply(EditorCommand::Insert {
            parent_id: Some("root".to_string()),
            index: 2,
            component: form,
        })
        .expect_err("duplicate form");
    assert!(error.to_string().contains("form id `contact`"), "{error}");
    assert_eq!(editor, before);
}

#[test]
fn cached_validation_cannot_admit_invalid_action_or_corrupt_redo() {
    let mut editor = editor();
    editor
        .apply(EditorCommand::Patch {
            component_id: "hero".to_string(),
            patch: ComponentPatch::default().merge_style(json!({"color": "blue"})),
        })
        .expect("valid edit");
    editor.undo().expect("undo");
    editor
        .apply(EditorCommand::Select {
            component_id: Some("hero".to_string()),
        })
        .expect("warm validation cache");
    let before = editor.clone();
    let error = editor
        .apply(EditorCommand::Patch {
            component_id: "hero".to_string(),
            patch: ComponentPatch::default().set_field(
                crate::FLY_ACTION_FIELD,
                json!({"kind": "not_a_supported_action"}),
            ),
        })
        .expect_err("invalid action must be rejected even after cache hit");
    assert!(matches!(error, FlyError::Validation(_)));
    assert_eq!(editor.document(), before.document());
    assert_eq!(editor.history(), before.history());
    assert_eq!(editor.revision(), before.revision());
    assert_eq!(editor.selection(), before.selection());
    editor.redo().expect("failed patch leaves redo available");
}

#[test]
fn cached_validation_cannot_admit_registered_but_illegal_child_type() {
    let document = GrapesJsCodec::decode_value(json!({
        "pages": [{
            "id": "home",
            "component": {
                "id": "root", "type": "wrapper",
                "components": [{
                    "id": "list", "type": "list",
                    "components": [{ "id": "item", "type": "list_item" }]
                }]
            }
        }]
    }))
    .expect("document");
    let mut editor = FlyEditor::new(document, RegistrySet::with_builtins());
    assert!(
        editor
            .apply(EditorCommand::Select {
                component_id: Some("item".to_string()),
            })
            .expect("warm validation cache")
            .is_valid()
    );
    let before = editor.clone();
    let error = editor
        .apply(EditorCommand::Patch {
            component_id: "item".to_string(),
            patch: ComponentPatch::default().set_component_type("text"),
        })
        .expect_err("registered type is not necessarily a legal child");
    assert!(matches!(error, FlyError::Validation(ref diagnostics)
        if diagnostics.iter().any(|d| d.code == "invalid_component_child")));
    assert_eq!(editor.document(), before.document());
    assert_eq!(editor.history(), before.history());
    assert_eq!(editor.revision(), before.revision());
}

#[test]
fn undo_and_redo_reject_disconnected_deserialized_history_without_mutation() {
    let mut original = editor();
    original
        .apply(EditorCommand::Patch {
            component_id: "hero".to_string(),
            patch: ComponentPatch::default().merge_style(json!({"color": "blue"})),
        })
        .expect("edit");
    for redo in [false, true] {
        let mut source = original.clone();
        if redo {
            source.undo().expect("undo");
        }
        let mut stored = serde_json::to_value(&source).expect("serialize");
        let (stack, endpoint) = if redo {
            ("redo", "before")
        } else {
            ("undo", "after")
        };
        // Replace the expected endpoint with a valid but unrelated document.
        stored["history"][stack][0][endpoint] =
            serde_json::to_value(editor().document()).expect("endpoint");
        if redo {
            stored["history"][stack][0][endpoint] =
                serde_json::to_value(original.document()).expect("different endpoint");
        }
        let mut restored: FlyEditor = serde_json::from_value(stored).expect("restore");
        let before = restored.clone();
        let error = if redo {
            restored.redo()
        } else {
            restored.undo()
        }
        .expect_err("disconnected history");
        assert!(matches!(error, FlyError::Validation(ref diagnostics)
            if diagnostics.iter().any(|d| d.code == "history_base_mismatch")));
        assert_eq!(restored, before);
    }
}

#[test]
fn exhausted_command_sequence_rejects_mutations_without_changing_history() {
    let mut original = editor();
    original
        .apply(EditorCommand::Patch {
            component_id: "hero".to_string(),
            patch: ComponentPatch::default().merge_style(json!({"color": "blue"})),
        })
        .expect("edit");
    let mut redo = original.clone();
    redo.undo().expect("undo");
    for (mut source, operation) in [
        (original.clone(), "apply"),
        (original, "undo"),
        (redo, "redo"),
    ] {
        source.revision_mut().command_sequence = u64::MAX;
        let before = source.clone();
        let result = match operation {
            "apply" => source
                .apply(EditorCommand::Patch {
                    component_id: "hero".to_string(),
                    patch: ComponentPatch::default().merge_style(json!({"color": "green"})),
                })
                .map(|_| ()),
            "undo" => source.undo().map(|_| ()),
            _ => source.redo().map(|_| ()),
        };
        assert!(matches!(result, Err(FlyError::Validation(ref diagnostics))
            if diagnostics.iter().any(|d| d.code == "command_sequence_exhausted")));
        assert_eq!(source, before);
        source
            .apply(EditorCommand::batch([]))
            .expect("no-op remains valid");
        assert_eq!(source.revision(), before.revision());
        assert_eq!(source.history(), before.history());
    }
}

#[test]
fn history_transitions_respect_current_validation_limits() {
    let mut source = editor();
    source
        .apply(EditorCommand::Insert {
            parent_id: Some("root".to_string()),
            index: 1,
            component: ComponentNode::Object(Box::new(crate::ComponentObject {
                id: Some("extra".to_string()),
                component_type: Some("section".to_string()),
                ..Default::default()
            })),
        })
        .expect("insert");
    let mut redo = source.clone();
    redo.undo().expect("undo while policy allows");
    let limits = crate::ValidationLimits {
        maximum_nodes: 2,
        ..Default::default()
    };
    let mut redo = redo.with_validation_limits(limits);
    let before = redo.clone();
    assert!(matches!(redo.redo(), Err(FlyError::Validation(_))));
    assert_eq!(redo, before);

    source
        .apply(EditorCommand::Remove {
            component_id: "extra".to_string(),
        })
        .expect("remove");
    let mut undo = source.with_validation_limits(limits);
    let before = undo.clone();
    assert!(matches!(undo.undo(), Err(FlyError::Validation(_))));
    assert_eq!(undo, before);
}
