//! Admin catalog facets: the owner's counts mapped into the shared grid contract.
//!
//! The Product owner counts the buckets (`CatalogService::admin_catalog_facets`), `rustok-grid`
//! owns the panel semantics, and this module is the seam between the two for the admin surface:
//! it maps the owner rows into [`GridFacet`] values and renders the panel view the admin grid
//! adapter paints.
//!
//! Two properties are deliberate:
//!
//! * the mapped facets carry the very same `code=<value>` vocabulary the admin attribute-filter
//!   URL parameter already uses, so a bucket toggle is just another selection entry;
//! * the admin panel counts every lifecycle status the list shows (drafts and archived rows
//!   included), which is exactly what separates it from the storefront panel over the same
//!   contract.

use rustok_grid::facet_panel::{
    FacetPanel, FacetPanelLabels, count_label, has_selection_for_key, selection_after_clear_key,
    selection_after_toggle,
};
use rustok_grid::{FacetDomain, FacetValue, GridFacet};
use rustok_ui_core::apply_ui_query_pairs;

use crate::catalog_controls::{ProductAdminListInput, serialize_attribute_filters};
use crate::i18n::t;
use crate::model::{AdminCatalogFacet, AdminCatalogFacetValue};

/// Maps owner-counted admin facets into the shared grid facet contract.
///
/// The domain is derived from the owner's `is_enumerable` flag first and from the stored value
/// type second, so a bounded dictionary keeps its `multi` flag and an unbounded domain keeps its
/// open semantics even if a future value type arrives with a different name.
pub fn admin_catalog_facets_to_grid(facets: &[AdminCatalogFacet]) -> Vec<GridFacet> {
    facets
        .iter()
        .map(|facet| {
            let domain = if !facet.is_enumerable {
                FacetDomain::Open
            } else if facet.value_type.eq_ignore_ascii_case("boolean") {
                FacetDomain::Boolean
            } else {
                FacetDomain::Dictionary {
                    multi: facet.value_type.eq_ignore_ascii_case("multiselect"),
                }
            };
            let mut mapped = GridFacet::from_buckets(
                facet.code.as_str(),
                facet.label.as_str(),
                domain,
                facet.total_products,
                facet.values.iter().map(|value| {
                    FacetValue::new(value.value.clone(), value.label.clone(), value.count)
                }),
            );
            // Truncation is either the owner cutting its own value limit or this mapping cutting
            // ours; both mean "there are more values than shown", and neither may be dropped.
            mapped.is_truncated |= facet.is_truncated;
            mapped
        })
        .collect()
}

/// Copy of the admin facet panel, resolved from the admin locale file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductAdminFacetLabels {
    pub title: String,
    pub unbounded_hint: String,
    pub truncated_hint: String,
    pub clear_label: String,
    pub empty_message: String,
    pub count_template: String,
    pub selected_marker: String,
    pub unselected_marker: String,
}

impl ProductAdminFacetLabels {
    fn to_grid_labels(&self) -> FacetPanelLabels {
        FacetPanelLabels {
            title: self.title.clone(),
            unbounded_hint: self.unbounded_hint.clone(),
            truncated_hint: self.truncated_hint.clone(),
            clear_label: self.clear_label.clone(),
            empty_message: self.empty_message.clone(),
            count_template: self.count_template.clone(),
            selected_marker: self.selected_marker.clone(),
            unselected_marker: self.unselected_marker.clone(),
        }
    }
}

pub fn build_product_admin_facet_labels(locale: Option<&str>) -> ProductAdminFacetLabels {
    ProductAdminFacetLabels {
        title: t(locale, "product.list.facetsLabel", "Filters"),
        unbounded_hint: t(
            locale,
            "product.list.facetsUnbounded",
            "Enter a value in the filter field above.",
        ),
        truncated_hint: t(
            locale,
            "product.list.facetsTruncated",
            "More values are available than shown.",
        ),
        clear_label: t(locale, "product.list.facetsClear", "Clear filters"),
        empty_message: t(
            locale,
            "product.list.facetsEmpty",
            "No filters are available for this catalog yet.",
        ),
        count_template: t(locale, "product.list.facetsCount", "({count})"),
        selected_marker: t(locale, "product.list.facetsSelected", "[x]"),
        unselected_marker: t(locale, "product.list.facetsUnselected", "[ ]"),
    }
}

