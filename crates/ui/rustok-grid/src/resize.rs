//! Column resizing: persisted widths and the pointer math behind the drag.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// User-overridden column widths, keyed by column id.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnWidths {
    widths: BTreeMap<String, u32>,
}

impl ColumnWidths {
    pub fn new() -> Self {
        Self::default()
    }

    /// Stored override, if the user ever resized this column.
    pub fn get_stored(&self, column_id: &str) -> Option<u32> {
        self.widths.get(column_id).copied()
    }

    pub fn get(&self, column_id: &str, default_width: u32) -> u32 {
        self.get_stored(column_id).unwrap_or(default_width)
    }

    /// Read a persisted width while enforcing the current column constraints.
    /// This protects the renderer from stale or hand-edited serialized state.
    pub fn get_clamped(&self, column_id: &str, default_width: u32, min: u32, max: u32) -> u32 {
        let (min, max) = ordered(min, max);
        self.get(column_id, default_width).clamp(min, max)
    }

    pub fn set(&mut self, column_id: impl Into<String>, width: u32) {
        self.widths.insert(column_id.into(), width);
    }

    /// Store a width while enforcing the column's constraints.
    ///
    /// Returns `true` when the stored value changed, so adapters can avoid
    /// re-rendering on every pointer move that does not move the edge.
    pub fn set_clamped(
        &mut self,
        column_id: impl Into<String>,
        width: u32,
        min: u32,
        max: u32,
    ) -> bool {
        let (min, max) = ordered(min, max);
        let width = width.clamp(min, max);
        let key = column_id.into();
        if self.widths.get(&key) == Some(&width) {
            return false;
        }
        self.widths.insert(key, width);
        true
    }

    pub fn remove(&mut self, column_id: &str) {
        self.widths.remove(column_id);
    }

    pub fn clear(&mut self) {
        self.widths.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.widths.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &u32)> {
        self.widths.iter()
    }
}

fn ordered(a: u32, b: u32) -> (u32, u32) {
    if a <= b { (a, b) } else { (b, a) }
}

/// Headless calculation of a resized column width with min/max bounds.
///
/// Defensive on purpose: pointer coordinates come from the browser and can be
/// non-finite, and the caller's bounds can be reversed. A `NaN` must never
/// reach a float-to-integer cast, where it would silently become `0`.
pub fn calculate_resized_width(
    initial_width: u32,
    delta_x: f64,
    min_width: u32,
    max_width: u32,
) -> u32 {
    let (min_width, max_width) = ordered(min_width, max_width);
    if !delta_x.is_finite() {
        return initial_width.clamp(min_width, max_width);
    }
    let proposed = (initial_width as f64 + delta_x).round();
    if proposed < min_width as f64 {
        min_width
    } else if proposed > max_width as f64 {
        max_width
    } else {
        proposed as u32
    }
}

/// Keyboard resizing step (px) used by adapters implementing the
/// WAI-ARIA "separator" pattern on the resize handle.
pub const KEYBOARD_RESIZE_STEP: i32 = 16;

/// Apply a keyboard-driven resize step, saturating at the bounds.
pub fn step_resized_width(current_width: u32, step_px: i32, min_width: u32, max_width: u32) -> u32 {
    calculate_resized_width(current_width, f64::from(step_px), min_width, max_width)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resize_within_bounds() {
        assert_eq!(calculate_resized_width(150, 25.0, 50, 400), 175);
        assert_eq!(calculate_resized_width(150, -30.0, 50, 400), 120);
    }

    #[test]
    fn test_resize_clamps_to_min() {
        assert_eq!(calculate_resized_width(150, -120.0, 50, 400), 50);
        // A drag far to the left must not wrap around through zero.
        assert_eq!(calculate_resized_width(150, -1.0e12, 50, 400), 50);
    }

    #[test]
    fn test_resize_clamps_to_max() {
        assert_eq!(calculate_resized_width(150, 300.0, 50, 400), 400);
        assert_eq!(calculate_resized_width(150, 1.0e12, 50, 400), 400);
    }

    #[test]
    fn test_resize_handles_invalid_input_and_reversed_bounds() {
        assert_eq!(calculate_resized_width(150, f64::NAN, 400, 50), 150);
        assert_eq!(calculate_resized_width(150, f64::INFINITY, 50, 400), 150);
        assert_eq!(calculate_resized_width(150, 10.0, 400, 50), 160);
    }

    #[test]
    fn keyboard_steps_respect_bounds() {
        assert_eq!(step_resized_width(150, KEYBOARD_RESIZE_STEP, 50, 400), 166);
        assert_eq!(step_resized_width(60, -KEYBOARD_RESIZE_STEP, 50, 400), 50);
    }

    #[test]
    fn stored_widths_are_clamped_and_change_detected() {
        let mut widths = ColumnWidths::new();
        assert!(widths.set_clamped("title", 9_000, 120, 300));
        assert_eq!(widths.get("title", 200), 300);
        // Already at the max: no further change.
        assert!(!widths.set_clamped("title", 9_001, 120, 300));
        assert_eq!(widths.get_clamped("title", 200, 300, 120), 300);
    }
}
