//! Column definitions: identity, alignment, sizing and pinning.
//!
//! Every type in this module keeps its invariants locally so that no renderer
//! ever has to defend itself against a nonsensical configuration
//! (`min > max`, `current` outside of the range, ...).

use serde::{Deserialize, Serialize};

use crate::filter::GridFilterType;

/// Identifier of the synthetic selection column rendered as a checkbox.
///
/// Adapters must use this constant instead of hard-coding the string, so the
/// "magic" id lives in exactly one place.
pub const CHECKBOX_COLUMN_ID: &str = "__checkbox";

/// Stable identifier of a grid column.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ColumnId(pub String);

impl ColumnId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

// NOTE: deliberately *not* a blanket `impl<S: Into<String>> From<S> for ColumnId`.
// A blanket impl in that shape permanently blocks `impl From<ColumnId> for String`
// (coherence with the reflexive `impl<T> From<T> for T`) and produces confusing
// inference errors at call sites. Explicit impls cover every realistic input.
impl From<String> for ColumnId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for ColumnId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<&String> for ColumnId {
    fn from(value: &String) -> Self {
        Self(value.clone())
    }
}

impl From<std::borrow::Cow<'_, str>> for ColumnId {
    fn from(value: std::borrow::Cow<'_, str>) -> Self {
        Self(value.into_owned())
    }
}

impl From<ColumnId> for String {
    fn from(value: ColumnId) -> Self {
        value.0
    }
}

impl AsRef<str> for ColumnId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::borrow::Borrow<str> for ColumnId {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl std::ops::Deref for ColumnId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl PartialEq<str> for ColumnId {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for ColumnId {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl std::fmt::Display for ColumnId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Horizontal alignment of a column's content.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColumnAlign {
    #[default]
    Left,
    Center,
    Right,
}

impl ColumnAlign {
    /// CSS `text-align` keyword, useful for adapters that build inline styles.
    pub fn text_align(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
        }
    }
}

/// Width constraints of a column. The invariant `min <= current <= max`
/// is upheld by every constructor and mutator in this module.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnWidth {
    pub min: u32,
    pub max: u32,
    pub current: u32,
}

impl Default for ColumnWidth {
    fn default() -> Self {
        Self {
            min: 50,
            max: 800,
            current: 150,
        }
    }
}

impl ColumnWidth {
    /// A column that cannot grow or shrink.
    pub fn fixed(px: u32) -> Self {
        Self {
            min: px,
            max: px,
            current: px,
        }
    }

    /// A resizable column. A reversed range is a configuration error, but
    /// silently producing a value that cannot be clamped is considerably
    /// worse, so the bounds are normalized here.
    pub fn resizable(default_px: u32, min_px: u32, max_px: u32) -> Self {
        let (min, max) = ordered(min_px, max_px);
        Self {
            min,
            max,
            current: default_px.clamp(min, max),
        }
    }

    /// Clamp an arbitrary width into this column's range.
    pub fn clamp(&self, px: u32) -> u32 {
        let (min, max) = ordered(self.min, self.max);
        px.clamp(min, max)
    }

    /// Set the preferred width, widening the range when the requested value
    /// does not fit. This keeps the builder order-independent: `.width(x)`
    /// never silently discards a previously configured `min`/`max`.
    pub fn set_preferred(&mut self, px: u32) {
        let (mut min, mut max) = ordered(self.min, self.max);
        min = min.min(px);
        max = max.max(px);
        self.min = min;
        self.max = max;
        self.current = px;
    }

    /// Raise the lower bound, pushing `max`/`current` up when required.
    pub fn set_min(&mut self, px: u32) {
        self.min = px;
        self.max = self.max.max(px);
        self.current = self.clamp(self.current);
    }

    /// Lower the upper bound, pulling `min`/`current` down when required.
    pub fn set_max(&mut self, px: u32) {
        self.max = px;
        self.min = self.min.min(px);
        self.current = self.clamp(self.current);
    }

    /// `true` when the column has an actual range to drag within.
    pub fn is_flexible(&self) -> bool {
        self.min != self.max
    }
}

fn ordered(a: u32, b: u32) -> (u32, u32) {
    if a <= b { (a, b) } else { (b, a) }
}

/// Side a column is pinned (frozen) to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PinnedSide {
    Left,
    Right,
}

/// Declarative description of one grid column.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GridColumnDef {
    pub id: ColumnId,
    pub title: String,
    pub align: ColumnAlign,
    pub width: ColumnWidth,
    pub sortable: bool,
    pub resizable: bool,
    pub filterable: bool,
    pub visible: bool,
    pub pinned: Option<PinnedSide>,
    pub filter_type: Option<GridFilterType>,
}

