//! The value vocabulary behind grid filtering and sorting.
//!
//! A grid column is owned by a module; a *cell* is what the grid knows about a row. The owner maps
//! a row plus a column id onto a [`CellValue`], and this module answers the two questions every
//! table has: does this row satisfy the active filter for this column
//! ([`CellValue::matches`]), and in which order do two cells sort
//! ([`CellValue::compare`]).
//!
//! Putting both answers here is what stops every admin table from re-implementing its own
//! trim/case-folding, range and date rules. `ColumnFilters::retain_matching` and
//! `SortState::sort_items` are the loop-shaped entry points a grid over raw rows uses; a
//! server-driven grid keeps pushing its predicates down instead and shares only the semantics.

use std::cmp::Ordering;

use crate::filter::FilterValue;

/// One cell of a row, in the vocabulary filters and sorting understand.
///
/// A cell carries no display concern: labels, formatting and links stay with the owner.
#[derive(Clone, Debug, PartialEq)]
pub enum CellValue {
    /// Any textual value, including an id or an ISO-8601 date.
    Text(String),
    /// A finite numeric value. Non-finite input is rejected by [`CellValue::number`].
    Number(f64),
    /// A tri-state value; "missing" is [`CellValue::Empty`], not a third boolean.
    Bool(bool),
    /// No value at all.
    Empty,
}

impl Default for CellValue {
    fn default() -> Self {
        Self::Empty
    }
}

impl CellValue {
    /// A text cell.
    pub fn text(value: impl Into<String>) -> Self {
        Self::Text(value.into())
    }

    /// A text cell read from an optional field.
    pub fn optional_text(value: Option<impl Into<String>>) -> Self {
        match value {
            Some(value) => Self::Text(value.into()),
            None => Self::Empty,
        }
    }

    /// A numeric cell. A non-finite value carries no constraint and becomes [`Self::Empty`].
    pub fn number(value: f64) -> Self {
        if value.is_finite() {
            Self::Number(value)
        } else {
            Self::Empty
        }
    }

    /// A numeric cell read from an optional field.
    pub fn optional_number(value: Option<f64>) -> Self {
        match value {
            Some(value) => Self::number(value),
            None => Self::Empty,
        }
    }

    /// A boolean cell.
    pub fn boolean(value: bool) -> Self {
        Self::Bool(value)
    }

    /// A cell with no value at all.
    pub fn empty() -> Self {
        Self::Empty
    }

