use crate::*;
use fly::{
    AssetCommand, ComponentNode, ComponentPatch, EditorCommand, ValidationDiagnostic,
    ValidationSeverity,
};
use serde_json::{Map, json};
use std::collections::{BTreeMap, BTreeSet};

fn legal_candidate() -> HitTestCandidate {
    HitTestCandidate {
        target_component_id: "root".to_string(),
        parent_component_id: None,
        index: 0,
        position: DropPosition::Inside,
        rect: CanvasRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 40.0,
        },
        score: 1.0,
        legal: true,
        reason: None,
    }
}

#[test]
fn full_and_inline_modes_can_drive_commands() {
    for presentation in [Presentation::Full, Presentation::Inline] {
        let mut machine = FlyUiStateMachine::new(presentation);
        let effects = machine
            .dispatch(UiIntent::execute(EditorCommand::Patch {
                component_id: "hero".to_string(),
                patch: Default::default(),
            }))
            .expect("editable presentation");
        assert!(matches!(&effects[0], UiEffect::Command(_)));
        assert!(machine.state.dirty.dirty);
    }
}

#[test]
fn presentation_switch_restores_edit_capabilities() {
    let mut machine = FlyUiStateMachine::new(Presentation::Full);
    machine
        .dispatch(UiIntent::SetPresentation(Presentation::ReadOnly))
        .expect("switch to read-only");
    assert!(!machine.state.capabilities.edit);
    machine
        .dispatch(UiIntent::SetPresentation(Presentation::Inline))
        .expect("switch to inline");
    assert!(machine.state.capabilities.edit);
    assert!(machine.state.capabilities.drag_drop);
}

#[test]
fn restricted_capabilities_survive_presentation_round_trip() {
    let restricted = CapabilityState {
        assets: false,
        publish: false,
        ..CapabilityState::full()
    };
    let mut machine =
        FlyUiStateMachine::new(Presentation::Full).with_editable_capabilities(restricted);
    assert_eq!(machine.editable_capabilities(), restricted);
    assert!(!machine.state.capabilities.assets);
    assert!(!machine.state.capabilities.publish);

    machine
        .dispatch(UiIntent::SetPresentation(Presentation::Preview))
        .expect("preview");
    assert_eq!(machine.state.capabilities, CapabilityState::read_only());

    machine
        .dispatch(UiIntent::SetPresentation(Presentation::Inline))
        .expect("inline");
    assert_eq!(machine.state.capabilities, restricted);
    assert!(!machine.state.capabilities.assets);
    assert!(!machine.state.capabilities.publish);
}

#[test]
fn withdrawing_drag_capability_cancels_active_drag_and_overlay() {
    let mut machine = FlyUiStateMachine::new(Presentation::Full);
    machine
        .dispatch(UiIntent::BeginDrag(DragSource::PaletteBlock {
            block_id: "section".to_string(),
            component: ComponentNode::object("section"),
        }))
        .expect("begin drag");
    machine
        .dispatch(UiIntent::UpdateHitTest(vec![legal_candidate()]))
        .expect("hit test");
    assert!(machine.state.drag.is_some());
    assert!(machine.state.overlays.insertion.is_some());

    machine
        .dispatch(UiIntent::SetEditableCapabilities(CapabilityState {
            drag_drop: false,
            ..CapabilityState::full()
        }))
        .expect("withdraw drag capability");
    assert!(machine.state.drag.is_none());
    assert!(machine.state.overlays.insertion.is_none());
    assert!(!machine.state.capabilities.drag_drop);
}

#[test]
fn author_profiles_save_drafts_and_publish_only_reviewers_do_not() {
    // Saving the working draft is an authoring action: a manager-style
    // profile (authoring without publish rights) can save its draft…
    let mut author =
        FlyUiStateMachine::new(Presentation::Full).with_editable_capabilities(CapabilityState {
            publish: false,
            ..CapabilityState::full()
        });
    assert!(matches!(
        author.dispatch(UiIntent::RequestSave),
        Ok(effects) if matches!(effects.first(), Some(UiEffect::Persist { .. }))
    ));

    // …while a publish-only reviewer can neither mutate the project nor
    // save a draft. The publish capability stays independent for the
    // real publication flow, which does not run through RequestSave.
    let mut reviewer =
        FlyUiStateMachine::new(Presentation::Full).with_editable_capabilities(CapabilityState {
            edit: false,
            publish: true,
            ..CapabilityState::full()
        });
    let mutation = reviewer.dispatch(UiIntent::execute(EditorCommand::Patch {
        component_id: "hero".to_string(),
        patch: Default::default(),
    }));
    assert_eq!(
        mutation,
        Err(UiError::CapabilityUnavailable("edit".to_string()))
    );
    assert_eq!(
        reviewer.dispatch(UiIntent::RequestSave),
        Err(UiError::CapabilityUnavailable("edit".to_string()))
    );
}

