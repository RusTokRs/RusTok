//! Panel semantics for server-computed facets.
//!
//! [`crate::facet`] carries *what* the owner counted; this module carries *how a grid panel shows
//! it and how a selection changes*: which bucket is active, what the count reads, which hint a
//! bounded or unbounded domain gets, and what the selection looks like after a toggle or a clear.
//!
//! Keeping that logic here means every adapter — the Leptos admin grid, the storefront panels, a
//! future React table — renders the same numbers and mutates the same selection language, and the
//! rules stay testable without a framework. The copy itself is *not* owned here: an adapter passes
//! [`FacetPanelLabels`], because only the adapter knows the locale.

use crate::facet::{
    FacetValue, GridFacet, MAX_GRID_FACETS, SELECTION_SEPARATOR, selection_except, selection_key,
};

/// Copy and markers an adapter supplies for one panel rendering.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FacetPanelLabels {
    pub title: String,
    /// Sub-label of one unbounded facet: the free-form input stays the UI.
    pub unbounded_hint: String,
    /// Shown under a facet whose bucket list the owner cut.
    pub truncated_hint: String,
    pub clear_label: String,
    pub empty_message: String,
    /// Bucket count template, e.g. `({count})`.
    pub count_template: String,
    pub selected_marker: String,
    pub unselected_marker: String,
}

impl FacetPanelLabels {
    /// Labels with the English defaults the audit-traced panels use; adapters override per locale.
    pub fn english() -> Self {
        Self {
            title: "Filters".to_string(),
            unbounded_hint: "Enter a value in the filter field above.".to_string(),
            truncated_hint: "More values are available than shown.".to_string(),
            clear_label: "Clear filters".to_string(),
            empty_message: "No filters are available for this catalog yet.".to_string(),
            count_template: "({count})".to_string(),
            selected_marker: "[x]".to_string(),
            unselected_marker: "[ ]".to_string(),
        }
    }
}

/// One bucket rendered by the adapter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FacetPanelValue {
    /// Selection value of the bucket in the `key=<value>` vocabulary.
    pub value: String,
    pub label: String,
    pub count: u64,
    /// Count rendered through the panel template.
    pub count_label: String,
    pub selected: bool,
    /// Checkbox-like marker the adapter prints in front of the label.
    pub marker: String,
}

impl FacetPanelValue {
    /// The selection entry this bucket toggles, e.g. `color=blue`.
    pub fn selection_entry(&self, key: &str) -> String {
        selection_entry(key, self.value.as_str())
    }
}

/// One facet of the panel with its bucket list and hints.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FacetPanelFacet {
    pub key: String,
    pub label: String,
    pub is_enumerable: bool,
    /// Set only for unbounded domains.
    pub unbounded_hint: Option<String>,
    pub is_truncated: bool,
    /// Set only for a truncated bucket list.
    pub truncated_hint: Option<String>,
    pub clear_label: String,
    /// True while at least one bucket of this facet is selected.
    pub has_selection: bool,
    /// Products matching every other active facet that carry a value for this facet.
    pub total: u64,
    pub values: Vec<FacetPanelValue>,
}

/// A whole panel: the facets to render, the copy, and the selection they act on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FacetPanel {
    pub title: String,
    pub facets: Vec<FacetPanelFacet>,
    pub clear_label: String,
    pub empty_message: String,
    /// True when the owner answered no countable facet at all.
    pub show_empty_state: bool,
    /// Selection the panel renders and rewrites, in its original order.
    pub selected: Vec<String>,
    /// True while any selection is active; drives the panel-wide clear action.
    pub has_selection: bool,
}

impl FacetPanel {
    /// Builds a panel from the owner's facets, the active selection and the adapter's copy.
    ///
    /// At most [`MAX_GRID_FACETS`] facets are rendered, mirroring the request limit, so a panel can
    /// never ask the UI to draw more than the grid is allowed to request.
    pub fn build(facets: &[GridFacet], selected: &[String], labels: &FacetPanelLabels) -> Self {
        let facets = facets
            .iter()
            .take(MAX_GRID_FACETS)
            .map(|facet| FacetPanelFacet {
                key: facet.key.clone(),
                label: facet.label.clone(),
                is_enumerable: facet.is_enumerable(),
                unbounded_hint: (!facet.is_enumerable()).then(|| labels.unbounded_hint.clone()),
                is_truncated: facet.is_truncated,
                truncated_hint: facet.is_truncated.then(|| labels.truncated_hint.clone()),
                clear_label: labels.clear_label.clone(),
                has_selection: has_selection_for_key(selected, facet.key.as_str()),
                total: facet.total,
                values: facet
                    .values
                    .iter()
                    .map(|value| panel_value(facet.key.as_str(), value, selected, labels))
                    .collect(),
            })
            .collect::<Vec<_>>();

        Self {
            title: labels.title.clone(),
            show_empty_state: facets.is_empty(),
            empty_message: labels.empty_message.clone(),
            clear_label: labels.clear_label.clone(),
            has_selection: !selected.is_empty(),
            facets,
            selected: selected.to_vec(),
        }
    }

