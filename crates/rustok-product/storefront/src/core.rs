use rustok_core::locale_tags_match;

use crate::i18n::t;
use crate::model::{
    ProductDetail, ProductPricingContext, ProductPricingDetail, ProductTranslation, ProductVariant,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductStorefrontRouteInput {
    pub handle: Option<String>,
    pub locale: Option<String>,
    pub currency_code: Option<String>,
    pub region_id: Option<String>,
    pub price_list_id: Option<String>,
    pub channel_id: Option<String>,
    pub channel_slug: Option<String>,
    pub quantity: Option<i32>,
}

#[allow(clippy::too_many_arguments)]
pub fn build_storefront_route_input(
    handle: Option<String>,
    locale: Option<String>,
    currency_code: Option<String>,
    region_id: Option<String>,
    price_list_id: Option<String>,
    channel_id: Option<String>,
    channel_slug: Option<String>,
    quantity: Option<String>,
) -> ProductStorefrontRouteInput {
    ProductStorefrontRouteInput {
        handle,
        locale,
        currency_code,
        region_id,
        price_list_id,
        channel_id,
        channel_slug,
        quantity: parse_storefront_quantity(quantity.as_deref()),
    }
}

pub fn parse_storefront_quantity(value: Option<&str>) -> Option<i32> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|value| value.parse::<i32>().ok())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectedProductViewModel {
    pub product_type: String,
    pub vendor: String,
    pub published_at: String,
    pub seller_boundary: String,
    pub title: String,
    pub description: String,
    pub catalog_snapshot: String,
    pub pricing_preview: String,
    pub pricing_context: Option<String>,
    pub inventory: i32,
    pub pricing_href: String,
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
    let pricing_href = build_storefront_pricing_href(
        pricing_route_base,
        selected_handle.or_else(|| translation.map(|item| item.handle.as_str())),
        resolution_context,
        variant,
    );

    SelectedProductViewModel {
        product_type: product
            .product_type
            .clone()
            .unwrap_or_else(|| t(locale, "product.selected.catalog", "catalog")),
        vendor: product.vendor.clone().unwrap_or_else(|| {
            t(
                locale,
                "product.selected.vendorFallback",
                "independent label",
            )
        }),
        published_at: product
            .published_at
            .clone()
            .unwrap_or_else(|| t(locale, "product.selected.unscheduled", "scheduled later")),
        seller_boundary: format_seller_boundary(locale, product.seller_id.as_deref()),
        title,
        description,
        catalog_snapshot,
        pricing_preview,
        pricing_context,
        inventory: variant.map(|item| item.inventory_quantity).unwrap_or(0),
        pricing_href,
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

pub fn build_storefront_pricing_href(
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
        let input = build_storefront_route_input(
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
        assert_eq!(parse_storefront_quantity(Some("bad")), None);
        assert_eq!(parse_storefront_quantity(Some("   ")), None);
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
            build_storefront_pricing_href("/products", Some(" boot "), Some(&context), None),
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
            build_storefront_pricing_href("/products", Some("boot"), None, Some(&variant)),
            "/products?handle=boot&currency=USD".to_string(),
        );
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
    }
}