#[test]
fn specialized_commands_cannot_bypass_disabled_capabilities() {
    let mut machine =
        FlyUiStateMachine::new(Presentation::Full).with_editable_capabilities(CapabilityState {
            properties: false,
            styles: false,
            assets: false,
            ..CapabilityState::full()
        });

    let property_error = machine
        .dispatch(UiIntent::execute(EditorCommand::Patch {
            component_id: "hero".to_string(),
            patch: ComponentPatch {
                fields: Map::from_iter([("content".to_string(), json!("Hello"))]),
                ..ComponentPatch::default()
            },
        }))
        .expect_err("property patch must require properties capability");
    assert_eq!(
        property_error,
        UiError::CapabilityUnavailable("properties".to_string())
    );

    let style_error = machine
        .dispatch(UiIntent::execute(EditorCommand::Patch {
            component_id: "hero".to_string(),
            patch: ComponentPatch {
                style: Some(json!({ "color": "red" })),
                ..ComponentPatch::default()
            },
        }))
        .expect_err("style patch must require styles capability");
    assert_eq!(
        style_error,
        UiError::CapabilityUnavailable("styles".to_string())
    );

    let asset_error = machine
        .dispatch(UiIntent::execute(EditorCommand::Asset {
            command: AssetCommand::Remove {
                asset_id: "logo".to_string(),
            },
        }))
        .expect_err("asset command must require assets capability");
    assert_eq!(
        asset_error,
        UiError::CapabilityUnavailable("assets".to_string())
    );
    assert!(!machine.state.dirty.dirty);
    assert_eq!(machine.state.dirty.command_sequence, 0);
}

#[test]
fn batch_commands_require_every_specialized_capability_before_dispatch() {
    let command = EditorCommand::batch([
        EditorCommand::Patch {
            component_id: "hero".to_string(),
            patch: ComponentPatch {
                style: Some(json!({ "display": "grid" })),
                ..ComponentPatch::default()
            },
        },
        EditorCommand::Asset {
            command: AssetCommand::Remove {
                asset_id: "logo".to_string(),
            },
        },
    ]);
    let mut machine =
        FlyUiStateMachine::new(Presentation::Full).with_editable_capabilities(CapabilityState {
            styles: false,
            assets: false,
            ..CapabilityState::full()
        });

    assert_eq!(
        machine.dispatch(UiIntent::execute(command.clone())),
        Err(UiError::CapabilityUnavailable("styles".to_string()))
    );
    machine
        .dispatch(UiIntent::SetEditableCapabilities(CapabilityState {
            assets: false,
            ..CapabilityState::full()
        }))
        .expect("enable styles while preserving asset denial");
    assert_eq!(
        machine.dispatch(UiIntent::execute(command)),
        Err(UiError::CapabilityUnavailable("assets".to_string()))
    );
    assert!(!machine.state.dirty.dirty);
    assert_eq!(machine.state.dirty.command_sequence, 0);
}

#[test]
fn read_only_mode_rejects_mutation() {
    let mut machine = FlyUiStateMachine::new(Presentation::ReadOnly);
    let error = machine
        .dispatch(UiIntent::Undo)
        .expect_err("read-only must reject history mutation");
    assert_eq!(error, UiError::ReadOnly);
}

