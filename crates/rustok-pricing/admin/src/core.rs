use std::collections::BTreeSet;

use uuid::Uuid;

use crate::i18n::t;
use crate::model::{
    PricingAdjustmentPreview, PricingChannelOption, PricingEffectivePrice, PricingPrice,
    PricingPriceListOption, PricingProductListItem, PricingProductTranslation,
    PricingResolutionContext, PricingVariant,
};

pub(crate) fn locale_tags_match(left: &str, right: &str) -> bool {
    left.trim()
        .replace('_', "-")
        .eq_ignore_ascii_case(&right.trim().replace('_', "-"))
}

#[derive(Clone)]
pub(crate) struct PricingSummary {
    pub(crate) variant_count: usize,
    pub(crate) priced_variants: usize,
    pub(crate) on_sale_variants: usize,
    pub(crate) currency_count: usize,
}

pub(crate) fn summarize_pricing(variants: &[PricingVariant]) -> PricingSummary {
    let priced_variants = variants
        .iter()
        .filter(|variant| !variant.prices.is_empty())
        .count();
    let on_sale_variants = variants
        .iter()
        .filter(|variant| {
            variant
                .effective_price
                .as_ref()
                .map(|price| price.on_sale)
                .unwrap_or(false)
                || variant.prices.iter().any(|price| price.on_sale)
        })
        .count();
    let currency_count = variants
        .iter()
        .flat_map(|variant| {
            variant
                .prices
                .iter()
                .map(|price| price.currency_code.clone())
                .chain(
                    variant
                        .effective_price
                        .iter()
                        .map(|price| price.currency_code.clone()),
                )
        })
        .collect::<BTreeSet<_>>()
        .len();

    PricingSummary {
        variant_count: variants.len(),
        priced_variants,
        on_sale_variants,
        currency_count,
    }
}

pub(crate) fn pricing_translation_for_locale<'a>(
    translations: &'a [PricingProductTranslation],
    requested_locale: Option<&str>,
) -> Option<&'a PricingProductTranslation> {
    requested_locale
        .and_then(|requested_locale| {
            translations
                .iter()
                .find(|translation| locale_tags_match(&translation.locale, requested_locale))
        })
        .or_else(|| translations.first())
}

pub(crate) fn localized_product_status(locale: Option<&str>, status: &str) -> String {
    match status {
        "ACTIVE" => t(locale, "pricing.status.active", "Active"),
        "ARCHIVED" => t(locale, "pricing.status.archived", "Archived"),
        _ => t(locale, "pricing.status.draft", "Draft"),
    }
}

pub(crate) fn format_product_meta(
    locale: Option<&str>,
    product: &PricingProductListItem,
) -> String {
    let vendor = product
        .vendor
        .clone()
        .unwrap_or_else(|| t(locale, "pricing.common.notSet", "not set"));
    let product_type = product
        .product_type
        .clone()
        .unwrap_or_else(|| t(locale, "pricing.common.notSet", "not set"));
    let seller_id = product
        .seller_id
        .clone()
        .unwrap_or_else(|| t(locale, "pricing.common.notSet", "not set"));
    format!(
        "handle: {} | vendor: {} | type: {} | seller: {}",
        product.handle, vendor, product_type, seller_id
    )
}

pub(crate) fn format_variant_identity(locale: Option<&str>, variant: &PricingVariant) -> String {
    let sku = variant
        .sku
        .clone()
        .unwrap_or_else(|| t(locale, "pricing.common.notSet", "not set"));
    let barcode = variant
        .barcode
        .clone()
        .unwrap_or_else(|| t(locale, "pricing.common.notSet", "not set"));
    format!("sku: {sku} | barcode: {barcode}")
}

