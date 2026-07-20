use crate::editor::{
    SsrDropRequest, SsrInternalPageLinkRemoveRequest, SsrInternalPageLinkRequest,
    SsrLocalePolicyRequest, SsrLocalizedPageMetadataRequest,
};
use crate::{AdminCanvasController, AdminCanvasEffect, AdminCanvasError};
use fly::{GrapesJsCodec, ProjectHash};
use fly_browser::{BrowserIntentEnvelope, BrowserIntentError};
use fly_ui::{EditorShortcut, KeyStroke, UiIntent, resolve_editor_shortcut};
use rustok_page_builder::dto::PageBuilderCapabilityRequest;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BrowserIntentDispatchResult {
    pub page_id: String,
    pub revision_id: String,
    pub project_hash: String,
    pub command_sequence: u64,
    pub dirty: bool,
    pub selected_component_id: Option<String>,
    pub project_data: Value,
    #[serde(default)]
    pub effects: Vec<BrowserIntentEffect>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BrowserIntentEffect {
    Request {
        request: PageBuilderCapabilityRequest,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expected_hash: Option<String>,
        command_sequence: u64,
    },
    Announce {
        message: String,
    },
}

pub fn dispatch_browser_intent(
    controller: &mut AdminCanvasController,
    envelope: BrowserIntentEnvelope,
) -> Result<BrowserIntentDispatchResult, BrowserIntentDispatchError> {
    let envelope = envelope.normalized()?;
    validate_revision(controller, &envelope)?;
    apply_selection_hint(controller, &envelope.payload)?;
    let effects = dispatch_named_intent(controller, &envelope.intent, &envelope.payload)?;
    result(controller, effects)
}

fn dispatch_named_intent(
    controller: &mut AdminCanvasController,
    intent: &str,
    payload: &Value,
) -> Result<Vec<AdminCanvasEffect>, BrowserIntentDispatchError> {
    let effects = match intent {
        "select" | "focus_requested" => {
            controller.dispatch(UiIntent::Select(optional_component_id(payload)))?
        }
        "hover" | "hover_requested" => {
            controller.dispatch(UiIntent::Hover(optional_component_id(payload)))?
        }
        "insert_block" => {
            let block_id = required_string(payload, "block_id")?;
            let intent = controller
                .insert_palette_block_intent(block_id)
                .map_err(BrowserIntentDispatchError::Authoring)?;
            controller.dispatch(intent)?
        }
        "drop" => {
            let request = serde_json::from_value::<SsrDropRequest>(payload.clone())
                .map_err(|error| BrowserIntentDispatchError::Payload(error.to_string()))?;
            let intent = controller
                .ssr_drop_intent(request)
                .map_err(BrowserIntentDispatchError::Authoring)?;
            controller.dispatch(intent)?
        }
        "set_locale_policy" => {
            let request = serde_json::from_value::<SsrLocalePolicyRequest>(payload.clone())
                .map_err(|error| BrowserIntentDispatchError::Payload(error.to_string()))?;
            let intent = controller
                .ssr_locale_policy_intent(request)
                .map_err(BrowserIntentDispatchError::Authoring)?;
            controller.dispatch(intent)?
        }
        "clear_locale_policy" => match controller.ssr_clear_locale_policy_intent() {
            Some(intent) => controller.dispatch(intent)?,
            None => Vec::new(),
        },
        "upsert_localized_page_metadata" => {
            let request =
                serde_json::from_value::<SsrLocalizedPageMetadataRequest>(payload.clone())
                    .map_err(|error| BrowserIntentDispatchError::Payload(error.to_string()))?;
            let intent = controller
                .ssr_localized_page_metadata_intent(request)
                .map_err(BrowserIntentDispatchError::Authoring)?;
            controller.dispatch(intent)?
        }
        "set_internal_page_link" => {
            let request = serde_json::from_value::<SsrInternalPageLinkRequest>(payload.clone())
                .map_err(|error| BrowserIntentDispatchError::Payload(error.to_string()))?;
            let intent = controller
                .ssr_internal_page_link_intent(request)
                .map_err(BrowserIntentDispatchError::Authoring)?;
            controller.dispatch(intent)?
        }
        "remove_internal_page_link" => {
            let request =
                serde_json::from_value::<SsrInternalPageLinkRemoveRequest>(payload.clone())
                    .map_err(|error| BrowserIntentDispatchError::Payload(error.to_string()))?;
            let intent = controller
                .ssr_remove_internal_page_link_intent(request)
                .map_err(BrowserIntentDispatchError::Authoring)?;
            controller.dispatch(intent)?
        }
        "begin_palette_drag" => {
            let block_id = required_string(payload, "block_id")?;
            let intent = controller
                .begin_palette_drag_intent(block_id)
                .map_err(BrowserIntentDispatchError::Authoring)?;
            controller.dispatch(intent)?
        }
        "begin_selected_move" => {
            let intent = controller
                .begin_selected_move_intent()
                .map_err(BrowserIntentDispatchError::Authoring)?;
            controller.dispatch(intent)?
        }
        "remove_selected" => {
            let intent = controller
                .remove_selected_intent()
                .map_err(BrowserIntentDispatchError::Authoring)?;
            controller.dispatch(intent)?
        }
        "move_selected_up" => {
            let intent = controller
                .move_selected_up_intent()
                .map_err(BrowserIntentDispatchError::Authoring)?;
            controller.dispatch(intent)?
        }
        "move_selected_down" => {
            let intent = controller
                .move_selected_down_intent()
                .map_err(BrowserIntentDispatchError::Authoring)?;
            controller.dispatch(intent)?
        }
        "undo" => controller.dispatch(UiIntent::Undo)?,
        "redo" => controller.dispatch(UiIntent::Redo)?,
        "copy" => controller.dispatch(UiIntent::CopySelection)?,
        "cut" => controller.dispatch(UiIntent::CutSelection)?,
        "paste" => controller.dispatch(UiIntent::PasteClipboard)?,
        "duplicate" => {
            let mut effects = controller.dispatch(UiIntent::CopySelection)?;
            effects.extend(controller.dispatch(UiIntent::PasteClipboard)?);
            effects
        }
        "cancel_drag" | "cancel_drag_requested" => controller.dispatch(UiIntent::CancelDrag)?,
        "save" => controller.dispatch(UiIntent::RequestSave)?,
        "activate_page" => {
            let page_index = integer_field(payload, "page_index")? as usize;
            let page_id = payload
                .get("page_id")
                .and_then(Value::as_str)
                .map(ToString::to_string);
            controller.dispatch(UiIntent::ActivatePage {
                page_id,
                page_index,
            })?
        }
        "key_stroke" => {
            let stroke_value = payload
                .get("stroke")
                .cloned()
                .unwrap_or_else(|| payload.clone());
            let stroke = serde_json::from_value::<KeyStroke>(stroke_value)
                .map_err(|error| BrowserIntentDispatchError::Payload(error.to_string()))?;
            dispatch_shortcut(controller, stroke)?
        }
        "drop_requested" | "drag_moved" => {
            return Err(BrowserIntentDispatchError::GeometryRequired(
                intent.to_string(),
            ));
        }
        other => match controller
            .ssr_form_intent(other, payload)
            .map_err(BrowserIntentDispatchError::Authoring)?
        {
            Some(intent) => controller.dispatch(intent)?,
            None => return Err(BrowserIntentDispatchError::Unsupported(other.to_string())),
        },
    };
    Ok(effects)
}

fn dispatch_shortcut(
    controller: &mut AdminCanvasController,
    stroke: KeyStroke,
) -> Result<Vec<AdminCanvasEffect>, BrowserIntentDispatchError> {
    let shortcut = resolve_editor_shortcut(&stroke)
        .ok_or_else(|| BrowserIntentDispatchError::Unsupported("key_stroke".to_string()))?;
    match shortcut {
        EditorShortcut::Undo => Ok(controller.dispatch(UiIntent::Undo)?),
        EditorShortcut::Redo => Ok(controller.dispatch(UiIntent::Redo)?),
        EditorShortcut::Save => Ok(controller.dispatch(UiIntent::RequestSave)?),
        EditorShortcut::Copy => Ok(controller.dispatch(UiIntent::CopySelection)?),
        EditorShortcut::Cut => Ok(controller.dispatch(UiIntent::CutSelection)?),
        EditorShortcut::Paste => Ok(controller.dispatch(UiIntent::PasteClipboard)?),
        EditorShortcut::Duplicate => {
            let mut effects = controller.dispatch(UiIntent::CopySelection)?;
            effects.extend(controller.dispatch(UiIntent::PasteClipboard)?);
            Ok(effects)
        }
        EditorShortcut::DeleteSelection => {
            let intent = controller
                .remove_selected_intent()
                .map_err(BrowserIntentDispatchError::Authoring)?;
            Ok(controller.dispatch(intent)?)
        }
        EditorShortcut::Cancel => Ok(controller.dispatch(UiIntent::CancelDrag)?),
        EditorShortcut::MoveSelectionUp => {
            let intent = controller
                .move_selected_up_intent()
                .map_err(BrowserIntentDispatchError::Authoring)?;
            Ok(controller.dispatch(intent)?)
        }
        EditorShortcut::MoveSelectionDown => {
            let intent = controller
                .move_selected_down_intent()
                .map_err(BrowserIntentDispatchError::Authoring)?;
            Ok(controller.dispatch(intent)?)
        }
    }
}

fn validate_revision(
    controller: &AdminCanvasController,
    envelope: &BrowserIntentEnvelope,
) -> Result<(), BrowserIntentDispatchError> {
    if !is_mutating_intent(envelope) {
        return Ok(());
    }
    if let Some(revision) = envelope.revision.as_deref() {
        if revision != controller.revision_id() {
            return Err(BrowserIntentDispatchError::RevisionConflict {
                expected: controller.revision_id().to_string(),
                actual: revision.to_string(),
            });
        }
    }
    if let Some(project_hash) = envelope.project_hash.as_deref() {
        let expected = controller.editor().revision().project_hash.hex();
        if project_hash != expected {
            return Err(BrowserIntentDispatchError::ProjectHashConflict {
                expected,
                actual: project_hash.to_string(),
            });
        }
    }
    Ok(())
}

fn is_mutating_intent(envelope: &BrowserIntentEnvelope) -> bool {
    envelope.is_mutating()
        || matches!(
            envelope.intent.as_str(),
            "patch_component_property"
                | "patch_page_metadata"
                | "set_locale_policy"
                | "clear_locale_policy"
                | "upsert_localized_page_metadata"
                | "set_internal_page_link"
                | "remove_internal_page_link"
                | "create_page"
                | "rename_page"
                | "remove_page"
        )
}

fn apply_selection_hint(
    controller: &mut AdminCanvasController,
    payload: &Value,
) -> Result<(), BrowserIntentDispatchError> {
    let Some(component_id) = payload
        .get("selected_component_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|component_id| !component_id.is_empty())
    else {
        return Ok(());
    };
    controller.dispatch(UiIntent::Select(Some(component_id.to_string())))?;
    Ok(())
}

fn result(
    controller: &AdminCanvasController,
    effects: Vec<AdminCanvasEffect>,
) -> Result<BrowserIntentDispatchResult, BrowserIntentDispatchError> {
    let revision = controller.editor().revision();
    Ok(BrowserIntentDispatchResult {
        page_id: controller.page_id().to_string(),
        revision_id: controller.revision_id().to_string(),
        project_hash: revision.project_hash.hex(),
        command_sequence: revision.command_sequence,
        dirty: revision.dirty,
        selected_component_id: controller.ui().state.selection.component_id.clone(),
        project_data: GrapesJsCodec::encode_value(controller.editor().document())?,
        effects: effects.into_iter().map(effect).collect(),
    })
}

fn effect(effect: AdminCanvasEffect) -> BrowserIntentEffect {
    match effect {
        AdminCanvasEffect::Request {
            request,
            expected_hash,
            command_sequence,
        } => BrowserIntentEffect::Request {
            request,
            expected_hash: expected_hash.map(ProjectHash::hex),
            command_sequence,
        },
        AdminCanvasEffect::Announce(message) => BrowserIntentEffect::Announce { message },
    }
}

fn optional_component_id(payload: &Value) -> Option<String> {
    payload
        .get("component_id")
        .or_else(|| payload.get("selected_component_id"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn required_string<'a>(
    payload: &'a Value,
    field: &str,
) -> Result<&'a str, BrowserIntentDispatchError> {
    payload
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| BrowserIntentDispatchError::MissingField(field.to_string()))
}

fn integer_field(payload: &Value, field: &str) -> Result<u64, BrowserIntentDispatchError> {
    payload
        .get(field)
        .and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
        })
        .ok_or_else(|| BrowserIntentDispatchError::MissingField(field.to_string()))
}