#[test]
fn drag_state_emits_framework_neutral_command() {
    let mut machine = FlyUiStateMachine::new(Presentation::Full);
    machine
        .dispatch(UiIntent::BeginDrag(DragSource::PaletteBlock {
            block_id: "section".to_string(),
            component: ComponentNode::object("section"),
        }))
        .expect("begin drag");
    machine
        .dispatch(UiIntent::UpdateHitTest(vec![legal_candidate()]))
        .expect("hit test");
    let effects = machine.dispatch(UiIntent::Drop).expect("drop");
    assert!(matches!(
        effects.first(),
        Some(UiEffect::Command(command)) if matches!(command.as_ref(), EditorCommand::Insert { .. })
    ));
    assert!(machine.state.drag.is_none());
}

#[test]
fn a_rejected_drop_does_not_destroy_drag_or_insertion_overlay() {
    let mut machine = FlyUiStateMachine::new(Presentation::Full);
    machine
        .dispatch(UiIntent::BeginDrag(DragSource::PaletteBlock {
            block_id: "section".to_string(),
            component: ComponentNode::object("section"),
        }))
        .expect("begin drag");
    assert!(matches!(
        machine.dispatch(UiIntent::Drop),
        Err(UiError::IllegalDrop(_))
    ));
    assert!(machine.state.drag.is_some());
    assert!(!machine.state.dirty.dirty);

    machine
        .dispatch(UiIntent::UpdateHitTest(vec![legal_candidate()]))
        .expect("new candidate");
    let overlay = machine.state.overlays.insertion;
    assert!(overlay.is_some());
    machine.dispatch(UiIntent::Drop).expect("retry drop");
    assert!(machine.state.drag.is_none());
    assert!(machine.state.overlays.insertion.is_none());

    machine
        .dispatch(UiIntent::BeginDrag(DragSource::ClipboardFragment))
        .expect("begin clipboard drag");
    machine
        .dispatch(UiIntent::UpdateHitTest(vec![legal_candidate()]))
        .expect("candidate");
    assert!(matches!(
        machine.dispatch(UiIntent::Drop),
        Err(UiError::IllegalDrop(_))
    ));
    assert!(machine.state.drag.is_some());
    assert_eq!(machine.state.overlays.insertion.is_some(), true);
    machine.dispatch(UiIntent::CancelDrag).expect("cancel");
    assert!(machine.state.drag.is_none());
}

#[test]
fn drop_rechecks_edit_permission_even_with_a_restored_drag() {
    let mut machine = FlyUiStateMachine::new(Presentation::Full);
    machine
        .dispatch(UiIntent::BeginDrag(DragSource::PaletteBlock {
            block_id: "section".to_string(),
            component: ComponentNode::object("section"),
        }))
        .expect("begin drag");
    machine
        .dispatch(UiIntent::UpdateHitTest(vec![legal_candidate()]))
        .expect("candidate");
    // A deserialized/restored state can contain a drag that was begun under a
    // different capability profile. Drop is its own mutation admission boundary.
    machine.state.presentation = Presentation::ReadOnly;
    assert_eq!(machine.dispatch(UiIntent::Drop), Err(UiError::ReadOnly));
    assert!(machine.state.drag.is_some());
    assert!(!machine.state.dirty.dirty);
    machine.state.presentation = Presentation::Full;
    machine.state.capabilities.drag_drop = false;
    assert!(matches!(
        machine.dispatch(UiIntent::Drop),
        Err(UiError::CapabilityUnavailable(_))
    ));
    assert!(machine.state.drag.is_some());
    assert!(!machine.state.dirty.dirty);
}

#[test]
fn deserializing_ui_does_not_restore_orphaned_save_in_progress() {
    let mut machine = FlyUiStateMachine::new(Presentation::Full);
    machine.dispatch(UiIntent::SaveStarted { request_id: "req-1".into() }).expect("begin");
    let restored: FlyUiStateMachine =
        serde_json::from_value(serde_json::to_value(machine).expect("serialize"))
            .expect("restore");
    assert!(!restored.state.dirty.save_in_progress);
    assert!(restored.state.dirty.save_started_sequence.is_none());
    assert!(restored.state.dirty.save_started_request_id.is_none());
    assert!(restored.state.dirty.save_started_hash.is_none());
}

