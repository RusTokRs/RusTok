use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CurrentTenant {
    pub id: String,
    pub slug: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PricingAdminBootstrap {
    #[serde(rename = "currentTenant")]
    pub current_tenant: CurrentTenant,
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