    /// `true` when the cell carries no value; a blank text counts as no value.
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Empty => true,
            Self::Text(text) => text.trim().is_empty(),
            Self::Number(_) | Self::Bool(_) => false,
        }
    }

    /// The text of a text cell, if this cell is one.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text.as_str()),
            _ => None,
        }
    }

    /// The number of a numeric cell, if this cell is one.
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Self::Number(value) => Some(*value),
            _ => None,
        }
    }

    /// The flag of a boolean cell, if this cell is one.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    /// The cell rendered as text, the form a text or select filter compares against.
    ///
    /// A number is formatted with `Display` and a flag as `true`/`false`, so a select option id of
    /// `"42"` addresses a numeric cell holding `42.0`. An empty cell renders as nothing.
    pub fn display_text(&self) -> String {
        match self {
            Self::Text(text) => text.clone(),
            Self::Number(value) => value.to_string(),
            Self::Bool(value) => value.to_string(),
            Self::Empty => String::new(),
        }
    }

    /// The number a numeric filter can use: a numeric cell as-is, or a decimal number in a text
    /// cell. Hex, `inf` and `NaN` spellings are deliberately not numbers here, so the filter means
    /// exactly what the ISO/JSON value the owner stored means.
    pub fn number_for_filter(&self) -> Option<f64> {
        match self {
            Self::Number(value) if value.is_finite() => Some(*value),
            Self::Text(text) => {
                let trimmed = text.trim();
                if trimmed.is_empty() || !is_decimal_spelling(trimmed) {
                    return None;
                }
                trimmed
                    .parse::<f64>()
                    .ok()
                    .filter(|value| value.is_finite())
            }
            _ => None,
        }
    }

    /// The ISO date part of the cell: everything before the first `T`, trimmed.
    ///
    /// Dates are ISO-8601 (`YYYY-MM-DD`), so byte ordering is chronological ordering and a
    /// timestamp `2026-05-01T10:00:00Z` still compares against a `2026-05-01` bound.
    pub fn date_part(&self) -> Option<&str> {
        let Self::Text(text) = self else {
            return None;
        };
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return None;
        }
        let date = match trimmed.find('T') {
            Some(index) => &trimmed[..index],
            None => trimmed,
        }
        .trim();
        (!date.is_empty()).then_some(date)
    }

    /// `true` when this cell satisfies `filter`.
    ///
    /// The rules are the ones the whole platform shares: text is a trimmed, case-insensitive
    /// substring; a select value is an exact id compared ASCII-case-insensitively; a number range
    /// is inclusive and also parses a numeric text cell; a date range compares the ISO date part; a
    /// boolean is exact. An empty cell never satisfies a range or a boolean filter, because a
    /// missing value is not a value. An empty filter accepts everything.
    pub fn matches(&self, filter: &FilterValue) -> bool {
        match filter {
            FilterValue::Empty => true,
            FilterValue::Text(query) => text_contains_ignore_case(&self.display_text(), query),
            FilterValue::Select(id) => {
                let wanted = id.trim();
                if wanted.is_empty() {
                    return true;
                }
                let current = self.display_text();
                !current.trim().is_empty() && current.eq_ignore_ascii_case(wanted)
            }
            FilterValue::NumberRange { min, max } => {
                let Some(value) = self.number_for_filter() else {
                    return false;
                };
                min.is_none_or(|bound| value >= bound) && max.is_none_or(|bound| value <= bound)
            }
            FilterValue::DateRange { from, to } => {
                let Some(date) = self.date_part() else {
                    return false;
                };
                from.as_deref().is_none_or(|bound| date >= bound)
                    && to.as_deref().is_none_or(|bound| date <= bound)
            }
            FilterValue::Boolean(wanted) => self.as_bool() == Some(*wanted),
        }
    }

    /// Rank used when two cells of different kinds are compared: empty < boolean < number < text.
    fn rank(&self) -> u8 {
        match self {
            Self::Empty => 0,
            Self::Bool(_) => 1,
            // A non-finite number can only arrive by building the variant directly; rank it with
            // the other valueless cells instead of inventing an ordering for it.
            Self::Number(value) if !value.is_finite() => 0,
            Self::Number(_) => 2,
            Self::Text(_) => 3,
        }
    }

    /// Total order of two cells, independent of locale: numbers numerically, booleans `false`
    /// before `true`, texts by code point (which is `str`'s own ordering), and [`Self::Empty`]
    /// before every value. Cross-kind comparisons fall back to the kind rank, so the order stays
    /// total and a page refresh can never reorder a tie.
    pub fn compare(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Number(left), Self::Number(right)) if left.is_finite() && right.is_finite() => {
                left.partial_cmp(right).unwrap_or(Ordering::Equal)
            }
            (Self::Bool(left), Self::Bool(right)) => left.cmp(right),
            (Self::Text(left), Self::Text(right)) => left.as_str().cmp(right.as_str()),
            _ => self.rank().cmp(&other.rank()),
        }
    }
}

/// `true` when `text` is a plain decimal number (`-12`, `3.5`, `2e-3`) and nothing else.
///
/// A cell may hold its number as text, and the two languages have permissive numeric parsers that
/// disagree on the edges (`0x10`, `Infinity`, `1e`): JavaScript's `Number` accepts all three, Rust's
/// accepts none. Requiring one explicit spelling in the core is what lets the TypeScript twin answer
/// identically instead of inheriting each runtime's idea of a number.
fn is_decimal_spelling(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut index = 0;
    if matches!(bytes.first(), Some(&b'+') | Some(&b'-')) {
        index += 1;
    }
    let integer_digits = take_digits(bytes, &mut index);
    let mut fraction_digits = 0;
    if bytes.get(index) == Some(&b'.') {
        index += 1;
        fraction_digits = take_digits(bytes, &mut index);
    }
    if integer_digits + fraction_digits == 0 {
        return false;
    }
    if matches!(bytes.get(index), Some(&b'e') | Some(&b'E')) {
        index += 1;
        if matches!(bytes.get(index), Some(&b'+') | Some(&b'-')) {
            index += 1;
        }
        if take_digits(bytes, &mut index) == 0 {
            return false;
        }
    }
    index == bytes.len()
}

fn take_digits(bytes: &[u8], index: &mut usize) -> usize {
    let mut count = 0;
    while bytes.get(*index).is_some_and(u8::is_ascii_digit) {
        *index += 1;
        count += 1;
    }
    count
}