#[derive(Debug, thiserror::Error)]
pub enum BrowserIntentDispatchError {
    #[error(transparent)]
    Envelope(#[from] BrowserIntentError),
    #[error(transparent)]
    Canvas(#[from] AdminCanvasError),
    #[error(transparent)]
    Fly(#[from] fly::FlyError),
    #[error("browser intent payload is invalid: {0}")]
    Payload(String),
    #[error("browser intent is missing field `{0}`")]
    MissingField(String),
    #[error("unsupported browser intent `{0}`")]
    Unsupported(String),
    #[error("browser intent `{0}` requires geometry-resolved hit-test state")]
    GeometryRequired(String),
    #[error("browser authoring intent failed: {0}")]
    Authoring(String),
    #[error("browser revision conflict: expected `{expected}`, received `{actual}`")]
    RevisionConflict { expected: String, actual: String },
    #[error("browser project hash conflict: expected `{expected}`, received `{actual}`")]
    ProjectHashConflict { expected: String, actual: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use fly_browser::FLY_BROWSER_PROTOCOL;
    use serde_json::json;

    fn controller() -> AdminCanvasController {
        AdminCanvasController::new(
            "home",
            "rev-1",
            json!({
                "pages": [{
                    "id": "home",
                    "component": {
                        "id": "root",
                        "type": "wrapper",
                        "components": [
                            { "id": "hero", "type": "section" },
                            { "id": "link", "type": "link" }
                        ]
                    }
                }, {
                    "id": "about",
                    "flyPageMeta": { "slug": "about" },
                    "component": { "id": "about-root", "type": "wrapper" }
                }]
            }),
        )
        .expect("controller")
    }

    fn intent(intent: &str, payload: Value) -> BrowserIntentEnvelope {
        BrowserIntentEnvelope {
            protocol: FLY_BROWSER_PROTOCOL.to_string(),
            instance_id: "canvas-a".to_string(),
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
    fn server_dispatch_selects_and_inserts_without_wasm() {
        let mut controller = controller();
        let selected = dispatch_browser_intent(
            &mut controller,
            intent("select", json!({ "component_id": "hero" })),
        )
        .expect("select");
        assert_eq!(selected.selected_component_id.as_deref(), Some("hero"));
        let inserted = dispatch_browser_intent(
            &mut controller,
            intent(
                "insert_block",
                json!({ "block_id": "text", "selected_component_id": "hero" }),
            ),
        )
        .expect("insert");
        assert!(inserted.dirty);
    }

    #[test]
    fn stateless_drop_dispatches_without_prior_drag_state() {
        let mut controller = controller();
        let result = dispatch_browser_intent(
            &mut controller,
            intent(
                "drop",
                json!({
                    "source": { "kind": "block", "block_id": "text" },
                    "target_component_id": "hero",
                    "position": "inside"
                }),
            ),
        )
        .expect("drop");
        assert!(result.dirty);
        assert_eq!(
            controller.editor().document().component_child_count("hero"),
            Some(1)
        );
    }

    #[test]
    fn no_hydration_property_form_uses_normal_patch_history() {
        let mut controller = controller();
        let result = dispatch_browser_intent(
            &mut controller,
            intent(
                "patch_component_property",
                json!({
                    "component_id": "hero",
                    "kind": "attribute",
                    "name": "aria-label",
                    "value": "Hero section",
                    "remove": false
                }),
            ),
        )
        .expect("form patch");
        assert!(result.dirty);
        assert_eq!(
            controller
                .editor()
                .document()
                .component("hero")
                .unwrap()
                .attributes["aria-label"],
            "Hero section"
        );
    }

    #[test]
    fn locale_policy_form_uses_revision_protected_translation_history() {
        let mut controller = controller();
        let result = dispatch_browser_intent(
            &mut controller,
            intent(
                "set_locale_policy",
                json!({
                    "default_locale": "ru",
                    "supported_locales": "ru, en",
                    "required_locales": "ru, en",
                    "fallback_locales": "en",
                    "enforce_required_locales": false
                }),
            ),
        )
        .expect("locale policy");
        assert!(result.dirty);
        assert_eq!(
            controller.editor().document().project.extensions["flyLocales"]["default_locale"],
            "ru"
        );
    }

    #[test]
    fn clearing_missing_locale_policy_is_a_clean_no_op() {
        let mut controller = controller();
        let result =
            dispatch_browser_intent(&mut controller, intent("clear_locale_policy", json!({})))
                .expect("clear missing locale policy");
        assert!(!result.dirty);
        assert_eq!(controller.editor().history().undo_len(), 0);
    }

    #[test]
    fn localized_metadata_form_uses_revision_protected_page_history() {
        let mut controller = controller();
        let result = dispatch_browser_intent(
            &mut controller,
            intent(
                "upsert_localized_page_metadata",
                json!({
                    "page_id": "home",
                    "metadata_json": "{\"title\":{\"en\":\"Home\",\"ru\":\"Главная\"}}",
                    "fallback_locale": "en"
                }),
            ),
        )
        .expect("localized metadata");
        assert!(result.dirty);
        assert!(
            controller.editor().document().project.pages[0].extensions["flyPageMeta"]["title"]
                ["$localized"]
                .is_object()
        );
    }

    #[test]
    fn internal_page_link_form_uses_revision_protected_patch_history() {
        let mut controller = controller();
        let result = dispatch_browser_intent(
            &mut controller,
            intent(
                "set_internal_page_link",
                json!({
                    "component_id": "link",
                    "page_id": "about",
                    "base_path": "/site",
                    "query": "source=hero",
                    "fragment": "team",
                    "fallback_href": "/fallback"
                }),
            ),
        )
        .expect("internal page link");
        assert!(result.dirty);
        let link = &controller
            .editor()
            .document()
            .component("link")
            .unwrap()
            .extensions["flyPageLink"];
        assert_eq!(link["page_id"], "about");
        assert_eq!(link["base_path"], "/site");
        controller.dispatch(UiIntent::Undo).expect("undo link");
        assert!(
            !controller
                .editor()
                .document()
                .component("link")
                .unwrap()
                .extensions
                .contains_key("flyPageLink")
        );
    }

    #[test]
    fn stale_mutation_is_rejected_before_command_dispatch() {
        let mut request = intent(
            "remove_selected",
            json!({ "selected_component_id": "hero" }),
        );
        request.revision = Some("old".to_string());
        let error =
            dispatch_browser_intent(&mut controller(), request).expect_err("revision conflict");
        assert!(matches!(
            error,
            BrowserIntentDispatchError::RevisionConflict { .. }
        ));
    }
}
