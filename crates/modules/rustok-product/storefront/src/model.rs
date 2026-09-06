use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductCatalogSearchOption {
    pub value: String,
    pub label: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ProductCatalogSearchOptions {
    pub category_options: Vec<ProductCatalogSearchOption>,
    pub attribute_options: Vec<ProductCatalogSearchOption>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StorefrontProductsData {
    pub products: ProductList,
    pub selected_product: Option<ProductDetail>,
    pub selected_pricing: Option<ProductPricingDetail>,
    pub selected_handle: Option<String>,
    pub resolution_context: Option<ProductPricingContext>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductList {
    pub items: Vec<ProductListItem>,
    pub total: u64,
    pub page: u64,
    #[serde(rename = "perPage")]
    pub per_page: u64,
    #[serde(rename = "hasNext")]
    pub has_next: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductListItem {
    pub id: String,
    pub status: String,
    pub title: String,
    pub handle: String,
    #[serde(rename = "sellerId", default)]
    pub seller_id: Option<String>,
    pub vendor: Option<String>,
    #[serde(rename = "productType")]
    pub product_type: Option<String>,
    pub tags: Vec<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "publishedAt")]
    pub published_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductDetail {
    pub id: String,
    pub status: String,
    #[serde(rename = "sellerId", default)]
    pub seller_id: Option<String>,
    pub vendor: Option<String>,
    #[serde(rename = "productType")]
    pub product_type: Option<String>,
    pub tags: Vec<String>,
    #[serde(rename = "publishedAt")]
    pub published_at: Option<String>,
    pub translations: Vec<ProductTranslation>,
    pub variants: Vec<ProductVariant>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductPricingContext {
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
pub struct ProductPricingDetail {
    pub variants: Vec<ProductPricingVariant>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductPricingVariant {
    pub id: String,
    pub title: String,
    pub sku: Option<String>,
    pub prices: Vec<ProductScopedPrice>,
    #[serde(rename = "effectivePrice")]
    pub effective_price: Option<ProductEffectivePrice>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductScopedPrice {
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
pub struct ProductEffectivePrice {
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
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductTranslation {
    pub locale: String,
    pub title: String,
    pub handle: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductVariant {
    pub id: String,
    pub title: String,
    pub sku: Option<String>,
    #[serde(rename = "inventoryQuantity")]
    pub inventory_quantity: i32,
    #[serde(rename = "inStock")]
    pub in_stock: bool,
    pub prices: Vec<ProductPrice>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductPrice {
    #[serde(rename = "currencyCode")]
    pub currency_code: String,
    pub amount: String,
    #[serde(rename = "compareAtAmount")]
    pub compare_at_amount: Option<String>,
    #[serde(rename = "onSale")]
    pub on_sale: bool,
}
