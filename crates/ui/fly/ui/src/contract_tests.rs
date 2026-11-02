use crate::*;

#[test]
fn test_package_metadata_contract() {
    assert_eq!(env!("CARGO_PKG_NAME"), "fly-ui");
    assert!(!env!("CARGO_PKG_VERSION").is_empty());
    assert!(!env!("CARGO_PKG_DESCRIPTION").is_empty());
}

#[test]
fn test_capability_policy_contract() {
    let policy = EditorCapabilityPolicy::default();
    let eval = policy.evaluate_detailed();
    assert!(eval.allows(EditorCapability::Edit));
    assert!(eval.allows(EditorCapability::Publish));

    // When provider is degraded, mutating capabilities should be restricted
    let mut degraded_policy = policy;
    degraded_policy.provider_state = EditorProviderState::Degraded;
    let degraded_eval = degraded_policy.evaluate_detailed();
    assert!(degraded_eval.allows(EditorCapability::Edit));
    assert!(!degraded_eval.allows(EditorCapability::Publish));
}

#[test]
fn test_viewport_presets_contract() {
    let presets = builtin_viewport_presets();
    assert!(!presets.is_empty(), "Builtin viewport presets must not be empty");

    let desktop = presets.iter().find(|p| p.device_class == DeviceClass::Desktop);
    assert!(desktop.is_some(), "Desktop preset must exist");

    let mobile = presets.iter().find(|p| p.device_class == DeviceClass::Mobile);
    assert!(mobile.is_some(), "Mobile preset must exist");

    // Zoom normalizer clamps within [0.1, 4.0]
    let normal_zoom = normalize_zoom(1.0);
    assert!((normal_zoom - 1.0).abs() < f32::EPSILON);

    let clamped_min = normalize_zoom(0.01);
    assert!((clamped_min - 0.1).abs() < f32::EPSILON);

    let clamped_max = normalize_zoom(10.0);
    assert!((clamped_max - 4.0).abs() < f32::EPSILON);
}

#[test]
fn test_editor_shortcut_resolution_contract() {
    let undo_key = KeyStroke {
        code: Some("KeyZ".to_string()),
        key: "z".to_string(),
        modifiers: ModifierState {
            control: true,
            meta: false,
            shift: false,
            alt: false,
        },
        repeat: false,
        editing_text: false,
    };
    let shortcut = resolve_editor_shortcut(&undo_key);
    assert_eq!(shortcut, Some(EditorShortcut::Undo));

    let redo_key = KeyStroke {
        code: Some("KeyZ".to_string()),
        key: "Z".to_string(),
        modifiers: ModifierState {
            control: true,
            meta: false,
            shift: true,
            alt: false,
        },
        repeat: false,
        editing_text: false,
    };
    let redo_shortcut = resolve_editor_shortcut(&redo_key);
    assert_eq!(redo_shortcut, Some(EditorShortcut::Redo));
}

#[test]
fn test_builtin_style_properties_contract() {
    let props = builtin_style_properties();
    assert!(!props.is_empty(), "Builtin style properties must not be empty");
    assert!(props.iter().any(|p| p.property == "color"));
    assert!(props.iter().any(|p| p.property == "width"));
    assert!(props.iter().any(|p| p.property == "padding"));
}
