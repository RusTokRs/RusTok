//! Server-computed facet descriptors and the drill-down selection semantics for grids.
//!
//! A grid can render server facets without knowing how they were counted: the owning service or
//! module answers with a count per bucket, and this module carries the contract that every grid
//! shares — bucket limits, the enumerable/open distinction, truncation, and the "exclude the
//! facet's own selection" rule that makes the counts behave as drill-down numbers
//! (OR within one facet, AND between facets).
//!
//! The types stay database- and framework-agnostic on purpose: computing counts is the owner's
//! job, rendering is the UI adapter's job, and everything in between is this contract.

use serde::{Deserialize, Serialize};

use crate::filter::FilterOption;

/// Maximum number of facets one grid request may ask for.
pub const MAX_GRID_FACETS: usize = 8;
/// Maximum number of bucket values one enumerable facet carries.
pub const MAX_GRID_FACET_VALUES: usize = 20;
/// Separator between the key and the value of one selection entry (`code=value`).
pub const SELECTION_SEPARATOR: char = '=';

/// The key part of one `key=value` selection entry.
///
/// Returns `None` when the entry has no separator or an empty key, so callers can ignore entries
/// that could never address a facet.
pub fn selection_key(entry: &str) -> Option<&str> {
    let (key, _) = entry.split_once(SELECTION_SEPARATOR)?;
    let key = key.trim();
    (!key.is_empty()).then_some(key)
}

/// The selection entries that belong to a facet other than `own_key`.
///
/// Counting a facet must ignore that facet's own selection, otherwise every bucket of an active
/// facet keeps reporting the filtered count instead of the drill-down count. Keys compare
/// case-insensitively; entries without a separator are dropped because they address nothing.
pub fn selection_except<'a>(selected: &'a [String], own_key: &str) -> Vec<&'a str> {
    let own_key = own_key.trim();
    selected
        .iter()
        .map(String::as_str)
        .filter(|entry| {
            selection_key(entry)
                .map(|key| !key.eq_ignore_ascii_case(own_key))
                .unwrap_or(false)
        })
        .collect()
}

/// One bucket of an enumerable facet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FacetValue {
    /// Selection value for this bucket, in the same `key=<value>` vocabulary the grid filters use.
    pub value: String,
    pub label: String,
    pub count: u64,
}

impl FacetValue {
    pub fn new(value: impl Into<String>, label: impl Into<String>, count: u64) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            count,
        }
    }

    /// The bucket as a select-filter option; the count is dropped because [`FilterOption`] is the
    /// chosen-option list, not a counted one.
    pub fn to_filter_option(&self) -> FilterOption {
        FilterOption::new(self.value.clone(), self.label.clone())
    }
}

/// Shape of a facet's value domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FacetDomain {
    /// Bounded dictionary of options; `multi` marks a multi-select domain.
    Dictionary { multi: bool },
    /// Boolean domain with `true`/`false` buckets.
    Boolean,
    /// Unbounded domain (free text, numbers, dates): values stay empty and the UI keeps its input.
    Open,
}

impl FacetDomain {
    /// True when the domain is bounded and its buckets can be enumerated.
    pub fn is_enumerable(self) -> bool {
        !matches!(self, Self::Open)
    }
}

/// One server-computed facet of a grid column.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GridFacet {
    pub key: String,
    pub label: String,
    pub domain: FacetDomain,
    /// True when the bucket list was cut at [`MAX_GRID_FACET_VALUES`].
    pub is_truncated: bool,
    /// Products matching every other active facet that carry a value for this facet.
    pub total: u64,
    pub values: Vec<FacetValue>,
}

