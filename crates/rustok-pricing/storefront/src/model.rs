use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StorefrontPricingData {
    pub products: PricingProductList,
    pub selected_product: Option<PricingProductDetail>,
    pub selected_handle: Option<String>,
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
    pub vendor: Option<String>,
    #[serde(rename = "productType")]
    pub product_type: Option<String>,
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
    pub title: String,
    pub sku: Option<String>,
    pub prices: Vec<PricingPrice>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PricingPrice {
    #[serde(rename = "currencyCode")]
    pub currency_code: String,
    pub amount: String,
    #[serde(rename = "compareAtAmount")]
    pub compare_at_amount: Option<String>,
    #[serde(rename = "onSale")]
    pub on_sale: bool,
}
