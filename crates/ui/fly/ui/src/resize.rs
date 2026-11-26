use crate::CanvasRect;
use fly::ComponentPatch;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResizeHandle {
    North,
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    West,
    NorthWest,
}

impl ResizeHandle {
    pub const fn changes_width(self) -> bool {
        matches!(
            self,
            Self::NorthEast
                | Self::East
                | Self::SouthEast
                | Self::SouthWest
                | Self::West
                | Self::NorthWest
        )
    }

    pub const fn changes_height(self) -> bool {
        matches!(
            self,
            Self::North
                | Self::NorthEast
                | Self::SouthEast
                | Self::South
                | Self::SouthWest
                | Self::NorthWest
        )
    }

    pub const fn moves_left_edge(self) -> bool {
        matches!(self, Self::SouthWest | Self::West | Self::NorthWest)
    }

    pub const fn moves_top_edge(self) -> bool {
        matches!(self, Self::North | Self::NorthEast | Self::NorthWest)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct ResizePolicy {
    pub minimum_width: f64,
    pub minimum_height: f64,
    pub maximum_width: Option<f64>,
    pub maximum_height: Option<f64>,
    pub grid_size: Option<f64>,
    pub preserve_aspect_ratio: bool,
}

impl Default for ResizePolicy {
    fn default() -> Self {
        Self {
            minimum_width: MIN_RESIZE_WIDTH_PX,
            minimum_height: MIN_RESIZE_HEIGHT_PX,
            maximum_width: None,
            maximum_height: None,
            grid_size: None,
            preserve_aspect_ratio: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResizeSession {
    pub component_id: String,
    pub handle: ResizeHandle,
    pub start_rect: CanvasRect,
    pub start_x: f64,
    pub start_y: f64,
    pub policy: ResizePolicy,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct ResizeResult {
    pub rect: CanvasRect,
    pub width_changed: bool,
    pub height_changed: bool,
}

/// A single pointer gesture is bounded independently of optional component size policy.
const MAX_RESIZE_POINTER_DELTA_PX: f64 = 1_000_000.0;

/// Default minimum width for Fly resize sessions and the Page Builder surface.
pub const MIN_RESIZE_WIDTH_PX: f64 = 24.0;

/// Default minimum height for Fly resize sessions and the Page Builder surface.
pub const MIN_RESIZE_HEIGHT_PX: f64 = 20.0;

/// Maximum persisted dimension used by the Page Builder resize surface.
///
/// General Fly resize sessions may opt into different limits through ResizePolicy.
pub const MAX_RESIZE_DIMENSION_PX: f64 = 100_000.0;

impl ResizeSession {
    pub fn update(&self, pointer_x: f64, pointer_y: f64) -> ResizeResult {
        // Browser geometry can become invalid during iframe disposal or a
        // malformed adapter event. Never generate NaNpx/infinitypx declarations.
        let unchanged = || ResizeResult {
            rect: self.start_rect,
            width_changed: false,
            height_changed: false,
        };
        if !pointer_x.is_finite()
            || !pointer_y.is_finite()
            || !self.start_x.is_finite()
            || !self.start_y.is_finite()
            || !self.start_rect.is_valid()
        {
            return unchanged();
        }
        let dx = pointer_x - self.start_x;
        let dy = pointer_y - self.start_y;
        if !dx.is_finite()
            || !dy.is_finite()
            || dx.abs() > MAX_RESIZE_POINTER_DELTA_PX
            || dy.abs() > MAX_RESIZE_POINTER_DELTA_PX
        {
            return unchanged();
        }
        let width_delta = if !self.handle.changes_width() {
            0.0
        } else if self.handle.moves_left_edge() {
            -dx
        } else {
            dx
        };
        let height_delta = if !self.handle.changes_height() {
            0.0
        } else if self.handle.moves_top_edge() {
            -dy
        } else {
            dy
        };
        if width_delta == 0.0 && height_delta == 0.0 {
            return unchanged();
        }

        let mut rect = self.start_rect;
        if self.aspect_ratio_locked() {
            // A collapsed or numerically unrepresentable ratio cannot be locked.
            if !self.matches_aspect_ratio(rect) {
                return unchanged();
            }
            // Constrain one scale, not two independent dimensions. Snapping and
            // clamping both dimensions separately can destroy the aspect ratio.
            let Some((min_width, max_width)) =
                dimension_bounds(self.policy.minimum_width, self.policy.maximum_width)
            else {
                return unchanged();
            };
            let Some((min_height, max_height)) =
                dimension_bounds(self.policy.minimum_height, self.policy.maximum_height)
            else {
                return unchanged();
            };
            let min_scale = (min_width / rect.width).max(min_height / rect.height);
            let max_scale = (max_width / rect.width).min(max_height / rect.height);
            if !min_scale.is_finite() || max_scale.is_nan() || min_scale > max_scale {
                return unchanged();
            }

            let scale = if dx.abs() >= dy.abs() {
                snap(rect.width + width_delta, self.policy.grid_size) / rect.width
            } else {
                snap(rect.height + height_delta, self.policy.grid_size) / rect.height
            };
            if !scale.is_finite() {
                return unchanged();
            }
            let scale = scale.clamp(min_scale, max_scale);
            rect.width = (rect.width * scale).clamp(min_width, max_width);
            rect.height = (rect.height * scale).clamp(min_height, max_height);
        } else {
            // Do not normalize an untouched axis: the preview and persisted patch
            // must describe the same dimensions, including off-grid starting sizes.
            if width_delta != 0.0 {
                let width = snap(rect.width + width_delta, self.policy.grid_size);
                if !width.is_finite() {
                    return unchanged();
                }
                let Some(width) = clamp_dimension(
                    width,
                    self.policy.minimum_width,
                    self.policy.maximum_width,
                ) else {
                    return unchanged();
                };
                rect.width = width;
            }
            if height_delta != 0.0 {
                let height = snap(rect.height + height_delta, self.policy.grid_size);
                if !height.is_finite() {
                    return unchanged();
                }
                let Some(height) = clamp_dimension(
                    height,
                    self.policy.minimum_height,
                    self.policy.maximum_height,
                ) else {
                    return unchanged();
                };
                rect.height = height;
            }
        }
        // The opposite edges belong to the initial rectangle, before aspect-ratio
        // adjustments, snapping or size limits.
        if self.handle.moves_left_edge() && rect.width != self.start_rect.width {
            rect.x = self.start_rect.x + (self.start_rect.width - rect.width);
        }
        if self.handle.moves_top_edge() && rect.height != self.start_rect.height {
            rect.y = self.start_rect.y + (self.start_rect.height - rect.height);
        }

        if !rect.is_valid() {
            return unchanged();
        }
        let width_changed = self.handle.changes_width() && rect.width != self.start_rect.width;
        let height_changed = self.handle.changes_height() && rect.height != self.start_rect.height;
        if self.aspect_ratio_locked()
            && (width_changed || height_changed)
            && (!width_changed || !height_changed || !self.matches_aspect_ratio(rect))
        {
            return unchanged();
        }
        ResizeResult {
            width_changed,
            height_changed,
            rect,
        }
    }

    fn aspect_ratio_locked(&self) -> bool {
        self.policy.preserve_aspect_ratio
            && self.handle.changes_width()
            && self.handle.changes_height()
    }

    fn matches_aspect_ratio(&self, rect: CanvasRect) -> bool {
        if self.start_rect.width <= 0.0
            || self.start_rect.height <= 0.0
            || rect.width <= 0.0
            || rect.height <= 0.0
        {
            return false;
        }
        let start_ratio = self.start_rect.width / self.start_rect.height;
        let result_ratio = rect.width / rect.height;
        start_ratio.is_normal()
            && result_ratio.is_normal()
            && (result_ratio / start_ratio - 1.0).abs() <= 1e-9
    }

    /// Produces a keyboard resize step using the same snapping and bounds as pointer input.
    ///
    /// Arrow keys move the active handle by 4 CSS pixels; holding Shift moves it by 16.
    /// Non-arrow keys return None so callers can leave normal keyboard behavior untouched.
    pub fn update_by_arrow_key(&self, key: &str, shift: bool) -> Option<ResizeResult> {
        let step = if shift { 16.0 } else { 4.0 };
        let (dx, dy) = match key {
            "ArrowLeft" => (-step, 0.0),
            "ArrowRight" => (step, 0.0),
            "ArrowUp" => (0.0, -step),
            "ArrowDown" => (0.0, step),
            _ => return None,
        };
        Some(self.update(self.start_x + dx, self.start_y + dy))
    }

    pub fn component_patch(&self, result: ResizeResult) -> ComponentPatch {
        let mut style = Map::new();
        // ResizeResult is public and deserializable. Recheck the policy at the
        // persistence boundary so forged or stale results cannot bypass bounds.
        if self.start_rect.is_valid() && result.rect.is_valid() {
            let width_within_bounds = if result.width_changed && self.handle.changes_width() {
                dimension_bounds(self.policy.minimum_width, self.policy.maximum_width)
                    .is_some_and(|(minimum, maximum)| {
                        result.rect.width >= minimum && result.rect.width <= maximum
                    })
            } else {
                true
            };
            let height_within_bounds = if result.height_changed && self.handle.changes_height() {
                dimension_bounds(self.policy.minimum_height, self.policy.maximum_height)
                    .is_some_and(|(minimum, maximum)| {
                        result.rect.height >= minimum && result.rect.height <= maximum
                    })
            } else {
                true
            };
            let actual_width_changed = result.rect.width != self.start_rect.width;
            let actual_height_changed = result.rect.height != self.start_rect.height;
            let aspect_ratio_valid = !self.aspect_ratio_locked()
                || (!result.width_changed && !result.height_changed
                    && !actual_width_changed && !actual_height_changed)
                || (result.width_changed
                    && result.height_changed
                    && actual_width_changed
                    && actual_height_changed
                    && self.matches_aspect_ratio(result.rect));

            if width_within_bounds && height_within_bounds && aspect_ratio_valid {
                if result.width_changed && self.handle.changes_width() {
                    style.insert("width".to_string(), Value::String(px(result.rect.width)));
                }
                if result.height_changed && self.handle.changes_height() {
                    style.insert("height".to_string(), Value::String(px(result.rect.height)));
                }
            }
        }
        ComponentPatch {
            style: Some(Value::Object(style)),
            ..ComponentPatch::default()
        }
    }
}

fn snap(value: f64, grid_size: Option<f64>) -> f64 {
    match grid_size.filter(|grid| grid.is_finite() && *grid > 0.0) {
        Some(grid) => (value / grid).round() * grid,
        None => value,
    }
}

fn dimension_bounds(minimum: f64, maximum: Option<f64>) -> Option<(f64, f64)> {
    if !minimum.is_finite() {
        return None;
    }
    let minimum = minimum.max(0.0);
    let maximum = match maximum {
        Some(maximum) if maximum.is_finite() && maximum >= minimum => maximum,
        Some(_) => return None,
        None => f64::INFINITY,
    };
    Some((minimum, maximum))
}

fn clamp_dimension(value: f64, minimum: f64, maximum: Option<f64>) -> Option<f64> {
    let (minimum, maximum) = dimension_bounds(minimum, maximum)?;
    Some(value.clamp(minimum, maximum))
}

fn px(value: f64) -> String {
    // The result has already been snapped, bounded and aspect-ratio checked.
    // Rounding here could move it outside its bounds or change the ratio.
    format!("{value}px")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(handle: ResizeHandle) -> ResizeSession {
        ResizeSession {
            component_id: "hero".to_string(),
            handle,
            start_rect: CanvasRect {
                x: 10.0,
                y: 20.0,
                width: 200.0,
                height: 100.0,
            },
            start_x: 200.0,
            start_y: 100.0,
            policy: ResizePolicy {
                grid_size: Some(8.0),
                ..ResizePolicy::default()
            },
        }
    }

    #[test]
    fn south_east_resize_snaps_dimensions() {
        let result = session(ResizeHandle::SouthEast).update(219.0, 117.0);
        assert!((result.rect.width - 216.0).abs() < f64::EPSILON);
        assert!((result.rect.height - 120.0).abs() < f64::EPSILON);
    }

    #[test]
    fn west_resize_preserves_right_edge() {
        let result = session(ResizeHandle::West).update(232.0, 100.0);
        assert!((result.rect.x + result.rect.width - 210.0).abs() < f64::EPSILON);
    }

    #[test]
    fn invalid_and_overflowing_pointer_geometry_cannot_generate_nan_css() {
        let session = session(ResizeHandle::SouthEast);
        for (x, y) in [
            (f64::NAN, 100.0),
            (f64::INFINITY, 100.0),
            (f64::MAX, f64::MAX),
        ] {
            let result = session.update(x, y);
            assert!(!result.width_changed);
            assert!(!result.height_changed);
            assert_eq!(result.rect, session.start_rect);
            let patch = session.component_patch(result);
            assert_eq!(patch.style, Some(Value::Object(Map::new())));
        }
    }

    #[test]
    fn keyboard_resize_uses_the_same_snap_grid_and_shift_multiplier() {
        let mut resize = session(ResizeHandle::East);
        resize.policy.grid_size = Some(4.0);

        let normal = resize
            .update_by_arrow_key("ArrowRight", false)
            .expect("arrow key should resize");
        let accelerated = resize
            .update_by_arrow_key("ArrowRight", true)
            .expect("shift+arrow should resize");

        assert_eq!(normal.rect.width, 204.0);
        assert_eq!(accelerated.rect.width, 216.0);
        assert!(resize.update_by_arrow_key("Enter", false).is_none());
    }

    #[test]
    fn keyboard_resize_respects_west_and_north_handle_directions() {
        let mut west = session(ResizeHandle::West);
        west.policy.grid_size = Some(4.0);
        let west_result = west
            .update_by_arrow_key("ArrowRight", false)
            .expect("arrow key should resize");
        assert_eq!(west_result.rect.x, 14.0);
        assert_eq!(west_result.rect.width, 196.0);

        let mut north = session(ResizeHandle::North);
        north.policy.grid_size = Some(4.0);
        let north_result = north
            .update_by_arrow_key("ArrowDown", false)
            .expect("arrow key should resize");
        assert_eq!(north_result.rect.y, 24.0);
        assert_eq!(north_result.rect.height, 96.0);
    }

    #[test]
    fn single_axis_resize_preserves_off_grid_perpendicular_dimension() {
        let mut resize = session(ResizeHandle::East);
        resize.start_rect.height = 103.0;
        let result = resize.update(219.0, 117.0);
        assert_eq!(result.rect.width, 216.0);
        assert_eq!(result.rect.height, 103.0);
        assert!(result.width_changed);
        assert!(!result.height_changed);
        let style = resize.component_patch(result).style.expect("style");
        assert_eq!(style["width"], "216px");
        assert!(style.get("height").is_none());

        let mut resize = session(ResizeHandle::North);
        resize.start_rect.width = 203.0;
        let result = resize.update(219.0, 117.0);
        assert_eq!(result.rect.width, 203.0);
        assert_eq!(result.rect.height, 80.0);
        assert!(!result.width_changed);
        assert!(result.height_changed);
    }

    #[test]
    fn no_movement_or_perpendicular_arrow_does_not_snap_geometry() {
        let mut resize = session(ResizeHandle::East);
        resize.start_rect.width = 203.0;
        resize.start_rect.height = 103.0;
        for result in [
            resize.update(resize.start_x, resize.start_y),
            resize.update_by_arrow_key("ArrowUp", false).expect("arrow"),
        ] {
            assert_eq!(result.rect, resize.start_rect);
            assert!(!result.width_changed);
            assert!(!result.height_changed);
        }
    }

    #[test]
    fn aspect_locked_corner_preserves_ratio_and_opposite_edges_under_bounds() {
        let mut resize = session(ResizeHandle::NorthWest);
        resize.policy.preserve_aspect_ratio = true;
        resize.policy.maximum_height = Some(110.0);
        let result = resize.update(160.0, 90.0);
        assert!((result.rect.width - 220.0).abs() < 1e-9);
        assert!((result.rect.height - 110.0).abs() < 1e-9);
        assert!((result.rect.x + result.rect.width - 210.0).abs() < 1e-9);
        assert!((result.rect.y + result.rect.height - 120.0).abs() < 1e-9);
        assert!(result.width_changed && result.height_changed);

        let result = resize.update(190.0, 80.0);
        assert!((result.rect.width / result.rect.height - 2.0).abs() < 1e-9);
        assert!((result.rect.x + result.rect.width - 210.0).abs() < 1e-9);
        assert!((result.rect.y + result.rect.height - 120.0).abs() < 1e-9);
    }

    #[test]
    fn locked_corner_rejects_collapsed_start_and_zero_scale() {
        for (width, height) in [(0.0, 100.0), (200.0, 0.0), (0.0, 0.0)] {
            let mut resize = session(ResizeHandle::SouthEast);
            resize.policy.preserve_aspect_ratio = true;
            resize.start_rect.width = width;
            resize.start_rect.height = height;
            let result = resize.update(240.0, 140.0);
            assert_eq!(result.rect, resize.start_rect);
            assert!(!result.width_changed && !result.height_changed);
            assert_eq!(
                resize.component_patch(result).style,
                Some(Value::Object(Map::new()))
            );
        }

        let mut resize = session(ResizeHandle::SouthEast);
        resize.policy.preserve_aspect_ratio = true;
        resize.policy.minimum_width = 0.0;
        resize.policy.minimum_height = 0.0;
        let result = resize.update(0.0, 0.0);
        assert_eq!(result.rect, resize.start_rect);
        assert!(!result.width_changed && !result.height_changed);
        assert_eq!(
            resize.component_patch(result).style,
            Some(Value::Object(Map::new()))
        );
    }

    #[test]
    fn unlocked_corner_can_resize_a_collapsed_element() {
        let mut resize = session(ResizeHandle::SouthEast);
        resize.start_rect.width = 0.0;
        resize.start_rect.height = 0.0;
        let result = resize.update(240.0, 140.0);
        assert_eq!(result.rect.width, 40.0);
        assert_eq!(result.rect.height, 40.0);
        let style = resize.component_patch(result).style.expect("style");
        assert_eq!(style["width"], "40px");
        assert_eq!(style["height"], "40px");
    }

    #[test]
    fn locked_corner_rejects_subnormal_ratio_drift_at_patch_boundary() {
        let mut resize = session(ResizeHandle::SouthEast);
        resize.policy.preserve_aspect_ratio = true;
        resize.policy.minimum_width = 0.0;
        resize.policy.minimum_height = 0.0;
        resize.start_rect.width = 1e-320;
        resize.start_rect.height = 100.0;
        let result = ResizeResult {
            rect: CanvasRect {
                width: 4e-320,
                height: 200.0,
                ..resize.start_rect
            },
            width_changed: true,
            height_changed: true,
        };
        assert_eq!(
            resize.component_patch(result).style,
            Some(Value::Object(Map::new()))
        );
    }

    #[test]
    fn locked_corner_rejects_mismatched_change_flags_and_subnormal_ratios() {
        let mut resize = session(ResizeHandle::SouthEast);
        resize.policy.preserve_aspect_ratio = true;
        let result = ResizeResult {
            rect: CanvasRect { height: 100.0 + 1e-10, ..resize.start_rect },
            width_changed: true,
            height_changed: true,
        };
        assert_eq!(resize.component_patch(result).style, Some(Value::Object(Map::new())));

        resize.start_rect.width = f64::from_bits(100);
        resize.start_rect.height = 100.0;
        resize.policy.minimum_width = 0.0;
        resize.policy.minimum_height = 0.0;
        let result = ResizeResult {
            rect: CanvasRect {
                width: f64::from_bits(101),
                height: 100.0001,
                ..resize.start_rect
            },
            width_changed: true,
            height_changed: true,
        };
        // Both quotients round to the same subnormal value, despite ratio drift.
        assert_eq!(resize.component_patch(result).style, Some(Value::Object(Map::new())));
    }

    #[test]
    fn incompatible_aspect_bounds_leave_geometry_unchanged() {
        let mut resize = session(ResizeHandle::SouthEast);
        resize.policy.preserve_aspect_ratio = true;
        resize.policy.minimum_width = 240.0;
        resize.policy.maximum_height = Some(110.0);
        let result = resize.update(240.0, 120.0);
        assert_eq!(result.rect, resize.start_rect);
        assert!(!result.width_changed);
        assert!(!result.height_changed);
    }

    #[test]
    fn invalid_explicit_maximums_leave_the_affected_resize_unchanged() {
        let mut width = session(ResizeHandle::East);
        width.policy.minimum_width = 240.0;
        width.policy.maximum_width = Some(200.0);
        let result = width.update(250.0, 20.0);
        assert_eq!(result.rect, width.start_rect);
        assert!(!result.width_changed);

        let mut height = session(ResizeHandle::South);
        height.policy.minimum_height = 120.0;
        height.policy.maximum_height = Some(80.0);
        let result = height.update(10.0, 120.0);
        assert_eq!(result.rect, height.start_rect);
        assert!(!result.height_changed);
    }

    #[test]
    fn component_patch_enforces_explicit_bounds_and_aspect_ratio() {
        let mut width = session(ResizeHandle::East);
        width.policy.maximum_width = Some(220.0);
        let outside_maximum = ResizeResult {
            rect: CanvasRect {
                width: 232.0,
                ..width.start_rect
            },
            width_changed: true,
            height_changed: false,
        };
        assert_eq!(
            width.component_patch(outside_maximum).style,
            Some(Value::Object(Map::new()))
        );

        let mut corner = session(ResizeHandle::SouthEast);
        corner.policy.preserve_aspect_ratio = true;
        let wrong_ratio = ResizeResult {
            rect: CanvasRect {
                width: 232.0,
                height: 110.0,
                ..corner.start_rect
            },
            width_changed: true,
            height_changed: true,
        };
        assert_eq!(
            corner.component_patch(wrong_ratio).style,
            Some(Value::Object(Map::new()))
        );

        // An absolute 1e-9 tolerance is far too permissive for very narrow elements:
        // it could accept a large relative aspect-ratio change.
        let mut narrow = session(ResizeHandle::SouthEast);
        narrow.policy.preserve_aspect_ratio = true;
        narrow.start_rect = CanvasRect {
            width: 1e-8,
            height: 100.0,
            ..narrow.start_rect
        };
        narrow.policy.minimum_width = 0.0;
        narrow.policy.minimum_height = 0.0;
        let narrow_wrong_ratio = ResizeResult {
            rect: CanvasRect {
                width: 2e-8,
                height: 100.0,
                ..narrow.start_rect
            },
            width_changed: true,
            height_changed: true,
        };
        assert_eq!(
            narrow.component_patch(narrow_wrong_ratio).style,
            Some(Value::Object(Map::new()))
        );
    }

    #[test]
    fn direct_resize_patch_rejects_invalid_geometry_and_uncontrolled_axes() {
        let resize = session(ResizeHandle::East);
        for width in [f64::NAN, f64::INFINITY, -1.0] {
            let result = ResizeResult {
                rect: CanvasRect {
                    width,
                    ..resize.start_rect
                },
                width_changed: true,
                height_changed: true,
            };
            assert_eq!(
                resize.component_patch(result).style,
                Some(Value::Object(Map::new()))
            );
        }
        let result = ResizeResult {
            rect: CanvasRect {
                width: 232.0,
                height: 120.0,
                ..resize.start_rect
            },
            width_changed: true,
            height_changed: true,
        };
        let style = resize.component_patch(result).style.expect("style");
        assert_eq!(style["width"], "232px");
        assert!(style.get("height").is_none());
    }

    #[test]
    fn finite_resize_dimension_remains_finite_in_css() {
        let resize = session(ResizeHandle::East);
        let result = ResizeResult {
            rect: CanvasRect {
                width: f64::MAX / 2.0,
                ..resize.start_rect
            },
            width_changed: true,
            height_changed: false,
        };
        let style = resize.component_patch(result).style.expect("style");
        let width = style["width"].as_str().expect("width string");
        let number = width
            .strip_suffix("px")
            .expect("CSS pixels")
            .parse::<f64>()
            .expect("numeric width");
        assert!(number.is_finite());
    }

    #[test]
    fn css_patch_preserves_fractional_resize_bounds() {
        let mut resize = session(ResizeHandle::East);
        resize.policy.minimum_width = 24.004;
        resize.policy.maximum_width = Some(200.006);

        for (pointer_x, expected) in [(0.0, 24.004), (300.0, 200.006)] {
            let result = resize.update(pointer_x, resize.start_y);
            assert!(result.width_changed);
            assert_eq!(result.rect.width, expected);
            let style = resize.component_patch(result).style.expect("style");
            assert_eq!(style["width"], format!("{expected}px"));
        }
    }

    #[test]
    fn css_patch_preserves_aspect_locked_fractional_geometry() {
        let mut resize = session(ResizeHandle::SouthEast);
        resize.start_rect.height = 103.0;
        resize.policy.preserve_aspect_ratio = true;
        let result = resize.update(219.0, 100.0);
        assert!(result.width_changed && result.height_changed);

        let style = resize.component_patch(result).style.expect("style");
        let width = style["width"]
            .as_str()
            .expect("width")
            .trim_end_matches("px")
            .parse::<f64>()
            .expect("numeric width");
        let height = style["height"]
            .as_str()
            .expect("height")
            .trim_end_matches("px")
            .parse::<f64>()
            .expect("numeric height");
        assert_eq!(width, result.rect.width);
        assert_eq!(height, result.rect.height);
    }

    #[test]
    fn resize_result_generates_style_patch() {
        let session = session(ResizeHandle::East);
        let result = session.update(232.0, 100.0);
        let patch = session.component_patch(result);
        assert_eq!(patch.style.expect("style")["width"], "232px");
    }
}
