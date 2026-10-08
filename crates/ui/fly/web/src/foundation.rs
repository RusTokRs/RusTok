//! Leptos/browser adapter foundation for Fly.
//!
//! This crate owns framework and browser concerns only. Canonical project state, drop legality,
//! commands, history, and editor policy remain in `fly` and `fly-ui`.

use fly_ui::{CanvasRect, DropPosition, HitTestCandidate};
use serde::{Deserialize, Serialize};

pub const FLY_IFRAME_PROTOCOL: &str = "fly_iframe";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct BrowserPoint {
    pub x: f64,
    pub y: f64,
}

impl BrowserPoint {
    /// Whether both coordinates are usable numbers.
    ///
    /// Geometry arrives from the iframe bridge as JSON, and an out-of-range literal deserializes
    /// to an infinity rather than an error. `edge_px`, `maximum_delta_px` and `edge_ratio` in
    /// this module already guard against that; points, rects and scores did not.
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }

    pub fn distance_squared(self, other: Self) -> f64 {
        let x = self.x - other.x;
        let y = self.y - other.y;
        x * x + y * y
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct BrowserRect {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

impl BrowserRect {
    /// Whether every edge is a usable number. See [`BrowserPoint::is_finite`].
    pub fn is_finite(self) -> bool {
        self.left.is_finite()
            && self.top.is_finite()
            && self.width.is_finite()
            && self.height.is_finite()
    }

    pub fn contains(self, point: BrowserPoint) -> bool {
        point.x >= self.left
            && point.y >= self.top
            && point.x <= self.left + self.width
            && point.y <= self.top + self.height
    }

    pub fn center(self) -> BrowserPoint {
        BrowserPoint {
            x: self.left + self.width / 2.0,
            y: self.top + self.height / 2.0,
        }
    }

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

/// Replace a non-finite offset with zero.
///
/// `normalized_zoom` already refuses to divide by a bad zoom; the offsets it is combined with
/// were left unguarded, so a single bad value from the bridge turned every converted coordinate
/// into NaN.
fn finite_or_zero(value: f64) -> f64 {
    if value.is_finite() { value } else { 0.0 }
}

impl CoordinateTransform {
    pub fn normalized_zoom(self) -> f64 {
        if self.zoom.is_finite() && self.zoom > 0.0 {
            self.zoom
        } else {
            1.0
        }
    }

    /// Combined translation along x, with non-finite components neutralized.
    fn offset_x(self) -> f64 {
        finite_or_zero(self.host_offset_x) + finite_or_zero(self.iframe_offset_x)
            - finite_or_zero(self.scroll_x)
    }

    /// Combined translation along y, with non-finite components neutralized.
    fn offset_y(self) -> f64 {
        finite_or_zero(self.host_offset_y) + finite_or_zero(self.iframe_offset_y)
            - finite_or_zero(self.scroll_y)
    }

    pub fn browser_to_canvas(self, point: BrowserPoint) -> BrowserPoint {
        let zoom = self.normalized_zoom();
        BrowserPoint {
            x: (point.x - self.offset_x()) / zoom,
            y: (point.y - self.offset_y()) / zoom,
        }
    }

    pub fn canvas_to_browser(self, point: BrowserPoint) -> BrowserPoint {
        let zoom = self.normalized_zoom();
        BrowserPoint {
            x: point.x * zoom + self.offset_x(),
            y: point.y * zoom + self.offset_y(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PointerKind {
    Mouse,
    Touch,
    Pen,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct PointerSample {
    pub pointer_id: i64,
    pub kind: PointerKind,
    pub position: BrowserPoint,
    pub buttons: u16,
    pub primary: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DropAxis {
    Vertical,
    Horizontal,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct DropZonePolicy {
    pub edge_ratio: f64,
    pub allow_inside: bool,
}

impl Default for DropZonePolicy {
    fn default() -> Self {
        Self {
            edge_ratio: 0.25,
            allow_inside: true,
        }
    }
}

impl DropZonePolicy {
    fn normalized_edge_ratio(self) -> f64 {
        if self.edge_ratio.is_finite() {
            self.edge_ratio.clamp(0.05, 0.45)
        } else {
            0.25
        }
    }
}

pub fn resolve_drop_position(
    pointer: BrowserPoint,
    rect: BrowserRect,
    axis: DropAxis,
    policy: DropZonePolicy,
) -> Option<DropPosition> {
    // Checked before `contains`, which cannot express the problem: an infinite rect contains
    // every point and passes the size guard, and then `(pointer.y - rect.top) / rect.height`
    // evaluates to NaN, so every comparison below is false and the function silently answers
    // `After` for a rect that does not exist.
    if !pointer.is_finite() || !rect.is_finite() {
        return None;
    }
    if !rect.contains(pointer) || rect.width <= 0.0 || rect.height <= 0.0 {
        return None;
    }

    let ratio = match axis {
        DropAxis::Vertical => (pointer.y - rect.top) / rect.height,
        DropAxis::Horizontal => (pointer.x - rect.left) / rect.width,
    };
    let edge = policy.normalized_edge_ratio();

    if ratio <= edge {
        Some(DropPosition::Before)
    } else if ratio >= 1.0 - edge {
        Some(DropPosition::After)
    } else if policy.allow_inside {
        Some(DropPosition::Inside)
    } else if ratio < 0.5 {
        Some(DropPosition::Before)
    } else {
        Some(DropPosition::After)
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
        // A target whose rectangle is not a real rectangle cannot be hit; passing it through
        // would only yield a garbage canvas rect downstream.
        .filter(|target| target.rect.is_finite())
        .map(|target| HitTestCandidate {
            target_component_id: target.component_id,
            parent_component_id: target.parent_component_id,
            index: target.index,
            position: target.position,
            rect: target.rect.to_canvas_rect(transform),
            score: finite_score(target.score),
            legal: target.legal,
            reason: target.reason,
        })
        .collect::<Vec<_>>();
    sort_candidates_by_score(&mut candidates);
    candidates
}

/// Replace a non-finite score with the lowest ranking value.
///
/// A NaN score is not merely wrong, it is corrosive: it makes any `partial_cmp`-based comparator
/// non-transitive, and `slice::sort_by` is entitled to panic when it detects that the ordering is
/// not total.
fn finite_score(score: f32) -> f32 {
    if score.is_finite() { score } else { f32::MIN }
}

/// Rank candidates by descending score under a total order.
///
/// `partial_cmp(..).unwrap_or(Equal)` looks like a safe fallback but silently produces a
/// comparator that is not a total order. `total_cmp` is one for every `f32`, including NaN.
fn sort_candidates_by_score(candidates: &mut [HitTestCandidate]) {
    candidates.sort_by(|left, right| right.score.total_cmp(&left.score));
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BrowserDropTarget {
    pub component_id: String,
    pub parent_component_id: Option<String>,
    pub index: usize,
    pub rect: BrowserRect,
    pub axis: DropAxis,
    pub policy: DropZonePolicy,
    pub legal: bool,
    pub reason: Option<String>,
    pub priority: f32,
}

/// Interpret browser geometry into framework-neutral Fly UI candidates.
///
/// This function never mutates DOM order or the Fly project. It only produces candidates; `fly-ui`
/// and `fly` remain responsible for capability checks, nesting rules, and command execution.
pub fn hit_test_drop_targets(
    pointer: BrowserPoint,
    targets: impl IntoIterator<Item = BrowserDropTarget>,
    transform: CoordinateTransform,
) -> Vec<HitTestCandidate> {
    let mut candidates = targets
        .into_iter()
        .filter_map(|target| {
            let position = resolve_drop_position(pointer, target.rect, target.axis, target.policy)?;
            let distance = pointer.distance_squared(target.rect.center()).sqrt();
            let proximity = (1.0 / (1.0 + distance)) as f32;
            let index = if position == DropPosition::After {
                target.index.saturating_add(1)
            } else {
                target.index
            };
            Some(HitTestCandidate {
                target_component_id: target.component_id,
                parent_component_id: target.parent_component_id,
                index,
                position,
                rect: target.rect.to_canvas_rect(transform),
                score: finite_score(finite_score(target.priority) + proximity),
                legal: target.legal,
                reason: target.reason,
            })
        })
        .collect::<Vec<_>>();

    sort_candidates_by_score(&mut candidates);
    candidates
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct AutoScrollPolicy {
    pub edge_px: f64,
    pub maximum_delta_px: f64,
}

impl Default for AutoScrollPolicy {
    fn default() -> Self {
        Self {
            edge_px: 48.0,
            maximum_delta_px: 24.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub struct AutoScrollDelta {
    pub x: f64,
    pub y: f64,
}

impl AutoScrollDelta {
    pub fn is_idle(self) -> bool {
        self.x == 0.0 && self.y == 0.0
    }
}

pub fn auto_scroll_delta(
    pointer: BrowserPoint,
    viewport: BrowserRect,
    policy: AutoScrollPolicy,
) -> AutoScrollDelta {
    let edge = if policy.edge_px.is_finite() {
        policy.edge_px.max(1.0)
    } else {
        48.0
    };
    let maximum = if policy.maximum_delta_px.is_finite() {
        policy.maximum_delta_px.max(0.0)
    } else {
        24.0
    };

    fn axis_delta(value: f64, start: f64, length: f64, edge: f64, maximum: f64) -> f64 {
        let end = start + length;
        if value < start + edge {
            -maximum * ((start + edge - value) / edge).clamp(0.0, 1.0)
        } else if value > end - edge {
            maximum * ((value - (end - edge)) / edge).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    AutoScrollDelta {
        x: axis_delta(pointer.x, viewport.left, viewport.width, edge, maximum),
        y: axis_delta(pointer.y, viewport.top, viewport.height, edge, maximum),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IframeBridgeMessage {
    Ready,
    ViewportChanged {
        width: u32,
        height: u32,
        scroll_x: f64,
        scroll_y: f64,
        zoom: f64,
    },
    PointerMoved {
        sample: PointerSample,
    },
    FocusRequested {
        component_id: Option<String>,
    },
    Teardown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IframeBridgeEnvelope {
    pub protocol: String,
    pub instance_id: String,
    pub sequence: u64,
    pub message: IframeBridgeMessage,
}

impl IframeBridgeEnvelope {
    pub fn new(
        instance_id: impl Into<String>,
        sequence: u64,
        message: IframeBridgeMessage,
    ) -> Self {
        Self {
            protocol: FLY_IFRAME_PROTOCOL.to_string(),
            instance_id: instance_id.into(),
            sequence,
            message,
        }
    }

    /// Reject cross-instance, wrong-protocol, and replayed messages before browser adapters turn
    /// them into editor intents.
    pub fn is_accepted(&self, expected_instance_id: &str, last_sequence: Option<u64>) -> bool {
        self.protocol == FLY_IFRAME_PROTOCOL
            && self.instance_id == expected_instance_id
            && last_sequence.is_none_or(|sequence| self.sequence > sequence)
    }
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
    fn invalid_zoom_falls_back_to_one() {
        assert_eq!(
            CoordinateTransform {
                zoom: 0.0,
                ..CoordinateTransform::default()
            }
            .normalized_zoom(),
            1.0
        );
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
    fn drop_position_uses_edges_and_inside_zone() {
        let rect = BrowserRect {
            left: 0.0,
            top: 0.0,
            width: 100.0,
            height: 100.0,
        };
        let policy = DropZonePolicy::default();
        assert_eq!(
            resolve_drop_position(
                BrowserPoint { x: 50.0, y: 10.0 },
                rect,
                DropAxis::Vertical,
                policy
            ),
            Some(DropPosition::Before)
        );
        assert_eq!(
            resolve_drop_position(
                BrowserPoint { x: 50.0, y: 50.0 },
                rect,
                DropAxis::Vertical,
                policy
            ),
            Some(DropPosition::Inside)
        );
        assert_eq!(
            resolve_drop_position(
                BrowserPoint { x: 50.0, y: 90.0 },
                rect,
                DropAxis::Vertical,
                policy
            ),
            Some(DropPosition::After)
        );
    }

    #[test]
    fn calculated_hit_test_advances_after_index() {
        let candidates = hit_test_drop_targets(
            BrowserPoint { x: 50.0, y: 95.0 },
            [BrowserDropTarget {
                component_id: "hero".to_string(),
                parent_component_id: Some("root".to_string()),
                index: 2,
                rect: BrowserRect {
                    left: 0.0,
                    top: 0.0,
                    width: 100.0,
                    height: 100.0,
                },
                axis: DropAxis::Vertical,
                policy: DropZonePolicy::default(),
                legal: true,
                reason: None,
                priority: 1.0,
            }],
            CoordinateTransform::default(),
        );
        assert_eq!(candidates[0].position, DropPosition::After);
        assert_eq!(candidates[0].index, 3);
    }

    #[test]
    fn auto_scroll_scales_near_viewport_edges() {
        let viewport = BrowserRect {
            left: 0.0,
            top: 0.0,
            width: 500.0,
            height: 300.0,
        };
        let delta = auto_scroll_delta(
            BrowserPoint { x: 250.0, y: 295.0 },
            viewport,
            AutoScrollPolicy::default(),
        );
        assert_eq!(delta.x, 0.0);
        assert!(delta.y > 0.0);
        assert!(delta.y <= 24.0);
    }

    #[test]
    fn iframe_envelope_rejects_replay_and_wrong_instance() {
        let envelope = IframeBridgeEnvelope::new("canvas-a", 4, IframeBridgeMessage::Ready);
        assert!(envelope.is_accepted("canvas-a", Some(3)));
        assert!(!envelope.is_accepted("canvas-a", Some(4)));
        assert!(!envelope.is_accepted("canvas-b", None));
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

    fn rect(left: f64, top: f64, width: f64, height: f64) -> BrowserRect {
        BrowserRect {
            left,
            top,
            width,
            height,
        }
    }

    fn drop_target(priority: f32, rect: BrowserRect) -> BrowserDropTarget {
        BrowserDropTarget {
            component_id: format!("c{priority}"),
            parent_component_id: None,
            index: 0,
            rect,
            axis: DropAxis::Vertical,
            policy: DropZonePolicy::default(),
            legal: true,
            reason: None,
            priority,
        }
    }

    #[test]
    fn an_unreal_rectangle_produces_no_drop_position() {
        // An infinite rect contains every point and passes the size guard, after which the
        // position ratio is NaN, every comparison is false, and the old code answered `After`
        // for a rectangle that does not exist.
        let pointer = BrowserPoint { x: 10.0, y: 10.0 };
        for bad in [
            rect(0.0, f64::NEG_INFINITY, 100.0, f64::INFINITY),
            rect(0.0, 0.0, f64::INFINITY, f64::INFINITY),
            rect(f64::NAN, 0.0, 100.0, 100.0),
            rect(0.0, 0.0, 100.0, f64::NAN),
        ] {
            assert_eq!(
                resolve_drop_position(pointer, bad, DropAxis::Vertical, DropZonePolicy::default()),
                None,
                "accepted {bad:?}"
            );
        }
    }

    #[test]
    fn a_non_finite_pointer_hits_nothing() {
        let good = rect(0.0, 0.0, 100.0, 100.0);
        for pointer in [
            BrowserPoint {
                x: f64::NAN,
                y: 10.0,
            },
            BrowserPoint {
                x: 10.0,
                y: f64::INFINITY,
            },
        ] {
            assert_eq!(
                resolve_drop_position(pointer, good, DropAxis::Vertical, DropZonePolicy::default()),
                None
            );
        }
    }

    #[test]
    fn candidate_ranking_is_a_total_order_even_with_unusable_scores() {
        // `partial_cmp(..).unwrap_or(Equal)` yields a non-transitive comparator, and `sort_by` is
        // entitled to panic once it notices. Ranking must survive a hostile score and still put
        // the real candidates in order.
        let targets = vec![
            BrowserHitTarget {
                component_id: "low".to_string(),
                parent_component_id: None,
                index: 0,
                rect: rect(0.0, 0.0, 10.0, 10.0),
                position: DropPosition::Before,
                legal: true,
                reason: None,
                score: 1.0,
            },
            BrowserHitTarget {
                component_id: "nan".to_string(),
                parent_component_id: None,
                index: 1,
                rect: rect(0.0, 0.0, 10.0, 10.0),
                position: DropPosition::Before,
                legal: true,
                reason: None,
                score: f32::NAN,
            },
            BrowserHitTarget {
                component_id: "high".to_string(),
                parent_component_id: None,
                index: 2,
                rect: rect(0.0, 0.0, 10.0, 10.0),
                position: DropPosition::Before,
                legal: true,
                reason: None,
                score: 9.0,
            },
        ];

        let ranked = normalize_hit_targets(targets, CoordinateTransform::default());
        let order = ranked
            .iter()
            .map(|candidate| candidate.target_component_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(order, vec!["high", "low", "nan"]);
        assert!(ranked.iter().all(|candidate| candidate.score.is_finite()));
    }

    #[test]
    fn hit_targets_with_unreal_rectangles_are_dropped() {
        let targets = vec![BrowserHitTarget {
            component_id: "bad".to_string(),
            parent_component_id: None,
            index: 0,
            rect: rect(0.0, 0.0, f64::INFINITY, 10.0),
            position: DropPosition::Before,
            legal: true,
            reason: None,
            score: 1.0,
        }];
        assert!(normalize_hit_targets(targets, CoordinateTransform::default()).is_empty());
    }

    #[test]
    fn a_hostile_priority_cannot_outrank_a_real_candidate() {
        let pointer = BrowserPoint { x: 50.0, y: 50.0 };
        let candidates = hit_test_drop_targets(
            pointer,
            vec![
                drop_target(f32::NAN, rect(0.0, 0.0, 100.0, 100.0)),
                drop_target(1.0, rect(0.0, 0.0, 100.0, 100.0)),
            ],
            CoordinateTransform::default(),
        );
        assert_eq!(candidates.len(), 2);
        // Both share a rectangle, so proximity is equal and only the priority separates them.
        assert_eq!(candidates[0].target_component_id, "c1");
        assert!(candidates[0].score > candidates[1].score);
        assert!(
            candidates
                .iter()
                .all(|candidate| candidate.score.is_finite())
        );
    }

    #[test]
    fn a_broken_transform_offset_does_not_poison_every_coordinate() {
        // `normalized_zoom` already refused a bad zoom; a bad offset used to turn every converted
        // coordinate into NaN instead.
        let transform = CoordinateTransform {
            host_offset_x: f64::NAN,
            host_offset_y: 20.0,
            iframe_offset_x: 30.0,
            iframe_offset_y: f64::INFINITY,
            scroll_x: 5.0,
            scroll_y: 7.0,
            zoom: 2.0,
        };
        let point = BrowserPoint { x: 140.0, y: 180.0 };
        let canvas = transform.browser_to_canvas(point);
        assert!(canvas.is_finite(), "{canvas:?}");
        assert!(transform.canvas_to_browser(canvas).is_finite());
    }
}
