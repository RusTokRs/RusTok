use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CurrentTenant {
    pub id: String,
    pub slug: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PricingChannelOption {
    pub id: String,
    pub slug: String,
    pub name: String,
    #[serde(rename = "isActive")]
    pub is_active: bool,
    #[serde(rename = "isDefault")]
    pub is_default: bool,
    pub status: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PricingPriceListOption {
    pub id: String,
    pub name: String,
    #[serde(rename = "listType")]
    pub list_type: String,
    #[serde(rename = "channelId", default)]
    pub channel_id: Option<String>,
    #[serde(rename = "channelSlug", default)]
    pub channel_slug: Option<String>,
    #[serde(rename = "ruleKind", default)]
    pub rule_kind: Option<String>,
    #[serde(rename = "adjustmentPercent", default)]
    pub adjustment_percent: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PricingAdminBootstrap {
    #[serde(rename = "currentTenant")]
    pub current_tenant: CurrentTenant,
    #[serde(rename = "availableChannels", default)]
    pub available_channels: Vec<PricingChannelOption>,
    #[serde(rename = "activePriceLists", default)]
    pub active_price_lists: Vec<PricingPriceListOption>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PricingProductList {
    pub items: Vec<PricingProductListItem>,
    pub total: u64,
    pub page: u64,
    #[serde(rename = "perPage")]
    pub per_page: u64,
    #[serde(rename = "hasNext")]
    pub has_next: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PricingProductListItem {
    pub id: String,
    pub status: String,
    #[serde(rename = "sellerId")]
    pub seller_id: Option<String>,
    pub title: String,
    pub handle: String,
    pub vendor: Option<String>,
    #[serde(rename = "productType")]
    pub product_type: Option<String>,
    #[serde(rename = "shippingProfileSlug")]
    pub shipping_profile_slug: Option<String>,
    pub tags: Vec<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "publishedAt")]
    pub published_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PricingProductDetail {
    pub id: String,
    pub status: String,
    #[serde(rename = "sellerId")]
    pub seller_id: Option<String>,
    pub vendor: Option<String>,
    #[serde(rename = "productType")]
    pub product_type: Option<String>,
    #[serde(rename = "shippingProfileSlug")]
    pub shipping_profile_slug: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    #[serde(rename = "publishedAt")]
    pub published_at: Option<String>,
    pub translations: Vec<PricingProductTranslation>,
    pub variants: Vec<PricingVariant>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PricingProductTranslation {
    pub locale: String,
    pub title: String,
    pub handle: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PricingVariant {
    pub id: String,
    pub sku: Option<String>,
    pub barcode: Option<String>,
    #[serde(rename = "shippingProfileSlug")]
    pub shipping_profile_slug: Option<String>,
    pub title: String,
    pub option1: Option<String>,
    pub option2: Option<String>,
    pub option3: Option<String>,
    pub prices: Vec<PricingPrice>,
    #[serde(rename = "effectivePrice")]
    pub effective_price: Option<PricingEffectivePrice>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PricingPrice {
    #[serde(rename = "currencyCode")]
    pub currency_code: String,
    pub amount: String,
    #[serde(rename = "compareAtAmount")]
    pub compare_at_amount: Option<String>,
    #[serde(rename = "discountPercent", default)]
    pub discount_percent: Option<String>,
    #[serde(rename = "onSale")]
    pub on_sale: bool,
    #[serde(rename = "priceListId", default)]
    pub price_list_id: Option<String>,
    #[serde(rename = "channelId", default)]
    pub channel_id: Option<String>,
    #[serde(rename = "channelSlug", default)]
    pub channel_slug: Option<String>,
    #[serde(rename = "minQuantity", default)]
    pub min_quantity: Option<i32>,
    #[serde(rename = "maxQuantity", default)]
    pub max_quantity: Option<i32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PricingPriceDraft {
    #[serde(rename = "currencyCode")]
    pub currency_code: String,
    pub amount: String,
    #[serde(rename = "compareAtAmount")]
    pub compare_at_amount: String,
    #[serde(rename = "priceListId")]
    pub price_list_id: String,
    #[serde(rename = "channelId")]
    pub channel_id: String,
    #[serde(rename = "channelSlug")]
    pub channel_slug: String,
    #[serde(rename = "minQuantity")]
    pub min_quantity: String,
    #[serde(rename = "maxQuantity")]
    pub max_quantity: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PricingDiscountDraft {
    #[serde(rename = "currencyCode")]
    pub currency_code: String,
    #[serde(rename = "discountPercent")]
    pub discount_percent: String,
    #[serde(rename = "priceListId")]
    pub price_list_id: String,
    #[serde(rename = "channelId")]
    pub channel_id: String,
    #[serde(rename = "channelSlug")]
    pub channel_slug: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PricingPriceListRuleDraft {
    #[serde(rename = "adjustmentPercent")]
    pub adjustment_percent: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PricingPriceListScopeDraft {
    #[serde(rename = "channelId")]
    pub channel_id: String,
    #[serde(rename = "channelSlug")]
    pub channel_slug: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PricingAdjustmentPreview {
    pub kind: String,
    #[serde(rename = "currencyCode")]
    pub currency_code: String,
    #[serde(rename = "currentAmount")]
    pub current_amount: String,
    #[serde(rename = "baseAmount")]
    pub base_amount: String,
    #[serde(rename = "adjustmentPercent")]
    pub adjustment_percent: String,
    #[serde(rename = "adjustedAmount")]
    pub adjusted_amount: String,
    #[serde(rename = "compareAtAmount")]
    pub compare_at_amount: Option<String>,
    #[serde(rename = "priceListId", default)]
    pub price_list_id: Option<String>,
    #[serde(rename = "channelId", default)]
    pub channel_id: Option<String>,
    #[serde(rename = "channelSlug", default)]
    pub channel_slug: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PricingResolutionContext {
    #[serde(rename = "currencyCode")]
    pub currency_code: String,
    #[serde(rename = "regionId")]
    pub region_id: Option<String>,
    #[serde(rename = "priceListId")]
    pub price_list_id: Option<String>,
    #[serde(rename = "channelId", default)]
    pub channel_id: Option<String>,
    #[serde(rename = "channelSlug", default)]
    pub channel_slug: Option<String>,
    pub quantity: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PricingEffectivePrice {
    #[serde(rename = "currencyCode")]
    pub currency_code: String,
    pub amount: String,
    #[serde(rename = "compareAtAmount")]
    pub compare_at_amount: Option<String>,
    #[serde(rename = "discountPercent", default)]
    pub discount_percent: Option<String>,
    #[serde(rename = "onSale")]
    pub on_sale: bool,
    #[serde(rename = "regionId")]
    pub region_id: Option<String>,
    #[serde(rename = "priceListId")]
    pub price_list_id: Option<String>,
    #[serde(rename = "channelId", default)]
    pub channel_id: Option<String>,
    #[serde(rename = "channelSlug", default)]
    pub channel_slug: Option<String>,
    #[serde(rename = "minQuantity")]
    pub min_quantity: Option<i32>,
    #[serde(rename = "maxQuantity")]
    pub max_quantity: Option<i32>,
}
