use crate::BrowserIntentDispatchError;
use fly_browser::BrowserIntentEnvelope;
use fly_ui::{
    resolve_editor_shortcut, CapabilityState, EditorCapability, EditorShortcut, KeyStroke,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const CAPABILITY_DENIAL_PREFIX: &str = "FLY_CAPABILITY_DENIED:";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrowserCapabilityDenial {
    pub intent: String,
    pub capability: EditorCapability,
}

pub fn browser_capability_denial(
    error: &BrowserIntentDispatchError,
) -> Option<BrowserCapabilityDenial> {
    let BrowserIntentDispatchError::Authoring(message) = error else {
        return None;
    };
    let payload = message.strip_prefix(CAPABILITY_DENIAL_PREFIX)?;
    serde_json::from_str(payload).ok()
}

pub fn validate_browser_capability_access(
    envelope: &BrowserIntentEnvelope,
    capabilities: CapabilityState,
) -> Result<(), BrowserIntentDispatchError> {
    let capabilities = capabilities.normalized();
    for capability in capability_requirements(envelope)? {
        if !capabilities.allows(capability) {
            return Err(capability_denied(envelope, capability));
        }
    }
    Ok(())
}

fn capability_denied(
    envelope: &BrowserIntentEnvelope,
    capability: EditorCapability,
) -> BrowserIntentDispatchError {
    let payload = serde_json::json!({
        "intent": envelope.intent.as_str(),
        "capability": capability,
    });
    BrowserIntentDispatchError::Authoring(format!(
        "{CAPABILITY_DENIAL_PREFIX}{}",
        payload
    ))
}

fn capability_requirements(
    envelope: &BrowserIntentEnvelope,
) -> Result<Vec<EditorCapability>, BrowserIntentDispatchError> {
    let requirements = match envelope.intent.as_str() {
        "select"
        | "focus_requested"
        | "hover"
        | "hover_requested"
        | "activate_page"
        | "cancel_drag"
        | "cancel_drag_requested" => Vec::new(),
        "undo" | "redo" => vec![EditorCapability::History],
        "copy" | "cut" | "paste" | "duplicate" => vec![EditorCapability::Clipboard],
        "save" => vec![EditorCapability::Publish],
        "begin_palette_drag"
        | "begin_selected_move"
        | "drop"
        | "drop_requested"
        | "drag_moved" => vec![EditorCapability::DragDrop],
        "patch_component_property" => {
            if property_kind(&envelope.payload) == Some("style") {
                vec![EditorCapability::Styles]
            } else {
                vec![EditorCapability::Properties]
            }
        }
        "patch_page_metadata"
        | "rename_page"
        | "set_locale_policy"
        | "clear_locale_policy"
        | "upsert_localized_page_metadata"
        | "set_internal_page_link"
        | "remove_internal_page_link"
        | "upsert_translation"
        | "remove_translation"
        | "set_component_action"
        | "remove_component_action"
        | "set_component_form"
        | "remove_component_form"
        | "set_native_form_field" => vec![EditorCapability::Properties],
        "upsert_asset" | "remove_asset" => vec![EditorCapability::Assets],
        "select_asset" => vec![EditorCapability::Assets, EditorCapability::Properties],
        "key_stroke" => return shortcut_requirements(envelope),
        "insert_block"
        | "remove_selected"
        | "move_selected_up"
        | "move_selected_down"
        | "create_page"
        | "remove_page" => vec![EditorCapability::Edit],
        _ if envelope.is_mutating() => vec![EditorCapability::Edit],
        _ => Vec::new(),
    };
    Ok(requirements)
}

fn shortcut_requirements(
    envelope: &BrowserIntentEnvelope,
) -> Result<Vec<EditorCapability>, BrowserIntentDispatchError> {
    let stroke_value = envelope
        .payload
        .get("stroke")
        .cloned()
        .unwrap_or_else(|| envelope.payload.clone());
    let stroke = serde_json::from_value::<KeyStroke>(stroke_value)
        .map_err(|error| BrowserIntentDispatchError::Payload(error.to_string()))?;
    let shortcut = resolve_editor_shortcut(&stroke)
        .ok_or_else(|| BrowserIntentDispatchError::Unsupported("key_stroke".to_string()))?;
    Ok(match shortcut {
        EditorShortcut::Undo | EditorShortcut::Redo => vec![EditorCapability::History],
        EditorShortcut::Save => vec![EditorCapability::Publish],
        EditorShortcut::Copy
        | EditorShortcut::Cut
        | EditorShortcut::Paste
        | EditorShortcut::Duplicate => vec![EditorCapability::Clipboard],
        EditorShortcut::DeleteSelection
        | EditorShortcut::MoveSelectionUp
        | EditorShortcut::MoveSelectionDown => vec![EditorCapability::Edit],
        EditorShortcut::Cancel => Vec::new(),
    })
}

fn property_kind(payload: &Value) -> Option<&str> {
    payload
        .get("kind")
        .and_then(Value::as_str)
        .map(str::trim)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fly_browser::FLY_BROWSER_PROTOCOL_V1;
    use serde_json::json;

    fn envelope(intent: &str, payload: Value) -> BrowserIntentEnvelope {
        BrowserIntentEnvelope {
            protocol: FLY_BROWSER_PROTOCOL_V1.to_string(),
            instance_id: "capability-test".to_string(),
            intent: intent.to_string(),
            payload,
            sequence: Some(1),
            page_id: Some("home".to_string()),
            revision: Some("rev-1".to_string()),
            project_hash: None,
            draft_token: None,
            draft_generation: None,
        }
    }

    #[test]
    fn publish_history_and_clipboard_intents_use_specific_capabilities() {
        let capabilities = CapabilityState {
            publish: false,
            history: false,
            clipboard: false,
            ..CapabilityState::full()
        };
        for (intent, capability) in [
            ("save", EditorCapability::Publish),
            ("undo", EditorCapability::History),
            ("copy", EditorCapability::Clipboard),
        ] {
            let error = validate_browser_capability_access(
                &envelope(intent, json!({})),
                capabilities,
            )
            .expect_err("capability denial");
            assert_eq!(
                browser_capability_denial(&error),
                Some(BrowserCapabilityDenial {
                    intent: intent.to_string(),
                    capability,
                })
            );
        }
        assert!(validate_browser_capability_access(
            &envelope("select", json!({ "component_id": "hero" })),
            capabilities,
        )
        .is_ok());
    }

    #[test]
    fn property_and_style_forms_are_distinguished() {
        let capabilities = CapabilityState {
            properties: true,
            styles: false,
            ..CapabilityState::full()
        };
        assert!(validate_browser_capability_access(
            &envelope(
                "patch_component_property",
                json!({ "kind": "field" }),
            ),
            capabilities,
        )
        .is_ok());
        let error = validate_browser_capability_access(
            &envelope(
                "patch_component_property",
                json!({ "kind": "style" }),
            ),
            capabilities,
        )
        .expect_err("style capability denial");
        assert_eq!(
            browser_capability_denial(&error)
                .map(|denial| denial.capability),
            Some(EditorCapability::Styles)
        );
    }

    #[test]
    fn page_rename_uses_properties_capability() {
        let capabilities = CapabilityState {
            properties: false,
            ..CapabilityState::full()
        };
        let error = validate_browser_capability_access(
            &envelope(
                "rename_page",
                json!({ "page_id": "home", "new_page_id": "landing" }),
            ),
            capabilities,
        )
        .expect_err("page rename is a PageCommand::Patch");
        assert_eq!(
            browser_capability_denial(&error)
                .map(|denial| denial.capability),
            Some(EditorCapability::Properties)
        );
    }

    #[test]
    fn selecting_an_asset_requires_asset_and_property_capabilities() {
        let missing_properties = CapabilityState {
            properties: false,
            ..CapabilityState::full()
        };
        let error = validate_browser_capability_access(
            &envelope("select_asset", json!({ "asset_id": "logo" })),
            missing_properties,
        )
        .expect_err("asset application changes component properties");
        assert_eq!(
            browser_capability_denial(&error)
                .map(|denial| denial.capability),
            Some(EditorCapability::Properties)
        );

        let missing_assets = CapabilityState {
            assets: false,
            ..CapabilityState::full()
        };
        let error = validate_browser_capability_access(
            &envelope("select_asset", json!({ "asset_id": "logo" })),
            missing_assets,
        )
        .expect_err("asset access is required before application");
        assert_eq!(
            browser_capability_denial(&error)
                .map(|denial| denial.capability),
            Some(EditorCapability::Assets)
        );
    }

    #[test]
    fn drag_and_drop_intents_require_drag_capability() {
        let capabilities = CapabilityState {
            drag_drop: false,
            ..CapabilityState::full()
        };
        for intent in ["begin_palette_drag", "begin_selected_move", "drop"] {
            assert!(validate_browser_capability_access(
                &envelope(intent, json!({})),
                capabilities,
            )
            .is_err());
        }
    }

    #[test]
    fn keyboard_shortcuts_share_the_same_capability_table() {
        let capabilities = CapabilityState {
            publish: false,
            ..CapabilityState::full()
        };
        let error = validate_browser_capability_access(
            &envelope(
                "key_stroke",
                json!({
                    "stroke": {
                        "key": "s",
                        "code": null,
                        "modifiers": {
                            "shift": false,
                            "alt": false,
                            "control": true,
                            "meta": false
                        },
                        "repeat": false,
                        "editing_text": false
                    }
                }),
            ),
            capabilities,
        )
        .expect_err("publish shortcut capability denial");
        assert_eq!(
            browser_capability_denial(&error)
                .map(|denial| denial.capability),
            Some(EditorCapability::Publish)
        );
    }

    #[test]
    fn supplied_profile_is_authoritative() {
        let capabilities = CapabilityState {
            edit: false,
            ..CapabilityState::full()
        }
        .normalized();
        let error = validate_browser_capability_access(
            &envelope("insert_block", json!({ "block_id": "text" })),
            capabilities,
        )
        .expect_err("edit capability denial");
        assert_eq!(
            browser_capability_denial(&error)
                .map(|denial| denial.capability),
            Some(EditorCapability::Edit)
        );
    }
}
