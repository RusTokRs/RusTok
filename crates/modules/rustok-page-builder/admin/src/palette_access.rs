use crate::AdminCanvasController;
use crate::browser_intent::{
    BrowserIntentDispatchError, BrowserIntentDispatchResult, dispatch_browser_intent,
};
use crate::editor::PaletteBlockView;
use fly_browser::BrowserIntentEnvelope;
use fly_ui::{PaletteBlockAccess, UiIntent};
use serde_json::Value;

impl AdminCanvasController {
    pub fn palette_blocks_with_access(&self, access: &PaletteBlockAccess) -> Vec<PaletteBlockView> {
        self.palette_blocks()
            .into_iter()
            .filter(|block| access.allows(&block.id))
            .collect()
    }

    pub fn palette_block_with_access(
        &self,
        block_id: &str,
        access: &PaletteBlockAccess,
    ) -> Option<PaletteBlockView> {
        access
            .allows(block_id)
            .then(|| self.palette_block(block_id))
            .flatten()
    }

    pub fn begin_palette_drag_intent_with_access(
        &self,
        block_id: &str,
        access: &PaletteBlockAccess,
    ) -> Result<UiIntent, String> {
        require_palette_access(block_id, access)?;
        self.begin_palette_drag_intent(block_id)
    }

    pub fn insert_palette_block_intent_with_access(
        &self,
        block_id: &str,
        access: &PaletteBlockAccess,
    ) -> Result<UiIntent, String> {
        require_palette_access(block_id, access)?;
        self.insert_palette_block_intent(block_id)
    }
}

pub fn dispatch_browser_intent_with_palette_access(
    controller: &mut AdminCanvasController,
    envelope: BrowserIntentEnvelope,
    access: &PaletteBlockAccess,
) -> Result<BrowserIntentDispatchResult, BrowserIntentDispatchError> {
    let envelope = envelope
        .normalized()
        .map_err(BrowserIntentDispatchError::from)?;
    validate_browser_palette_access(&envelope, access)?;
    dispatch_browser_intent(controller, envelope)
}

pub fn validate_browser_palette_access(
    envelope: &BrowserIntentEnvelope,
    access: &PaletteBlockAccess,
) -> Result<(), BrowserIntentDispatchError> {
    let block_id = match envelope.intent.as_str() {
        "insert_block" | "begin_palette_drag" => {
            Some(required_payload_string(&envelope.payload, "block_id")?)
        }
        "drop" => block_drop_id(&envelope.payload)?,
        _ => None,
    };
    if let Some(block_id) = block_id {
        require_palette_access(block_id, access).map_err(BrowserIntentDispatchError::Authoring)?;
    }
    Ok(())
}

fn block_drop_id(payload: &Value) -> Result<Option<&str>, BrowserIntentDispatchError> {
    let Some(source) = payload.get("source") else {
        return Ok(None);
    };
    let Some(kind) = source.get("kind").and_then(Value::as_str) else {
        return Ok(None);
    };
    if kind.trim() != "block" {
        return Ok(None);
    }
    required_payload_string(source, "block_id").map(Some)
}

fn required_payload_string<'a>(
    payload: &'a Value,
    field: &str,
) -> Result<&'a str, BrowserIntentDispatchError> {
    payload
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| BrowserIntentDispatchError::MissingField(field.to_string()))
}

fn require_palette_access(block_id: &str, access: &PaletteBlockAccess) -> Result<(), String> {
    if access.allows(block_id) {
        Ok(())
    } else {
        Err(format!(
            "palette block `{}` is unavailable for the active contribution assembly",
            block_id.trim()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fly_ui::{ContributionAssemblyResult, ContributionDescriptor, ContributionRegistry};
    use serde_json::{Map, json};
    use std::collections::{BTreeMap, BTreeSet};

    fn controller() -> AdminCanvasController {
        AdminCanvasController::new(
            "home",
            "rev-1",
            json!({
                "pages": [{
                    "id": "home",
                    "component": { "id": "root", "type": "wrapper" }
                }]
            }),
        )
        .expect("controller")
    }

    fn access() -> PaletteBlockAccess {
        let mut registry = ContributionRegistry::default();
        registry
            .register(ContributionDescriptor {
                id: "pages.blocks".to_string(),
                provider: "fly.builtin".to_string(),
                required_capabilities: BTreeSet::new(),
                blocks: vec!["fly.hero".to_string()],
                renderers: Vec::new(),
                property_editors: Vec::new(),
                messages: BTreeMap::new(),
                metadata: Map::new(),
            })
            .expect("registry");
        PaletteBlockAccess::from_assembly(&ContributionAssemblyResult {
            registry,
            registered_contributions: 1,
            ..ContributionAssemblyResult::default()
        })
    }

    fn envelope(intent: &str, payload: Value) -> BrowserIntentEnvelope {
        BrowserIntentEnvelope {
            protocol: fly_browser::FLY_BROWSER_PROTOCOL.to_string(),
            instance_id: "test".to_string(),
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
    fn namespaced_blocks_are_filtered_for_ui_and_controller_helpers() {
        let controller = controller();
        let access = access();
        assert!(
            controller
                .palette_block_with_access("text", &access)
                .is_some()
        );
        assert!(
            controller
                .palette_block_with_access("fly.hero", &access)
                .is_some()
        );
        assert!(
            controller
                .palette_block_with_access("fly.cta", &access)
                .is_none()
        );
        assert!(
            controller
                .insert_palette_block_intent_with_access("fly.cta", &access)
                .is_err()
        );
    }

    #[test]
    fn browser_insert_and_drop_cannot_bypass_contribution_filtering() {
        let access = access();
        let mut controller = controller();
        assert!(
            dispatch_browser_intent_with_palette_access(
                &mut controller,
                envelope("insert_block", json!({ "block_id": "fly.cta" })),
                &access,
            )
            .is_err()
        );
        assert!(
            dispatch_browser_intent_with_palette_access(
                &mut controller,
                envelope(
                    "drop",
                    json!({
                        "source": { "kind": "block", "block_id": "fly.cta" },
                        "target_component_id": "root",
                        "position": "inside"
                    }),
                ),
                &access,
            )
            .is_err()
        );
    }

    #[test]
    fn primitive_and_contributed_blocks_remain_available() {
        let access = access();
        let controller = controller();
        assert!(
            controller
                .begin_palette_drag_intent_with_access("text", &access)
                .is_ok()
        );
        assert!(
            controller
                .begin_palette_drag_intent_with_access("fly.hero", &access)
                .is_ok()
        );
    }
}
