use crate::BrowserIntentDispatchError;
use fly::{AssetCommand, ComponentPatch, EditorCommand};
use fly_browser::{BrowserIntentEnvelope, BrowserIntentKind};
use fly_ui::{
    CapabilityState, CommandCapabilityRequirement, EditorCapability, EditorShortcut, KeyStroke,
    resolve_editor_shortcut,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{collections::BTreeSet, fmt};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrowserCapabilityDenial {
    pub intent: BrowserIntentKind,
    /// Backward-compatible primary capability. New clients should use `required` and `missing`.
    pub capability: EditorCapability,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required: Vec<EditorCapability>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<EditorCapability>,
}

/// Framework-neutral capability failure contract used by browser, HTTP and UI adapters.
pub type CapabilityFailure = BrowserCapabilityDenial;

impl BrowserCapabilityDenial {
    fn from_requirements(
        intent: BrowserIntentKind,
        requirements: impl IntoIterator<Item = EditorCapability>,
        capabilities: CapabilityState,
    ) -> Option<Self> {
        let required = requirements
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let missing = required
            .iter()
            .copied()
            .filter(|capability| !capabilities.allows(*capability))
            .collect::<Vec<_>>();
        let capability = missing.first().copied()?;
        Some(Self {
            intent,
            capability,
            required,
            missing,
        })
    }
}

impl fmt::Display for BrowserCapabilityDenial {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.missing.len() <= 1 {
            return write!(
                formatter,
                "browser intent `{}` requires editor capability `{}`",
                self.intent.as_str(),
                self.capability.as_str()
            );
        }
        let required = self
            .required
            .iter()
            .map(|capability| capability.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let missing = self
            .missing
            .iter()
            .map(|capability| capability.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        write!(
            formatter,
            "browser intent `{}` requires editor capabilities [{}]; missing [{}]",
            self.intent.as_str(),
            required,
            missing
        )
    }
}

impl std::error::Error for BrowserCapabilityDenial {}

#[derive(Debug, thiserror::Error)]
pub enum BrowserCapabilityAccessError {
    #[error(transparent)]
    Denied(#[from] BrowserCapabilityDenial),
    #[error(transparent)]
    Dispatch(#[from] BrowserIntentDispatchError),
}

pub fn browser_capability_denial(
    error: &BrowserCapabilityAccessError,
) -> Option<&BrowserCapabilityDenial> {
    match error {
        BrowserCapabilityAccessError::Denied(denial) => Some(denial),
        BrowserCapabilityAccessError::Dispatch(_) => None,
    }
}

pub fn validate_browser_capability_access(
    envelope: &BrowserIntentEnvelope,
    capabilities: CapabilityState,
) -> Result<(), BrowserCapabilityAccessError> {
    let capabilities = capabilities.normalized();
    let Some((intent, requirements)) = capability_requirements(envelope)? else {
        return Ok(());
    };
    let Some(denial) =
        BrowserCapabilityDenial::from_requirements(intent, requirements, capabilities)
    else {
        return Ok(());
    };
    Err(denial.into())
}

fn capability_requirements(
    envelope: &BrowserIntentEnvelope,
) -> Result<Option<(BrowserIntentKind, Vec<EditorCapability>)>, BrowserCapabilityAccessError> {
    let Some(kind) = envelope.kind() else {
        return Ok(None);
    };
    let requirements = match kind {
        BrowserIntentKind::Select
        | BrowserIntentKind::FocusRequested
        | BrowserIntentKind::Hover
        | BrowserIntentKind::HoverRequested
        | BrowserIntentKind::ActivatePage
        | BrowserIntentKind::CancelDrag
        | BrowserIntentKind::CancelDragRequested => Vec::new(),
        BrowserIntentKind::Undo | BrowserIntentKind::Redo => vec![EditorCapability::History],
        BrowserIntentKind::Copy
        | BrowserIntentKind::Cut
        | BrowserIntentKind::Paste
        | BrowserIntentKind::Duplicate => vec![EditorCapability::Clipboard],
        BrowserIntentKind::Save => vec![EditorCapability::Publish],
        BrowserIntentKind::BeginPaletteDrag
        | BrowserIntentKind::BeginSelectedMove
        | BrowserIntentKind::Drop
        | BrowserIntentKind::DropRequested
        | BrowserIntentKind::DragMoved => vec![EditorCapability::DragDrop],
        BrowserIntentKind::PatchComponentProperty => {
            command_requirements(component_patch_command(&envelope.payload))
        }
        BrowserIntentKind::PatchPageMetadata
        | BrowserIntentKind::RenamePage
        | BrowserIntentKind::SetLocalePolicy
        | BrowserIntentKind::ClearLocalePolicy
        | BrowserIntentKind::UpsertLocalizedPageMetadata
        | BrowserIntentKind::SetInternalPageLink
        | BrowserIntentKind::RemoveInternalPageLink
        | BrowserIntentKind::UpsertTranslation
        | BrowserIntentKind::RemoveTranslation
        | BrowserIntentKind::SetComponentAction
        | BrowserIntentKind::RemoveComponentAction
        | BrowserIntentKind::SetComponentForm
        | BrowserIntentKind::RemoveComponentForm
        | BrowserIntentKind::SetNativeFormField
        | BrowserIntentKind::SetRuntimeContext
        | BrowserIntentKind::SetRuntimeLocale => command_requirements(property_command()),
        BrowserIntentKind::UpsertAsset | BrowserIntentKind::RemoveAsset => {
            command_requirements(asset_command())
        }
        BrowserIntentKind::SelectAsset => {
            command_requirements(EditorCommand::batch([asset_command(), property_command()]))
        }
        BrowserIntentKind::KeyStroke => return Ok(Some((kind, shortcut_requirements(envelope)?))),
        BrowserIntentKind::InsertBlock
        | BrowserIntentKind::RemoveSelected
        | BrowserIntentKind::MoveSelected
        | BrowserIntentKind::MoveSelectedUp
        | BrowserIntentKind::MoveSelectedDown
        | BrowserIntentKind::PatchSelected
        | BrowserIntentKind::CreatePage
        | BrowserIntentKind::RemovePage => command_requirements(structural_command()),
    };
    Ok(Some((kind, requirements)))
}

fn shortcut_requirements(
    envelope: &BrowserIntentEnvelope,
) -> Result<Vec<EditorCapability>, BrowserCapabilityAccessError> {
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
        | EditorShortcut::MoveSelectionDown => command_requirements(structural_command()),
        EditorShortcut::Cancel => Vec::new(),
    })
}

fn command_requirements(command: EditorCommand) -> Vec<EditorCapability> {
    CommandCapabilityRequirement::for_command(&command)
        .capabilities()
        .collect()
}

fn structural_command() -> EditorCommand {
    EditorCommand::Remove {
        component_id: "__capability_probe__".to_string(),
    }
}

fn property_command() -> EditorCommand {
    EditorCommand::Patch {
        component_id: "__capability_probe__".to_string(),
        patch: ComponentPatch {
            fields: Map::from_iter([("__capability_probe__".to_string(), Value::Null)]),
            ..ComponentPatch::default()
        },
    }
}

fn component_patch_command(payload: &Value) -> EditorCommand {
    let patch = if property_kind(payload) == Some("style") {
        ComponentPatch {
            style: Some(Value::Object(Map::new())),
            ..ComponentPatch::default()
        }
    } else {
        ComponentPatch {
            fields: Map::from_iter([("__capability_probe__".to_string(), Value::Null)]),
            ..ComponentPatch::default()
        }
    };
    EditorCommand::Patch {
        component_id: "__capability_probe__".to_string(),
        patch,
    }
}

fn asset_command() -> EditorCommand {
    EditorCommand::Asset {
        command: AssetCommand::Remove {
            asset_id: "__capability_probe__".to_string(),
        },
    }
}

fn property_kind(payload: &Value) -> Option<&str> {
    payload.get("kind").and_then(Value::as_str).map(str::trim)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fly_browser::FLY_BROWSER_PROTOCOL;
    use serde_json::json;

    fn envelope(intent: &str, payload: Value) -> BrowserIntentEnvelope {
        BrowserIntentEnvelope {
            protocol: FLY_BROWSER_PROTOCOL.to_string(),
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
        for (intent, kind, capability) in [
            ("save", BrowserIntentKind::Save, EditorCapability::Publish),
            ("undo", BrowserIntentKind::Undo, EditorCapability::History),
            ("copy", BrowserIntentKind::Copy, EditorCapability::Clipboard),
        ] {
            let error =
                validate_browser_capability_access(&envelope(intent, json!({})), capabilities)
                    .expect_err("capability denial");
            assert_eq!(
                browser_capability_denial(&error),
                Some(&BrowserCapabilityDenial {
                    intent: kind,
                    capability,
                    required: vec![capability],
                    missing: vec![capability],
                })
            );
        }
        assert!(
            validate_browser_capability_access(
                &envelope("select", json!({ "component_id": "hero" })),
                capabilities,
            )
            .is_ok()
        );
    }

    #[test]
    fn property_and_style_forms_are_distinguished() {
        let capabilities = CapabilityState {
            properties: true,
            styles: false,
            ..CapabilityState::full()
        };
        assert!(
            validate_browser_capability_access(
                &envelope("patch_component_property", json!({ "kind": "field" }),),
                capabilities,
            )
            .is_ok()
        );
        let error = validate_browser_capability_access(
            &envelope("patch_component_property", json!({ "kind": "style" })),
            capabilities,
        )
        .expect_err("style capability denial");
        assert_eq!(
            browser_capability_denial(&error).map(|denial| denial.capability),
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
                BrowserIntentKind::RenamePage.as_str(),
                json!({ "page_id": "home", "new_page_id": "landing" }),
            ),
            capabilities,
        )
        .expect_err("page rename is a PageCommand::Patch");
        let denial = browser_capability_denial(&error).expect("typed denial");
        assert_eq!(denial.intent, BrowserIntentKind::RenamePage);
        assert_eq!(denial.capability, EditorCapability::Properties);
    }

    #[test]
    fn runtime_preview_context_uses_properties_capability() {
        let capabilities = CapabilityState {
            properties: false,
            ..CapabilityState::full()
        };
        for kind in [
            BrowserIntentKind::SetRuntimeContext,
            BrowserIntentKind::SetRuntimeLocale,
        ] {
            let error = validate_browser_capability_access(
                &envelope(kind.as_str(), json!({})),
                capabilities,
            )
            .expect_err("runtime preview properties denial");
            let denial = browser_capability_denial(&error).expect("typed denial");
            assert_eq!(denial.intent, kind);
            assert_eq!(denial.capability, EditorCapability::Properties);
        }
    }

    #[test]
    fn selecting_an_asset_requires_asset_and_property_capabilities() {
        let missing_properties = CapabilityState {
            properties: false,
            ..CapabilityState::full()
        };
        let error = validate_browser_capability_access(
            &envelope(
                BrowserIntentKind::SelectAsset.as_str(),
                json!({ "asset_id": "logo" }),
            ),
            missing_properties,
        )
        .expect_err("asset application changes component properties");
        let denial = browser_capability_denial(&error).expect("typed denial");
        assert_eq!(denial.intent, BrowserIntentKind::SelectAsset);
        assert_eq!(denial.capability, EditorCapability::Properties);
        assert_eq!(
            denial.required,
            vec![EditorCapability::Properties, EditorCapability::Assets]
        );
        assert_eq!(denial.missing, vec![EditorCapability::Properties]);

        let missing_assets = CapabilityState {
            assets: false,
            ..CapabilityState::full()
        };
        let error = validate_browser_capability_access(
            &envelope(
                BrowserIntentKind::SelectAsset.as_str(),
                json!({ "asset_id": "logo" }),
            ),
            missing_assets,
        )
        .expect_err("asset access is required before application");
        assert_eq!(
            browser_capability_denial(&error).map(|denial| denial.capability),
            Some(EditorCapability::Assets)
        );

        let missing_both = CapabilityState {
            properties: false,
            assets: false,
            ..CapabilityState::full()
        };
        let error = validate_browser_capability_access(
            &envelope(
                BrowserIntentKind::SelectAsset.as_str(),
                json!({ "asset_id": "logo" }),
            ),
            missing_both,
        )
        .expect_err("all missing capabilities are returned");
        assert_eq!(
            browser_capability_denial(&error).map(|denial| denial.missing.clone()),
            Some(vec![EditorCapability::Properties, EditorCapability::Assets])
        );
    }

    #[test]
    fn drag_and_drop_intents_require_drag_capability() {
        let capabilities = CapabilityState {
            drag_drop: false,
            ..CapabilityState::full()
        };
        for kind in [
            BrowserIntentKind::BeginPaletteDrag,
            BrowserIntentKind::BeginSelectedMove,
            BrowserIntentKind::Drop,
        ] {
            assert!(
                validate_browser_capability_access(
                    &envelope(kind.as_str(), json!({})),
                    capabilities,
                )
                .is_err()
            );
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
                BrowserIntentKind::KeyStroke.as_str(),
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
        let denial = browser_capability_denial(&error).expect("typed denial");
        assert_eq!(denial.intent, BrowserIntentKind::KeyStroke);
        assert_eq!(denial.capability, EditorCapability::Publish);
    }

    #[test]
    fn malformed_shortcut_remains_a_typed_dispatch_error() {
        let error = validate_browser_capability_access(
            &envelope(
                BrowserIntentKind::KeyStroke.as_str(),
                json!({ "stroke": "invalid" }),
            ),
            CapabilityState::full(),
        )
        .expect_err("malformed shortcut");
        assert!(matches!(
            error,
            BrowserCapabilityAccessError::Dispatch(BrowserIntentDispatchError::Payload(_))
        ));
        assert!(browser_capability_denial(&error).is_none());
    }

    #[test]
    fn supplied_profile_is_authoritative() {
        let capabilities = CapabilityState {
            edit: false,
            ..CapabilityState::full()
        }
        .normalized();
        let error = validate_browser_capability_access(
            &envelope(
                BrowserIntentKind::InsertBlock.as_str(),
                json!({ "block_id": "text" }),
            ),
            capabilities,
        )
        .expect_err("edit capability denial");
        let denial = browser_capability_denial(&error).expect("typed denial");
        assert_eq!(denial.intent, BrowserIntentKind::InsertBlock);
        assert_eq!(denial.capability, EditorCapability::Edit);
    }
}