impl GridFacet {
    /// Builds a facet from counted buckets.
    ///
    /// The value list is cut at [`MAX_GRID_FACET_VALUES`] and [`GridFacet::is_truncated`] reports
    /// it, so a caller can pass the owner's rows without re-checking the limit; open domains keep
    /// an empty value list by construction.
    pub fn from_buckets(
        key: impl Into<String>,
        label: impl Into<String>,
        domain: FacetDomain,
        total: u64,
        buckets: impl IntoIterator<Item = FacetValue>,
    ) -> Self {
        let (values, is_truncated) = if domain.is_enumerable() {
            let mut values = Vec::new();
            let mut truncated = false;
            for bucket in buckets {
                if values.len() == MAX_GRID_FACET_VALUES {
                    truncated = true;
                    break;
                }
                values.push(bucket);
            }
            (values, truncated)
        } else {
            (Vec::new(), false)
        };

        Self {
            key: key.into(),
            label: label.into(),
            domain,
            is_truncated,
            total,
            values,
        }
    }

    /// Builds a facet for an unbounded domain: no buckets, only the carrying-product count.
    pub fn open(key: impl Into<String>, label: impl Into<String>, total: u64) -> Self {
        Self::from_buckets(key, label, FacetDomain::Open, total, Vec::new())
    }

    pub fn is_enumerable(&self) -> bool {
        self.domain.is_enumerable()
    }

    /// The facet's buckets as select-filter options, in bucket order (count descending).
    pub fn filter_options(&self) -> Vec<FilterOption> {
        self.values
            .iter()
            .map(FacetValue::to_filter_option)
            .collect()
    }

    /// The selection entries the caller must keep while *this* facet is counted.
    pub fn other_selection<'a>(&self, selected: &'a [String]) -> Vec<&'a str> {
        selection_except(selected, &self.key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn selected(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn selection_key_reads_the_part_before_the_separator() {
        assert_eq!(selection_key("color=blue"), Some("color"));
        assert_eq!(selection_key("  size =m=1 "), Some("size"));
        assert_eq!(selection_key("color=blue=green"), Some("color"));
        assert_eq!(selection_key("standalone"), None);
        assert_eq!(selection_key("=blue"), None);
    }

    #[test]
    fn drill_down_ignores_only_the_own_selection() {
        let selection = selected(&["color=blue", "SIZE=m", "noise", "size=l"]);
        assert_eq!(selection_except(&selection, "size"), vec!["color=blue"]);
        assert_eq!(
            selection_except(&selection, "color"),
            vec!["SIZE=m", "size=l"]
        );
        assert_eq!(selection_except(&selection, "unknown").len(), 3);
    }

    #[test]
    fn bounded_domains_are_truncated_at_the_bucket_limit() {
        let buckets = (0..MAX_GRID_FACET_VALUES + 3)
            .map(|index| FacetValue::new(format!("option_{index}"), format!("Option {index}"), 5));
        let facet = GridFacet::from_buckets(
            "color",
            "Color",
            FacetDomain::Dictionary { multi: true },
            42,
            buckets,
        );

        assert_eq!(facet.values.len(), MAX_GRID_FACET_VALUES);
        assert!(facet.is_truncated);
        assert_eq!(facet.total, 42);
        assert_eq!(facet.filter_options().len(), MAX_GRID_FACET_VALUES);
        assert_eq!(facet.values[0].count, 5);
        assert_eq!(
            facet.other_selection(&selected(&["color=blue"])),
            Vec::<&str>::new()
        );
    }

    #[test]
    fn open_domains_never_carry_buckets() {
        let facet = GridFacet::open("price", "Price", 7);
        assert!(!facet.is_enumerable());
        assert!(facet.values.is_empty());
        assert!(!facet.is_truncated);
        assert!(facet.filter_options().is_empty());
        assert_eq!(
            facet.other_selection(&selected(&["price=10"])),
            Vec::<&str>::new()
        );
    }

    #[test]
    fn a_full_but_cut_list_is_not_marked_truncated() {
        let buckets = (0..MAX_GRID_FACET_VALUES)
            .map(|index| FacetValue::new(format!("option_{index}"), "Option", 1));
        let facet = GridFacet::from_buckets("size", "Size", FacetDomain::Boolean, 3, buckets);
        assert_eq!(facet.values.len(), MAX_GRID_FACET_VALUES);
        assert!(!facet.is_truncated);
    }
}
