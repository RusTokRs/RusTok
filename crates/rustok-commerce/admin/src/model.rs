use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CurrentTenant {
    pub id: String,
    pub slug: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CurrentUser {
    pub id: String,
    pub email: String,
    pub name: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CommerceAdminBootstrap {
    #[serde(rename = "currentTenant")]
    pub current_tenant: CurrentTenant,
    pub me: CurrentUser,
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
pub struct ProductDetail {
    pub id: String,
    pub status: String,
    pub vendor: Option<String>,
    #[serde(rename = "productType")]
    pub product_type: Option<String>,
    #[serde(rename = "shippingProfileSlug")]
    pub shipping_profile_slug: Option<String>,
    pub tags: Vec<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    #[serde(rename = "publishedAt")]
    pub published_at: Option<String>,
    pub translations: Vec<ProductTranslation>,
    pub options: Vec<ProductOption>,
    pub variants: Vec<ProductVariant>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductTranslation {
    pub locale: String,
    pub title: String,
    pub handle: String,
    pub description: Option<String>,
    #[serde(rename = "metaTitle")]
    pub meta_title: Option<String>,
    #[serde(rename = "metaDescription")]
    pub meta_description: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductOption {
    pub id: String,
    pub name: String,
    pub values: Vec<String>,
    pub position: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductVariant {
    pub id: String,
    pub sku: Option<String>,
    pub barcode: Option<String>,
    #[serde(rename = "shippingProfileSlug")]
    pub shipping_profile_slug: Option<String>,
    pub title: String,
    pub option1: Option<String>,
    pub option2: Option<String>,
    pub option3: Option<String>,
    pub prices: Vec<ProductPrice>,
    #[serde(rename = "inventoryQuantity")]
    pub inventory_quantity: i32,
    #[serde(rename = "inventoryPolicy")]
    pub inventory_policy: String,
    #[serde(rename = "inStock")]
    pub in_stock: bool,
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

#[derive(Clone, Debug)]
pub struct ProductDraft {
    pub locale: String,
    pub title: String,
    pub handle: String,
    pub description: String,
    pub vendor: String,
    pub product_type: String,
    pub shipping_profile_slug: Option<String>,
    pub sku: String,
    pub barcode: String,
    pub currency_code: String,
    pub amount: String,
    pub compare_at_amount: String,
    pub inventory_quantity: i32,
    pub publish_now: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ShippingOptionList {
    pub items: Vec<ShippingOption>,
    pub total: u64,
    pub page: u64,
    #[serde(rename = "perPage")]
    pub per_page: u64,
    #[serde(rename = "hasNext")]
    pub has_next: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ShippingOption {
    pub id: String,
    #[serde(rename = "tenantId")]
    pub tenant_id: String,
    pub name: String,
    #[serde(rename = "currencyCode")]
    pub currency_code: String,
    pub amount: String,
    #[serde(rename = "providerId")]
    pub provider_id: String,
    pub active: bool,
    #[serde(rename = "allowedShippingProfileSlugs")]
    pub allowed_shipping_profile_slugs: Option<Vec<String>>,
    pub metadata: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Clone, Debug)]
pub struct ShippingOptionDraft {
    pub name: String,
    pub currency_code: String,
    pub amount: String,
    pub provider_id: String,
    pub allowed_shipping_profile_slugs: Vec<String>,
    pub metadata_json: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ShippingProfileList {
    pub items: Vec<ShippingProfile>,
    pub total: u64,
    pub page: u64,
    #[serde(rename = "perPage")]
    pub per_page: u64,
    #[serde(rename = "hasNext")]
    pub has_next: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ShippingProfile {
    pub id: String,
    #[serde(rename = "tenantId")]
    pub tenant_id: String,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub active: bool,
    pub metadata: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Clone, Debug)]
pub struct ShippingProfileDraft {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub metadata_json: String,
}
