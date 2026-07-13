use crate::editor::CanvasComponentGeometry;
use crate::AdminCanvasController;
use fly::{ComponentNode, ComponentObject};
use fly_leptos::{
    hit_test_drop_targets, BrowserDropTarget, BrowserPoint, CoordinateTransform, DropAxis,
    DropZonePolicy,
};
use fly_ui::{DragSource, DropPosition, HitTestCandidate};
use serde_json::{Map, Value};

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

    pub fn layer_items(&self) -> Vec<LayerItemView> {
        let mut items = Vec::new();
        for page in &self.editor().document().project.pages {
            let Some(root) = page.component.as_ref() else {
                continue;
            };
            collect_layers(root, None, 0, 0, &mut items);
        }
        items
    }

    pub fn selected_component_view(&self) -> Option<SelectedComponentView> {
        let component_id = self.ui().state.selection.component_id.as_deref()?;
        let component = self.editor().document().component(component_id)?;
        let location = self.editor().document().component_location(component_id)?;
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
        let viewport = self.ui().state.viewport;
        let document = self.editor().document();
        let registries = self.editor().registries();

        let targets = geometries
            .into_iter()
            .filter_map(|geometry| {
                let target_type = document.component_type_for_id(&geometry.component_id)?;
                let allow_inside = registries.accepts_child_type(Some(target_type), &child_type);
                let depth = document
                    .component_location(&geometry.component_id)
                    .map(|location| location.depth)
                    .unwrap_or_default();
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
                    priority: depth as f32,
                })
            })
            .collect::<Vec<_>>();

        let transform = CoordinateTransform {
            scroll_x: viewport.scroll_x,
            scroll_y: viewport.scroll_y,
            zoom: f64::from(viewport.zoom),
            ..CoordinateTransform::default()
        };
        let mut candidates = hit_test_drop_targets(pointer, targets, transform);
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
            right
                .legal
                .cmp(&left.legal)
                .then_with(|| {
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
        DragSource::ExistingComponent { component_id } => controller
            .editor()
            .document()
            .component(component_id)
            .map(|component| {
                (
                    Some(component_id.clone()),
                    component.component_type().to_string(),
                )
            }),
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
        collect_layers(
            child,
            Some(id.clone()),
            child_index,
            depth + 1,
            items,
        );
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
    use fly_ui::{DragSource, UiIntent};
    use serde_json::json;

    fn controller() -> AdminCanvasController {
        AdminCanvasController::new(
            "home",
            "rev-1",
            json!({
                "pages": [{
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
    fn palette_and_layers_are_derived_from_engine_state() {
        let controller = controller();
        assert!(controller.palette_blocks().iter().any(|block| block.id == "section"));
        assert_eq!(controller.layer_items().len(), 3);
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