/// Case-insensitive containment used by [`FilterValue::Text`].
///
/// Both sides are trimmed and lowercased with Unicode full case conversion, so a Cyrillic query
/// finds a Cyrillic value; the select counterpart deliberately folds ASCII only, because an option
/// id is not prose.
pub fn text_contains_ignore_case(haystack: &str, needle: &str) -> bool {
    let query = needle.trim().to_lowercase();
    if query.is_empty() {
        return true;
    }
    haystack.to_lowercase().contains(&query)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_filter_is_a_trimmed_case_insensitive_substring() {
        let cell = CellValue::text("  Красное Apple ");
        assert!(cell.matches(&FilterValue::Text("красное".into())));
        assert!(cell.matches(&FilterValue::Text("  PLE  ".into())));
        assert!(!cell.matches(&FilterValue::Text("green".into())));
        assert!(cell.matches(&FilterValue::Empty));
        assert!(cell.matches(&FilterValue::Text("   ".into())));
    }

    #[test]
    fn select_filter_compares_ids_as_opaque_values() {
        let cell = CellValue::text("ACTIVE");
        assert!(cell.matches(&FilterValue::Select("active".into())));
        assert!(!cell.matches(&FilterValue::Select("inactive".into())));
        // A select id may address a numeric cell, and a missing cell matches nothing.
        assert!(CellValue::number(42.0).matches(&FilterValue::Select("42".into())));
        assert!(!CellValue::Empty.matches(&FilterValue::Select("active".into())));
        assert!(CellValue::Empty.matches(&FilterValue::Select("   ".into())));
    }

    #[test]
    fn ranges_accept_the_boundary_and_reject_a_missing_value() {
        let price = FilterValue::NumberRange {
            min: Some(10.0),
            max: Some(20.0),
        };
        assert!(CellValue::number(10.0).matches(&price));
        assert!(CellValue::number(20.0).matches(&price));
        assert!(!CellValue::number(20.5).matches(&price));
        assert!(CellValue::text("15").matches(&price));
        assert!(!CellValue::text("15px").matches(&price));
        assert!(!CellValue::text("").matches(&price));
        assert!(!CellValue::boolean(true).matches(&price));

        let created = FilterValue::DateRange {
            from: Some("2026-05-01".into()),
            to: Some("2026-05-31".into()),
        };
        assert!(CellValue::text("2026-05-01T10:00:00Z").matches(&created));
        assert!(!CellValue::text("2026-04-30").matches(&created));
        assert!(!CellValue::number(1.0).matches(&created));
    }

    #[test]
    fn boolean_filter_treats_false_as_a_real_answer() {
        assert!(CellValue::boolean(false).matches(&FilterValue::Boolean(false)));
        assert!(!CellValue::boolean(true).matches(&FilterValue::Boolean(false)));
        assert!(!CellValue::text("false").matches(&FilterValue::Boolean(false)));
        assert!(!CellValue::Empty.matches(&FilterValue::Boolean(false)));
    }

    #[test]
    fn ordering_is_total_kind_aware_and_locale_free() {
        let cells = [
            CellValue::text("b"),
            CellValue::Empty,
            CellValue::number(2.0),
            CellValue::boolean(true),
            CellValue::text("а"),
            CellValue::number(1.5),
        ];
        let mut sorted = cells.to_vec();
        sorted.sort_by(|left, right| left.compare(right));
        assert_eq!(
            sorted,
            vec![
                CellValue::Empty,
                CellValue::boolean(true),
                CellValue::number(1.5),
                CellValue::number(2.0),
                // Code-point order, not locale collation: Cyrillic 'а' (U+0430) after 'b'.
                CellValue::text("b"),
                CellValue::text("а"),
            ]
        );
        assert_eq!(CellValue::Empty.compare(&CellValue::Empty), Ordering::Equal);
    }

    #[test]
    fn only_plain_decimal_spellings_are_numbers() {
        assert!(is_decimal_spelling("42"));
        assert!(is_decimal_spelling("-3.5"));
        assert!(is_decimal_spelling(".5"));
        assert!(is_decimal_spelling("2e-3"));
        assert!(is_decimal_spelling("+7"));
        assert!(!is_decimal_spelling("15px"));
        assert!(!is_decimal_spelling("0x10"));
        assert!(!is_decimal_spelling("Infinity"));
        assert!(!is_decimal_spelling("NaN"));
        assert!(!is_decimal_spelling("1e"));
        assert!(!is_decimal_spelling("."));
        assert!(!is_decimal_spelling("1.2.3"));
        assert!(!is_decimal_spelling("1 000"));
    }

    #[test]
    fn non_finite_numbers_never_reach_the_filter_or_the_order() {
        let nan = CellValue::Number(f64::NAN);
        assert!(!nan.matches(&FilterValue::NumberRange {
            min: None,
            max: None
        }));
        assert_eq!(nan.rank(), CellValue::Empty.rank());
        assert!(matches!(CellValue::number(f64::INFINITY), CellValue::Empty));
    }
}
