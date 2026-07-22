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
fn reviewer_profile_can_publish_but_cannot_mutate() {
    let mut machine =
        FlyUiStateMachine::new(Presentation::Full).with_editable_capabilities(CapabilityState {
            edit: false,
            publish: true,
            ..CapabilityState::full()
        });
    let mutation = machine.dispatch(UiIntent::execute(EditorCommand::Patch {
        component_id: "hero".to_string(),
        patch: Default::default(),
    }));
    assert_eq!(
        mutation,
        Err(UiError::CapabilityUnavailable("edit".to_string()))
    );
    assert!(matches!(
        machine.dispatch(UiIntent::RequestSave),
        Ok(effects) if matches!(effects.first(), Some(UiEffect::Persist { .. }))
    ));
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
