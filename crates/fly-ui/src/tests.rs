use crate::*;
use fly::{ComponentNode, EditorCommand, ValidationDiagnostic, ValidationSeverity};
use serde_json::Map;
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
            .dispatch(UiIntent::Execute(EditorCommand::Patch {
                component_id: "hero".to_string(),
                patch: Default::default(),
            }))
            .expect("editable presentation");
        assert!(matches!(effects[0], UiEffect::Command(_)));
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
        Some(UiEffect::Command(EditorCommand::Insert { .. }))
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
            provider_version: "1".to_string(),
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
