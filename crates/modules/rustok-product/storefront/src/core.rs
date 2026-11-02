pub(crate) mod buyer;

use rustok_api::locale_tags_match;
use rustok_pricing_storefront::{
    StorefrontPricingQueryError, parse_optional_currency_code, parse_optional_uuid_string,
    parse_pricing_route_quantity, parse_resolution_quantity, pricing_context_matches_request,
};
use rustok_ui_core::apply_ui_query_pairs;
// `resolve_requested_locale` is the only user, and it is an SSR-only entry point.
#[cfg(feature = "ssr")]
use rustok_ui_core::normalize_optional_ui_text;

use rustok_grid::facet_panel::{FacetPanel, count_label as grid_count_label};
use rustok_grid::{FacetDomain, FacetValue, GridFacet};

use crate::catalog_controls::{
    CatalogFacetLabels, CatalogListInput, clear_attribute_filter_code, serialize_attribute_filters,
    toggle_attribute_filter,
};
use crate::i18n::t;
use crate::model::{
    PricingResolutionContext, ProductCatalogFacet, ProductCatalogSearchOptions, ProductDetail,
    ProductPricingDetail, ProductTranslation, ProductVariant,
};

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RouteInput {
    pub handle: Option<String>,
    pub locale: Option<String>,
    pub currency_code: Option<String>,
    pub region_id: Option<String>,
    pub price_list_id: Option<String>,
    pub channel_id: Option<String>,
    pub channel_slug: Option<String>,
    pub quantity: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FetchRequest {
    pub selected_handle: Option<String>,
    pub locale: Option<String>,
    pub currency_code: Option<String>,
    pub region_id: Option<String>,
    pub price_list_id: Option<String>,
    pub channel_id: Option<String>,
    pub channel_slug: Option<String>,
    pub quantity: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShellViewModel {
    pub badge: String,
    pub title: String,
    pub subtitle: String,
    pub load_error: String,
}

pub const DEFAULT_ROUTE_SEGMENT: &str = "products";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductTransportErrorDomEvidence {
    pub failed_path: String,
    pub fallback_attempted: String,
    pub native_error: String,
    pub graphql_error: String,
    pub message: String,
}

pub fn build_transport_error_dom_evidence(
    context: &str,
    failed_path: &str,
    fallback_attempted: bool,
    native_error: Option<&str>,
    graphql_error: Option<&str>,
    error_message: &str,
) -> ProductTransportErrorDomEvidence {
    ProductTransportErrorDomEvidence {
        failed_path: failed_path.to_string(),
        fallback_attempted: fallback_attempted.to_string(),
        native_error: native_error.unwrap_or_default().to_string(),
        graphql_error: graphql_error.unwrap_or_default().to_string(),
        message: format!("{context}: {error_message}"),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn build_route_input(
    handle: Option<String>,
    locale: Option<String>,
    currency_code: Option<String>,
    region_id: Option<String>,
    price_list_id: Option<String>,
    channel_id: Option<String>,
    channel_slug: Option<String>,
    quantity: Option<String>,
) -> Result<RouteInput, StorefrontPricingQueryError> {
    let quantity = parse_pricing_route_quantity(quantity.as_deref())?;
    let request = FetchRequest {
        selected_handle: handle.clone(),
        locale: locale.clone(),
        currency_code: currency_code.clone(),
        region_id: region_id.clone(),
        price_list_id: price_list_id.clone(),
        channel_id: channel_id.clone(),
        channel_slug: channel_slug.clone(),
        quantity,
    };
    validate_fetch_request(&request)?;
    Ok(RouteInput {
        handle,
        locale,
        currency_code,
        region_id,
        price_list_id,
        channel_id,
        channel_slug,
        quantity,
    })
}

pub fn build_fetch_request(input: &RouteInput) -> FetchRequest {
    FetchRequest {
        selected_handle: input.handle.clone(),
        locale: input.locale.clone(),
        currency_code: input.currency_code.clone(),
        region_id: input.region_id.clone(),
        price_list_id: input.price_list_id.clone(),
        channel_id: input.channel_id.clone(),
        channel_slug: input.channel_slug.clone(),
        quantity: input.quantity,
    }
}

pub fn resolve_route_segment(route_segment: Option<&str>) -> String {
    route_segment
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_ROUTE_SEGMENT)
        .to_string()
}

/// Syntactic admission runs before IO; currency derivation happens only after a real catalog read.
pub(crate) fn validate_fetch_request(
    request: &FetchRequest,
) -> Result<(), StorefrontPricingQueryError> {
    parse_optional_currency_code(request.currency_code.clone())?;
    parse_optional_uuid_string(request.region_id.clone(), "region_id")?;
    parse_optional_uuid_string(request.price_list_id.clone(), "price_list_id")?;
    parse_optional_uuid_string(request.channel_id.clone(), "channel_id")?;
    parse_resolution_quantity(request.quantity)?;
    if request
        .channel_slug
        .as_deref()
        .is_some_and(|slug| slug.trim().is_empty())
    {
        return Err(StorefrontPricingQueryError::InvalidChannelSlug);
    }
    Ok(())
}

#[cfg(feature = "ssr")]
pub fn resolve_requested_locale(
    requested: Option<String>,
    request_context_locale: Option<&str>,
    tenant_default_locale: &str,
) -> String {
    normalize_optional_ui_text(requested)
        .or_else(|| {
            request_context_locale
                .and_then(|value| normalize_optional_ui_text(Some(value.to_string())))
        })
        .or_else(|| normalize_optional_ui_text(Some(tenant_default_locale.to_string())))
        .unwrap_or_default()
}

pub fn build_shell_view_model(locale: Option<&str>) -> ShellViewModel {
    ShellViewModel {
        badge: t(locale, "product.badge", "product"),
        title: t(
            locale,
            "product.title",
            "Published catalog from the product module",
        ),
        subtitle: t(
            locale,
            "product.subtitle",
            "Published products with sparse SKU selection and authoritative price observations through the selected native or GraphQL transport.",
        ),
        load_error: t(
            locale,
            "product.error.load",
            "Failed to load storefront product data",
        ),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectedProductEmptyViewModel {
    pub title: String,
    pub body: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectedProductViewModel {
    /// Owner-resolved product gallery for the detail panel.
    pub gallery: Vec<SelectedProductImageViewModel>,
    pub product_type: String,
    pub vendor: String,
    pub published_at: String,
    pub metadata_items: Vec<String>,
    pub seller_boundary: String,
    pub title: String,
    pub description: String,
    /// Header of the specifications block ("Specifications").
    pub attributes_label: String,
    /// Storefront-safe specifications: already localized by the Product owner, so the view only
    /// prints them. Empty when the cataloguer filled nothing for the storefront.
    pub attributes: Vec<SelectedProductAttributeViewModel>,
}

/// One specification row of the product detail panel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectedProductAttributeViewModel {
    pub code: String,
    pub label: String,
    /// One row per value: dictionaries contribute one entry per selected option, multiselect several.
    pub values: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectedProductImageViewModel {
    pub url: String,
    pub alt_text: String,
    pub is_primary: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductCatalogRailLabels {
    pub title: String,
    pub total_template: String,
    pub empty_message: String,
    pub open_label: String,
    pub catalog_fallback_label: String,
    pub vendor_fallback_label: String,
    /// Catalog-card price prefix, e.g. `from {value}`.
    pub price_from_template: String,
    /// Shown when the product has no base price yet.
    pub price_missing_label: String,
    /// Alt-text fallback for catalog-card media.
    pub image_alt_fallback: String,
}

pub fn build_product_catalog_rail_labels(locale: Option<&str>) -> ProductCatalogRailLabels {
    ProductCatalogRailLabels {
        title: t(locale, "product.list.title", "Published products"),
        total_template: t(locale, "product.list.total", "{count} total"),
        empty_message: t(
            locale,
            "product.list.empty",
            "No published products are available yet.",
        ),
        open_label: t(locale, "product.list.open", "Open"),
        catalog_fallback_label: t(locale, "product.selected.catalog", "catalog"),
        vendor_fallback_label: t(locale, "product.list.vendorFallback", "Independent label"),
        price_from_template: t(locale, "product.list.priceFrom", "from {value}"),
        price_missing_label: t(locale, "product.list.noPrice", "Price on request"),
        image_alt_fallback: t(locale, "product.list.imageAlt", "Product image: "),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductCatalogRailItemViewModel {
    pub product_type: String,
    pub title: String,
    pub vendor: String,
    pub seller_boundary: String,
    pub published_at: String,
    pub href: String,
    /// Catalog-card media URL resolved by the Product owner; `None` when the
    /// product has no image yet.
    pub image_url: Option<String>,
    pub image_alt: String,
    /// Catalog-card price snapshot label; `None` when the product has no base
    /// price yet.
    pub price_label: Option<String>,
    pub on_sale: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductCatalogRailViewModel {
    pub title: String,
    pub total_label: String,
    pub empty_message: String,
    pub open_label: String,
    /// Shown in the price slot when the product has no base price yet.
    pub price_missing_label: String,
    pub show_empty_state: bool,
    pub items: Vec<ProductCatalogRailItemViewModel>,
}

pub fn build_selected_product_empty_view_model(
    locale: Option<&str>,
) -> SelectedProductEmptyViewModel {
    SelectedProductEmptyViewModel {
        title: t(
            locale,
            "product.selected.emptyTitle",
            "No published product selected",
        ),
        body: t(
            locale,
            "product.selected.emptyBody",
            "Publish a product from the product admin package or open one with `?handle=`.",
        ),
    }
}

pub fn build_selected_product_view_model(
    product: &ProductDetail,
    locale: Option<&str>,
) -> SelectedProductViewModel {
    let translation = product_translation_for_locale(product.translations.as_slice(), locale);
    let title = translation
        .map(|item| item.title.clone())
        .unwrap_or_else(|| t(locale, "product.selected.untitled", "Untitled product"));
    let description = translation
        .and_then(|item| item.description.clone())
        .unwrap_or_else(|| {
            t(
                locale,
                "product.selected.noDescription",
                "No localized merchandising copy yet.",
            )
        });
    let product_type = product
        .product_type
        .clone()
        .unwrap_or_else(|| t(locale, "product.selected.catalog", "catalog"));
    let vendor = product.vendor.clone().unwrap_or_else(|| {
        t(
            locale,
            "product.selected.vendorFallback",
            "independent label",
        )
    });
    let published_at = product
        .published_at
        .clone()
        .unwrap_or_else(|| t(locale, "product.selected.unscheduled", "scheduled later"));
    let metadata_items = vec![product_type.clone(), vendor.clone(), published_at.clone()];
    // The owner sends display-ready values; only the boolean vocabulary needs the locale, because
    // it is the one value the owner cannot phrase (its i18n lives with the storefronts).
    let attributes = product
        .attributes
        .iter()
        .map(|attribute| SelectedProductAttributeViewModel {
            code: attribute.code.clone(),
            label: attribute.label.clone(),
            values: attribute
                .values
                .iter()
                .map(|value| {
                    if attribute.value_type.eq_ignore_ascii_case("boolean") {
                        match value.text.trim().to_ascii_lowercase().as_str() {
                            "true" => t(locale, "product.selected.attributeYes", "Yes"),
                            "false" => t(locale, "product.selected.attributeNo", "No"),
                            _ => value.text.clone(),
                        }
                    } else {
                        value.text.clone()
                    }
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    let gallery = product
        .images
        .iter()
        .enumerate()
        .map(|(index, image)| SelectedProductImageViewModel {
            url: image.url.clone(),
            alt_text: image
                .alt_text
                .clone()
                .unwrap_or_else(|| format!("{} {}", title.as_str(), index + 1)),
            is_primary: index == 0,
        })
        .collect::<Vec<_>>();

    SelectedProductViewModel {
        gallery,
        product_type,
        vendor,
        published_at,
        metadata_items,
        seller_boundary: format_seller_boundary(locale, product.seller_id.as_deref()),
        title,
        description,
        attributes_label: t(locale, "product.selected.attributes", "Specifications"),
        attributes,
    }
}

pub fn product_translation_for_locale<'a>(
    translations: &'a [ProductTranslation],
    requested_locale: Option<&str>,
) -> Option<&'a ProductTranslation> {
    requested_locale
        .and_then(|locale| {
            translations
                .iter()
                .find(|translation| locale_tags_match(&translation.locale, locale))
        })
        .or_else(|| translations.first())
}

pub fn format_seller_boundary(locale: Option<&str>, seller_id: Option<&str>) -> String {
    match seller_id.map(str::trim).filter(|value| !value.is_empty()) {
        Some(seller_id) => format!(
            "{}: {seller_id}",
            t(locale, "product.common.sellerId", "seller id")
        ),
        None => t(
            locale,
            "product.common.sellerUnassigned",
            "seller id: unassigned",
        ),
    }
}

pub fn format_product_price(
    locale: Option<&str>,
    currency_code: &str,
    amount: &str,
    compare_at_amount: Option<&str>,
    discount_percent: Option<&str>,
) -> String {
    let mut label = if let Some(compare_at_amount) = compare_at_amount {
        format!(
            "{} {} ({})",
            currency_code,
            amount,
            crate::i18n::format(
                locale,
                "product.selected.compareAt",
                Some(&rustok_ui_i18n::fluent_args!("value" => compare_at_amount.to_string())),
                "compare-at {value}",
            ),
        )
    } else {
        format!("{currency_code} {amount}")
    };

    if let Some(discount_percent) = discount_percent.filter(|value| !value.trim().is_empty()) {
        label.push_str(format!(" (-{discount_percent}%)").as_str());
    }

    label
}

pub fn format_pricing_scope(
    locale: Option<&str>,
    price_list_id: Option<&str>,
    channel_slug: Option<&str>,
    channel_id: Option<&str>,
) -> Option<String> {
    let price_list_id = price_list_id.filter(|value| !value.trim().is_empty());
    let channel_slug = channel_slug.filter(|value| !value.trim().is_empty());
    let channel_id = channel_id.filter(|value| !value.trim().is_empty());

    if price_list_id.is_none() && channel_slug.is_none() && channel_id.is_none() {
        return None;
    }

    let mut parts = Vec::new();
    if let Some(price_list_id) = price_list_id {
        parts.push(t(locale, "product.selected.priceList", "price list") + " " + price_list_id);
    }
    match (channel_slug, channel_id) {
        (Some(channel_slug), Some(channel_id)) => parts.push(
            t(locale, "product.selected.channel", "channel")
                + " "
                + channel_slug
                + " ("
                + channel_id
                + ")",
        ),
        (Some(channel_slug), None) => {
            parts.push(t(locale, "product.selected.channel", "channel") + " " + channel_slug)
        }
        (None, Some(channel_id)) => {
            parts.push(t(locale, "product.selected.channel", "channel") + " " + channel_id)
        }
        (None, None) => {}
    }

    Some(parts.join(" | "))
}

pub fn format_pricing_context(locale: Option<&str>, context: &PricingResolutionContext) -> String {
    let mut parts = vec![
        format!(
            "{} {}",
            t(locale, "product.selected.currency", "currency"),
            context.currency_code,
        ),
        format!(
            "{} {}",
            t(locale, "product.selected.quantity", "qty"),
            context.quantity,
        ),
    ];

    if let Some(region_id) = context.region_id.as_deref() {
        parts.push(format!(
            "{} {}",
            t(locale, "product.selected.region", "region"),
            region_id,
        ));
    }
    if let Some(scope) = format_pricing_scope(
        locale,
        context.price_list_id.as_deref(),
        context.channel_slug.as_deref(),
        context.channel_id.as_deref(),
    ) {
        parts.push(scope);
    }

    parts.join(" | ")
}

pub fn count_label(template: &str, total: u64) -> String {
    grid_count_label(template, total)
}

/// One facet bucket rendered as a toggle link; selection always round-trips through the URL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogFacetValueViewModel {
    pub value: String,
    pub label: String,
    pub count_label: String,
    pub selected: bool,
    /// Checkbox-like marker the adapter renders in front of the label.
    pub marker: String,
    pub href: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogFacetViewModel {
    pub code: String,
    pub label: String,
    pub is_enumerable: bool,
    /// Shown when the facet domain is unbounded and the storefront keeps its free-form input.
    pub unbounded_hint: Option<String>,
    pub is_truncated: bool,
    /// Shown when the owner cut the bucket list at its facet-value limit.
    pub truncated_hint: Option<String>,
    /// Link that drops this facet's selections; `None` while nothing of it is selected.
    pub clear_href: Option<String>,
    pub clear_label: String,
    pub values: Vec<CatalogFacetValueViewModel>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogFacetFiltersViewModel {
    pub title: String,
    pub facets: Vec<CatalogFacetViewModel>,
    pub show_empty_state: bool,
    pub empty_message: String,
    /// Link that drops every attribute-filter selection; `None` when nothing is selected.
    pub clear_href: Option<String>,
    pub clear_label: String,
}

/// Attribute codes the facet panel asks the owner to count, in catalog search-option order.
///
/// The owner counts only the facets a client asks for, so the storefront derives the request from
/// the same search options it offers as filter inputs — blank and duplicate codes are dropped.
pub fn build_catalog_facet_codes(options: &ProductCatalogSearchOptions) -> Vec<String> {
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

/// Maps one owner facet into the shared table facet contract.
///
/// The domain is derived from the owner's `is_enumerable` flag first and from the stored value type
/// second, so a bounded dictionary keeps its `multi` flag and an unbounded domain keeps its open
/// semantics even if a future value type arrives with a different name.
fn catalog_facet_to_grid(facet: &ProductCatalogFacet) -> GridFacet {
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
        facet
            .values
            .iter()
            .map(|value| FacetValue::new(value.value.clone(), value.label.clone(), value.count)),
    );
    // Truncation is either the owner cutting its own value limit or this mapping cutting ours;
    // both mean "there are more values than shown", and neither may be dropped silently.
    mapped.is_truncated |= facet.is_truncated;
    mapped
}

/// Query of the current catalog page with one facet selection flipped.
pub fn build_catalog_facet_toggle_query(
    module_route_base: &str,
    controls: &CatalogListInput,
    code: &str,
    value: &str,
) -> String {
    let filters = toggle_attribute_filter(controls.attribute_filters.as_slice(), code, value);
    build_catalog_query_with_filters(module_route_base, controls, filters)
}

/// Query of the current catalog page without the selections of one facet.
pub fn build_catalog_facet_clear_code_query(
    module_route_base: &str,
    controls: &CatalogListInput,
    code: &str,
) -> String {
    let filters = clear_attribute_filter_code(controls.attribute_filters.as_slice(), code);
    build_catalog_query_with_filters(module_route_base, controls, filters)
}

/// Query of the current catalog page without any attribute-filter selection.
pub fn build_catalog_facet_clear_query(
    module_route_base: &str,
    controls: &CatalogListInput,
) -> String {
    build_catalog_query_with_filters(module_route_base, controls, Vec::new())
}

fn build_catalog_query_with_filters(
    module_route_base: &str,
    controls: &CatalogListInput,
    filters: Vec<String>,
) -> String {
    let serialized = serialize_attribute_filters(filters.as_slice());
    apply_ui_query_pairs(
        module_route_base,
        &[
            ("search", controls.search.clone()),
            ("category_id", controls.category_id.clone()),
            ("sort_by", controls.sort_by.clone()),
            ("sort_direction", controls.sort_direction.clone()),
            (
                "attribute_filters",
                (!serialized.is_empty()).then_some(serialized),
            ),
            ("currency", controls.currency_code.clone()),
        ],
    )
}

pub fn build_catalog_facet_filters_view_model(
    module_route_base: &str,
    facets: &[ProductCatalogFacet],
    controls: &CatalogListInput,
    labels: CatalogFacetLabels,
) -> CatalogFacetFiltersViewModel {
    // The panel semantics (selection state, markers, count rendering, hints, limits) live in the
    // shared table toolkit; only the links below are product-specific.
    let grid_facets: Vec<GridFacet> = facets.iter().map(catalog_facet_to_grid).collect();
    let panel = FacetPanel::build(
        grid_facets.as_slice(),
        controls.attribute_filters.as_slice(),
        &labels.to_grid_labels(),
    );

    let facets: Vec<CatalogFacetViewModel> = panel
        .facets
        .iter()
        .map(|facet| CatalogFacetViewModel {
            code: facet.key.clone(),
            label: facet.label.clone(),
            is_enumerable: facet.is_enumerable,
            unbounded_hint: facet.unbounded_hint.clone(),
            is_truncated: facet.is_truncated,
            truncated_hint: facet.truncated_hint.clone(),
            clear_href: facet.has_selection.then(|| {
                build_catalog_facet_clear_code_query(
                    module_route_base,
                    controls,
                    facet.key.as_str(),
                )
            }),
            clear_label: facet.clear_label.clone(),
            values: facet
                .values
                .iter()
                .map(|value| CatalogFacetValueViewModel {
                    value: value.value.clone(),
                    label: value.label.clone(),
                    count_label: value.count_label.clone(),
                    selected: value.selected,
                    marker: value.marker.clone(),
                    href: build_catalog_facet_toggle_query(
                        module_route_base,
                        controls,
                        facet.key.as_str(),
                        value.value.as_str(),
                    ),
                })
                .collect(),
        })
        .collect();

    CatalogFacetFiltersViewModel {
        title: panel.title,
        show_empty_state: panel.show_empty_state,
        empty_message: panel.empty_message,
        clear_href: (!controls.attribute_filters.is_empty())
            .then(|| build_catalog_facet_clear_query(module_route_base, controls)),
        clear_label: panel.clear_label,
        facets,
    }
}

/// Formats the catalog-card "from" price snapshot provided by the Product owner.
pub fn format_product_list_price_from(
    template: &str,
    price: &crate::model::ProductListPrice,
) -> String {
    let value = match price.compare_at_amount.as_deref() {
        Some(compare_at) if price.on_sale => {
            format!("{} {} ({compare_at})", price.currency_code, price.amount)
        }
        _ => format!("{} {}", price.currency_code, price.amount),
    };
    template.replace("{value}", value.as_str())
}

pub fn build_catalog_rail_view_model(
    module_route_base: &str,
    items: &[crate::model::ProductListItem],
    total: u64,
    locale: Option<&str>,
    labels: ProductCatalogRailLabels,
) -> ProductCatalogRailViewModel {
    let items: Vec<ProductCatalogRailItemViewModel> = items
        .iter()
        .map(|product| ProductCatalogRailItemViewModel {
            product_type: product
                .product_type
                .clone()
                .unwrap_or_else(|| labels.catalog_fallback_label.clone()),
            title: product.title.clone(),
            vendor: product
                .vendor
                .clone()
                .unwrap_or_else(|| labels.vendor_fallback_label.clone()),
            seller_boundary: format_seller_boundary(locale, product.seller_id.as_deref()),
            published_at: product
                .published_at
                .clone()
                .unwrap_or_else(|| product.created_at.clone()),
            href: apply_ui_query_pairs(
                module_route_base,
                &[("handle", Some(product.handle.as_str()))],
            ),
            image_url: product
                .primary_image
                .as_ref()
                .map(|image| image.url.clone()),
            image_alt: product
                .primary_image
                .as_ref()
                .and_then(|image| image.alt_text.clone())
                .unwrap_or_else(|| {
                    format!(
                        "{}{}",
                        labels.image_alt_fallback.as_str(),
                        product.title.as_str()
                    )
                }),
            price_label: product.price_from.as_ref().map(|price| {
                format_product_list_price_from(labels.price_from_template.as_str(), price)
            }),
            on_sale: product
                .price_from
                .as_ref()
                .is_some_and(|price| price.on_sale),
        })
        .collect();

    ProductCatalogRailViewModel {
        title: labels.title,
        total_label: count_label(labels.total_template.as_str(), total),
        empty_message: labels.empty_message,
        open_label: labels.open_label,
        price_missing_label: labels.price_missing_label,
        show_empty_state: items.is_empty(),
        items,
    }
}

pub(crate) fn pricing_quote_matches_request(
    product: &ProductPricingDetail,
    requested: Option<&PricingResolutionContext>,
) -> bool {
    match (requested, product.resolution_context.as_ref()) {
        (None, None) => {
            product.resolved_at.is_none()
                && product
                    .variants
                    .iter()
                    .all(|variant| variant.effective_price.is_none())
        }
        (Some(expected), Some(actual)) => {
            pricing_context_matches_request(actual, expected)
                && product
                    .resolved_at
                    .as_deref()
                    .is_some_and(rustok_pricing_storefront::valid_quote_time)
                && product.variants.iter().all(|variant| {
                    variant
                        .effective_price
                        .as_ref()
                        .is_none_or(|price| price.currency_code == expected.currency_code)
                })
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        PricingResolutionContext, ProductAttribute, ProductAttributeValue, ProductDetail,
        ProductImage, ProductPrice, ProductTranslation,
    };

    #[test]
    fn quote_observation_cannot_downgrade_to_inspection_or_change_currency_or_quantity() {
        let expected = PricingResolutionContext {
            currency_code: "JPY".into(),
            region_id: None,
            price_list_id: None,
            channel_id: None,
            channel_slug: None,
            quantity: 2,
        };
        let mut product = ProductPricingDetail {
            id: "product-1".into(),
            resolution_context: None,
            resolved_at: None,
            variants: vec![crate::model::ProductPricingVariant {
                id: "sku".into(),
                title: "SKU".into(),
                sku: None,
                effective_price: None,
            }],
        };
        assert!(pricing_quote_matches_request(&product, None));
        assert!(!pricing_quote_matches_request(&product, Some(&expected)));
        product.resolution_context = Some(expected.clone());
        assert!(!pricing_quote_matches_request(&product, Some(&expected)));
        product.resolved_at = Some("2026-10-30T10:00:00Z".into());
        assert!(pricing_quote_matches_request(&product, Some(&expected)));
        assert!(!pricing_quote_matches_request(&product, None));
        let mut other = expected.clone();
        other.currency_code = "EUR".into();
        assert!(!pricing_quote_matches_request(&product, Some(&other)));
        other.currency_code = "JPY".into();
        other.quantity = 3;
        assert!(!pricing_quote_matches_request(&product, Some(&other)));
    }

    fn catalog_controls_with_attribute_filters(filters: &[&str]) -> CatalogListInput {
        CatalogListInput {
            search: Some("bag".to_string()),
            category_id: Some("category-1".to_string()),
            sort_by: Some("created_at".to_string()),
            sort_direction: Some("asc".to_string()),
            attribute_filters: filters.iter().map(|value| (*value).to_string()).collect(),
            currency_code: Some("USD".to_string()),
        }
    }

    #[test]
    fn facet_codes_follow_search_options_without_blanks_and_duplicates() {
        let options = ProductCatalogSearchOptions {
            category_options: Vec::new(),
            attribute_options: vec![
                crate::model::ProductCatalogSearchOption {
                    value: "color".to_string(),
                    label: "Color".to_string(),
                },
                crate::model::ProductCatalogSearchOption {
                    value: " color ".to_string(),
                    label: "Color again".to_string(),
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

        assert_eq!(build_catalog_facet_codes(&options), vec!["color", "size"]);
    }

    #[test]
    fn facet_toggle_query_flips_one_selection_and_keeps_the_rest() {
        let controls = catalog_controls_with_attribute_filters(&["color=red", "size=m"]);
        let href = build_catalog_facet_toggle_query("/products", &controls, "color", "red");
        assert_eq!(
            href,
            "/products?search=bag&category_id=category-1&sort_by=created_at&sort_direction=asc&attribute_filters=size%3Dm&currency=USD"
        );

        let href = build_catalog_facet_toggle_query("/products", &controls, "color", "blue");
        assert_eq!(
            href,
            "/products?search=bag&category_id=category-1&sort_by=created_at&sort_direction=asc&attribute_filters=color%3Dred%3Bsize%3Dm%3Bcolor%3Dblue&currency=USD"
        );
    }

    #[test]
    fn facet_clear_queries_drop_selections_without_touching_other_controls() {
        let controls = catalog_controls_with_attribute_filters(&["color=red", "size=m"]);
        assert_eq!(
            build_catalog_facet_clear_query("/products", &controls),
            "/products?search=bag&category_id=category-1&sort_by=created_at&sort_direction=asc&currency=USD"
        );
        let href = build_catalog_facet_clear_code_query("/products", &controls, "size");
        assert!(href.contains("attribute_filters=color%3Dred"));
        assert!(!href.contains("size"));
    }

    #[test]
    fn facet_view_model_marks_selection_counts_and_unbounded_domains() {
        let facets = vec![
            ProductCatalogFacet {
                code: "color".to_string(),
                label: "Color".to_string(),
                value_type: "select".to_string(),
                is_localized: true,
                is_enumerable: true,
                is_truncated: true,
                total_products: 12,
                values: vec![crate::model::ProductCatalogFacetValue {
                    value: "red".to_string(),
                    label: "Red".to_string(),
                    count: 7,
                }],
            },
            ProductCatalogFacet {
                code: "weight".to_string(),
                label: "Weight".to_string(),
                value_type: "decimal".to_string(),
                is_localized: false,
                is_enumerable: false,
                is_truncated: false,
                total_products: 3,
                values: Vec::new(),
            },
        ];
        let controls = catalog_controls_with_attribute_filters(&["color=red", "size=m"]);
        let view_model = build_catalog_facet_filters_view_model(
            "/products",
            facets.as_slice(),
            &controls,
            crate::catalog_controls::build_catalog_facet_labels(Some("en")),
        );

        assert!(!view_model.show_empty_state);
        assert_eq!(view_model.facets.len(), 2);
        let color = view_model.facets.first().expect("color facet");
        assert_eq!(color.label, "Color");
        assert!(color.is_truncated);
        assert!(color.truncated_hint.is_some());
        let red = color.values.first().expect("red bucket");
        assert!(red.selected);
        assert_eq!(red.count_label, "(7)");
        assert!(red.href.contains("attribute_filters="));
        let weight = view_model.facets.get(1).expect("weight facet");
        assert!(!weight.is_enumerable);
        assert!(weight.unbounded_hint.is_some());
        assert!(weight.values.is_empty());
        assert!(view_model.clear_href.is_some());
    }

    #[test]
    fn facet_view_model_stays_empty_and_clears_nothing_without_selection() {
        let controls = CatalogListInput::default();
        let view_model = build_catalog_facet_filters_view_model(
            "/products",
            &[],
            &controls,
            crate::catalog_controls::build_catalog_facet_labels(None),
        );
        assert!(view_model.show_empty_state);
        assert!(view_model.clear_href.is_none());
        assert!(view_model.facets.is_empty());
    }

    fn without_bidi_isolates(value: &str) -> String {
        value.replace(['\u{2068}', '\u{2069}'], "")
    }

    #[test]
    fn route_input_parses_quantity_without_ui_runtime() {
        let input = build_route_input(
            Some("boot".to_string()),
            Some("en".to_string()),
            Some("USD".to_string()),
            None,
            None,
            None,
            None,
            Some(" 3 ".to_string()),
        )
        .expect("valid price context");

        assert_eq!(input.handle.as_deref(), Some("boot"));
        assert_eq!(input.quantity, Some(3));
        assert!(
            build_route_input(None, None, None, None, None, None, None, Some("bad".into()))
                .is_err()
        );
        assert!(
            build_route_input(
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(String::new())
            )
            .is_err()
        );

        let request = build_fetch_request(&input);
        assert_eq!(request.selected_handle.as_deref(), Some("boot"));
        assert_eq!(request.locale.as_deref(), Some("en"));
        assert_eq!(request.currency_code.as_deref(), Some("USD"));
        assert_eq!(request.quantity, Some(3));
    }

    #[test]
    fn storefront_route_segment_fallback_is_core_owned() {
        assert_eq!(resolve_route_segment(Some("catalog")), "catalog");
        assert_eq!(resolve_route_segment(Some(" products ")), "products");
        assert_eq!(resolve_route_segment(Some("   ")), DEFAULT_ROUTE_SEGMENT);
        assert_eq!(resolve_route_segment(None), DEFAULT_ROUTE_SEGMENT);
    }

    #[test]
    fn storefront_shell_view_model_is_built_without_ui_runtime() {
        let view_model = build_shell_view_model(Some("en"));

        assert_eq!(view_model.badge, "product");
        assert_eq!(
            view_model.title,
            "Published catalog from the product module"
        );
        assert_eq!(
            view_model.subtitle,
            "Published products with sparse SKU selection and authoritative price observations through the selected native or GraphQL transport."
        );
        assert_eq!(
            view_model.load_error,
            "Failed to load storefront product data"
        );
    }

    #[test]
    fn pricing_scope_and_seller_labels_are_stable() {
        assert_eq!(
            format_pricing_scope(Some("en"), Some("list-1"), Some("web"), Some("channel-1")),
            Some("price list list-1 | channel web (channel-1)".to_string()),
        );
        assert_eq!(format_pricing_scope(Some("en"), None, None, None), None);
        assert_eq!(
            format_seller_boundary(Some("en"), Some(" seller-1 ")),
            "seller id: seller-1".to_string(),
        );
        assert_eq!(
            format_seller_boundary(Some("en"), Some("  ")),
            "seller id: unassigned".to_string(),
        );
    }

    #[test]
    fn catalog_rail_labels_are_built_without_ui_runtime() {
        let labels = build_product_catalog_rail_labels(Some("en"));

        assert_eq!(labels.title, "Published products");
        assert_eq!(labels.total_template, "{count} total");
        assert_eq!(
            labels.empty_message,
            "No published products are available yet."
        );
        assert_eq!(labels.open_label, "Open");
        assert_eq!(labels.catalog_fallback_label, "catalog");
        assert_eq!(labels.vendor_fallback_label, "Independent label");
        assert_eq!(labels.price_from_template, "from {value}");
        assert_eq!(labels.price_missing_label, "Price on request");
        assert_eq!(labels.image_alt_fallback, "Product image: ");
    }

    #[test]
    fn catalog_rail_view_model_is_built_without_ui_runtime() {
        let item = crate::model::ProductListItem {
            id: "product-1".to_string(),
            status: "published".to_string(),
            seller_id: Some("seller-1".to_string()),
            vendor: None,
            product_type: None,
            tags: vec!["featured".to_string()],
            title: "Trail boot".to_string(),
            handle: "trail-boot".to_string(),
            primary_image: Some(crate::model::ProductImage {
                media_id: "00000000-0000-0000-0000-000000000001".to_string(),
                url: "/api/v1/media/00000000-0000-0000-0000-000000000001".to_string(),
                alt_text: Some("Trail boot side view".to_string()),
                position: 0,
            }),
            price_from: Some(crate::model::ProductListPrice {
                currency_code: "USD".to_string(),
                amount: "129.00".to_string(),
                compare_at_amount: Some("159.00".to_string()),
                on_sale: true,
            }),
            published_at: None,
            created_at: "2026-05-29T00:00:00Z".to_string(),
        };

        let view_model = build_catalog_rail_view_model(
            "/products",
            &[item],
            3,
            Some("en"),
            ProductCatalogRailLabels {
                title: "Published products".to_string(),
                total_template: "{count} total".to_string(),
                empty_message: "No products".to_string(),
                open_label: "Open".to_string(),
                catalog_fallback_label: "catalog".to_string(),
                vendor_fallback_label: "Independent label".to_string(),
                price_from_template: "from {value}".to_string(),
                price_missing_label: "Price on request".to_string(),
                image_alt_fallback: "Product image: ".to_string(),
            },
        );

        assert_eq!(view_model.title, "Published products");
        assert_eq!(view_model.total_label, "3 total");
        assert_eq!(view_model.open_label, "Open");
        assert!(!view_model.show_empty_state);
        assert_eq!(view_model.items.len(), 1);
        let item = &view_model.items[0];
        assert_eq!(item.product_type, "catalog");
        assert_eq!(item.title, "Trail boot");
        assert_eq!(item.vendor, "Independent label");
        assert_eq!(item.seller_boundary, "seller id: seller-1");
        assert_eq!(item.published_at, "2026-05-29T00:00:00Z");
        assert_eq!(item.href, "/products?handle=trail-boot");
        assert_eq!(
            item.image_url.as_deref(),
            Some("/api/v1/media/00000000-0000-0000-0000-000000000001")
        );
        assert_eq!(item.image_alt, "Trail boot side view");
        assert_eq!(
            item.price_label.as_deref(),
            Some("from USD 129.00 (159.00)")
        );
        assert!(item.on_sale);
    }

    #[test]
    fn catalog_rail_item_without_media_or_price_hides_both_slots() {
        let item = crate::model::ProductListItem {
            id: "product-2".to_string(),
            status: "published".to_string(),
            seller_id: None,
            vendor: None,
            product_type: None,
            tags: Vec::new(),
            title: "Untitled draft".to_string(),
            handle: "untitled-draft".to_string(),
            primary_image: None,
            price_from: None,
            published_at: None,
            created_at: "2026-05-29T00:00:00Z".to_string(),
        };

        let view_model = build_catalog_rail_view_model(
            "/products",
            &[item],
            1,
            Some("en"),
            ProductCatalogRailLabels {
                title: "Published products".to_string(),
                total_template: "{count} total".to_string(),
                empty_message: "No products".to_string(),
                open_label: "Open".to_string(),
                catalog_fallback_label: "catalog".to_string(),
                vendor_fallback_label: "Independent label".to_string(),
                price_from_template: "from {value}".to_string(),
                price_missing_label: "Price on request".to_string(),
                image_alt_fallback: "Product image: ".to_string(),
            },
        );

        let item = &view_model.items[0];
        assert!(item.image_url.is_none());
        assert_eq!(item.image_alt, "Product image: Untitled draft");
        assert!(item.price_label.is_none());
        assert!(!item.on_sale);
    }

    #[test]
    fn catalog_rail_empty_state_policy_is_built_without_ui_runtime() {
        let view_model = build_catalog_rail_view_model(
            "/products",
            &[],
            0,
            Some("en"),
            ProductCatalogRailLabels {
                title: "Published products".to_string(),
                total_template: "{count} total".to_string(),
                empty_message: "No products".to_string(),
                open_label: "Open".to_string(),
                catalog_fallback_label: "catalog".to_string(),
                vendor_fallback_label: "Independent label".to_string(),
                price_from_template: "from {value}".to_string(),
                price_missing_label: "Price on request".to_string(),
                image_alt_fallback: "Product image: ".to_string(),
            },
        );

        assert!(view_model.show_empty_state);
        assert_eq!(view_model.empty_message, "No products");
        assert!(view_model.items.is_empty());
    }

    #[test]
    fn selected_product_view_model_is_built_without_ui_runtime() {
        let product = ProductDetail {
            // Attributes are owner-formatted; the view model only phrases the boolean pair.
            id: "product-1".to_string(),
            status: "published".to_string(),
            seller_id: Some("seller-1".to_string()),
            vendor: Some("Acme".to_string()),
            product_type: Some("Boots".to_string()),
            tags: vec!["featured".to_string()],
            published_at: Some("2026-05-29T00:00:00Z".to_string()),
            images: vec![
                ProductImage {
                    media_id: "8b5d3d0e-6f4f-4d63-9d80-1f2c3a4b5c6d".to_string(),
                    url: "/api/v1/media/8b5d3d0e-6f4f-4d63-9d80-1f2c3a4b5c6d".to_string(),
                    alt_text: Some("Trail boot side view".to_string()),
                    position: 0,
                },
                ProductImage {
                    media_id: "1a2b3c4d-5e6f-4a7b-8c9d-0e1f2a3b4c5d".to_string(),
                    url: "/api/v1/media/1a2b3c4d-5e6f-4a7b-8c9d-0e1f2a3b4c5d".to_string(),
                    alt_text: None,
                    position: 1,
                },
            ],
            translations: vec![ProductTranslation {
                locale: "en".to_string(),
                title: "Trail boot".to_string(),
                handle: "trail-boot".to_string(),
                description: Some("Ready for mud".to_string()),
            }],
            variant_axes: Vec::new(),
            variants: vec![ProductVariant {
                id: "variant-1".to_string(),
                title: "Default".to_string(),
                sku: Some("BOOT-1".to_string()),
                inventory_quantity: 7,
                inventory_policy: "deny".into(),
                combination_identity: None,
                axis_values: Vec::new(),
                in_stock: true,
                prices: vec![ProductPrice {
                    currency_code: "USD".to_string(),
                    amount: "25.00".to_string(),
                    compare_at_amount: Some("30.00".to_string()),
                    on_sale: true,
                }],
            }],
            attributes: vec![
                ProductAttribute {
                    code: "material".to_string(),
                    label: "Материал".to_string(),
                    value_type: "multiselect".to_string(),
                    is_localized: false,
                    values: vec![
                        ProductAttributeValue {
                            text: "Leather".to_string(),
                        },
                        ProductAttributeValue {
                            text: "Rubber".to_string(),
                        },
                    ],
                },
                ProductAttribute {
                    code: "waterproof".to_string(),
                    label: "Waterproof".to_string(),
                    value_type: "boolean".to_string(),
                    is_localized: false,
                    values: vec![ProductAttributeValue {
                        text: "true".to_string(),
                    }],
                },
            ],
        };
        let view_model = build_selected_product_view_model(&product, Some("en"));

        assert_eq!(view_model.product_type, "Boots");
        assert_eq!(view_model.vendor, "Acme");
        assert_eq!(view_model.gallery.len(), 2);
        assert_eq!(
            view_model.gallery[0].url,
            "/api/v1/media/8b5d3d0e-6f4f-4d63-9d80-1f2c3a4b5c6d"
        );
        assert_eq!(
            view_model.gallery[0].alt_text,
            "Trail boot side view".to_string()
        );
        assert!(view_model.gallery[0].is_primary);
        assert!(!view_model.gallery[1].is_primary);
        assert_eq!(view_model.gallery[1].alt_text, "Trail boot 2".to_string());
        assert_eq!(view_model.published_at, "2026-05-29T00:00:00Z");
        assert_eq!(
            view_model.metadata_items,
            vec![
                "Boots".to_string(),
                "Acme".to_string(),
                "2026-05-29T00:00:00Z".to_string(),
            ],
        );
        assert_eq!(view_model.title, "Trail boot");
        assert_eq!(view_model.description, "Ready for mud");
        assert_eq!(view_model.seller_boundary, "seller id: seller-1");
        assert_eq!(view_model.attributes_label, "Specifications");
        assert_eq!(view_model.attributes.len(), 2);
        assert_eq!(view_model.attributes[0].code, "material");
        assert_eq!(view_model.attributes[0].label, "Материал");
        assert_eq!(
            view_model.attributes[0].values,
            vec!["Leather".to_string(), "Rubber".to_string()],
        );
        // The owner sends the boolean vocabulary; the view model phrases it per locale.
        assert_eq!(
            view_model.attributes[1].values,
            vec!["Yes".to_string()],
            "boolean values must be localized for the storefront",
        );
    }

    #[test]
    fn storefront_attributes_are_dropped_when_the_owner_sent_none() {
        let mut product = ProductDetail {
            id: "product-2".to_string(),
            status: "published".to_string(),
            seller_id: None,
            vendor: None,
            product_type: None,
            tags: Vec::new(),
            published_at: None,
            images: Vec::new(),
            translations: Vec::new(),
            variant_axes: Vec::new(),
            variants: Vec::new(),
            attributes: Vec::new(),
        };
        let view_model = build_selected_product_view_model(&product, Some("ru"));
        assert!(view_model.attributes.is_empty());

        // A hidden definition never reaches the contract, but a boolean the owner did send is
        // phrased in the requested locale.
        product.attributes = vec![ProductAttribute {
            code: "waterproof".to_string(),
            label: "Waterproof".to_string(),
            value_type: "boolean".to_string(),
            is_localized: false,
            values: vec![
                ProductAttributeValue {
                    text: "false".to_string(),
                },
                ProductAttributeValue {
                    text: "true".to_string(),
                },
            ],
        }];
        let view_model = build_selected_product_view_model(&product, Some("ru"));
        assert_eq!(view_model.attributes_label, "Характеристики");
        assert_eq!(
            view_model.attributes[0].values,
            vec!["Нет".to_string(), "Да".to_string()],
        );
    }

    #[test]
    fn selected_product_empty_view_model_is_built_without_ui_runtime() {
        let view_model = build_selected_product_empty_view_model(Some("en"));

        assert_eq!(view_model.title, "No published product selected");
        assert_eq!(
            view_model.body,
            "Publish a product from the product admin package or open one with `?handle=`."
        );
    }
    #[test]
    fn transport_error_dom_evidence_is_built_without_ui_runtime() {
        let evidence = build_transport_error_dom_evidence(
            "Failed to load storefront product data",
            "graphql",
            true,
            Some("server function unavailable"),
            Some("network unavailable"),
            "product transport fallback failed: native_server=server function unavailable; graphql=network unavailable",
        );

        assert_eq!(evidence.failed_path, "graphql");
        assert_eq!(evidence.fallback_attempted, "true");
        assert_eq!(evidence.native_error, "server function unavailable");
        assert_eq!(evidence.graphql_error, "network unavailable");
        assert_eq!(
            evidence.message,
            "Failed to load storefront product data: product transport fallback failed: native_server=server function unavailable; graphql=network unavailable"
        );
    }
}