#[test]
fn save_start_and_acknowledgement_respect_conflicts_and_submitted_hash() {
    let mut machine = FlyUiStateMachine::new(Presentation::Full);
    let document = fly::GrapesJsCodec::decode_value(json!({ "pages": [] })).expect("document");
    let snapshot_hash = fly::ProjectHash::from_document(&document);
    machine.state.dirty.project_hash = Some(snapshot_hash);
    machine.dispatch(UiIntent::SaveStarted { request_id: "req-1".into() }).expect("save started");
    let wrong_hash = fly::ProjectHash::from_bytes(b"other snapshot");
    assert!(matches!(
        machine.dispatch(UiIntent::SaveSucceeded {
            request_id: "wrong-request".into(),
            revision: "rev-2".to_string(),
            project_hash: wrong_hash,
        }),
        Err(UiError::CapabilityUnavailable(_))
    ));
    assert!(machine.state.dirty.save_in_progress);
    assert_eq!(machine.state.dirty.save_started_hash, Some(snapshot_hash));
    machine.dispatch(UiIntent::SaveConflict {
        request_id: Some("req-1".into()),
        expected: "rev-1".to_string(),
        actual: "rev-2".to_string(),
    }).expect("remote conflict");
    assert!(machine.dispatch(UiIntent::SaveStarted { request_id: "req-2".into() }).is_err());
    assert!(machine.state.dirty.save_conflict);
    assert!(!machine.state.dirty.save_in_progress);
}

#[test]
fn stale_save_responses_cannot_acknowledge_or_fail_a_new_attempt() {
    let mut machine = FlyUiStateMachine::new(Presentation::Full);
    machine.dispatch(UiIntent::SaveStarted { request_id: "request-a".into() }).expect("first start");
    machine.dispatch(UiIntent::SaveFailed { request_id: Some("request-a".into()) }).expect("first failure");
    machine.dispatch(UiIntent::SaveStarted { request_id: "request-b".into() }).expect("retry start");

    let document = fly::GrapesJsCodec::decode_value(json!({ "pages": [] })).expect("document");
    let hash = fly::ProjectHash::from_document(&document);
    assert!(machine.dispatch(UiIntent::SaveSucceeded {
        request_id: "request-a".into(),
        revision: "stale-revision".into(),
        project_hash: hash,
    }).is_err());
    assert!(machine.dispatch(UiIntent::SaveFailed {
        request_id: Some("request-a".into()),
    }).is_err());
    assert!(machine.dispatch(UiIntent::SaveConflict {
        request_id: Some("request-a".into()),
        expected: "rev-1".into(),
        actual: "rev-2".into(),
    }).is_err());
    assert_eq!(machine.state.dirty.save_started_request_id.as_deref(), Some("request-b"));
    assert!(machine.state.dirty.save_in_progress);

    machine.dispatch(UiIntent::SaveSucceeded {
        request_id: "request-b".into(),
        revision: "current-revision".into(),
        project_hash: hash,
    }).expect("current response");
    assert!(!machine.state.dirty.save_in_progress);
}

#[test]
fn uncorrelated_failure_cannot_clear_an_active_save() {
    let mut machine = FlyUiStateMachine::new(Presentation::Full);
    machine.dispatch(UiIntent::SaveStarted { request_id: "request-a".into() }).expect("start");
    assert!(machine.dispatch(UiIntent::SaveFailed { request_id: None }).is_err());
    assert_eq!(machine.state.dirty.save_started_request_id.as_deref(), Some("request-a"));
    assert!(machine.state.dirty.save_in_progress);
}