impl GridColumnDef {
    pub fn new(id: impl Into<ColumnId>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            align: ColumnAlign::Left,
            width: ColumnWidth::default(),
            sortable: true,
            resizable: true,
            filterable: true,
            visible: true,
            pinned: None,
            filter_type: None,
        }
    }

    /// Synthetic row-selection column.
    pub fn checkbox() -> Self {
        Self {
            id: ColumnId::new(CHECKBOX_COLUMN_ID),
            title: String::new(),
            align: ColumnAlign::Center,
            width: ColumnWidth::fixed(44),
            sortable: false,
            resizable: false,
            filterable: false,
            visible: true,
            pinned: Some(PinnedSide::Left),
            filter_type: None,
        }
    }

    /// Trailing column reserved for row actions.
    pub fn actions(id: impl Into<ColumnId>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            align: ColumnAlign::Right,
            width: ColumnWidth::fixed(80),
            sortable: false,
            resizable: false,
            filterable: false,
            visible: true,
            pinned: Some(PinnedSide::Right),
            filter_type: None,
        }
    }

    /// `true` for the synthetic selection column.
    pub fn is_checkbox(&self) -> bool {
        self.id.as_str() == CHECKBOX_COLUMN_ID
    }

    /// `true` when this column should render an interactive filter control.
    pub fn has_filter(&self) -> bool {
        self.filterable && self.filter_type.is_some()
    }

    /// Preferred width, widening min/max when necessary (order-independent).
    pub fn width(mut self, px: u32) -> Self {
        self.width.set_preferred(px);
        self
    }

    /// Full width specification in one call.
    pub fn width_range(mut self, default_px: u32, min_px: u32, max_px: u32) -> Self {
        self.width = ColumnWidth::resizable(default_px, min_px, max_px);
        self
    }

    pub fn min_width(mut self, min_px: u32) -> Self {
        self.width.set_min(min_px);
        self
    }

    pub fn max_width(mut self, max_px: u32) -> Self {
        self.width.set_max(max_px);
        self
    }

    pub fn not_resizable(mut self) -> Self {
        self.resizable = false;
        self
    }

    pub fn not_sortable(mut self) -> Self {
        self.sortable = false;
        self
    }

    pub fn hidden(mut self) -> Self {
        self.visible = false;
        self
    }

    pub fn align(mut self, align: ColumnAlign) -> Self {
        self.align = align;
        self
    }

    pub fn filter(mut self, filter_type: GridFilterType) -> Self {
        self.filterable = true;
        self.filter_type = Some(filter_type);
        self
    }

    pub fn not_filterable(mut self) -> Self {
        self.filterable = false;
        self.filter_type = None;
        self
    }

    pub fn pin(mut self, side: PinnedSide) -> Self {
        self.pinned = Some(side);
        self
    }

    pub fn unpin(mut self) -> Self {
        self.pinned = None;
        self
    }
}

/// Helpers over a column set. Implemented as free functions so both the core
/// and any adapter can use them on plain slices.
pub fn visible_columns(columns: &[GridColumnDef]) -> impl Iterator<Item = &GridColumnDef> {
    columns.iter().filter(|c| c.visible)
}

/// Number of columns actually rendered — the only correct value for `colspan`.
pub fn visible_column_count(columns: &[GridColumnDef]) -> usize {
    visible_columns(columns).count()
}

/// `true` when at least one visible column renders a filter control.
pub fn has_filter_row(columns: &[GridColumnDef]) -> bool {
    visible_columns(columns).any(GridColumnDef::has_filter)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_builders_preserve_invariants() {
        let width = ColumnWidth::resizable(10, 300, 100);
        assert_eq!((width.min, width.max, width.current), (100, 300, 100));
    }

    #[test]
    fn preferred_width_widens_the_range_instead_of_being_clamped_away() {
        // `actions()` is a fixed 80px column; asking for 180px must win.
        let col = GridColumnDef::actions("actions", "Actions").width(180);
        assert_eq!(col.width.current, 180);
        assert_eq!(col.width.max, 180);
        assert_eq!(col.width.min, 80);
    }

    #[test]
    fn builder_order_does_not_change_the_outcome() {
        let a = GridColumnDef::new("t", "T").width(320).min_width(220);
        let b = GridColumnDef::new("t", "T").min_width(220).width(320);
        assert_eq!(a.width, b.width);
        assert_eq!((a.width.min, a.width.current), (220, 320));
    }

    #[test]
    fn min_greater_than_max_pushes_the_upper_bound() {
        let col = GridColumnDef::new("t", "T").max_width(120).min_width(200);
        assert!(col.width.min <= col.width.max);
        assert_eq!((col.width.min, col.width.max, col.width.current), (200, 200, 200));
    }

    #[test]
    fn column_helpers_count_only_visible_columns() {
        let cols = vec![
            GridColumnDef::checkbox(),
            GridColumnDef::new("a", "A").hidden(),
            GridColumnDef::new("b", "B"),
        ];
        assert_eq!(visible_column_count(&cols), 2);
        assert!(!has_filter_row(&cols));
        assert!(cols[0].is_checkbox());
    }
}
