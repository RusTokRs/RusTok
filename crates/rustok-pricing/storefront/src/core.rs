use std::collections::BTreeSet;

use rustok_core::locale_tags_match;

use crate::i18n::t;
use crate::model::{
    PricingChannelOption, PricingPrice, PricingPriceListOption, PricingResolutionContext,
    PricingVariant,
};

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct StorefrontPricingQuery {
    pub(crate) selected_handle: Option<String>,
    pub(crate) locale: Option<String>,
    pub(crate) currency_code: Option<String>,
    pub(crate) region_id: Option<String>,
    pub(crate) price_list_id: Option<String>,
    pub(crate) channel_id: Option<String>,
    pub(crate) channel_slug: Option<String>,
    pub(crate) quantity: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StorefrontPricingQueryError {
    InvalidCurrencyCode,
    InvalidUuid { field_name: &'static str },
    MissingCurrencyCodeForResolutionContext,
    NonPositiveQuantity,
}

impl std::fmt::Display for StorefrontPricingQueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidCurrencyCode => write!(f, "currency_code must be a 3-letter code"),
            Self::InvalidUuid { field_name } => write!(f, "Invalid {field_name}"),
            Self::MissingCurrencyCodeForResolutionContext => {
                write!(
                    f,
                    "currency_code is required for pricing resolution context"
                )
            }
            Self::NonPositiveQuantity => write!(f, "quantity must be at least 1"),
        }
    }
}

impl std::error::Error for StorefrontPricingQueryError {}

pub(crate) fn text_or_none(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub(crate) fn parse_optional_currency_code(
    currency_code: Option<String>,
) -> Result<Option<String>, StorefrontPricingQueryError> {
    let Some(currency_code) = currency_code.and_then(text_or_none) else {
        return Ok(None);
    };
    let normalized = currency_code.to_ascii_uppercase();
    if normalized.len() != 3 || !normalized.chars().all(|ch| ch.is_ascii_alphabetic()) {
        return Err(StorefrontPricingQueryError::InvalidCurrencyCode);
    }

    Ok(Some(normalized))
}

pub(crate) fn parse_optional_uuid_string(
    value: Option<String>,
    field_name: &'static str,
) -> Result<Option<String>, StorefrontPricingQueryError> {
    let Some(value) = value.and_then(text_or_none) else {
        return Ok(None);
    };

    uuid::Uuid::parse_str(value.as_str())
        .map(|_| Some(value))
        .map_err(|_| StorefrontPricingQueryError::InvalidUuid { field_name })
}

pub(crate) fn sanitize_channel_slug(channel_slug: Option<String>) -> Option<String> {
    channel_slug
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
}

#[cfg(feature = "ssr")]
pub(crate) fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

#[cfg(feature = "ssr")]
pub(crate) fn resolve_requested_locale(
    requested: Option<String>,
    request_context_locale: Option<&str>,
    tenant_default_locale: &str,
) -> String {
    normalize_optional(requested)
        .or_else(|| {
            request_context_locale.and_then(|value| normalize_optional(Some(value.to_string())))
        })
        .or_else(|| normalize_optional(Some(tenant_default_locale.to_string())))
        .unwrap_or_default()
}

pub(crate) fn parse_resolution_quantity(
    quantity: Option<i32>,
) -> Result<i32, StorefrontPricingQueryError> {
    match quantity {
        Some(value) if value < 1 => Err(StorefrontPricingQueryError::NonPositiveQuantity),
        Some(value) => Ok(value),
        None => Ok(1),
    }
}

pub(crate) fn sanitize_resolution_context(
    currency_code: Option<String>,
    region_id: Option<String>,
    price_list_id: Option<String>,
    channel_id: Option<String>,
    channel_slug: Option<String>,
    quantity: Option<i32>,
) -> Result<Option<PricingResolutionContext>, StorefrontPricingQueryError> {
    let channel_id = parse_optional_uuid_string(channel_id, "channel_id")?;
    let requires_currency = region_id
        .as_ref()
        .and_then(|value| text_or_none(value.clone()))
        .is_some()
        || price_list_id
            .as_ref()
            .and_then(|value| text_or_none(value.clone()))
            .is_some()
        || quantity.is_some();
    let Some(currency_code) = parse_optional_currency_code(currency_code)? else {
        if requires_currency {
            return Err(StorefrontPricingQueryError::MissingCurrencyCodeForResolutionContext);
        }
        return Ok(None);
    };

    Ok(Some(PricingResolutionContext {
        currency_code,
        region_id: parse_optional_uuid_string(region_id, "region_id")?,
        price_list_id: parse_optional_uuid_string(price_list_id, "price_list_id")?,
        channel_id,
        channel_slug: sanitize_channel_slug(channel_slug),
        quantity: parse_resolution_quantity(quantity)?,
    }))
}

#[cfg(feature = "ssr")]
pub(crate) fn normalize_public_channel_slug(channel_slug: Option<&str>) -> Option<String> {
    channel_slug
        .map(str::trim)
        .filter(|slug| !slug.is_empty())
        .map(|slug| slug.to_ascii_lowercase())
}

pub(crate) struct PricingSummary {
    pub(crate) currency_count: usize,
    pub(crate) sale_variant_count: usize,
    pub(crate) variant_count: usize,
}

pub(crate) fn summarize_pricing(variants: &[PricingVariant]) -> PricingSummary {
    let mut currencies = BTreeSet::new();
    let sale_variant_count = variants
        .iter()
        .filter(|variant| {
            variant.prices.iter().any(|price| {
                currencies.insert(price.currency_code.clone());
                price.on_sale
            })
        })
        .count();

    for variant in variants {
        for price in &variant.prices {
            currencies.insert(price.currency_code.clone());
        }
    }

    PricingSummary {
        currency_count: currencies.len(),
        sale_variant_count,
        variant_count: variants.len(),
    }
}

pub(crate) fn pricing_translation_for_locale<'a>(
    translations: &'a [crate::model::PricingProductTranslation],
    requested_locale: Option<&str>,
) -> Option<&'a crate::model::PricingProductTranslation> {
    requested_locale
        .and_then(|locale| {
            translations
                .iter()
                .find(|translation| locale_tags_match(&translation.locale, locale))
        })
        .or_else(|| translations.first())
}