#[test]
fn save_acknowledgement_cannot_clean_edits_made_during_save() {
    let mut machine = FlyUiStateMachine::new(Presentation::Full);
    let document = fly::GrapesJsCodec::decode_value(json!({ "pages": [] })).expect("document");
    let saved_hash = fly::ProjectHash::from_document(&document);
    machine
        .dispatch(UiIntent::execute(EditorCommand::Patch {
            component_id: "hero".to_string(),
            patch: Default::default(),
        }))
        .expect("first edit");
    machine.dispatch(UiIntent::SaveStarted { request_id: "req-1".into() }).expect("save starts");
    assert!(machine.dispatch(UiIntent::RequestSave).is_err());
    machine
        .dispatch(UiIntent::execute(EditorCommand::Patch {
            component_id: "hero".to_string(),
            patch: Default::default(),
        }))
        .expect("second edit");
    let effects = machine
        .dispatch(UiIntent::SaveSucceeded {
            request_id: "req-1".into(),
            revision: "rev-2".to_string(),
            project_hash: saved_hash,
        })
        .expect("earlier snapshot saved");
    assert!(machine.state.dirty.dirty);
    assert!(!machine.state.dirty.save_in_progress);
    assert_eq!(machine.state.dirty.last_acknowledged_revision.as_deref(), Some("rev-2"));
    assert!(machine.state.dirty.project_hash.is_none());
    assert!(effects.iter().any(|effect| matches!(
        effect,
        UiEffect::Announce(message) if message.contains("remain unsaved")
    )));
    machine.dispatch(UiIntent::SaveStarted { request_id: "req-2".into() }).expect("next save starts");
    machine
        .dispatch(UiIntent::SaveSucceeded {
            request_id: "req-2".into(),
            revision: "rev-3".to_string(),
            project_hash: saved_hash,
        })
        .expect("latest save");
    assert!(!machine.state.dirty.dirty);
    assert_eq!(machine.state.dirty.project_hash, Some(saved_hash));
}

#[test]
fn viewport_and_overlay_geometry_are_bounded_at_the_state_machine_boundary() {
    let mut machine = FlyUiStateMachine::new(Presentation::Full);
    machine.dispatch(UiIntent::SetViewport(ViewportState {
        width: 0,
        height: u32::MAX,
        zoom: f32::NAN,
        scroll_x: f64::NEG_INFINITY,
        scroll_y: -17.0,
    })).expect("normalized viewport");
    assert_eq!(machine.state.viewport.width, 240);
    assert_eq!(machine.state.viewport.height, 4320);
    assert_eq!(machine.state.viewport.zoom, 1.0);
    assert_eq!(machine.state.viewport.scroll_x, 0.0);
    assert_eq!(machine.state.viewport.scroll_y, 0.0);

    let bad_rect = CanvasRect {
        x: 0.0, y: 0.0, width: f64::INFINITY, height: 10.0,
    };
    assert!(!bad_rect.contains(1.0, 1.0));
    assert!(matches!(
        machine.dispatch(UiIntent::SetSelectedOverlay(Some(bad_rect))),
        Err(UiError::InvalidGeometry(_))
    ));
    assert!(machine.state.overlays.selected.is_none());
    assert!(matches!(
        machine.dispatch(UiIntent::SetHoveredOverlay(Some(bad_rect))),
        Err(UiError::InvalidGeometry(_))
    ));

    machine.dispatch(UiIntent::BeginDrag(DragSource::PaletteBlock {
        block_id: "section".to_string(),
        component: ComponentNode::object("section"),
    })).expect("begin drag");
    let mut candidate = legal_candidate();
    candidate.score = f32::NAN;
    assert!(matches!(
        machine.dispatch(UiIntent::UpdateHitTest(vec![candidate])),
        Err(UiError::InvalidGeometry(_))
    ));
    assert!(machine.state.drag.as_ref().is_some_and(|drag| drag.candidates.is_empty()));
}

#[test]
fn blocking_diagnostics_prevent_save() {
    let mut machine = FlyUiStateMachine::new(Presentation::Full);
    machine
        .dispatch(UiIntent::ReplaceDiagnostics(vec![ValidationDiagnostic {
            severity: ValidationSeverity::Error,
            code: "invalid".to_string(),
            path: "project".to_string(),
            message: "invalid project".to_string(),
        }]))
        .expect("replace diagnostics");
    assert!(machine.dispatch(UiIntent::RequestSave).is_err());
}

#[test]
fn contribution_filtering_is_capability_driven() {
    let mut registry = ContributionRegistry::default();
    registry
        .register(ContributionDescriptor {
            id: "rustok.pages.hero".to_string(),
            provider: "rustok.pages".to_string(),
            required_capabilities: BTreeSet::from(["pages.read".to_string()]),
            blocks: vec!["rustok.pages.hero".to_string()],
            renderers: Vec::new(),
            property_editors: Vec::new(),
            messages: BTreeMap::new(),
            metadata: Map::new(),
        })
        .expect("register");
    let capabilities = BTreeSet::from(["pages.read".to_string()]);
    assert_eq!(registry.available(&capabilities).count(), 1);
    assert_eq!(registry.available(&BTreeSet::new()).count(), 0);
}

