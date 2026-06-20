use std::collections::BTreeSet;

use crate::core::routing::format_channel_scope_text;
use crate::i18n::t;
use crate::model::{
    PricingAdjustmentPreview, PricingEffectivePrice, PricingPrice, PricingPriceListOption,
    PricingProductDetail, PricingProductListItem, PricingProductTranslation,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PricingProductListItemViewModel {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) status_label: String,
    pub(crate) status_badge_class: &'static str,
    pub(crate) shipping_profile_label: String,
    pub(crate) meta_line: String,
}

pub(crate) fn build_product_list_item_view_model(
    locale: Option<&str>,
    product: &PricingProductListItem,
) -> PricingProductListItemViewModel {
    PricingProductListItemViewModel {
        id: product.id.clone(),
        title: product.title.clone(),
        status_label: localized_product_status(locale, product.status.as_str()),
        status_badge_class: status_badge(product.status.as_str()),
        shipping_profile_label: product
            .shipping_profile_slug
            .clone()
            .unwrap_or_else(|| t(locale, "pricing.common.unassigned", "unassigned")),
        meta_line: format_product_meta(locale, product),
    }
}

pub(crate) fn pricing_product_list_item_class(is_selected: bool) -> &'static str {
    if is_selected {
        "rounded-2xl border border-primary/40 bg-background p-5 shadow-sm"
    } else {
        "rounded-2xl border border-border bg-background p-5 transition hover:border-primary/40"
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PricingProductDetailHeaderViewModel {
    pub(crate) title: String,
    pub(crate) status_label: String,
    pub(crate) status_badge_class: &'static str,
    pub(crate) meta_line: String,
    pub(crate) seller_line: String,
    pub(crate) shipping_line: String,
    pub(crate) created_line: String,
    pub(crate) published_line: String,
}

pub(crate) fn build_product_detail_header_view_model(
    locale: Option<&str>,
    effective_locale: Option<&str>,
    detail: &PricingProductDetail,
) -> PricingProductDetailHeaderViewModel {
    let resolved_translation =
        pricing_translation_for_locale(detail.translations.as_slice(), effective_locale);
    let title = resolved_translation
        .map(|item| item.title.clone())
        .unwrap_or_else(|| t(locale, "pricing.detail.untitled", "Untitled"));
    let handle = resolved_translation
        .map(|item| item.handle.clone())
        .unwrap_or_else(|| "-".to_string());
    let shipping_profile = detail
        .shipping_profile_slug
        .clone()
        .unwrap_or_else(|| t(locale, "pricing.common.unassigned", "unassigned"));
    let vendor = detail
        .vendor
        .clone()
        .unwrap_or_else(|| t(locale, "pricing.common.notSet", "not set"));
    let seller_id = detail
        .seller_id
        .clone()
        .unwrap_or_else(|| t(locale, "pricing.common.notSet", "not set"));
    let product_type = detail
        .product_type
        .clone()
        .unwrap_or_else(|| t(locale, "pricing.common.notSet", "not set"));

    PricingProductDetailHeaderViewModel {
        title,
        status_label: localized_product_status(locale, detail.status.as_str()),
        status_badge_class: status_badge(detail.status.as_str()),
        meta_line: format!("handle: {handle} | vendor: {vendor} | type: {product_type}"),
        seller_line: format!(
            "{}: {}",
            t(locale, "pricing.detail.seller", "seller"),
            seller_id
        ),
        shipping_line: format!(
            "shipping profile: {shipping_profile} | updated {}",
            detail.updated_at
        ),
        created_line: format!("created {}", detail.created_at),
        published_line: format!(
            "published {}",
            detail
                .published_at
                .clone()
                .unwrap_or_else(|| "-".to_string())
        ),
    }
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PricingVariantCardViewModel {
    pub(crate) title: String,
    pub(crate) health_label: String,
    pub(crate) health_badge_class: &'static str,
    pub(crate) identity_line: String,
    pub(crate) profile_line: String,
    pub(crate) effective_price_line: Option<String>,
    pub(crate) price_table: String,
}

pub(crate) fn build_variant_card_view_model(
    locale: Option<&str>,
    variant: &PricingVariant,
    price_list_options: &[PricingPriceListOption],
) -> PricingVariantCardViewModel {
    let profile_label = variant.shipping_profile_slug.clone().unwrap_or_else(|| {
        t(
            locale,
            "pricing.common.inheritProductProfile",
            "inherits product profile",
        )
    });

    PricingVariantCardViewModel {
        title: variant.title.clone(),
        health_label: pricing_health_label(locale, variant),
        health_badge_class: pricing_health_badge(variant),
        identity_line: format_variant_identity(locale, variant),
        profile_line: format!("profile: {profile_label}"),
        effective_price_line: variant
            .effective_price
            .as_ref()
            .map(|price| format_effective_price(locale, price)),
        price_table: format_variant_prices(locale, variant.prices.as_slice(), price_list_options),
    }
}

pub(crate) fn format_variant_price_editor_title(
    locale: Option<&str>,
    price: &PricingPrice,
) -> String {
    format!(
        "{} ({})",
        t(locale, "pricing.edit.updatePrice", "Update price"),
        format_price_scope(locale, price.min_quantity, price.max_quantity)
    )
}

pub(crate) fn default_variant_price_editor_currency(
    variant: &PricingVariant,
    default_currency: Option<String>,
) -> String {
    default_currency
        .or_else(|| {
            variant
                .prices
                .first()
                .map(|price| price.currency_code.clone())
        })
        .unwrap_or_default()
}

pub(crate) fn format_variant_count_label(locale: Option<&str>, count: usize) -> String {
    format!("{} {}", count, t(locale, "pricing.common.items", "items"))
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

    fn product_list_item() -> PricingProductListItem {
        PricingProductListItem {
            id: "product-1".to_string(),
            status: "ACTIVE".to_string(),
            seller_id: None,
            title: "Pricing Product".to_string(),
            handle: "pricing-product".to_string(),
            vendor: Some("Acme".to_string()),
            product_type: None,
            shipping_profile_slug: None,
            tags: Vec::new(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            published_at: None,
        }
    }

    fn product_detail() -> PricingProductDetail {
        PricingProductDetail {
            id: "product-1".to_string(),
            status: "ACTIVE".to_string(),
            seller_id: None,
            vendor: Some("Acme".to_string()),
            product_type: None,
            shipping_profile_slug: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-02T00:00:00Z".to_string(),
            published_at: None,
            translations: vec![PricingProductTranslation {
                locale: "en-US".to_string(),
                title: "Translated product".to_string(),
                handle: "translated-product".to_string(),
                description: None,
            }],
            variants: Vec::new(),
        }
    }

    #[test]
    fn product_list_item_view_model_collects_status_profile_and_meta_policy() {
        let view_model = build_product_list_item_view_model(Some("en-US"), &product_list_item());

        assert_eq!(view_model.id, "product-1");
        assert_eq!(view_model.title, "Pricing Product");
        assert_eq!(view_model.status_label, "Active");
        assert!(view_model.status_badge_class.contains("emerald"));
        assert_eq!(view_model.shipping_profile_label, "unassigned");
        assert_eq!(
            view_model.meta_line,
            "handle: pricing-product | vendor: Acme | type: not set | seller: not set"
        );
        assert_eq!(
            pricing_product_list_item_class(true),
            "rounded-2xl border border-primary/40 bg-background p-5 shadow-sm"
        );
        assert_eq!(
            pricing_product_list_item_class(false),
            "rounded-2xl border border-border bg-background p-5 transition hover:border-primary/40"
        );
    }

    #[test]
    fn product_detail_header_view_model_resolves_copy_and_fallbacks() {
        let header =
            build_product_detail_header_view_model(Some("en-US"), Some("en_US"), &product_detail());

        assert_eq!(header.title, "Translated product");
        assert_eq!(header.status_label, "Active");
        assert!(header.status_badge_class.contains("emerald"));
        assert_eq!(
            header.meta_line,
            "handle: translated-product | vendor: Acme | type: not set"
        );
        assert_eq!(header.seller_line, "seller: not set");
        assert_eq!(
            header.shipping_line,
            "shipping profile: unassigned | updated 2026-01-02T00:00:00Z"
        );
        assert_eq!(header.created_line, "created 2026-01-01T00:00:00Z");
        assert_eq!(header.published_line, "published -");
    }

    #[test]
    fn variant_card_view_model_collects_render_policy() {
        let mut variant = variant(vec![price("USD", true)], None);
        variant.sku = Some("sku-1".to_string());
        variant.shipping_profile_slug = Some("fragile".to_string());

        let card = build_variant_card_view_model(Some("en-US"), &variant, &[]);

        assert_eq!(card.title, "Variant");
        assert_eq!(card.health_label, "On sale");
        assert!(card.health_badge_class.contains("amber"));
        assert_eq!(card.identity_line, "sku: sku-1 | barcode: not set");
        assert_eq!(card.profile_line, "profile: fragile");
        assert_eq!(card.effective_price_line, None);
        assert!(card.price_table.contains("USD 10.00"));
    }

    #[test]
    fn variant_editor_copy_and_defaults_are_core_owned() {
        let mut tiered_price = price("USD", false);
        tiered_price.min_quantity = Some(2);
        tiered_price.max_quantity = Some(5);
        let editor_title = format_variant_price_editor_title(Some("en-US"), &tiered_price);

        assert_eq!(editor_title, "Update price (2-5)");
        assert_eq!(format_variant_count_label(Some("en-US"), 3), "3 items");
        assert_eq!(
            default_variant_price_editor_currency(&variant(vec![tiered_price], None), None),
            "USD"
        );
        assert_eq!(
            default_variant_price_editor_currency(
                &variant(vec![price("USD", false)], None),
                Some("EUR".to_string())
            ),
            "EUR"
        );
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
}
