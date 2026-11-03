use std::collections::BTreeSet;

use rustok_api::locale_tags_match;
use rustok_ui_core::apply_ui_query_pairs;
#[cfg(feature = "ssr")]
use rustok_ui_core::normalize_optional_ui_text;

use crate::i18n::t;
use crate::model::{
    PricingChannelOption, PricingPrice, PricingPriceListOption, PricingProductDetail,
    PricingResolutionContext, PricingVariant,
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
pub enum StorefrontPricingQueryError {
    InvalidCurrencyCode,
    InvalidUuid { field_name: &'static str },
    MissingCurrencyCodeForResolutionContext,
    NonPositiveQuantity,
    InvalidQuantity,
    InvalidChannelSlug,
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
            Self::InvalidQuantity => write!(f, "quantity must be a positive 32-bit integer"),
            Self::InvalidChannelSlug => write!(f, "channel_slug must not be empty"),
        }
    }
}

impl std::error::Error for StorefrontPricingQueryError {}

/// A malformed explicit URL quantity is not an omitted/default-one quantity.
pub fn parse_pricing_route_quantity(
    value: Option<&str>,
) -> Result<Option<i32>, StorefrontPricingQueryError> {
    value
        .map(|value| {
            let value = value.trim();
            if value.is_empty()
                || value.len() > 10
                || !value.bytes().all(|digit| digit.is_ascii_digit())
            {
                return Err(StorefrontPricingQueryError::InvalidQuantity);
            }
            value
                .parse::<i32>()
                .ok()
                .filter(|quantity| *quantity > 0)
                .ok_or(StorefrontPricingQueryError::InvalidQuantity)
        })
        .transpose()
}

pub fn parse_optional_currency_code(
    currency_code: Option<String>,
) -> Result<Option<String>, StorefrontPricingQueryError> {
    let Some(currency_code) = currency_code.map(|value| value.trim().to_string()) else {
        return Ok(None);
    };
    let normalized = currency_code.to_ascii_uppercase();
    if normalized.len() != 3 || !normalized.chars().all(|ch| ch.is_ascii_alphabetic()) {
        return Err(StorefrontPricingQueryError::InvalidCurrencyCode);
    }

    Ok(Some(normalized))
}

pub fn parse_optional_uuid_string(
    value: Option<String>,
    field_name: &'static str,
) -> Result<Option<String>, StorefrontPricingQueryError> {
    let Some(value) = value.map(|value| value.trim().to_string()) else {
        return Ok(None);
    };

    let parsed = uuid::Uuid::parse_str(value.as_str())
        .map_err(|_| StorefrontPricingQueryError::InvalidUuid { field_name })?;

    if parsed.is_nil() {
        return Err(StorefrontPricingQueryError::InvalidUuid { field_name });
    }

    Ok(Some(parsed.to_string()))
}

pub(crate) fn sanitize_channel_slug(channel_slug: Option<String>) -> Option<String> {
    channel_slug.map(|value| value.trim().to_ascii_lowercase())
}

