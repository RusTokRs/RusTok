use crate::BrowserIntentDispatchError;
use fly_browser::BrowserIntentEnvelope;
use fly_ui::{resolve_editor_shortcut, CapabilityState, EditorShortcut, KeyStroke};
use serde_json::Value;

pub fn validate_browser_capability_access(
    envelope: &BrowserIntentEnvelope,
    capabilities: CapabilityState,
) -> Result<(), BrowserIntentDispatchError> {
    let capabilities = capabilities.normalized();
    if let Some((name, enabled)) = capability_requirement(envelope, capabilities)? {
        if !enabled {
            return Err(BrowserIntentDispatchError::Authoring(format!(
                "browser intent `{}` requires editor capability `{name}`",
                envelope.intent
            )));
        }
    }
    Ok(())
}

fn capability_requirement(
    envelope: &BrowserIntentEnvelope,
    capabilities: CapabilityState,
) -> Result<Option<(&'static str, bool)>, BrowserIntentDispatchError> {
    let requirement = match envelope.intent.as_str() {
        "select"
        | "focus_requested"
        | "hover"
        | "hover_requested"
        | "activate_page"
        | "cancel_drag"
        | "cancel_drag_requested" => None,
        "undo" | "redo" => Some(("history", capabilities.history)),
        "copy" | "cut" | "paste" | "duplicate" => {
            Some(("clipboard", capabilities.clipboard))
        }
        "save" => Some(("publish", capabilities.publish)),
        "begin_palette_drag"
        | "begin_selected_move"
        | "drop"
        | "drop_requested"
        | "drag_moved" => Some(("drag_drop", capabilities.drag_drop)),
        "patch_component_property" => {
            if property_kind(&envelope.payload) == Some("style") {
                Some(("styles", capabilities.styles))
            } else {
                Some(("properties", capabilities.properties))
            }
        }
        "patch_page_metadata"
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
        | "set_native_form_field" => Some(("properties", capabilities.properties)),
        "upsert_asset" | "remove_asset" | "select_asset" => {
            Some(("assets", capabilities.assets))
        }
        "key_stroke" => return shortcut_requirement(envelope, capabilities),
        "insert_block"
        | "remove_selected"
        | "move_selected_up"
        | "move_selected_down"
        | "create_page"
        | "rename_page"
        | "remove_page" => Some(("edit", capabilities.edit)),
        _ if envelope.is_mutating() => Some(("edit", capabilities.edit)),
        _ => None,
    };
    Ok(requirement)
}

fn shortcut_requirement(
    envelope: &BrowserIntentEnvelope,
    capabilities: CapabilityState,
) -> Result<Option<(&'static str, bool)>, BrowserIntentDispatchError> {
    let stroke_value = envelope
        .payload
        .get("stroke")
        .cloned()
        .unwrap_or_else(|| envelope.payload.clone());
    let stroke = serde_json::from_value::<KeyStroke>(stroke_value)
        .map_err(|error| BrowserIntentDispatchError::Payload(error.to_string()))?;
    let shortcut = resolve_editor_shortcut(&stroke)
        .ok_or_else(|| BrowserIntentDispatchError::Unsupported("key_stroke".to_string()))?;
    Ok(Some(match shortcut {
        EditorShortcut::Undo | EditorShortcut::Redo => ("history", capabilities.history),
        EditorShortcut::Save => ("publish", capabilities.publish),
        EditorShortcut::Copy
        | EditorShortcut::Cut
        | EditorShortcut::Paste
        | EditorShortcut::Duplicate => ("clipboard", capabilities.clipboard),
        EditorShortcut::DeleteSelection
        | EditorShortcut::MoveSelectionUp
        | EditorShortcut::MoveSelectionDown => ("edit", capabilities.edit),
        EditorShortcut::Cancel => return Ok(None),
    }))
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
        for intent in ["save", "undo", "copy"] {
            assert!(validate_browser_capability_access(
                &envelope(intent, json!({})),
                capabilities,
            )
            .is_err());
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
        assert!(validate_browser_capability_access(
            &envelope(
                "patch_component_property",
                json!({ "kind": "style" }),
            ),
            capabilities,
        )
        .is_err());
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
        assert!(validate_browser_capability_access(
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
        .is_err());
    }

    #[test]
    fn supplied_profile_is_authoritative() {
        let capabilities = CapabilityState {
            edit: false,
            ..CapabilityState::full()
        }
        .normalized();
        assert!(validate_browser_capability_access(
            &envelope("insert_block", json!({ "block_id": "text" })),
            capabilities,
        )
        .is_err());
    }
}