pub(crate) fn format_seller_boundary(locale: Option<&str>, seller_id: Option<&str>) -> String {
    match seller_id.map(str::trim).filter(|value| !value.is_empty()) {
        Some(seller_id) => format!(
            "{}: {seller_id}",
            t(locale, "pricing.common.sellerId", "seller id")
        ),
        None => t(
            locale,
            "pricing.common.sellerUnassigned",
            "seller id: unassigned",
        ),
    }
}

pub(crate) fn format_variant_identity(locale: Option<&str>, variant: &PricingVariant) -> String {
    if let Some(sku) = variant
        .sku
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        format!(
            "{}: {}",
            t(locale, "pricing.variant.sku", "SKU"),
            sku.trim()
        )
    } else {
        t(locale, "pricing.variant.noSku", "SKU not assigned")
    }
}

pub(crate) fn format_variant_prices(locale: Option<&str>, prices: &[PricingPrice]) -> String {
    if prices.is_empty() {
        return t(locale, "pricing.variant.noPrices", "No prices assigned");
    }

    prices
        .iter()
        .map(|price| {
            if let Some(compare) = price.compare_at_amount.as_deref() {
                format!(
                    "{} {} ({}){}",
                    price.currency_code,
                    price.amount,
                    t(locale, "pricing.variant.compareAt", "compare-at {value}")
                        .replace("{value}", compare),
                    format_discount_suffix(price.discount_percent.as_deref()),
                )
            } else {
                format!(
                    "{} {}{}",
                    price.currency_code,
                    price.amount,
                    format_discount_suffix(price.discount_percent.as_deref())
                )
            }
        })
        .collect::<Vec<_>>()
        .join(" • ")
}

pub(crate) fn pricing_health_label(locale: Option<&str>, variant: &PricingVariant) -> String {
    if variant.effective_price.is_some() {
        return t(locale, "pricing.health.effective", "effective");
    }
    if variant.prices.is_empty() {
        return t(locale, "pricing.health.missing", "missing");
    }
    if variant.prices.iter().any(|price| price.on_sale) {
        return t(locale, "pricing.health.sale", "sale");
    }
    t(locale, "pricing.health.covered", "covered")
}