pub(crate) fn format_variant_prices(
    locale: Option<&str>,
    prices: &[PricingPrice],
    price_list_options: &[PricingPriceListOption],
) -> String {
    if prices.is_empty() {
        return t(locale, "pricing.common.noPricing", "no pricing");
    }

    prices
        .iter()
        .map(|price| {
            let amount = match price.compare_at_amount.as_deref() {
                Some(compare_at) if !compare_at.is_empty() => {
                    format!(
                        "{} {} (compare-at {})",
                        price.currency_code, price.amount, compare_at
                    )
                }
                _ => format!("{} {}", price.currency_code, price.amount),
            };
            let discount_suffix = format_discount_suffix(price.discount_percent.as_deref());
            format!(
                "{amount}{discount_suffix} [{}]",
                format_price_row_scope(locale, price, price_list_options)
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn pricing_health_label(locale: Option<&str>, variant: &PricingVariant) -> String {
    if variant.effective_price.is_some() {
        t(locale, "pricing.health.effective", "Effective")
    } else if variant.prices.is_empty() {
        t(locale, "pricing.health.missing", "No pricing")
    } else if variant.prices.iter().any(|price| price.on_sale) {
        t(locale, "pricing.health.sale", "On sale")
    } else {
        t(locale, "pricing.health.base", "Base price")
    }
}

pub(crate) fn pricing_health_badge(variant: &PricingVariant) -> &'static str {
    if variant.effective_price.is_some() {
        "border-primary/30 bg-primary/5 text-primary"
    } else if variant.prices.is_empty() {
        "border-rose-200 bg-rose-50 text-rose-700"
    } else if variant.prices.iter().any(|price| price.on_sale) {
        "border-amber-200 bg-amber-50 text-amber-700"
    } else {
        "border-emerald-200 bg-emerald-50 text-emerald-700"
    }
}

pub(crate) fn format_price_list_option_label(
    locale: Option<&str>,
    option: &PricingPriceListOption,
) -> String {
    let mut label = format!(
        "{} ({} {})",
        option.name,
        t(locale, "pricing.detail.priceListTypeLabel", "type"),
        option.list_type
    );
    if option.rule_kind.as_deref() == Some("percentage_discount") {
        if let Some(adjustment_percent) = option.adjustment_percent.as_deref() {
            label.push_str(format!(" | -{adjustment_percent}%").as_str());
        }
    }
    if let Some(channel_scope) = format_channel_scope_text(
        locale,
        option.channel_id.as_deref(),
        option.channel_slug.as_deref(),
    ) {
        label.push_str(format!(" | {channel_scope}").as_str());
    }
    label
}

pub(crate) fn resolve_price_list_label(
    locale: Option<&str>,
    price_list_id: Option<&str>,
    options: &[PricingPriceListOption],
    base_fallback_key: &str,
    base_fallback: &str,
) -> String {
    let Some(price_list_id) = price_list_id.filter(|value| !value.trim().is_empty()) else {
        return t(locale, base_fallback_key, base_fallback);
    };

    options
        .iter()
        .find(|option| option.id == price_list_id)
        .map(|option| format_price_list_option_label(locale, option))
        .unwrap_or_else(|| price_list_id.to_string())
}

pub(crate) fn format_effective_context(
    locale: Option<&str>,
    context: &PricingResolutionContext,
    price_list_options: &[PricingPriceListOption],
) -> String {
    let region = context.region_id.clone().unwrap_or_else(|| {
        t(
            locale,
            "pricing.detail.globalRegionFallback",
            "global region",
        )
    });
    let price_list = resolve_price_list_label(
        locale,
        context.price_list_id.as_deref(),
        price_list_options,
        "pricing.detail.basePriceListFallback",
        "base prices",
    );
    let mut parts = vec![
        format!(
            "{} {}",
            t(locale, "pricing.detail.currencyInput", "currency"),
            context.currency_code
        ),
        format!(
            "{} {}",
            t(locale, "pricing.detail.regionInput", "region"),
            region
        ),
        format!(
            "{} {}",
            t(locale, "pricing.detail.priceListInput", "price list"),
            price_list
        ),
    ];
    if let Some(channel_scope) = format_channel_scope_text(
        locale,
        context.channel_id.as_deref(),
        context.channel_slug.as_deref(),
    ) {
        parts.push(channel_scope);
    }
    parts.push(format!(
        "{} {}",
        t(locale, "pricing.detail.quantityInput", "qty"),
        context.quantity
    ));
    parts.join(" | ")
}

pub(crate) fn format_effective_price(
    locale: Option<&str>,
    price: &PricingEffectivePrice,
) -> String {
    let base = if let Some(compare_at_amount) = price.compare_at_amount.as_deref() {
        format!(
            "{} {} (compare-at {})",
            price.currency_code, price.amount, compare_at_amount
        )
    } else {
        format!("{} {}", price.currency_code, price.amount)
    };

    let scope = format_price_scope(locale, price.min_quantity, price.max_quantity);
    let discount_suffix = format_discount_suffix(price.discount_percent.as_deref());

    format!(
        "{}{} | {} {}",
        base,
        discount_suffix,
        t(locale, "pricing.detail.effectiveContext", "effective"),
        scope
    )
}

pub(crate) fn format_discount_suffix(discount_percent: Option<&str>) -> String {
    discount_percent
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!(" (-{value}%)"))
        .unwrap_or_default()
}

pub(crate) fn format_price_scope(
    locale: Option<&str>,
    min_quantity: Option<i32>,
    max_quantity: Option<i32>,
) -> String {
    match (min_quantity, max_quantity) {
        (Some(min_quantity), Some(max_quantity)) => format!(
            "{} {}-{}",
            t(locale, "pricing.detail.quantityRange", "tier"),
            min_quantity,
            max_quantity
        ),
        (Some(min_quantity), None) => format!(
            "{} {}+",
            t(locale, "pricing.detail.quantityRange", "tier"),
            min_quantity
        ),
        (None, Some(max_quantity)) => format!(
            "{} 1-{}",
            t(locale, "pricing.detail.quantityRange", "tier"),
            max_quantity
        ),
        _ => t(locale, "pricing.detail.quantityDefault", "default quantity").to_string(),
    }
}

pub(crate) fn format_channel_scope_text(
    locale: Option<&str>,
    channel_id: Option<&str>,
    channel_slug: Option<&str>,
) -> Option<String> {
    let channel_slug = channel_slug
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let channel_id = channel_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    if channel_slug.is_none() && channel_id.is_none() {
        return None;
    }

    let channel_label = t(locale, "pricing.detail.channelInput", "channel");
    match (channel_slug, channel_id) {
        (Some(channel_slug), Some(channel_id)) => {
            Some(format!("{channel_label} {channel_slug} ({channel_id})"))
        }
        (Some(channel_slug), None) => Some(format!("{channel_label} {channel_slug}")),
        (None, Some(channel_id)) => Some(format!("{channel_label} {channel_id}")),
        (None, None) => None,
    }
}

pub(crate) const GLOBAL_CHANNEL_KEY: &str = "__global__";
pub(crate) const LEGACY_CHANNEL_KEY: &str = "__legacy__";

pub(crate) fn normalize_channel_value(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub(crate) fn selected_channel_key(
    channel_id: &str,
    channel_slug: &str,
    available_channels: &[PricingChannelOption],
) -> String {
    let normalized_channel_id = normalize_channel_value(channel_id);
    let normalized_channel_slug = normalize_channel_value(channel_slug);

    if normalized_channel_id.is_none() && normalized_channel_slug.is_none() {
        return GLOBAL_CHANNEL_KEY.to_string();
    }

    if let Some(option) = available_channels.iter().find(|option| {
        normalized_channel_id.as_deref() == Some(option.id.as_str())
            || normalized_channel_slug.as_deref() == Some(option.slug.as_str())
    }) {
        return option.id.clone();
    }

    LEGACY_CHANNEL_KEY.to_string()
}

pub(crate) fn apply_selected_channel_option(
    selected_key: &str,
    fallback_channel_id: &str,
    fallback_channel_slug: &str,
    available_channels: &[PricingChannelOption],
) -> (String, String) {
    match selected_key {
        GLOBAL_CHANNEL_KEY => (String::new(), String::new()),
        LEGACY_CHANNEL_KEY => (
            fallback_channel_id.to_string(),
            fallback_channel_slug.to_string(),
        ),
        _ => available_channels
            .iter()
            .find(|option| option.id == selected_key)
            .map(|option| (option.id.clone(), option.slug.clone()))
            .unwrap_or_default(),
    }
}

pub(crate) fn format_channel_option_label(
    locale: Option<&str>,
    option: &PricingChannelOption,
) -> String {
    let mut label = format!("{} ({})", option.name, option.slug);
    if option.is_default {
        label.push_str(format!(" | {}", t(locale, "pricing.channel.default", "default")).as_str());
    }
    if !option.is_active {
        label
            .push_str(format!(" | {}", t(locale, "pricing.channel.inactive", "inactive")).as_str());
    }
    label
}

pub(crate) fn format_price_row_scope(
    locale: Option<&str>,
    price: &PricingPrice,
    price_list_options: &[PricingPriceListOption],
) -> String {
    let tier_scope = format_price_scope(locale, price.min_quantity, price.max_quantity);
    let channel_scope = format_channel_scope_text(
        locale,
        price.channel_id.as_deref(),
        price.channel_slug.as_deref(),
    );
    if price.price_list_id.is_some() {
        let price_list_label = resolve_price_list_label(
            locale,
            price.price_list_id.as_deref(),
            price_list_options,
            "pricing.detail.priceListInput",
            "price list",
        );
        match channel_scope {
            Some(channel_scope) => format!("{price_list_label} | {channel_scope} | {tier_scope}"),
            None => format!("{price_list_label} | {tier_scope}"),
        }
    } else {
        match channel_scope {
            Some(channel_scope) => format!("base | {channel_scope} | {tier_scope}"),
            None => format!("base | {tier_scope}"),
        }
    }
}

pub(crate) fn text_or_none(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub(crate) fn normalized_currency_code(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.len() == 3 {
        Some(trimmed.to_ascii_uppercase())
    } else {
        None
    }
}

pub(crate) fn normalized_region_id(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Uuid::parse_str(trimmed).ok().map(|_| trimmed.to_string())
    }
}

pub(crate) fn normalized_price_list_id(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Uuid::parse_str(trimmed).ok().map(|_| trimmed.to_string())
    }
}

pub(crate) fn normalized_quantity(value: String) -> Option<i32> {
    value
        .trim()
        .parse::<i32>()
        .ok()
        .filter(|quantity| *quantity > 0)
}

pub(crate) fn build_product_admin_href(module_route_base: &str, product_id: &str) -> String {
    format!("{module_route_base}?id={product_id}")
}

pub(crate) fn build_resolution_context(
    currency_value: String,
    region_value: String,
    price_list_value: String,
    channel_id_value: String,
    channel_slug_value: String,
    quantity_value: String,
) -> Option<PricingResolutionContext> {
    let currency_code = normalized_currency_code(currency_value)?;
    Some(PricingResolutionContext {
        currency_code,
        region_id: normalized_region_id(region_value),
        price_list_id: normalized_price_list_id(price_list_value),
        channel_id: normalize_channel_value(&channel_id_value),
        channel_slug: normalize_channel_value(&channel_slug_value),
        quantity: normalized_quantity(quantity_value).unwrap_or(1),
    })
}

pub(crate) fn status_badge(status: &str) -> &'static str {
    match status {
        "ACTIVE" => "border-emerald-200 bg-emerald-50 text-emerald-700",
        "ARCHIVED" => "border-slate-200 bg-slate-100 text-slate-700",
        _ => "border-amber-200 bg-amber-50 text-amber-700",
    }
}

pub(crate) fn format_adjustment_preview(
    preview_label: &str,
    preview: &PricingAdjustmentPreview,
) -> String {
    let mut label = format!(
        "{} {} -> {} {} ({} {}%)",
        preview.currency_code,
        preview.base_amount,
        preview.currency_code,
        preview.adjusted_amount,
        preview_label,
        preview.adjustment_percent
    );
    if let Some(channel_scope) = format_channel_scope_text(
        None,
        preview.channel_id.as_deref(),
        preview.channel_slug.as_deref(),
    ) {
        label.push_str(format!(" | {channel_scope}").as_str());
    }
    label
}

#[cfg(test)]
mod tests {
    use super::*;

    fn price(currency_code: &str, on_sale: bool) -> PricingPrice {
        PricingPrice {
            currency_code: currency_code.to_string(),
            amount: "10.00".to_string(),
            compare_at_amount: None,
            discount_percent: None,
            on_sale,
            price_list_id: None,
            channel_id: None,
            channel_slug: None,
            min_quantity: None,
            max_quantity: None,
        }
    }

    fn variant(
        prices: Vec<PricingPrice>,
        effective_price: Option<PricingEffectivePrice>,
    ) -> PricingVariant {
        PricingVariant {
            id: "variant-1".to_string(),
            sku: None,
            barcode: None,
            shipping_profile_slug: None,
            title: "Variant".to_string(),
            option1: None,
            option2: None,
            option3: None,
            prices,
            effective_price,
        }
    }

    #[test]
    fn summarize_pricing_counts_priced_sale_and_distinct_currencies() {
        let summary = summarize_pricing(&[
            variant(vec![price("USD", true)], None),
            variant(
                Vec::new(),
                Some(PricingEffectivePrice {
                    currency_code: "EUR".to_string(),
                    amount: "9.00".to_string(),
                    compare_at_amount: None,
                    discount_percent: None,
                    on_sale: false,
                    region_id: None,
                    price_list_id: None,
                    channel_id: None,
                    channel_slug: None,
                    min_quantity: None,
                    max_quantity: None,
                }),
            ),
        ]);

        assert_eq!(summary.variant_count, 2);
        assert_eq!(summary.priced_variants, 1);
        assert_eq!(summary.on_sale_variants, 1);
        assert_eq!(summary.currency_count, 2);
    }

    #[test]
    fn build_resolution_context_normalizes_admin_inputs() {
        let context = build_resolution_context(
            " usd ".to_string(),
            "not-a-uuid".to_string(),
            "550e8400-e29b-41d4-a716-446655440000".to_string(),
            " channel-id ".to_string(),
            " channel-slug ".to_string(),
            "0".to_string(),
        )
        .expect("valid currency should build a context");

        assert_eq!(context.currency_code, "USD");
        assert_eq!(context.region_id, None);
        assert_eq!(
            context.price_list_id.as_deref(),
            Some("550e8400-e29b-41d4-a716-446655440000")
        );
        assert_eq!(context.channel_id.as_deref(), Some("channel-id"));
        assert_eq!(context.channel_slug.as_deref(), Some("channel-slug"));
        assert_eq!(context.quantity, 1);
    }

    #[test]
    fn selected_channel_key_preserves_global_known_and_legacy_scopes() {
        let channels = [PricingChannelOption {
            id: "channel-id".to_string(),
            slug: "web".to_string(),
            name: "Web".to_string(),
            is_active: true,
            is_default: true,
            status: "ACTIVE".to_string(),
        }];

        assert_eq!(selected_channel_key("", "", &channels), GLOBAL_CHANNEL_KEY);
        assert_eq!(selected_channel_key("", " web ", &channels), "channel-id");
        assert_eq!(
            selected_channel_key("unknown", "", &channels),
            LEGACY_CHANNEL_KEY
        );
    }
}