#[test]
fn a_read_only_session_cannot_slip_an_empty_batch_past_the_gate() {
    // The state machine skips its whole guarded branch when a command requires no capability.
    // An empty batch used to compute an empty requirement, so it bypassed the read-only check
    // and still reached `FlyEditor::apply`, which records a history entry and marks the revision
    // changed — enough for a read-only session to make the real editor's save fail on a conflict.
    let mut machine = FlyUiStateMachine::new(Presentation::ReadOnly);
    let error = machine
        .dispatch(UiIntent::execute(EditorCommand::Batch { commands: vec![] }))
        .expect_err("read-only must reject an empty batch");
    assert_eq!(error, UiError::ReadOnly);
}

#[test]
fn an_empty_batch_is_still_accepted_when_editing_is_allowed() {
    // Failing closed must not break a legitimate session.
    let mut machine = FlyUiStateMachine::new(Presentation::Full);
    machine
        .dispatch(UiIntent::execute(EditorCommand::Batch { commands: vec![] }))
        .expect("an editing session may dispatch an empty batch");
    assert!(!machine.state.dirty.dirty);
    assert_eq!(machine.state.dirty.command_sequence, 0);
}

#[test]
fn selection_remains_possible_without_edit_capability() {
    // `Select` requires no capability by design: inspecting a read-only preview is legitimate.
    // The empty-batch fix must not have swept that up.
    let mut machine = FlyUiStateMachine::new(Presentation::ReadOnly);
    machine
        .dispatch(UiIntent::execute(EditorCommand::Select {
            component_id: Some("hero".to_string()),
        }))
        .expect("selection must stay available in a read-only session");
}

#[test]
fn edit_after_failed_save_keeps_the_failure_signal() {
    let mut machine = FlyUiStateMachine::new(Presentation::Full);
    machine.dispatch(UiIntent::SaveStarted { request_id: "req-1".into() }).expect("started");
    machine.dispatch(UiIntent::SaveFailed { request_id: Some("req-1".into()) }).expect("failed");
    assert!(machine.state.dirty.save_failed);

    // The rejected changes are still unsaved: a further edit must not clear the signal.
    machine
        .dispatch(UiIntent::execute(EditorCommand::Patch {
            component_id: "hero".to_string(),
            patch: Default::default(),
        }))
        .expect("edit after failed save");
    assert!(machine.state.dirty.dirty);
    assert!(machine.state.dirty.save_failed);

    // The next save attempt does.
    machine.dispatch(UiIntent::SaveStarted { request_id: "req-2".into() }).expect("retry");
    assert!(!machine.state.dirty.save_failed);
}

#[test]
fn save_conflict_is_a_distinct_lost_update_state() {
    let mut machine = FlyUiStateMachine::new(Presentation::Full);
    machine.dispatch(UiIntent::SaveStarted { request_id: "req-1".into() }).expect("started");
    let effects = machine
        .dispatch(UiIntent::SaveConflict {
            request_id: Some("req-1".into()),
            expected: "rev-1".to_string(),
            actual: "rev-2".to_string(),
        })
        .expect("conflict");
    let state = &machine.state;
    assert!(state.dirty.save_conflict);
    assert!(state.dirty.dirty);
    assert!(state.dirty.save_failed);
    assert!(matches!(
        machine.dispatch(UiIntent::RequestSave),
        Err(UiError::CapabilityUnavailable(_))
    ));
    assert!(!state.dirty.save_in_progress);
    assert!(effects.iter().any(|effect| matches!(
        effect,
        UiEffect::Announce(message) if message.contains("another session")
    )));
    // A response without an active submitted save cannot make the conflict vanish.
    let document = fly::GrapesJsCodec::decode_value(json!({ "pages": [] })).expect("document");
    assert!(machine
        .dispatch(UiIntent::SaveSucceeded {
            request_id: "req-1".into(),
            revision: "rev-3".to_string(),
            project_hash: fly::ProjectHash::from_document(&document),
        })
        .is_err());
    assert!(machine.state.dirty.save_conflict);
    machine.dispatch(UiIntent::SaveFailed { request_id: None }).expect("failed response");
    assert!(machine.state.dirty.save_conflict);
}