pub(crate) fn pricing_health_badge(variant: &PricingVariant) -> &'static str {
    if variant.effective_price.is_some() {
        "border-primary/30 text-primary"
    } else if variant.prices.is_empty() {
        "border-destructive/30 text-destructive"
    } else if variant.prices.iter().any(|price| price.on_sale) {
        "border-emerald-500/30 text-emerald-700"
    } else {
        "border-border text-muted-foreground"
    }
}

pub(crate) fn format_price_list_option_label(
    locale: Option<&str>,
    option: &PricingPriceListOption,
) -> String {
    let mut label = format!(
        "{} ({} {})",
        option.name,
        t(locale, "pricing.selected.priceListTypeLabel", "type"),
        option.list_type
    );
    if option.rule_kind.as_deref() == Some("percentage_discount") {
        if let Some(adjustment_percent) = option.adjustment_percent.as_deref() {
            label.push_str(format!(" | -{adjustment_percent}%").as_str());
        }
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
            "pricing.selected.globalRegionFallback",
            "global region",
        )
    });
    let price_list = resolve_price_list_label(
        locale,
        context.price_list_id.as_deref(),
        price_list_options,
        "pricing.selected.basePriceListFallback",
        "base prices",
    );
    let mut parts = vec![
        format!(
            "{} {}",
            t(locale, "pricing.selected.currencyLabel", "currency"),
            context.currency_code
        ),
        format!(
            "{} {}",
            t(locale, "pricing.selected.regionLabel", "region"),
            region
        ),
        format!(
            "{} {}",
            t(locale, "pricing.selected.priceListLabel", "price list"),
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
        t(locale, "pricing.selected.quantityLabel", "qty"),
        context.quantity
    ));
    parts.join(" | ")
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

    let channel_label = t(locale, "pricing.selected.channelLabel", "channel");
    match (channel_slug, channel_id) {
        (Some(channel_slug), Some(channel_id)) => {
            Some(format!("{channel_label} {channel_slug} ({channel_id})"))
        }
        (Some(channel_slug), None) => Some(format!("{channel_label} {channel_slug}")),
        (None, Some(channel_id)) => Some(format!("{channel_label} {channel_id}")),
        (None, None) => None,
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

pub(crate) fn format_effective_price(
    locale: Option<&str>,
    price: &crate::model::PricingEffectivePrice,
) -> String {
    let base = if let Some(compare_at_amount) = price.compare_at_amount.as_deref() {
        format!(
            "{} {} ({}){}",
            price.currency_code,
            price.amount,
            t(locale, "pricing.variant.compareAt", "compare-at {value}")
                .replace("{value}", compare_at_amount),
            format_discount_suffix(price.discount_percent.as_deref()),
        )
    } else {
        format!(
            "{} {}{}",
            price.currency_code,
            price.amount,
            format_discount_suffix(price.discount_percent.as_deref())
        )
    };

    let scope = match (price.min_quantity, price.max_quantity) {
        (Some(min_quantity), Some(max_quantity)) => format!(
            "{} {}-{}",
            t(locale, "pricing.selected.quantityRange", "tier"),
            min_quantity,
            max_quantity
        ),
        (Some(min_quantity), None) => format!(
            "{} {}+",
            t(locale, "pricing.selected.quantityRange", "tier"),
            min_quantity
        ),
        _ => t(
            locale,
            "pricing.selected.quantityDefault",
            "default quantity",
        )
        .to_string(),
    };

    format!(
        "{} | {} {}",
        base,
        t(locale, "pricing.selected.effectiveLabel", "effective"),
        scope
    )
}

pub(crate) fn format_discount_suffix(discount_percent: Option<&str>) -> String {
    discount_percent
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!(" (-{value}%)"))
        .unwrap_or_default()
}

pub(crate) fn selector_badge_class(active: bool) -> &'static str {
    if active {
        "inline-flex items-center rounded-full border border-primary/30 bg-primary/5 px-3 py-1 text-xs font-medium text-primary"
    } else {
        "inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground transition hover:border-primary/30 hover:text-primary"
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct PricingRouteParams<'a> {
    pub(crate) selected_handle: Option<&'a str>,
    pub(crate) currency_code: Option<&'a str>,
    pub(crate) region_id: Option<&'a str>,
    pub(crate) price_list_id: Option<&'a str>,
    pub(crate) channel_id: Option<&'a str>,
    pub(crate) channel_slug: Option<&'a str>,
    pub(crate) quantity: Option<i32>,
}

