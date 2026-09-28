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

    pub fn set(&mut self, column_id: impl Into<String>, width: u32) {
        self.widths.insert(column_id.into(), width);
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
}
