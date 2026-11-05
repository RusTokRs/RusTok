use uuid::Uuid;

use crate::model::{
    CurrentTenant, PricingAdjustmentPreview, PricingChannelOption, PricingEffectivePrice,
    PricingPriceListOption, PricingProductDetail, PricingProductList, PricingProductListItem,
    PricingProductTranslation, PricingResolutionContext, PricingVariant,
};

pub(crate) fn map_adjustment_kind(kind: rustok_pricing::PriceAdjustmentKind) -> String {
    match kind {
        rustok_pricing::PriceAdjustmentKind::PercentageDiscount => {
            "percentage_discount".to_string()
        }
    }
}

#[cfg(feature = "ssr")]
pub(crate) fn map_adjustment_preview(
    value: rustok_pricing::PriceAdjustmentPreview,
) -> PricingAdjustmentPreview {
    PricingAdjustmentPreview {
        kind: map_adjustment_kind(value.kind),
        currency_code: value.currency_code,
        current_amount: value.current_amount.normalize().to_string(),
        base_amount: value.base_amount.normalize().to_string(),
        adjustment_percent: value.adjustment_percent.normalize().to_string(),
        adjusted_amount: value.adjusted_amount.normalize().to_string(),
        compare_at_amount: value
            .compare_at_amount
            .map(|item| item.normalize().to_string()),
        price_list_id: value.price_list_id.map(|item| item.to_string()),
        channel_id: value.channel_id.map(|item| item.to_string()),
        channel_slug: value.channel_slug,
    }
}

#[cfg(feature = "ssr")]

pub(crate) fn map_current_tenant(tenant: &rustok_api::TenantContext) -> CurrentTenant {
    CurrentTenant {
        id: tenant.id.to_string(),
        slug: tenant.slug.clone(),
        name: tenant.name.clone(),
    }
}

#[cfg(feature = "ssr")]
pub(crate) fn map_native_price_list_option(
    value: rustok_pricing::ActivePriceListOption,
) -> PricingPriceListOption {
    PricingPriceListOption {
        id: value.id.to_string(),
        name: value.name,
        list_type: value.list_type,
        channel_id: value.channel_id.map(|item| item.to_string()),
        channel_slug: value.channel_slug,
        rule_kind: value.rule_kind,
        adjustment_percent: value
            .adjustment_percent
            .map(|item| item.normalize().to_string()),
    }
}

#[cfg(feature = "ssr")]
pub(crate) fn map_channel_option(value: rustok_channel::ChannelResponse) -> PricingChannelOption {
    PricingChannelOption {
        id: value.id.to_string(),
        slug: value.slug,
        name: value.name,
        is_active: value.is_active,
        is_default: value.is_default,
        status: value.status,
    }
}

#[cfg(feature = "ssr")]

pub(crate) fn map_native_list_item(
    value: rustok_pricing::AdminPricingProductListItem,
) -> PricingProductListItem {
    PricingProductListItem {
        id: value.id.to_string(),
        status: value.status.to_string(),
        seller_id: value.seller_id,
        title: value.title,
        handle: value.handle,
        vendor: value.vendor,
        product_type: value.product_type,
        shipping_profile_slug: value.shipping_profile_slug,
        tags: value.tags,
        created_at: value.created_at.to_rfc3339(),
        published_at: value.published_at.map(|item| item.to_rfc3339()),
    }
}

#[cfg(feature = "ssr")]
pub(crate) fn map_native_list(value: rustok_pricing::AdminPricingProductList) -> PricingProductList {
    PricingProductList {
        items: value.items.into_iter().map(map_native_list_item).collect(),
        total: value.total,
        page: value.page,
        per_page: value.per_page,
        has_next: value.has_next,
    }
}

#[cfg(feature = "ssr")]
pub(crate) fn map_native_effective_price(value: rustok_pricing::ResolvedPrice) -> PricingEffectivePrice {
    PricingEffectivePrice {
        source: crate::model::PricingPriceSource {
            price_id: value.source.price_id().to_string(),
            kind: value.source.kind().to_string(),
            rule_kind: value
                .source
                .rule()
                .map(|rule| rule.kind.as_str().to_string()),
            adjustment_percent: value
                .source
                .rule()
                .map(|rule| rule.adjustment_percent.normalize().to_string()),
        },
        currency_code: value.currency_code,
        amount: value.amount.normalize().to_string(),
        compare_at_amount: value
            .compare_at_amount
            .map(|item| item.normalize().to_string()),
        discount_percent: value
            .discount_percent
            .map(|item| item.normalize().to_string()),
        on_sale: value.on_sale,
        region_id: value.region_id.map(|item| item.to_string()),
        price_list_id: value.price_list_id.map(|item| item.to_string()),
        channel_id: value.channel_id.map(|item| item.to_string()),
        channel_slug: value.channel_slug,
        min_quantity: value.min_quantity,
        max_quantity: value.max_quantity,
    }
}

#[cfg(feature = "ssr")]
pub(crate) fn map_native_detail(value: rustok_pricing::AdminPricingProductDetail) -> PricingProductDetail {
    PricingProductDetail {
        resolution_context: None,
        resolved_at: None,
        id: value.id.to_string(),
        status: value.status.to_string(),
        seller_id: value.seller_id,
        vendor: value.vendor,
        product_type: value.product_type,
        shipping_profile_slug: value.shipping_profile_slug,
        created_at: value.created_at.to_rfc3339(),
        updated_at: value.updated_at.to_rfc3339(),
        published_at: value.published_at.map(|item| item.to_rfc3339()),
        translations: value
            .translations
            .into_iter()
            .map(|translation| PricingProductTranslation {
                locale: translation.locale,
                title: translation.title,
                handle: translation.handle,
                description: translation.description,
            })
            .collect(),
        variants: value
            .variants
            .into_iter()
            .map(|variant| PricingVariant {
                id: variant.id.to_string(),
                sku: variant.sku,
                barcode: variant.barcode,
                shipping_profile_slug: variant.shipping_profile_slug,
                title: variant.title,
                combination_identity: variant.combination_identity,
                prices: variant
                    .prices
                    .into_iter()
                    .map(|price| crate::model::PricingPrice {
                        currency_code: price.currency_code,
                        amount: price.amount.normalize().to_string(),
                        compare_at_amount: price
                            .compare_at_amount
                            .map(|item| item.normalize().to_string()),
                        discount_percent: price
                            .discount_percent
                            .map(|item| item.normalize().to_string()),
                        on_sale: price.on_sale,
                        price_list_id: price.price_list_id.map(|item| item.to_string()),
                        channel_id: price.channel_id.map(|item| item.to_string()),
                        channel_slug: price.channel_slug,
                        min_quantity: price.min_quantity,
                        max_quantity: price.max_quantity,
                    })
                    .collect(),
                effective_price: None,
            })
            .collect(),
    }
}


#[cfg(feature = "ssr")]
pub(crate) fn map_resolution_context(
    value: rustok_pricing::PriceResolutionContext,
) -> crate::model::PricingResolutionContext {
    crate::model::PricingResolutionContext {
        currency_code: value.currency_code,
        region_id: value.region_id.map(|id| id.to_string()),
        price_list_id: value.price_list_id.map(|id| id.to_string()),
        channel_id: value.channel_id.map(|id| id.to_string()),
        channel_slug: value.channel_slug,
        quantity: value.quantity.unwrap_or(1),
    }
}
