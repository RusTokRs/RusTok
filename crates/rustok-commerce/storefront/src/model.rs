use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StorefrontCommerceData {
    pub products: ProductList,
    pub selected_product: Option<ProductDetail>,
    pub selected_handle: Option<String>,
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
    pub tags: Vec<String>,
    #[serde(rename = "publishedAt")]
    pub published_at: Option<String>,
    pub translations: Vec<ProductTranslation>,
    pub variants: Vec<ProductVariant>,
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
