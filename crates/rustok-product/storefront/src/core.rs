use rustok_api::locale_tags_match;
use rustok_ui_core::normalize_optional_ui_text;

use crate::i18n::t;
use crate::model::{
    ProductDetail, ProductPricingContext, ProductPricingDetail, ProductTranslation, ProductVariant,
};

#[derive(Clone, Debug, PartialEq, Eq)]
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
) -> RouteInput {
    RouteInput {
        handle,
        locale,
        currency_code,
        region_id,
        price_list_id,
        channel_id,
        channel_slug,
        quantity: parse_quantity(quantity.as_deref()),
    }
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

fn sanitize_currency_code(currency_code: Option<String>) -> Option<String> {
    currency_code
        .map(|value| value.trim().to_ascii_uppercase())
        .filter(|value| value.len() == 3)
}

pub(crate) fn sanitize_uuid_string(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty() && uuid::Uuid::parse_str(item).is_ok())
}

pub(crate) fn sanitize_channel_slug(channel_slug: Option<String>) -> Option<String> {
    channel_slug
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
}

#[cfg_attr(not(feature = "ssr"), allow(dead_code))]
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

fn sanitize_quantity(quantity: Option<i32>) -> Option<i32> {
    quantity.filter(|value| *value > 0)
}

fn derive_pricing_currency(
    selected_product: Option<&ProductDetail>,
    currency_code: Option<String>,
) -> Option<String> {
    sanitize_currency_code(currency_code).or_else(|| {
        selected_product.and_then(|product| {
            product
                .variants
                .first()
                .and_then(|variant| variant.prices.first())
                .map(|price| price.currency_code.clone())
                .and_then(|currency| sanitize_currency_code(Some(currency)))
        })
    })
}

pub fn build_pricing_context(
    selected_product: Option<&ProductDetail>,
    currency_code: Option<String>,
    region_id: Option<String>,
    price_list_id: Option<String>,
    channel_id: Option<String>,
    channel_slug: Option<String>,
    quantity: Option<i32>,
) -> Option<ProductPricingContext> {
    let currency_code = derive_pricing_currency(selected_product, currency_code)?;
    Some(ProductPricingContext {
        currency_code,
        region_id: sanitize_uuid_string(region_id),
        price_list_id: sanitize_uuid_string(price_list_id),
        channel_id: sanitize_uuid_string(channel_id),
        channel_slug: sanitize_channel_slug(channel_slug),
        quantity: sanitize_quantity(quantity).unwrap_or(1),
    })
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
            "This storefront route reads published catalog data through the product-owned package, with GraphQL kept as a fallback path.",
        ),
        load_error: t(
            locale,
            "product.error.load",
            "Failed to load storefront product data",
        ),
    }
}

