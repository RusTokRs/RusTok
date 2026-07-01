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
pub struct ProductAdminBootstrap {
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
    #[serde(rename = "sellerId")]
    pub seller_id: Option<String>,
    pub vendor: Option<String>,
    #[serde(rename = "productType")]
    pub product_type: Option<String>,
    #[serde(rename = "shippingProfileSlug")]
    pub shipping_profile_slug: Option<String>,
    #[serde(rename = "primaryCategoryId")]
    pub primary_category_id: Option<String>,
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
    #[serde(rename = "sellerId")]
    pub seller_id: Option<String>,
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
    #[serde(rename = "effectiveForm", default)]
    pub effective_form: Option<ProductEffectiveForm>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductPricingDetail {
    pub variants: Vec<ProductPricingVariant>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductPricingVariant {
    pub id: String,
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
    pub seller_id: String,
    pub vendor: String,
    pub product_type: String,
    pub shipping_profile_slug: Option<String>,
    pub primary_category_id: Option<String>,
    pub sku: String,
    pub barcode: String,
    pub currency_code: String,
    pub amount: String,
    pub compare_at_amount: String,
    pub inventory_quantity: i32,
    pub publish_now: bool,
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductAttributeList {
    pub items: Vec<ProductAttributeSummary>,
    pub total: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductAttributeSummary {
    pub id: String,
    pub code: String,
    #[serde(rename = "valueType")]
    pub value_type: String,
    #[serde(rename = "isLocalized")]
    pub is_localized: bool,
    #[serde(rename = "isFilterable")]
    pub is_filterable: bool,
    #[serde(rename = "isSearchable")]
    pub is_searchable: bool,
    #[serde(rename = "isSortable")]
    pub is_sortable: bool,
    #[serde(rename = "showOnStorefront")]
    pub show_on_storefront: bool,
    pub label: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CatalogCategoryList {
    pub items: Vec<CatalogCategorySummary>,
    pub total: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CatalogCategorySummary {
    pub id: String,
    pub code: String,
    pub slug: String,
    pub path: String,
    pub kind: String,
    pub name: String,
    #[serde(rename = "parentId")]
    pub parent_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductAttributeSchemaList {
    pub items: Vec<ProductAttributeSchemaSummary>,
    pub total: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductAttributeSchemaSummary {
    pub id: String,
    pub code: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductEffectiveForm {
    #[serde(rename = "categoryId")]
    pub category_id: String,
    pub attributes: Vec<ProductEffectiveFormAttribute>,
    #[serde(rename = "detachedAttributeIds")]
    pub detached_attribute_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductEffectiveFormAttribute {
    #[serde(rename = "attributeId")]
    pub attribute_id: String,
    pub code: String,
    pub label: String,
    #[serde(rename = "valueType")]
    pub value_type: String,
    #[serde(rename = "isLocalized")]
    pub is_localized: bool,
    pub options: Vec<ProductAttributeOptionSummary>,
    #[serde(rename = "groupCode")]
    pub group_code: Option<String>,
    #[serde(rename = "groupLabel")]
    pub group_label: Option<String>,
    #[serde(rename = "isRequired")]
    pub is_required: bool,
    #[serde(rename = "isDisabled")]
    pub is_disabled: bool,
    pub position: i32,
    pub source: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductAttributeOptionSummary {
    pub id: String,
    pub code: String,
    pub label: String,
    pub position: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductAttributeValueItem {
    #[serde(rename = "attributeId")]
    pub attribute_id: String,
    pub kind: String,
    pub text: Option<String>,
    pub integer: Option<i64>,
    pub decimal: Option<String>,
    pub boolean: Option<bool>,
    pub date: Option<String>,
    pub datetime: Option<String>,
    #[serde(rename = "optionId")]
    pub option_id: Option<String>,
    #[serde(rename = "optionIds")]
    pub option_ids: Option<Vec<String>>,
    pub json: Option<serde_json::Value>,
    pub detached: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductAttributeValuePatchDraft {
    #[serde(rename = "attributeId")]
    pub attribute_id: String,
    pub kind: String,
    pub text: Option<String>,
    pub integer: Option<i64>,
    pub decimal: Option<String>,
    pub boolean: Option<bool>,
    pub date: Option<String>,
    pub datetime: Option<String>,
    #[serde(rename = "optionId")]
    pub option_id: Option<String>,
    #[serde(rename = "optionIds")]
    pub option_ids: Option<Vec<String>>,
    pub json: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductAttributeDraft {
    pub code: String,
    #[serde(rename = "valueType")]
    pub value_type: String,
    pub label: String,
    #[serde(rename = "helpText")]
    pub help_text: Option<String>,
    #[serde(rename = "isLocalized")]
    pub is_localized: bool,
    #[serde(rename = "isFilterable")]
    pub is_filterable: bool,
    #[serde(rename = "isSearchable")]
    pub is_searchable: bool,
    #[serde(rename = "isSortable")]
    pub is_sortable: bool,
    #[serde(rename = "showOnStorefront")]
    pub show_on_storefront: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductAttributeOptionDraft {
    #[serde(rename = "attributeId")]
    pub attribute_id: String,
    pub code: String,
    pub label: String,
    pub position: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CatalogCategoryDraft {
    #[serde(rename = "parentId")]
    pub parent_id: Option<String>,
    pub code: String,
    pub slug: String,
    pub kind: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductAttributeSchemaDraft {
    pub code: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProductAttributeSchemaGroupDraft {
    #[serde(rename = "schemaId")]
    pub schema_id: String,
    pub code: String,
    pub label: String,
    pub position: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CategoryAttributeGroupDraft {
    #[serde(rename = "categoryId")]
    pub category_id: String,
    pub code: String,
    pub label: String,
    pub position: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SetCategorySchemaModeDraft {
    #[serde(rename = "categoryId")]
    pub category_id: String,
    pub mode: String,
    #[serde(rename = "schemaId")]
    pub schema_id: Option<String>,
    #[serde(rename = "cloneFromCategoryId")]
    pub clone_from_category_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BindSchemaAttributeDraft {
    #[serde(rename = "schemaId")]
    pub schema_id: String,
    #[serde(rename = "attributeId")]
    pub attribute_id: String,
    #[serde(rename = "groupCode")]
    pub group_code: Option<String>,
    #[serde(rename = "isRequired")]
    pub is_required: bool,
    #[serde(rename = "isDisabled")]
    pub is_disabled: bool,
    pub position: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BindCategoryAttributeDraft {
    #[serde(rename = "categoryId")]
    pub category_id: String,
    #[serde(rename = "attributeId")]
    pub attribute_id: String,
    #[serde(rename = "groupCode")]
    pub group_code: Option<String>,
    #[serde(rename = "bindingKind")]
    pub binding_kind: String,
    #[serde(rename = "isRequired")]
    pub is_required: Option<bool>,
    #[serde(rename = "isDisabled")]
    pub is_disabled: bool,
    pub position: Option<i32>,
}