/// One bucket of the rendered panel: a link that toggles exactly this selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductAdminFacetValueView {
    pub value: String,
    pub label: String,
    pub count_label: String,
    pub selected: bool,
    pub marker: String,
    pub href: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductAdminFacetView {
    pub code: String,
    pub label: String,
    pub is_enumerable: bool,
    pub unbounded_hint: Option<String>,
    pub is_truncated: bool,
    pub truncated_hint: Option<String>,
    /// Link that drops this facet's selections; `None` while nothing of it is selected.
    pub clear_href: Option<String>,
    pub clear_label: String,
    pub values: Vec<ProductAdminFacetValueView>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductAdminFacetPanelView {
    pub title: String,
    pub facets: Vec<ProductAdminFacetView>,
    pub show_empty_state: bool,
    pub empty_message: String,
    /// Link that drops every attribute-filter selection; `None` when nothing is selected.
    pub clear_href: Option<String>,
    pub clear_label: String,
}

/// Attribute codes the admin panel asks the owner to count, in catalog search-option order.
///
/// Blank and duplicate codes are dropped, and the order follows the options the admin offers as
/// filter inputs, so the panel never asks for a facet the operator cannot recognise.
pub fn build_product_admin_facet_codes(
    options: &crate::model::ProductCatalogSearchOptions,
) -> Vec<String> {
    let mut codes: Vec<String> = Vec::new();
    for option in &options.attribute_options {
        let code = option.value.trim();
        if code.is_empty() || codes.iter().any(|existing| existing == code) {
            continue;
        }
        codes.push(code.to_string());
    }
    codes
}

/// Builds the whole admin panel view model: buckets, selection state and the exact links.
///
/// Selection always round-trips through the URL, so the panel keeps no client-side state and every
/// filtered grid stays a shareable link.
pub fn build_product_admin_facet_panel(
    module_route_base: &str,
    facets: &[AdminCatalogFacet],
    controls: &ProductAdminListInput,
    labels: ProductAdminFacetLabels,
) -> ProductAdminFacetPanelView {
    let grid_facets = admin_catalog_facets_to_grid(facets);
    let panel = FacetPanel::build(
        grid_facets.as_slice(),
        controls.attribute_filters.as_slice(),
        &labels.to_grid_labels(),
    );

    let facets = panel
        .facets
        .iter()
        .map(|facet| {
            let clear_selection = selection_after_clear_key(
                controls.attribute_filters.as_slice(),
                facet.key.as_str(),
            );
            ProductAdminFacetView {
                code: facet.key.clone(),
                label: facet.label.clone(),
                is_enumerable: facet.is_enumerable,
                unbounded_hint: facet.unbounded_hint.clone(),
                is_truncated: facet.is_truncated,
                truncated_hint: facet.truncated_hint.clone(),
                clear_href: has_selection_for_key(
                    controls.attribute_filters.as_slice(),
                    facet.key.as_str(),
                )
                .then(|| build_facet_href(module_route_base, controls, clear_selection)),
                clear_label: facet.clear_label.clone(),
                values: facet
                    .values
                    .iter()
                    .map(|value| ProductAdminFacetValueView {
                        value: value.value.clone(),
                        label: value.label.clone(),
                        count_label: count_label(labels.count_template.as_str(), value.count),
                        selected: value.selected,
                        marker: value.marker.clone(),
                        href: build_facet_href(
                            module_route_base,
                            controls,
                            selection_after_toggle(
                                controls.attribute_filters.as_slice(),
                                facet.key.as_str(),
                                value.value.as_str(),
                            ),
                        ),
                    })
                    .collect(),
            }
        })
        .collect::<Vec<_>>();

    ProductAdminFacetPanelView {
        title: panel.title,
        clear_href: panel
            .has_selection
            .then(|| build_facet_href(module_route_base, controls, Vec::new())),
        clear_label: panel.clear_label,
        show_empty_state: panel.show_empty_state,
        empty_message: panel.empty_message,
        facets,
    }
}

fn build_facet_href(
    module_route_base: &str,
    controls: &ProductAdminListInput,
    filters: Vec<String>,
) -> String {
    let serialized = serialize_attribute_filters(filters.as_slice());
    apply_ui_query_pairs(
        module_route_base,
        &[
            ("category_id", controls.category_id.clone()),
            ("sort_by", controls.sort_by.clone()),
            ("sort_direction", controls.sort_direction.clone()),
            (
                "attribute_filters",
                (!serialized.is_empty()).then_some(serialized),
            ),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ProductCatalogSearchOptions;

    fn controls(filters: &[&str]) -> ProductAdminListInput {
        ProductAdminListInput {
            category_id: Some("category-1".to_string()),
            sort_by: Some("created_at".to_string()),
            sort_direction: Some("asc".to_string()),
            attribute_filters: filters.iter().map(|value| (*value).to_string()).collect(),
            ..ProductAdminListInput::default()
        }
    }

    fn facets() -> Vec<AdminCatalogFacet> {
        vec![
            AdminCatalogFacet {
                code: "color".to_string(),
                label: "Color".to_string(),
                value_type: "select".to_string(),
                is_localized: false,
                is_enumerable: true,
                is_truncated: false,
                total_products: 6,
                values: vec![
                    AdminCatalogFacetValue {
                        value: "red".to_string(),
                        label: "Red".to_string(),
                        count: 2,
                    },
                    AdminCatalogFacetValue {
                        value: "blue".to_string(),
                        label: "Blue".to_string(),
                        count: 4,
                    },
                ],
            },
            AdminCatalogFacet {
                code: "weight".to_string(),
                label: "Weight".to_string(),
                value_type: "decimal".to_string(),
                is_localized: false,
                is_enumerable: false,
                is_truncated: false,
                total_products: 3,
                values: Vec::new(),
            },
        ]
    }

    #[test]
    fn owner_rows_map_into_the_shared_grid_contract() {
        let mapped = admin_catalog_facets_to_grid(facets().as_slice());
        assert_eq!(mapped.len(), 2);
        assert_eq!(mapped[0].domain, FacetDomain::Dictionary { multi: false });
        assert!(mapped[0].is_enumerable());
        assert_eq!(mapped[0].values.len(), 2);
        assert_eq!(mapped[0].total, 6);
        assert_eq!(mapped[1].domain, FacetDomain::Open);
        assert!(mapped[1].values.is_empty());
    }

    #[test]
    fn panel_view_marks_selection_and_builds_exact_links() {
        let panel = build_product_admin_facet_panel(
            "/modules/product",
            facets().as_slice(),
            &controls(&["color=red"]),
            build_product_admin_facet_labels(Some("en")),
        );

        assert!(!panel.show_empty_state);
        assert_eq!(panel.facets.len(), 2);
        let color = &panel.facets[0];
        assert_eq!(
            color.clear_href.as_deref(),
            Some("/modules/product?category_id=category-1&sort_by=created_at&sort_direction=asc")
        );
        assert!(color.values[0].selected);
        assert_eq!(color.values[0].marker, "[x]");
        assert_eq!(color.values[0].count_label, "(2)");
        assert_eq!(
            color.values[1].href,
            "/modules/product?category_id=category-1&sort_by=created_at&sort_direction=asc&attribute_filters=color%3Dred%3Bcolor%3Dblue"
        );
        assert_eq!(panel.facets[1].unbounded_hint.is_some(), true);
        assert_eq!(panel.facets[1].clear_href, None);
        assert_eq!(
            panel.clear_href.as_deref(),
            Some("/modules/product?category_id=category-1&sort_by=created_at&sort_direction=asc")
        );
    }

    #[test]
    fn facet_codes_follow_options_without_blanks_or_duplicates() {
        let options = ProductCatalogSearchOptions {
            category_options: Vec::new(),
            attribute_options: vec![
                crate::model::ProductCatalogSearchOption {
                    value: " color ".to_string(),
                    label: "Color".to_string(),
                },
                crate::model::ProductCatalogSearchOption {
                    value: "color".to_string(),
                    label: "Color".to_string(),
                },
                crate::model::ProductCatalogSearchOption {
                    value: "   ".to_string(),
                    label: "Blank".to_string(),
                },
                crate::model::ProductCatalogSearchOption {
                    value: "size".to_string(),
                    label: "Size".to_string(),
                },
            ],
        };
        assert_eq!(
            build_product_admin_facet_codes(&options),
            vec!["color".to_string(), "size".to_string()]
        );
    }

    #[test]
    fn empty_owner_answer_renders_the_empty_state_without_a_clear_link() {
        let panel = build_product_admin_facet_panel(
            "/modules/product",
            &[],
            &controls(&[]),
            build_product_admin_facet_labels(None),
        );
        assert!(panel.show_empty_state);
        assert_eq!(panel.clear_href, None);
        assert_eq!(panel.clear_label, "Clear filters");
    }
}