pub fn parse_quantity(value: Option<&str>) -> Option<i32> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|value| value.parse::<i32>().ok())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectedProductEmptyViewModel {
    pub title: String,
    pub body: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectedProductViewModel {
    pub product_type: String,
    pub vendor: String,
    pub published_at: String,
    pub metadata_items: Vec<String>,
    pub seller_boundary: String,
    pub title: String,
    pub description: String,
    pub catalog_snapshot: String,
    pub pricing_preview: String,
    pub pricing_context: Option<String>,
    pub inventory: i32,
    pub pricing_href: String,
    pub preview_context_label: String,
    pub pricing_ownership_note: String,
    pub catalog_snapshot_label: String,
    pub pricing_preview_label: String,
    pub inventory_label: String,
    pub open_pricing_label: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductCatalogRailLabels {
    pub title: String,
    pub total_template: String,
    pub empty_message: String,
    pub open_label: String,
    pub catalog_fallback_label: String,
    pub vendor_fallback_label: String,
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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductCatalogRailViewModel {
    pub title: String,
    pub total_label: String,
    pub empty_message: String,
    pub open_label: String,
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
    pricing: Option<&ProductPricingDetail>,
    resolution_context: Option<&ProductPricingContext>,
    selected_handle: Option<&str>,
    locale: Option<&str>,
    pricing_route_base: &str,
) -> SelectedProductViewModel {
    let translation = product_translation_for_locale(product.translations.as_slice(), locale);
    let variant = product.variants.first();
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
    let catalog_snapshot = variant
        .and_then(|item| item.prices.first())
        .map(|item| {
            format_product_price(
                locale,
                item.currency_code.as_str(),
                item.amount.as_str(),
                item.compare_at_amount.as_deref(),
                None,
            )
        })
        .unwrap_or_else(|| t(locale, "product.selected.noPrice", "No pricing yet"));
    let pricing_preview = format_pricing_preview(locale, pricing);
    let pricing_context = resolution_context.map(|context| format_pricing_context(locale, context));
    let pricing_href = build_pricing_href(
        pricing_route_base,
        selected_handle.or_else(|| translation.map(|item| item.handle.as_str())),
        resolution_context,
        variant,
    );

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

    SelectedProductViewModel {
        product_type,
        vendor,
        published_at,
        metadata_items,
        seller_boundary: format_seller_boundary(locale, product.seller_id.as_deref()),
        title,
        description,
        catalog_snapshot,
        pricing_preview,
        pricing_context,
        inventory: variant.map(|item| item.inventory_quantity).unwrap_or(0),
        pricing_href,
        preview_context_label: t(locale, "product.selected.previewContext", "pricing preview"),
        pricing_ownership_note: t(
            locale,
            "product.selected.pricingOwnershipNote",
            "Catalog snapshot stays product-owned; resolved pricing comes from the pricing module preview.",
        ),
        catalog_snapshot_label: t(
            locale,
            "product.selected.catalogSnapshot",
            "Catalog snapshot",
        ),
        pricing_preview_label: t(
            locale,
            "product.selected.pricingPreview",
            "Pricing module preview",
        ),
        inventory_label: t(locale, "product.selected.inventory", "Inventory"),
        open_pricing_label: t(
            locale,
            "product.selected.openPricing",
            "Open pricing module",
        ),
    }
}

pub fn format_pricing_preview(
    locale: Option<&str>,
    pricing: Option<&ProductPricingDetail>,
) -> String {
    let Some(pricing) = pricing else {
        return t(
            locale,
            "product.selected.noPricingPreview",
            "Pricing module preview is unavailable.",
        );
    };

    let Some(variant) = pricing.variants.first() else {
        return t(locale, "product.selected.noPrice", "No pricing yet");
    };

    if let Some(price) = variant.effective_price.as_ref() {
        let mut label = format_product_price(
            locale,
            price.currency_code.as_str(),
            price.amount.as_str(),
            price.compare_at_amount.as_deref(),
            price.discount_percent.as_deref(),
        );
        if let Some(scope) = format_pricing_scope(
            locale,
            price.price_list_id.as_deref(),
            price.channel_slug.as_deref(),
            price.channel_id.as_deref(),
        ) {
            label.push_str(format!(" | {scope}").as_str());
        }
        return label;
    }

    variant
        .prices
        .first()
        .map(|price| {
            format_product_price(
                locale,
                price.currency_code.as_str(),
                price.amount.as_str(),
                price.compare_at_amount.as_deref(),
                price.discount_percent.as_deref(),
            )
        })
        .unwrap_or_else(|| t(locale, "product.selected.noPrice", "No pricing yet"))
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
            t(locale, "product.selected.compareAt", "compare-at {value}")
                .replace("{value}", compare_at_amount),
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

pub fn format_pricing_context(locale: Option<&str>, context: &ProductPricingContext) -> String {
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
    template.replace("{count}", &total.to_string())
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
            href: format!("{module_route_base}?handle={}", product.handle),
        })
        .collect();

    ProductCatalogRailViewModel {
        title: labels.title,
        total_label: count_label(labels.total_template.as_str(), total),
        empty_message: labels.empty_message,
        open_label: labels.open_label,
        show_empty_state: items.is_empty(),
        items,
    }
}

pub fn build_pricing_href(
    module_route_base: &str,
    handle: Option<&str>,
    resolution_context: Option<&ProductPricingContext>,
    variant: Option<&ProductVariant>,
) -> String {
    let mut params = Vec::new();
    if let Some(handle) = handle.map(str::trim).filter(|value| !value.is_empty()) {
        params.push(format!("handle={handle}"));
    }

    let fallback_currency = variant
        .and_then(|item| item.prices.first())
        .map(|price| price.currency_code.as_str());
    let currency_code = resolution_context
        .map(|context| context.currency_code.as_str())
        .or(fallback_currency);
    if let Some(currency_code) = currency_code
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        params.push(format!("currency={currency_code}"));
    }
    if let Some(region_id) = resolution_context
        .and_then(|context| context.region_id.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        params.push(format!("region_id={region_id}"));
    }
    if let Some(price_list_id) = resolution_context
        .and_then(|context| context.price_list_id.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        params.push(format!("price_list_id={price_list_id}"));
    }
    if let Some(channel_id) = resolution_context
        .and_then(|context| context.channel_id.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        params.push(format!("channel_id={channel_id}"));
    }
    if let Some(channel_slug) = resolution_context
        .and_then(|context| context.channel_slug.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        params.push(format!("channel_slug={channel_slug}"));
    }
    if let Some(quantity) = resolution_context
        .map(|context| context.quantity)
        .filter(|value| *value > 0)
    {
        params.push(format!("quantity={quantity}"));
    }

    if params.is_empty() {
        module_route_base.to_string()
    } else {
        format!("{module_route_base}?{}", params.join("&"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ProductDetail, ProductPrice, ProductPricingContext, ProductTranslation};

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
        );

        assert_eq!(input.handle.as_deref(), Some("boot"));
        assert_eq!(input.quantity, Some(3));
        assert_eq!(parse_quantity(Some("bad")), None);
        assert_eq!(parse_quantity(Some("   ")), None);

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
            "This storefront route reads product-owned catalog data and shows resolved pricing through a separate pricing-module hook, with GraphQL kept as a fallback path."
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
    fn pricing_href_preserves_context_params() {
        let context = ProductPricingContext {
            currency_code: "EUR".to_string(),
            region_id: Some("region-1".to_string()),
            price_list_id: Some("list-1".to_string()),
            channel_id: Some("channel-1".to_string()),
            channel_slug: Some("web".to_string()),
            quantity: 2,
        };

        assert_eq!(
            build_pricing_href("/products", Some(" boot "), Some(&context), None),
            "/products?handle=boot&currency=EUR&region_id=region-1&price_list_id=list-1&channel_id=channel-1&channel_slug=web&quantity=2".to_string(),
        );
    }

    #[test]
    fn pricing_href_uses_variant_currency_fallback() {
        let variant = ProductVariant {
            id: "variant-1".to_string(),
            title: "Variant".to_string(),
            sku: None,
            inventory_quantity: 1,
            in_stock: true,
            prices: vec![ProductPrice {
                currency_code: "USD".to_string(),
                amount: "10.00".to_string(),
                compare_at_amount: None,
                on_sale: false,
            }],
        };

        assert_eq!(
            build_pricing_href("/products", Some("boot"), None, Some(&variant)),
            "/products?handle=boot&currency=USD".to_string(),
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
            },
        );

        assert!(view_model.show_empty_state);
        assert_eq!(view_model.empty_message, "No products");
        assert!(view_model.items.is_empty());
    }

    #[test]
    fn selected_product_view_model_is_built_without_ui_runtime() {
        let product = ProductDetail {
            id: "product-1".to_string(),
            status: "published".to_string(),
            seller_id: Some("seller-1".to_string()),
            vendor: Some("Acme".to_string()),
            product_type: Some("Boots".to_string()),
            tags: vec!["featured".to_string()],
            published_at: Some("2026-05-29T00:00:00Z".to_string()),
            translations: vec![ProductTranslation {
                locale: "en".to_string(),
                title: "Trail boot".to_string(),
                handle: "trail-boot".to_string(),
                description: Some("Ready for mud".to_string()),
            }],
            variants: vec![ProductVariant {
                id: "variant-1".to_string(),
                title: "Default".to_string(),
                sku: Some("BOOT-1".to_string()),
                inventory_quantity: 7,
                in_stock: true,
                prices: vec![ProductPrice {
                    currency_code: "USD".to_string(),
                    amount: "25.00".to_string(),
                    compare_at_amount: Some("30.00".to_string()),
                    on_sale: true,
                }],
            }],
        };
        let context = ProductPricingContext {
            currency_code: "USD".to_string(),
            region_id: None,
            price_list_id: Some("list-1".to_string()),
            channel_id: None,
            channel_slug: Some("web".to_string()),
            quantity: 4,
        };

        let view_model = build_selected_product_view_model(
            &product,
            None,
            Some(&context),
            None,
            Some("en"),
            "/pricing",
        );

        assert_eq!(view_model.product_type, "Boots");
        assert_eq!(view_model.vendor, "Acme");
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
        assert_eq!(view_model.catalog_snapshot, "USD 25.00 (compare-at 30.00)");
        assert_eq!(
            view_model.pricing_preview,
            "Pricing module preview is unavailable."
        );
        assert_eq!(
            view_model.pricing_context,
            Some("currency USD | qty 4 | price list list-1 | channel web".to_string()),
        );
        assert_eq!(view_model.inventory, 7);
        assert_eq!(
            view_model.pricing_href,
            "/pricing?handle=trail-boot&currency=USD&price_list_id=list-1&channel_slug=web&quantity=4",
        );
        assert_eq!(view_model.preview_context_label, "pricing preview");
        assert_eq!(view_model.catalog_snapshot_label, "Catalog snapshot");
        assert_eq!(view_model.pricing_preview_label, "Pricing module preview");
        assert_eq!(view_model.inventory_label, "Inventory");
        assert_eq!(view_model.open_pricing_label, "Open pricing module");
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
