use serde::{Deserialize, Serialize};

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
pub struct StorefrontPricingData {
    pub products: PricingProductList,
    pub selected_product: Option<PricingProductDetail>,
    pub selected_handle: Option<String>,
    pub resolution_context: Option<PricingResolutionContext>,
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
    pub title: String,
    pub handle: String,
    #[serde(rename = "sellerId", default)]
    pub seller_id: Option<String>,
    pub vendor: Option<String>,
    #[serde(rename = "productType")]
    pub product_type: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "publishedAt")]
    pub published_at: Option<String>,
    #[serde(rename = "variantCount")]
    pub variant_count: u64,
    #[serde(rename = "saleVariantCount")]
    pub sale_variant_count: u64,
    pub currencies: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PricingProductDetail {
    pub id: String,
    pub status: String,
    #[serde(rename = "sellerId", default)]
    pub seller_id: Option<String>,
    pub vendor: Option<String>,
    #[serde(rename = "productType")]
    pub product_type: Option<String>,
    #[serde(rename = "publishedAt")]
    pub published_at: Option<String>,
    pub translations: Vec<PricingProductTranslation>,
    pub variants: Vec<PricingVariant>,
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
pub struct PricingProductTranslation {
    pub locale: String,
    pub title: String,
    pub handle: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PricingVariant {
    pub id: String,
    pub title: String,
    pub sku: Option<String>,
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
