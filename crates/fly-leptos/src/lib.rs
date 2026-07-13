//! Leptos/browser adapter foundation for Fly.
//!
//! This crate owns framework and browser concerns only. Canonical project state and editor policy
//! remain in `fly` and `fly-ui`.

use fly_ui::{CanvasRect, DropPosition, HitTestCandidate};
use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct BrowserPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct BrowserRect {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

impl BrowserRect {
    pub fn to_canvas_rect(self, transform: CoordinateTransform) -> CanvasRect {
        let origin = transform.browser_to_canvas(BrowserPoint {
            x: self.left,
            y: self.top,
        });
        let zoom = transform.normalized_zoom();
        CanvasRect {
            x: origin.x,
            y: origin.y,
            width: self.width / zoom,
            height: self.height / zoom,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct CoordinateTransform {
    pub host_offset_x: f64,
    pub host_offset_y: f64,
    pub iframe_offset_x: f64,
    pub iframe_offset_y: f64,
    pub scroll_x: f64,
    pub scroll_y: f64,
    pub zoom: f64,
}

impl Default for CoordinateTransform {
    fn default() -> Self {
        Self {
            host_offset_x: 0.0,
            host_offset_y: 0.0,
            iframe_offset_x: 0.0,
            iframe_offset_y: 0.0,
            scroll_x: 0.0,
            scroll_y: 0.0,
            zoom: 1.0,
        }
    }
}

impl CoordinateTransform {
    pub fn normalized_zoom(self) -> f64 {
        if self.zoom <= 0.0 { 1.0 } else { self.zoom }
    }

    pub fn browser_to_canvas(self, point: BrowserPoint) -> BrowserPoint {
        let zoom = self.normalized_zoom();
        BrowserPoint {
            x: (point.x - self.host_offset_x - self.iframe_offset_x + self.scroll_x) / zoom,
            y: (point.y - self.host_offset_y - self.iframe_offset_y + self.scroll_y) / zoom,
        }
    }

    pub fn canvas_to_browser(self, point: BrowserPoint) -> BrowserPoint {
        let zoom = self.normalized_zoom();
        BrowserPoint {
            x: point.x * zoom + self.host_offset_x + self.iframe_offset_x - self.scroll_x,
            y: point.y * zoom + self.host_offset_y + self.iframe_offset_y - self.scroll_y,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BrowserHitTarget {
    pub component_id: String,
    pub parent_component_id: Option<String>,
    pub index: usize,
    pub rect: BrowserRect,
    pub position: DropPosition,
    pub legal: bool,
    pub reason: Option<String>,
    pub score: f32,
}

pub fn normalize_hit_targets(
    targets: impl IntoIterator<Item = BrowserHitTarget>,
    transform: CoordinateTransform,
) -> Vec<HitTestCandidate> {
    let mut candidates = targets
        .into_iter()
        .map(|target| HitTestCandidate {
            target_component_id: target.component_id,
            parent_component_id: target.parent_component_id,
            index: target.index,
            position: target.position,
            rect: target.rect.to_canvas_rect(transform),
            score: target.score,
            legal: target.legal,
            reason: target.reason,
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    candidates
}

pub struct CleanupRegistry {
    callbacks: Vec<Box<dyn FnOnce()>>,
}

impl CleanupRegistry {
    pub fn new() -> Self {
        Self {
            callbacks: Vec::new(),
        }
    }

    pub fn register(&mut self, callback: impl FnOnce() + 'static) {
        self.callbacks.push(Box::new(callback));
    }

    pub fn len(&self) -> usize {
        self.callbacks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.callbacks.is_empty()
    }

    pub fn cleanup(&mut self) {
        while let Some(callback) = self.callbacks.pop() {
            callback();
        }
    }
}

impl Default for CleanupRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for CleanupRegistry {
    fn drop(&mut self) {
        self.cleanup();
    }
}

#[component]
pub fn FlyFullEditor(children: Children) -> impl IntoView {
    view! {
        <section class="fly-editor fly-editor--full" role="application" aria-label="Fly full editor">
            {children()}
        </section>
    }
}

#[component]
pub fn FlyInlineEditor(children: Children) -> impl IntoView {
    view! {
        <section class="fly-editor fly-editor--inline" role="application" aria-label="Fly inline editor">
            {children()}
        </section>
    }
}

#[component]
pub fn FlyPreview(children: Children) -> impl IntoView {
    view! {
        <section class="fly-editor fly-editor--preview" aria-label="Fly preview">
            {children()}
        </section>
    }
}

#[component]
pub fn FlyReadOnly(children: Children) -> impl IntoView {
    view! {
        <section class="fly-editor fly-editor--read-only" aria-label="Fly read-only view">
            {children()}
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn coordinate_transform_round_trips() {
        let transform = CoordinateTransform {
            host_offset_x: 10.0,
            host_offset_y: 20.0,
            iframe_offset_x: 30.0,
            iframe_offset_y: 40.0,
            scroll_x: 5.0,
            scroll_y: 7.0,
            zoom: 2.0,
        };
        let browser = BrowserPoint { x: 140.0, y: 180.0 };
        let canvas = transform.browser_to_canvas(browser);
        assert_eq!(transform.canvas_to_browser(canvas), browser);
    }

    #[test]
    fn hit_targets_are_normalized_and_sorted() {
        let candidates = normalize_hit_targets(
            [
                BrowserHitTarget {
                    component_id: "child".to_string(),
                    parent_component_id: Some("root".to_string()),
                    index: 1,
                    rect: BrowserRect {
                        left: 20.0,
                        top: 20.0,
                        width: 80.0,
                        height: 40.0,
                    },
                    position: DropPosition::Before,
                    legal: true,
                    reason: None,
                    score: 0.5,
                },
                BrowserHitTarget {
                    component_id: "root".to_string(),
                    parent_component_id: None,
                    index: 0,
                    rect: BrowserRect {
                        left: 0.0,
                        top: 0.0,
                        width: 200.0,
                        height: 100.0,
                    },
                    position: DropPosition::Inside,
                    legal: true,
                    reason: None,
                    score: 1.0,
                },
            ],
            CoordinateTransform::default(),
        );
        assert_eq!(candidates[0].target_component_id, "root");
        assert_eq!(candidates[1].parent_component_id.as_deref(), Some("root"));
        assert_eq!(candidates[1].index, 1);
    }

    #[test]
    fn cleanup_registry_runs_in_reverse_order() {
        let order = Rc::new(Cell::new(0_u8));
        let mut registry = CleanupRegistry::new();
        let first = Rc::clone(&order);
        registry.register(move || {
            assert_eq!(first.get(), 1);
            first.set(2);
        });
        let second = Rc::clone(&order);
        registry.register(move || {
            assert_eq!(second.get(), 0);
            second.set(1);
        });
        registry.cleanup();
        assert_eq!(order.get(), 2);
    }
}
