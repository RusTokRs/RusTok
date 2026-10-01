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
            minimum_width: 24.0,
            minimum_height: 20.0,
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

impl ResizeSession {
    pub fn update(&self, pointer_x: f64, pointer_y: f64) -> ResizeResult {
        let dx = pointer_x - self.start_x;
        let dy = pointer_y - self.start_y;
        let aspect = if self.start_rect.height > 0.0 {
            self.start_rect.width / self.start_rect.height
        } else {
            1.0
        };

        let mut rect = self.start_rect;
        if self.handle.changes_width() {
            if self.handle.moves_left_edge() {
                rect.x += dx;
                rect.width -= dx;
            } else {
                rect.width += dx;
            }
        }
        if self.handle.changes_height() {
            if self.handle.moves_top_edge() {
                rect.y += dy;
                rect.height -= dy;
            } else {
                rect.height += dy;
            }
        }

        if self.policy.preserve_aspect_ratio
            && self.handle.changes_width()
            && self.handle.changes_height()
        {
            if dx.abs() >= dy.abs() {
                rect.height = rect.width / aspect.max(f64::EPSILON);
            } else {
                rect.width = rect.height * aspect;
            }
        }

        let original_right = rect.x + rect.width;
        let original_bottom = rect.y + rect.height;
        rect.width = clamp_dimension(
            snap(rect.width, self.policy.grid_size),
            self.policy.minimum_width,
            self.policy.maximum_width,
        );
        rect.height = clamp_dimension(
            snap(rect.height, self.policy.grid_size),
            self.policy.minimum_height,
            self.policy.maximum_height,
        );
        if self.handle.moves_left_edge() {
            rect.x = original_right - rect.width;
        }
        if self.handle.moves_top_edge() {
            rect.y = original_bottom - rect.height;
        }

        ResizeResult {
            width_changed: self.handle.changes_width()
                && (rect.width - self.start_rect.width).abs() > f64::EPSILON,
            height_changed: self.handle.changes_height()
                && (rect.height - self.start_rect.height).abs() > f64::EPSILON,
            rect,
        }
    }

    pub fn component_patch(&self, result: ResizeResult) -> ComponentPatch {
        let mut style = Map::new();
        if result.width_changed {
            style.insert("width".to_string(), Value::String(px(result.rect.width)));
        }
        if result.height_changed {
            style.insert("height".to_string(), Value::String(px(result.rect.height)));
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

fn clamp_dimension(value: f64, minimum: f64, maximum: Option<f64>) -> f64 {
    let minimum = if minimum.is_finite() {
        minimum.max(0.0)
    } else {
        0.0
    };
    let maximum = maximum.filter(|maximum| maximum.is_finite() && *maximum >= minimum);
    maximum.map_or_else(
        || value.max(minimum),
        |maximum| value.clamp(minimum, maximum),
    )
}

fn px(value: f64) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    format!("{rounded}px")
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
        assert_eq!(result.rect.width, 216.0);
        assert_eq!(result.rect.height, 120.0);
    }

    #[test]
    fn west_resize_preserves_right_edge() {
        let result = session(ResizeHandle::West).update(232.0, 100.0);
        assert_eq!(result.rect.x + result.rect.width, 210.0);
    }

    #[test]
    fn resize_result_generates_style_patch() {
        let session = session(ResizeHandle::East);
        let result = session.update(232.0, 100.0);
        let patch = session.component_patch(result);
        assert_eq!(patch.style.expect("style")["width"], "232px");
    }
}
