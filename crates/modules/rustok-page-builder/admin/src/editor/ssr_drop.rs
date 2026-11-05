use crate::AdminCanvasController;
use fly::EditorCommand;
use fly_ui::{DropPosition, UiIntent};
use serde::{Deserialize, Serialize};

/// Self-contained drag source used by the classic SSR browser adapter.
///
/// Unlike the hydrated/WASM path, an SSR request cannot depend on a previous in-memory
/// `UiState.drag`. The browser therefore sends the source again with the final drop request and
/// Rust re-evaluates the complete placement against the current persisted project.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SsrDropSource {
    Block { block_id: String },
    Component { component_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SsrDropRequest {
    pub source: SsrDropSource,
    pub target_component_id: String,
    pub position: DropPosition,
}

impl AdminCanvasController {
    pub fn ssr_drop_intent(&self, request: SsrDropRequest) -> Result<UiIntent, String> {
        let target_id = request.target_component_id.trim();
        if target_id.is_empty() {
            return Err("drop target component id must not be empty".to_string());
        }
        let document = self.editor().document();
        let target_location = document
            .component_location(target_id)
            .ok_or_else(|| format!("drop target `{target_id}` does not exist"))?;
        if target_location.page_index != self.active_page_index() {
            return Err("drop target is outside the active page".to_string());
        }

        let (parent_id, index) = match request.position {
            DropPosition::Inside => {
                let child_count = document
                    .component_child_count(target_id)
                    .ok_or_else(|| format!("drop target `{target_id}` is opaque"))?;
                (Some(target_id.to_string()), child_count)
            }
            DropPosition::Before => {
                if target_location.depth == 0 {
                    return Err("the page root cannot have a before drop position".to_string());
                }
                (
                    target_location.parent_component_id.clone(),
                    target_location.index,
                )
            }
            DropPosition::After => {
                if target_location.depth == 0 {
                    return Err("the page root cannot have an after drop position".to_string());
                }
                (
                    target_location.parent_component_id.clone(),
                    target_location.index.saturating_add(1),
                )
            }
        };

        match request.source {
            SsrDropSource::Block { block_id } => {
                let block = self
                    .palette_block(block_id.trim())
                    .ok_or_else(|| format!("palette block `{block_id}` is not registered"))?;
                let child_type = block
                    .component
                    .as_object()
                    .map(|component| component.component_type().to_string())
                    .ok_or_else(|| format!("palette block `{block_id}` is opaque"))?;
                let decision = self.editor().registries().evaluate_placement(
                    document,
                    None,
                    &child_type,
                    parent_id.as_deref(),
                    index,
                );
                if !decision.legal {
                    return Err(decision
                        .reason
                        .unwrap_or_else(|| "palette drop was rejected".to_string()));
                }
                Ok(UiIntent::execute(EditorCommand::Insert {
                    parent_id,
                    index,
                    component: block.component,
                }))
            }
            SsrDropSource::Component { component_id } => {
                let component_id = component_id.trim();
                let source_location = document
                    .component_location(component_id)
                    .ok_or_else(|| format!("moving component `{component_id}` does not exist"))?;
                if source_location.page_index != self.active_page_index() {
                    return Err("moving component is outside the active page".to_string());
                }
                if source_location.depth == 0 {
                    return Err("the page root cannot be moved".to_string());
                }
                let child_type = document
                    .component_type_for_id(component_id)
                    .ok_or_else(|| format!("moving component `{component_id}` is opaque"))?;
                let decision = self.editor().registries().evaluate_placement(
                    document,
                    Some(component_id),
                    child_type,
                    parent_id.as_deref(),
                    index,
                );
                if !decision.legal {
                    return Err(decision
                        .reason
                        .unwrap_or_else(|| "component move was rejected".to_string()));
                }
                Ok(UiIntent::execute(EditorCommand::Move {
                    component_id: component_id.to_string(),
                    new_parent_id: parent_id,
                    index,
                }))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AdminCanvasController;
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
                            { "id": "first", "type": "section" },
                            { "id": "second", "type": "section" }
                        ]
                    }
                }]
            }),
        )
        .expect("controller")
    }

    #[test]
    fn block_drop_is_stateless_and_registry_checked() {
        let mut controller = controller();
        let intent = controller
            .ssr_drop_intent(SsrDropRequest {
                source: SsrDropSource::Block {
                    block_id: "text".to_string(),
                },
                target_component_id: "first".to_string(),
                position: DropPosition::Inside,
            })
            .expect("drop intent");
        controller.dispatch(intent).expect("drop command");
        assert_eq!(
            controller
                .editor()
                .document()
                .component_child_count("first"),
            Some(1)
        );
    }

    #[test]
    fn component_drop_moves_without_prior_drag_state() {
        let mut controller = controller();
        let intent = controller
            .ssr_drop_intent(SsrDropRequest {
                source: SsrDropSource::Component {
                    component_id: "second".to_string(),
                },
                target_component_id: "first".to_string(),
                position: DropPosition::Before,
            })
            .expect("move intent");
        controller.dispatch(intent).expect("move command");
        assert_eq!(
            controller
                .editor()
                .document()
                .component_location("second")
                .unwrap()
                .index,
            0
        );
    }

    #[test]
    fn page_root_rejects_before_and_after_drop() {
        let controller = controller();
        assert!(
            controller
                .ssr_drop_intent(SsrDropRequest {
                    source: SsrDropSource::Block {
                        block_id: "text".to_string(),
                    },
                    target_component_id: "root".to_string(),
                    position: DropPosition::Before,
                })
                .is_err()
        );
    }
}