#[cfg(feature = "ssr")]
pub(crate) fn resolve_requested_locale(
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

pub fn parse_resolution_quantity(
    quantity: Option<i32>,
) -> Result<i32, StorefrontPricingQueryError> {
    match quantity {
        Some(value) if value < 1 => Err(StorefrontPricingQueryError::NonPositiveQuantity),
        Some(value) => Ok(value),
        None => Ok(1),
    }
}

pub fn normalize_pricing_context(
    currency_code: Option<String>,
    region_id: Option<String>,
    price_list_id: Option<String>,
    channel_id: Option<String>,
    channel_slug: Option<String>,
    quantity: Option<i32>,
) -> Result<Option<PricingResolutionContext>, StorefrontPricingQueryError> {
    let region_id = parse_optional_uuid_string(region_id, "region_id")?;
    let price_list_id = parse_optional_uuid_string(price_list_id, "price_list_id")?;
    let channel_id = parse_optional_uuid_string(channel_id, "channel_id")?;
    let channel_slug = sanitize_channel_slug(channel_slug);
    if channel_slug.as_deref().is_some_and(str::is_empty) {
        return Err(StorefrontPricingQueryError::InvalidChannelSlug);
    }
    let requires_currency = region_id.is_some() || price_list_id.is_some() || quantity.is_some();
    let parsed_quantity = parse_resolution_quantity(quantity)?;
    let Some(currency_code) = parse_optional_currency_code(currency_code)? else {
        if requires_currency {
            return Err(StorefrontPricingQueryError::MissingCurrencyCodeForResolutionContext);
        }
        return Ok(None);
    };
    Ok(Some(PricingResolutionContext {
        currency_code,
        region_id,
        price_list_id,
        channel_id,
        channel_slug,
        quantity: parsed_quantity,
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
                    crate::i18n::format(
                        locale,
                        "pricing.variant.compareAt",
                        Some(&rustok_ui_i18n::fluent_args!("value" => compare.to_string())),
                        "compare-at {value}",
                    ),
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
    if let Some(adjustment_percent) = option
        .adjustment_percent
        .as_deref()
        .filter(|_| option.rule_kind.as_deref() == Some("percentage_discount"))
    {
        label.push_str(format!(" | -{adjustment_percent}%").as_str());
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
            crate::i18n::format(
                locale,
                "pricing.variant.compareAt",
                Some(&rustok_ui_i18n::fluent_args!("value" => compare_at_amount.to_string())),
                "compare-at {value}",
            ),
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
    apply_ui_query_pairs(
        module_route_base,
        &[
            ("handle", params.selected_handle.map(str::to_string)),
            ("currency", params.currency_code.map(str::to_string)),
            ("region_id", params.region_id.map(str::to_string)),
            ("price_list_id", params.price_list_id.map(str::to_string)),
            ("channel_id", params.channel_id.map(str::to_string)),
            ("channel_slug", params.channel_slug.map(str::to_string)),
            (
                "quantity",
                params
                    .quantity
                    .filter(|quantity| *quantity > 0)
                    .map(|quantity| quantity.to_string()),
            ),
        ],
    )
}

pub(crate) fn build_product_storefront_href(
    module_route_base: &str,
    handle: Option<&str>,
) -> String {
    apply_ui_query_pairs(module_route_base, &[("handle", handle.map(str::to_string))])
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PricingSelectorOption {
    pub label: String,
    pub href: String,
    pub selected: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PricingSelectorViewModel {
    pub base_price_list_href: String,
    pub base_price_list_selected: bool,
    pub current_channel_href: String,
    pub current_channel_selected: bool,
    pub lists: Vec<PricingSelectorOption>,
    pub channels: Vec<PricingSelectorOption>,
}

pub(crate) fn build_resolution_selector(
    locale: Option<&str>,
    base: &str,
    handle: Option<&str>,
    context: &PricingResolutionContext,
    channels: &[PricingChannelOption],
    lists: &[PricingPriceListOption],
) -> PricingSelectorViewModel {
    let params = PricingRouteParams {
        selected_handle: handle,
        currency_code: Some(&context.currency_code),
        region_id: context.region_id.as_deref(),
        quantity: Some(context.quantity),
        channel_id: context.channel_id.as_deref(),
        channel_slug: context.channel_slug.as_deref(),
        price_list_id: context.price_list_id.as_deref(),
    };
    PricingSelectorViewModel {
        base_price_list_href: build_pricing_route_href(
            base,
            PricingRouteParams {
                price_list_id: None,
                ..params
            },
        ),
        base_price_list_selected: context.price_list_id.is_none(),
        current_channel_href: build_pricing_route_href(
            base,
            PricingRouteParams {
                channel_id: None,
                channel_slug: None,
                price_list_id: None,
                ..params
            },
        ),
        current_channel_selected: context.channel_id.is_none() && context.channel_slug.is_none(),
        lists: lists
            .iter()
            .map(|list| PricingSelectorOption {
                label: format_price_list_option_label(locale, list),
                selected: context.price_list_id.as_deref() == Some(list.id.as_str()),
                href: build_pricing_route_href(
                    base,
                    PricingRouteParams {
                        price_list_id: Some(&list.id),
                        ..params
                    },
                ),
            })
            .collect(),
        channels: channels
            .iter()
            .map(|channel| PricingSelectorOption {
                label: format_channel_option_label(locale, channel),
                selected: context.channel_id.as_deref() == Some(channel.id.as_str())
                    || context.channel_slug.as_deref() == Some(channel.slug.as_str()),
                href: build_pricing_route_href(
                    base,
                    PricingRouteParams {
                        channel_id: Some(&channel.id),
                        channel_slug: Some(&channel.slug),
                        price_list_id: None,
                        ..params
                    },
                ),
            })
            .collect(),
    }
}

pub(crate) fn build_currency_links(
    base: &str,
    handle: Option<&str>,
    product: &crate::model::PricingProductDetail,
) -> Vec<PricingSelectorOption> {
    let context = product.resolution_context.as_ref();
    let mut currencies = product
        .variants
        .iter()
        .flat_map(|variant| variant.prices.iter())
        .map(|price| price.currency_code.clone())
        .collect::<BTreeSet<_>>();
    if let Some(context) = context {
        currencies.insert(context.currency_code.clone());
    }
    currencies
        .into_iter()
        .map(|currency| {
            let selected = context.is_some_and(|context| context.currency_code == currency);
            let href = build_pricing_route_href(
                base,
                PricingRouteParams {
                    selected_handle: handle,
                    currency_code: Some(&currency),
                    // Currency selection is explicit; never silently convert a region's currency.
                    region_id: context
                        .filter(|_| selected)
                        .and_then(|context| context.region_id.as_deref()),
                    price_list_id: context.and_then(|context| context.price_list_id.as_deref()),
                    channel_id: context.and_then(|context| context.channel_id.as_deref()),
                    channel_slug: context.and_then(|context| context.channel_slug.as_deref()),
                    quantity: context.map(|context| context.quantity).or(Some(1)),
                },
            );
            PricingSelectorOption {
                label: currency,
                href,
                selected,
            }
        })
        .collect()
}

/// Only absent channel selectors permit host/Channel-owner completion. Region and list nulls
/// are exact, not wildcard eligibility. This validates wire observations, not customer rights.
pub fn pricing_context_matches_request(
    actual: &PricingResolutionContext,
    expected: &PricingResolutionContext,
) -> bool {
    actual.currency_code == expected.currency_code
        && actual.quantity == expected.quantity
        && actual.region_id == expected.region_id
        && actual.price_list_id == expected.price_list_id
        && expected
            .channel_id
            .as_ref()
            .is_none_or(|id| actual.channel_id.as_ref() == Some(id))
        && expected
            .channel_slug
            .as_ref()
            .is_none_or(|slug| actual.channel_slug.as_ref() == Some(slug))
}

/// Shared wire-time admission, not a client freshness clock or reservation policy.
pub fn valid_quote_time(time: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(time).is_ok()
}

/// Wire observation integrity only. Identity/scope completion and money policy stay in Pricing.
pub(crate) fn pricing_quote_matches_request(
    product: &PricingProductDetail,
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
                && product.resolved_at.as_deref().is_some_and(valid_quote_time)
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

    #[test]
    fn malformed_explicit_route_quantity_never_becomes_an_omitted_quantity() {
        assert_eq!(parse_pricing_route_quantity(None), Ok(None));
        assert_eq!(parse_pricing_route_quantity(Some("2")), Ok(Some(2)));
        for value in ["", "0", "-1", "+1", "1.5", "2tail", "2147483648"] {
            assert_eq!(
                parse_pricing_route_quantity(Some(value)),
                Err(StorefrontPricingQueryError::InvalidQuantity)
            );
        }
    }

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
        let mut product = PricingProductDetail {
            resolution_context: None,
            resolved_at: None,
            id: "product".into(),
            status: "active".into(),
            seller_id: None,
            vendor: None,
            product_type: None,
            published_at: None,
            translations: vec![],
            variants: vec![PricingVariant {
                id: "sku".into(),
                title: "SKU".into(),
                sku: None,
                prices: vec![],
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
    fn pricing_links_encode_values_and_keep_query_and_fragment_boundaries() {
        let href = build_pricing_route_href(
            "/pricing?keep=1#quote",
            PricingRouteParams {
                selected_handle: Some("boots&channel_id=foreign"),
                channel_slug: Some("web store"),
                ..PricingRouteParams::default()
            },
        );
        assert_eq!(
            href,
            "/pricing?keep=1&handle=boots%26channel_id%3Dforeign&channel_slug=web+store#quote"
        );
    }

    #[test]
    fn clearing_a_list_keeps_channel_but_switching_channel_clears_the_list() {
        let context = PricingResolutionContext {
            currency_code: "EUR".into(),
            region_id: None,
            price_list_id: Some("list-web".into()),
            channel_id: Some("web-id".into()),
            channel_slug: Some("web".into()),
            quantity: 3,
        };
        let model =
            build_resolution_selector(Some("en"), "/pricing", Some("boots"), &context, &[], &[]);
        assert!(model.base_price_list_href.contains("channel_id=web-id"));
        assert!(!model.base_price_list_href.contains("price_list_id="));
        assert!(!model.current_channel_href.contains("channel_id="));
        assert!(!model.current_channel_href.contains("price_list_id="));
        assert!(model.current_channel_href.contains("quantity=3"));
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
            normalize_pricing_context(Some("EU1".to_string()), None, None, None, None, Some(1))
                .expect_err("invalid currency should be rejected");

        assert_eq!(error, StorefrontPricingQueryError::InvalidCurrencyCode);
    }

    #[test]
    fn storefront_pricing_resolution_context_rejects_non_positive_quantity() {
        let error =
            normalize_pricing_context(Some("EUR".to_string()), None, None, None, None, Some(0))
                .expect_err("invalid quantity should be rejected");

        assert_eq!(error, StorefrontPricingQueryError::NonPositiveQuantity);
    }

    #[test]
    fn storefront_pricing_resolution_context_rejects_modifiers_without_currency_code() {
        let error = normalize_pricing_context(
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
        let error =
            normalize_pricing_context(None, None, None, Some("not-a-uuid".to_string()), None, None)
                .expect_err("invalid channel_id should be rejected");

        assert_eq!(
            error,
            StorefrontPricingQueryError::InvalidUuid {
                field_name: "channel_id"
            }
        );

        let nil_channel_error = normalize_pricing_context(
            None,
            None,
            None,
            Some(uuid::Uuid::nil().to_string()),
            None,
            None,
        )
        .expect_err("nil channel_id should be rejected");

        assert_eq!(
            nil_channel_error,
            StorefrontPricingQueryError::InvalidUuid {
                field_name: "channel_id"
            }
        );

        let nil_region_error = normalize_pricing_context(
            Some("USD".to_string()),
            Some(uuid::Uuid::nil().to_string()),
            None,
            None,
            None,
            None,
        )
        .expect_err("nil region_id should be rejected");

        assert_eq!(
            nil_region_error,
            StorefrontPricingQueryError::InvalidUuid {
                field_name: "region_id"
            }
        );

        let nil_price_list_error = normalize_pricing_context(
            Some("USD".to_string()),
            None,
            Some(uuid::Uuid::nil().to_string()),
            None,
            None,
            None,
        )
        .expect_err("nil price_list_id should be rejected");

        assert_eq!(
            nil_price_list_error,
            StorefrontPricingQueryError::InvalidUuid {
                field_name: "price_list_id"
            }
        );
    }
}
