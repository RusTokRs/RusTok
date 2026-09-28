use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnWidths {
    widths: BTreeMap<String, u32>,
}

impl ColumnWidths {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, column_id: &str, default_width: u32) -> u32 {
        self.widths.get(column_id).copied().unwrap_or(default_width)
    }

    /// Read a persisted width while enforcing the current column constraints.
    /// This protects the renderer from stale or hand-edited serialized state.
    pub fn get_clamped(&self, column_id: &str, default_width: u32, min: u32, max: u32) -> u32 {
        self.get(column_id, default_width).clamp(min.min(max), min.max(max))
    }

    pub fn set(&mut self, column_id: impl Into<String>, width: u32) {
        self.widths.insert(column_id.into(), width);
    }

    /// Store a width while enforcing the column's constraints.
    pub fn set_clamped(&mut self, column_id: impl Into<String>, width: u32, min: u32, max: u32) {
        self.set(column_id, width.clamp(min.min(max), min.max(max)));
    }

    pub fn remove(&mut self, column_id: &str) {
        self.widths.remove(column_id);
    }
}

/// Headless calculation of resized column width with minimum and maximum bounds.
pub fn calculate_resized_width(
    initial_width: u32,
    delta_x: f64,
    min_width: u32,
    max_width: u32,
) -> u32 {
    let (min_width, max_width) = (min_width.min(max_width), min_width.max(max_width));
    // Browser pointer coordinates can technically produce non-finite values.
    // Never let NaN reach a float-to-integer cast (which would yield a surprising
    // platform-dependent result).
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
    }

    #[test]
    fn test_resize_clamps_to_max() {
        assert_eq!(calculate_resized_width(150, 300.0, 50, 400), 400);
    }

    #[test]
    fn test_resize_handles_invalid_input_and_reversed_bounds() {
        assert_eq!(calculate_resized_width(150, f64::NAN, 400, 50), 150);
        assert_eq!(calculate_resized_width(150, 10.0, 400, 50), 160);
    }
}
