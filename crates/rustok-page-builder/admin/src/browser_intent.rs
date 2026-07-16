use crate::{AdminCanvasController, AdminCanvasEffect, AdminCanvasError};
use fly::{GrapesJsV1Codec, ProjectHash};
use fly_browser::{BrowserIntentEnvelope, BrowserIntentError};
use fly_ui::{resolve_editor_shortcut, EditorShortcut, KeyStroke, UiIntent};
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
    Announce { message: String },
}

pub fn dispatch_browser_intent(
    controller: &mut AdminCanvasController,
    envelope: BrowserIntentEnvelope,
) -> Result<BrowserIntentDispatchResult, BrowserIntentDispatchError> {
    let envelope = envelope.normalized()?;
    validate_revision(controller, &envelope)?;
    apply_selection_hint(controller, &envelope.payload)?;
    let effects = match envelope.intent.as_str() {
        "select" | "focus_requested" => controller.dispatch(UiIntent::Select(
            optional_component_id(&envelope.payload),
        ))?,
        "hover" | "hover_requested" => controller.dispatch(UiIntent::Hover(
            optional_component_id(&envelope.payload),
        ))?,
        "insert_block" => {
            let block_id = required_string(&envelope.payload, "block_id")?;
            let intent = controller
                .insert_palette_block_intent(block_id)
                .map_err(BrowserIntentDispatchError::Authoring)?;
            controller.dispatch(intent)?
        }
        "begin_palette_drag" => {
            let block_id = required_string(&envelope.payload, "block_id")?;
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
        "cancel_drag" | "cancel_drag_requested" => controller.dispatch(UiIntent::CancelDrag)?,
        "save" => controller.dispatch(UiIntent::RequestSave)?,
        "activate_page" => {
            let page_index = envelope
                .payload
                .get("page_index")
                .and_then(Value::as_u64)
                .ok_or_else(|| BrowserIntentDispatchError::MissingField("page_index".to_string()))?
                as usize;
            let page_id = envelope
                .payload
                .get("page_id")
                .and_then(Value::as_str)
                .map(ToString::to_string);
            controller.dispatch(UiIntent::ActivatePage {
                page_id,
                page_index,
            })?
        }
        "key_stroke" => {
            let stroke_value = envelope
                .payload
                .get("stroke")
                .cloned()
                .unwrap_or_else(|| envelope.payload.clone());
            let stroke = serde_json::from_value::<KeyStroke>(stroke_value)
                .map_err(|error| BrowserIntentDispatchError::Payload(error.to_string()))?;
            dispatch_shortcut(controller, stroke)?
        }
        "drop_requested" | "drag_moved" => {
            return Err(BrowserIntentDispatchError::GeometryRequired(
                envelope.intent,
            ));
        }
        intent => return Err(BrowserIntentDispatchError::Unsupported(intent.to_string())),
    };

    result(controller, effects)
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
    if !envelope.is_mutating() {
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
        project_data: GrapesJsV1Codec::encode_value(controller.editor().document())?,
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
    #[error("browser intent is missing field `{0}")]
    MissingField(String),
    #[error("unsupported browser intent `{0}")]
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
    use fly_browser::FLY_BROWSER_PROTOCOL_V1;
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
                        "components": [{ "id": "hero", "type": "section" }]
                    }
                }]
            }),
        )
        .expect("controller")
    }

    fn intent(intent: &str, payload: Value) -> BrowserIntentEnvelope {
        BrowserIntentEnvelope {
            protocol: FLY_BROWSER_PROTOCOL_V1.to_string(),
            instance_id: "canvas-a".to_string(),
            intent: intent.to_string(),
            payload,
            sequence: Some(1),
            page_id: Some("home".to_string()),
            revision: Some("rev-1".to_string()),
            project_hash: None,
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
        assert!(inserted.command_sequence > 0);
    }

    #[test]
    fn nested_iframe_key_stroke_payload_is_accepted() {
        let mut controller = controller();
        controller
            .dispatch(UiIntent::Select(Some("hero".to_string())))
            .unwrap();
        let result = dispatch_browser_intent(
            &mut controller,
            intent(
                "key_stroke",
                json!({
                    "stroke": {
                        "key": "Delete",
                        "code": "Delete",
                        "ctrl": false,
                        "meta": false,
                        "shift": false,
                        "alt": false,
                        "repeat": false
                    }
                }),
            ),
        )
        .expect("delete shortcut");
        assert!(result.dirty);
    }

    #[test]
    fn stale_mutation_is_rejected_before_command_dispatch() {
        let mut request = intent("remove_selected", json!({ "selected_component_id": "hero" }));
        request.revision = Some("old".to_string());
        let error = dispatch_browser_intent(&mut controller(), request)
            .expect_err("revision conflict");
        assert!(matches!(error, BrowserIntentDispatchError::RevisionConflict { .. }));
    }

    #[test]
    fn save_returns_canonical_consumer_request_effect() {
        let mut controller = controller();
        dispatch_browser_intent(
            &mut controller,
            intent(
                "insert_block",
                json!({ "block_id": "text", "selected_component_id": "hero" }),
            ),
        )
        .expect("mutation");
        let mut save = intent("save", json!({}));
        save.project_hash = Some(controller.editor().revision().project_hash.hex());
        let result = dispatch_browser_intent(&mut controller, save).expect("save");
        assert!(matches!(
            result.effects.first(),
            Some(BrowserIntentEffect::Request { .. })
        ));
    }
}