    /// Selection of this panel after `key=value` is flipped.
    pub fn toggle(&self, key: &str, value: &str) -> Vec<String> {
        selection_after_toggle(self.selected.as_slice(), key, value)
    }

    /// Selection of this panel without any selection of `key`.
    pub fn clear_key(&self, key: &str) -> Vec<String> {
        selection_after_clear_key(self.selected.as_slice(), key)
    }

    /// Selection of this panel without any facet selection.
    pub fn clear(&self) -> Vec<String> {
        selection_after_clear(self.selected.as_slice())
    }

    /// True when `key=value` is active in this panel.
    pub fn is_selected(&self, key: &str, value: &str) -> bool {
        is_selection_selected(self.selected.as_slice(), key, value)
    }

    /// The selected entries that belong to `key`.
    pub fn selected_for_key<'a>(&'a self, key: &str) -> Vec<&'a str> {
        selection_for_key(self.selected.as_slice(), key)
    }

    /// The selection entries the owner must keep while counting `facet`.
    pub fn other_selection_for_key<'a>(&'a self, key: &str) -> Vec<&'a str> {
        selection_except(self.selected.as_slice(), key)
    }
}

fn panel_value(
    key: &str,
    value: &FacetValue,
    selected: &[String],
    labels: &FacetPanelLabels,
) -> FacetPanelValue {
    let is_selected = is_selection_selected(selected, key, value.value.as_str());
    FacetPanelValue {
        value: value.value.clone(),
        label: value.label.clone(),
        count: value.count,
        count_label: count_label(labels.count_template.as_str(), value.count),
        selected: is_selected,
        marker: if is_selected {
            labels.selected_marker.clone()
        } else {
            labels.unselected_marker.clone()
        },
    }
}

/// Renders a bucket count through the adapter's template.
pub fn count_label(template: &str, count: u64) -> String {
    template.replace("{count}", &count.to_string())
}

/// The `key=value` selection entry a bucket toggles.
pub fn selection_entry(key: &str, value: &str) -> String {
    let mut entry = String::with_capacity(key.len() + value.len() + 1);
    entry.push_str(key);
    entry.push(SELECTION_SEPARATOR);
    entry.push_str(value);
    entry
}

/// Splits one selection entry into `(key, value)`; `None` when it addresses no facet.
pub fn split_selection(entry: &str) -> Option<(&str, &str)> {
    let (key, value) = entry.split_once(SELECTION_SEPARATOR)?;
    let key = key.trim();
    let value = value.trim();
    (!key.is_empty() && !value.is_empty()).then_some((key, value))
}

/// True when `key=value` is part of `selected`; keys compare case-insensitively, values exactly.
pub fn is_selection_selected(selected: &[String], key: &str, value: &str) -> bool {
    let key = key.trim();
    let value = value.trim();
    selected.iter().any(|entry| {
        matches!(split_selection(entry), Some((entry_key, entry_value))
            if entry_key.eq_ignore_ascii_case(key) && entry_value == value)
    })
}

/// True when any value of `key` is selected; keys compare case-insensitively.
pub fn has_selection_for_key(selected: &[String], key: &str) -> bool {
    let key = key.trim();
    !key.is_empty()
        && selected
            .iter()
            .any(|entry| matches!(selection_key(entry), Some(entry_key) if entry_key.eq_ignore_ascii_case(key)))
}

/// The selected entries that belong to `key`, in their original order.
pub fn selection_for_key<'a>(selected: &'a [String], key: &str) -> Vec<&'a str> {
    let key = key.trim();
    if key.is_empty() {
        return Vec::new();
    }
    selected
        .iter()
        .map(String::as_str)
        .filter(|entry| {
            matches!(split_selection(entry), Some((entry_key, _)) if entry_key.eq_ignore_ascii_case(key))
        })
        .collect()
}

/// Flips `key=value`: an active entry is dropped, an unknown one is appended, and every surviving
/// entry keeps its position, so repeated toggles produce stable links.
pub fn selection_after_toggle(selected: &[String], key: &str, value: &str) -> Vec<String> {
    let key = key.trim();
    let value = value.trim();
    if key.is_empty() || value.is_empty() {
        return selected.to_vec();
    }
    let mut toggled = Vec::with_capacity(selected.len() + 1);
    let mut removed = false;
    for entry in selected {
        match split_selection(entry) {
            Some((entry_key, entry_value))
                if !removed && entry_key.eq_ignore_ascii_case(key) && entry_value == value =>
            {
                removed = true;
            }
            _ => toggled.push(entry.clone()),
        }
    }
    if !removed {
        toggled.push(selection_entry(key, value));
    }
    toggled
}

/// Drops every selection of `key`, keeping the other facets' selections.
pub fn selection_after_clear_key(selected: &[String], key: &str) -> Vec<String> {
    let key = key.trim();
    if key.is_empty() {
        return selected.to_vec();
    }
    selected
        .iter()
        .filter(|entry| {
            !matches!(split_selection(entry), Some((entry_key, _)) if entry_key.eq_ignore_ascii_case(key))
        })
        .cloned()
        .collect()
}

