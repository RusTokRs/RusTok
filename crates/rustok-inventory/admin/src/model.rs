use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CurrentTenant {
    pub id: String,
    pub slug: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InventoryAdminBootstrap {
    #[serde(rename = "currentTenant")]
    pub current_tenant: CurrentTenant,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InventoryProductList {
    pub items: Vec<InventoryProductListItem>,
    pub total: u64,
    pub page: u64,
    #[serde(rename = "perPage")]
    pub per_page: u64,
    #[serde(rename = "hasNext")]
    pub has_next: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InventoryProductListItem {
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
pub struct InventoryProductDetail {
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
    pub translations: Vec<InventoryProductTranslation>,
    pub variants: Vec<InventoryVariant>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InventoryProductTranslation {
    pub locale: String,
    pub title: String,
    pub handle: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InventoryVariant {
    pub id: String,
    pub sku: Option<String>,
    pub barcode: Option<String>,
    #[serde(rename = "shippingProfileSlug")]
    pub shipping_profile_slug: Option<String>,
    pub title: String,
    pub option1: Option<String>,
    pub option2: Option<String>,
    pub option3: Option<String>,
    pub prices: Vec<InventoryPrice>,
    #[serde(rename = "inventoryQuantity")]
    pub inventory_quantity: i32,
    #[serde(rename = "inventoryPolicy")]
    pub inventory_policy: String,
    #[serde(rename = "inStock")]
    pub in_stock: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InventoryPrice {
    #[serde(rename = "currencyCode")]
    pub currency_code: String,
    pub amount: String,
    #[serde(rename = "compareAtAmount")]
    pub compare_at_amount: Option<String>,
    #[serde(rename = "onSale")]
    pub on_sale: bool,
}
