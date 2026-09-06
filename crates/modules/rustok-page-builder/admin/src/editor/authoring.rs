use crate::AdminCanvasController;
use fly::{ComponentNode, ComponentObject, EditorCommand};
use fly_leptos::{
    BrowserDropTarget, BrowserPoint, CoordinateTransform, DropAxis, DropZonePolicy,
    hit_test_drop_targets,
};
use fly_ui::{DragSource, DropPosition, HitTestCandidate, UiIntent};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Geometry reported by the isolated canvas and consumed by the editor's
/// platform-neutral hit-testing policy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CanvasComponentGeometry {
    pub component_id: String,
    pub parent_component_id: Option<String>,
    pub index: usize,
    pub rect: fly_leptos::BrowserRect,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PaletteBlockView {
    pub id: String,
    pub label: String,
    pub category: String,
    pub component: ComponentNode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerItemView {
    pub id: String,
    pub component_type: String,
    pub depth: usize,
    pub parent_component_id: Option<String>,
    pub index: usize,
    pub child_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectedComponentView {
    pub id: String,
    pub component_type: String,
    pub tag_name: Option<String>,
    pub attributes: Map<String, Value>,
    pub style: Option<Value>,
    pub fields: Map<String, Value>,
    pub child_count: usize,
    pub is_root: bool,
}

impl AdminCanvasController {
    pub fn palette_blocks(&self) -> Vec<PaletteBlockView> {
        self.editor()
            .registries()
            .blocks
            .iter()
            .map(|(_, block)| PaletteBlockView {
                id: block.id.clone(),
                label: block.label.clone(),
                category: block.category.clone(),
                component: block.component.clone(),
            })
            .collect()
    }

    pub fn palette_block(&self, block_id: &str) -> Option<PaletteBlockView> {
        self.editor()
            .registries()
            .blocks
            .get(block_id)
            .map(|block| PaletteBlockView {
                id: block.id.clone(),
                label: block.label.clone(),
                category: block.category.clone(),
                component: block.component.clone(),
            })
    }

    pub fn layer_items(&self) -> Vec<LayerItemView> {
        let mut items = Vec::new();
        let Some(root) = self
            .editor()
            .document()
            .project
            .pages
            .get(self.active_page_index())
            .and_then(|page| page.component.as_ref())
        else {
            return items;
        };
        collect_layers(root, None, 0, 0, &mut items);
        items
    }

    pub fn selected_component_view(&self) -> Option<SelectedComponentView> {
        let component_id = self.ui().state.selection.component_id.as_deref()?;
        let component = self.editor().document().component(component_id)?;
        let location = self.editor().document().component_location(component_id)?;
        if location.page_index != self.active_page_index() {
            return None;
        }
        Some(SelectedComponentView {
            id: component_id.to_string(),
            component_type: component.component_type().to_string(),
            tag_name: component.tag_name.clone(),
            attributes: component.attributes.clone(),
            style: component.style.clone(),
            fields: component.extensions.clone(),
            child_count: component.children().len(),
            is_root: location.depth == 0,
        })
    }

    pub fn begin_palette_drag_intent(&self, block_id: &str) -> Result<UiIntent, String> {
        let block = self
            .palette_block(block_id)
            .ok_or_else(|| format!("palette block `{block_id}` is not registered"))?;
        Ok(UiIntent::BeginDrag(DragSource::PaletteBlock {
            block_id: block.id,
            component: block.component,
        }))
    }

    pub fn insert_palette_block_intent(&self, block_id: &str) -> Result<UiIntent, String> {
        let block = self
            .palette_block(block_id)
            .ok_or_else(|| format!("palette block `{block_id}` is not registered"))?;
        let child_type = block
            .component
            .as_object()
            .map(|component| component.component_type().to_string())
            .ok_or_else(|| format!("palette block `{block_id}` has an opaque component"))?;
        let document = self.editor().document();
        let registries = self.editor().registries();

        let (parent_id, index) = match self.ui().state.selection.component_id.as_deref() {
            Some(selected_id) => {
                let location = document
                    .component_location(selected_id)
                    .ok_or_else(|| format!("selected component `{selected_id}` has no location"))?;
                if location.page_index != self.active_page_index() {
                    return Err("selected component is outside the active page".to_string());
                }
                let selected = document
                    .component(selected_id)
                    .ok_or_else(|| format!("selected component `{selected_id}` does not exist"))?;
                if registries
                    .accepts_child_type(Some(selected.component_type()), child_type.as_str())
                    || location.depth == 0
                {
                    (Some(selected_id.to_string()), selected.children().len())
                } else {
                    (
                        location.parent_component_id,
                        location.index.saturating_add(1),
                    )
                }
            }
            None => {
                let root_id = self
                    .active_root_id()
                    .ok_or_else(|| "active page has no editable root".to_string())?;
                let child_count = document
                    .component_child_count(&root_id)
                    .ok_or_else(|| "active page root is opaque or missing".to_string())?;
                (Some(root_id), child_count)
            }
        };

        let decision = registries.evaluate_placement(
            document,
            None,
            child_type.as_str(),
            parent_id.as_deref(),
            index,
        );
        if !decision.legal {
            return Err(decision
                .reason
                .unwrap_or_else(|| "palette insertion was rejected".to_string()));
        }

        Ok(UiIntent::execute(EditorCommand::Insert {
            parent_id,
            index,
            component: block.component,
        }))
    }

    pub fn begin_selected_move_intent(&self) -> Result<UiIntent, String> {
        let selected = self
            .selected_component_view()
            .ok_or_else(|| "select a component before moving it".to_string())?;
        if selected.is_root {
            return Err("the page root cannot be moved".to_string());
        }
        Ok(UiIntent::BeginDrag(DragSource::ExistingComponent {
            component_id: selected.id,
        }))
    }

    pub fn remove_selected_intent(&self) -> Result<UiIntent, String> {
        let selected = self
            .selected_component_view()
            .ok_or_else(|| "select a component before removing it".to_string())?;
        if selected.is_root {
            return Err("the page root cannot be removed".to_string());
        }
        Ok(UiIntent::execute(EditorCommand::Remove {
            component_id: selected.id,
        }))
    }

    pub fn hit_candidates(
        &self,
        pointer: BrowserPoint,
        geometries: impl IntoIterator<Item = CanvasComponentGeometry>,
    ) -> Vec<HitTestCandidate> {
        let Some(drag) = self.ui().state.drag.as_ref() else {
            return Vec::new();
        };
        let Some((moving_component_id, child_type)) = drag_source_identity(self, &drag.source)
        else {
            return Vec::new();
        };
        let document = self.editor().document();
        let registries = self.editor().registries();
        let active_page_index = self.active_page_index();

        let targets = geometries
            .into_iter()
            .filter_map(|geometry| {
                let location = document.component_location(&geometry.component_id)?;
                if location.page_index != active_page_index {
                    return None;
                }
                let target_type = document.component_type_for_id(&geometry.component_id)?;
                let allow_inside = registries.accepts_child_type(Some(target_type), &child_type);
                Some(BrowserDropTarget {
                    component_id: geometry.component_id,
                    parent_component_id: geometry.parent_component_id,
                    index: geometry.index,
                    rect: geometry.rect,
                    axis: DropAxis::Vertical,
                    policy: DropZonePolicy {
                        edge_ratio: 0.24,
                        allow_inside,
                    },
                    legal: true,
                    reason: None,
                    priority: location.depth as f32,
                })
            })
            .collect::<Vec<_>>();

        let mut candidates =
            hit_test_drop_targets(pointer, targets, CoordinateTransform::default());
        for candidate in &mut candidates {
            let parent_component_id = match candidate.position {
                DropPosition::Inside => Some(candidate.target_component_id.as_str()),
                DropPosition::Before | DropPosition::After => {
                    candidate.parent_component_id.as_deref()
                }
            };
            if candidate.position == DropPosition::Inside {
                candidate.index = document
                    .component_child_count(&candidate.target_component_id)
                    .unwrap_or_default();
            }
            let decision = registries.evaluate_placement(
                document,
                moving_component_id.as_deref(),
                &child_type,
                parent_component_id,
                candidate.index,
            );
            candidate.legal = decision.legal;
            candidate.reason = decision.reason;
        }
        candidates.sort_by(|left, right| {
            right.legal.cmp(&left.legal).then_with(|| {
                right
                    .score
                    .partial_cmp(&left.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        });
        candidates
    }
}

fn drag_source_identity(
    controller: &AdminCanvasController,
    source: &DragSource,
) -> Option<(Option<String>, String)> {
    match source {
        DragSource::ExistingComponent { component_id } => {
            let document = controller.editor().document();
            let location = document.component_location(component_id)?;
            if location.page_index != controller.active_page_index() {
                return None;
            }
            document.component(component_id).map(|component| {
                (
                    Some(component_id.clone()),
                    component.component_type().to_string(),
                )
            })
        }
        DragSource::PaletteBlock { component, .. } => component
            .as_object()
            .map(|component| (None, component.component_type().to_string())),
        DragSource::ClipboardFragment => None,
    }
}

fn collect_layers(
    node: &ComponentNode,
    parent_component_id: Option<String>,
    index: usize,
    depth: usize,
    items: &mut Vec<LayerItemView>,
) {
    let Some(component) = node.as_object() else {
        return;
    };
    let Some(id) = component.id.clone() else {
        return;
    };
    items.push(layer_item(
        &id,
        component,
        parent_component_id.clone(),
        index,
        depth,
    ));
    for (child_index, child) in component.children().iter().enumerate() {
        collect_layers(child, Some(id.clone()), child_index, depth + 1, items);
    }
}

fn layer_item(
    id: &str,
    component: &ComponentObject,
    parent_component_id: Option<String>,
    index: usize,
    depth: usize,
) -> LayerItemView {
    LayerItemView {
        id: id.to_string(),
        component_type: component.component_type().to_string(),
        depth,
        parent_component_id,
        index,
        child_count: component.children().len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fly::{PageCommand, blank_page};
    use fly_ui::{DragSource, UiIntent};
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
                        "components": [{
                            "id": "section",
                            "type": "section",
                            "components": [{ "id": "text", "type": "text" }]
                        }]
                    }
                }]
            }),
        )
        .expect("controller")
    }

    #[test]
    fn palette_and_layers_are_derived_from_active_page() {
        let mut controller = controller();
        controller
            .dispatch(UiIntent::execute(EditorCommand::Page {
                command: PageCommand::Add {
                    index: 1,
                    page: Box::new(blank_page("about", "About")),
                },
            }))
            .expect("add page");
        assert_eq!(controller.layer_items().len(), 3);
        controller
            .dispatch(UiIntent::ActivatePage {
                page_id: Some("about".to_string()),
                page_index: 1,
            })
            .expect("activate about");
        assert_eq!(controller.layer_items().len(), 1);
        assert!(
            controller
                .palette_blocks()
                .iter()
                .any(|block| block.id == "section")
        );
    }

    #[test]
    fn immediate_insert_prefers_selected_container() {
        let mut controller = controller();
        controller
            .dispatch(UiIntent::Select(Some("section".to_string())))
            .expect("select");
        let intent = controller
            .insert_palette_block_intent("text")
            .expect("insert intent");
        let UiIntent::Execute(command) = intent else {
            panic!("expected insert command");
        };
        let EditorCommand::Insert {
            parent_id, index, ..
        } = *command
        else {
            panic!("expected insert command");
        };
        assert_eq!(parent_id.as_deref(), Some("section"));
        assert_eq!(index, 1);
    }

    #[test]
    fn immediate_insert_without_selection_targets_active_root() {
        let controller = controller();
        let intent = controller
            .insert_palette_block_intent("text")
            .expect("insert intent");
        let UiIntent::Execute(command) = intent else {
            panic!("expected insert command");
        };
        let EditorCommand::Insert {
            parent_id, index, ..
        } = *command
        else {
            panic!("expected insert command");
        };
        assert_eq!(parent_id.as_deref(), Some("root"));
        assert_eq!(index, 1);
    }

    #[test]
    fn hit_candidates_reject_recursive_inside_drop() {
        let mut controller = controller();
        controller
            .dispatch(UiIntent::BeginDrag(DragSource::ExistingComponent {
                component_id: "section".to_string(),
            }))
            .expect("begin drag");
        let candidates = controller.hit_candidates(
            BrowserPoint { x: 50.0, y: 50.0 },
            [CanvasComponentGeometry {
                component_id: "text".to_string(),
                parent_component_id: Some("section".to_string()),
                index: 0,
                rect: fly_leptos::BrowserRect {
                    left: 0.0,
                    top: 0.0,
                    width: 100.0,
                    height: 100.0,
                },
            }],
        );
        assert!(candidates.iter().all(|candidate| !candidate.legal));
    }
}