/// Drops every facet selection.
pub fn selection_after_clear(_selected: &[String]) -> Vec<String> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::facet::{FacetDomain, MAX_GRID_FACET_VALUES};

    fn selected(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    fn color_facet() -> GridFacet {
        GridFacet::from_buckets(
            "color",
            "Color",
            FacetDomain::Dictionary { multi: true },
            9,
            vec![
                FacetValue::new("red", "Red", 4),
                FacetValue::new("blue", "Blue", 5),
            ],
        )
    }

    fn panel(facets: &[GridFacet], selection: &[&str]) -> FacetPanel {
        FacetPanel::build(facets, &selected(selection), &FacetPanelLabels::english())
    }

    #[test]
    fn panel_marks_selection_counts_and_hints() {
        let weight = GridFacet::open("weight", "Weight", 3);
        let facets = vec![color_facet(), weight.clone()];
        let panel = panel(&facets, &["color=red"]);

        assert!(!panel.show_empty_state);
        assert!(panel.has_selection);
        assert_eq!(panel.facets.len(), 2);

        let color = &panel.facets[0];
        assert_eq!(color.key, "color");
        assert!(color.has_selection);
        assert!(color.is_enumerable);
        assert_eq!(color.unbounded_hint, None);
        assert_eq!(color.values.len(), 2);
        assert_eq!(color.values[0].marker, "[x]");
        assert_eq!(color.values[0].count_label, "(4)");
        assert_eq!(color.values[1].marker, "[ ]");
        assert_eq!(color.values[1].selection_entry("color"), "color=blue");

        let weight = &panel.facets[1];
        assert!(!weight.is_enumerable);
        assert_eq!(
            weight.unbounded_hint.as_deref(),
            Some("Enter a value in the filter field above.")
        );
        assert!(!weight.has_selection);
        assert!(weight.values.is_empty());
        assert!(panel.is_selected("color", "red"));
        assert!(!panel.is_selected("color", "green"));
        assert_eq!(panel.selected_for_key("color"), vec!["color=red"]);
    }

    #[test]
    fn panel_marks_truncated_bucket_lists() {
        let buckets = (0..MAX_GRID_FACET_VALUES + 2)
            .map(|index| FacetValue::new(format!("option_{index}"), "Option", 1));
        let facet = GridFacet::from_buckets(
            "size",
            "Size",
            FacetDomain::Dictionary { multi: false },
            2,
            buckets,
        );
        let panel = panel(&[facet], &[]);

        let size = &panel.facets[0];
        assert!(size.is_truncated);
        assert_eq!(
            size.truncated_hint.as_deref(),
            Some("More values are available than shown.")
        );
        assert_eq!(size.values.len(), MAX_GRID_FACET_VALUES);
    }

    #[test]
    fn panel_renders_an_empty_state_without_facets() {
        let panel = panel(&[], &["color=red"]);
        assert!(panel.show_empty_state);
        assert_eq!(panel.facets.len(), 0);
        assert!(panel.has_selection);
    }

    #[test]
    fn panel_is_capped_at_the_grid_facet_limit() {
        let facets = (0..MAX_GRID_FACETS + 3)
            .map(|index| GridFacet::open(format!("facet_{index}"), "Facet", 1))
            .collect::<Vec<_>>();
        let panel = panel(&facets, &[]);
        assert_eq!(panel.facets.len(), MAX_GRID_FACETS);
    }

    #[test]
    fn toggling_keeps_order_and_round_trips() {
        let panel = panel(&[color_facet()], &["color=red", "size=m"]);

        assert_eq!(
            panel.toggle("color", "blue"),
            selected(&["color=red", "size=m", "color=blue"])
        );
        assert_eq!(panel.toggle("color", "red"), selected(&["size=m"]));
        assert_eq!(
            panel
                .toggle("color", "red")
                .iter()
                .filter(|entry| *entry == "color=blue")
                .count(),
            0
        );
        assert_eq!(panel.toggle("   ", "blue"), panel.selected);
        assert_eq!(panel.clear_key("color"), selected(&["size=m"]));
        assert_eq!(panel.clear_key("missing"), panel.selected);
        assert!(panel.clear().is_empty());
        assert_eq!(panel.other_selection_for_key("color"), vec!["size=m"]);
    }

    #[test]
    fn selection_keys_compare_case_insensitively_and_values_exactly() {
        let selection = selected(&["SIZE=m", "color=red"]);
        assert!(is_selection_selected(&selection, "size", "m"));
        assert!(!is_selection_selected(&selection, "size", "M"));
        assert!(has_selection_for_key(&selection, "COLOR"));
        assert!(!has_selection_for_key(&selection, "   "));
        assert_eq!(selection_for_key(&selection, "size"), vec!["SIZE=m"]);
        assert_eq!(split_selection("noise"), None);
        assert_eq!(split_selection("color="), None);
        assert_eq!(selection_entry("color", "red"), "color=red");
        assert_eq!(count_label("({count})", 7), "(7)");
    }
}