pub(crate) fn build_pricing_route_href(
    module_route_base: &str,
    params: PricingRouteParams<'_>,
) -> String {
    let mut query_params = Vec::new();

    if let Some(handle) = params
        .selected_handle
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        query_params.push(format!("handle={handle}"));
    }
    if let Some(currency_code) = params
        .currency_code
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        query_params.push(format!("currency={currency_code}"));
    }
    if let Some(region_id) = params
        .region_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        query_params.push(format!("region_id={region_id}"));
    }
    if let Some(price_list_id) = params
        .price_list_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        query_params.push(format!("price_list_id={price_list_id}"));
    }
    if let Some(channel_id) = params
        .channel_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        query_params.push(format!("channel_id={channel_id}"));
    }
    if let Some(channel_slug) = params
        .channel_slug
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        query_params.push(format!("channel_slug={channel_slug}"));
    }
    if let Some(quantity) = params.quantity.filter(|value| *value > 0) {
        query_params.push(format!("quantity={quantity}"));
    }

    if query_params.is_empty() {
        module_route_base.to_string()
    } else {
        format!("{module_route_base}?{}", query_params.join("&"))
    }
}

pub(crate) fn build_product_storefront_href(
    module_route_base: &str,
    handle: Option<&str>,
) -> String {
    match handle.map(str::trim).filter(|value| !value.is_empty()) {
        Some(handle) => format!("{module_route_base}?handle={handle}"),
        None => module_route_base.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pricing_route_href_omits_blank_and_non_positive_values() {
        let href = build_pricing_route_href(
            "/pricing",
            PricingRouteParams {
                selected_handle: Some(" sample-product "),
                currency_code: Some(" "),
                region_id: Some("region-1"),
                price_list_id: None,
                channel_id: Some("channel-1"),
                channel_slug: Some("main"),
                quantity: Some(0),
            },
        );

        assert_eq!(
            href,
            "/pricing?handle=sample-product&region_id=region-1&channel_id=channel-1&channel_slug=main"
        );
    }

    #[test]
    fn channel_scope_text_prefers_slug_with_id_context() {
        assert_eq!(
            format_channel_scope_text(Some("en"), Some("channel-1"), Some(" main ")),
            Some("channel main (channel-1)".to_string())
        );
        assert_eq!(format_channel_scope_text(Some("en"), Some(" "), None), None);
    }

    #[test]
    fn storefront_pricing_resolution_context_rejects_non_letter_currency_code() {
        let error =
            sanitize_resolution_context(Some("EU1".to_string()), None, None, None, None, Some(1))
                .expect_err("invalid currency should be rejected");

        assert_eq!(error, StorefrontPricingQueryError::InvalidCurrencyCode);
    }

    #[test]
    fn storefront_pricing_resolution_context_rejects_non_positive_quantity() {
        let error =
            sanitize_resolution_context(Some("EUR".to_string()), None, None, None, None, Some(0))
                .expect_err("invalid quantity should be rejected");

        assert_eq!(error, StorefrontPricingQueryError::NonPositiveQuantity);
    }

    #[test]
    fn storefront_pricing_resolution_context_rejects_modifiers_without_currency_code() {
        let error = sanitize_resolution_context(
            None,
            Some(uuid::Uuid::new_v4().to_string()),
            None,
            None,
            None,
            None,
        )
        .expect_err("region_id without currency should be rejected");

        assert_eq!(
            error,
            StorefrontPricingQueryError::MissingCurrencyCodeForResolutionContext
        );
    }

    #[test]
    fn storefront_pricing_resolution_context_rejects_invalid_channel_id() {
        let error = sanitize_resolution_context(
            None,
            None,
            None,
            Some("not-a-uuid".to_string()),
            None,
            None,
        )
        .expect_err("invalid channel_id should be rejected");

        assert_eq!(
            error,
            StorefrontPricingQueryError::InvalidUuid {
                field_name: "channel_id"
            }
        );
    }
}
