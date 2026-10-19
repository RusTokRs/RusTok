use rustok_graphql::{GraphqlHttpError, GraphqlRequest, execute as execute_graphql, graphql_url};
use serde::{Deserialize, Serialize};

use crate::model::{
    CatalogCategoryList, ProductAdminBootstrap, ProductAttributeList, ProductAttributeSchemaList,
    ProductAttributeValueItem, ProductDetail, ProductEffectiveForm, ProductList,
    ProductPricingDetail, ShippingProfileList,
};

pub type ApiError = GraphqlHttpError;

const BOOTSTRAP_QUERY: &str =
    "query ProductAdminBootstrap { currentTenant { id slug name } me { id email name } }";
const PRODUCTS_QUERY: &str = "query ProductAdminProducts($tenantId: UUID!, $locale: String, $filter: ProductsFilter) { products(tenantId: $tenantId, locale: $locale, filter: $filter) { total page perPage hasNext items { id status title handle sellerId vendor productType shippingProfileSlug tags createdAt publishedAt } } }";
const PRODUCT_QUERY: &str = "query ProductAdminProduct($tenantId: UUID!, $id: UUID!, $locale: String) { product(tenantId: $tenantId, id: $id, locale: $locale) { id status sellerId vendor productType shippingProfileSlug primaryCategoryId tags createdAt updatedAt publishedAt revision translations { locale title handle description metaTitle metaDescription } variants { id sku barcode shippingProfileSlug title combinationIdentity axisValues { attributeId optionId code label } inventoryQuantity inventoryPolicy inStock prices { currencyCode amount compareAtAmount onSale } } variantAxes { id attributeId code name position allowedValues { optionId value position } } } }";
const PRODUCT_PRICING_QUERY: &str = "query ProductAdminPricingProduct($tenantId: UUID!, $id: UUID!, $locale: String, $currencyCode: String, $quantity: Int) { adminPricingProduct(tenantId: $tenantId, id: $id, locale: $locale, currencyCode: $currencyCode, quantity: $quantity) { variants { id prices { currencyCode amount compareAtAmount discountPercent onSale } effectivePrice { currencyCode amount compareAtAmount discountPercent onSale priceListId channelId channelSlug } } } }";
const SHIPPING_PROFILES_QUERY: &str = "query ProductAdminShippingProfiles($tenantId: UUID!, $filter: ShippingProfilesFilter) { shippingProfiles(tenantId: $tenantId, filter: $filter) { total page perPage hasNext items { id tenantId slug name description active metadata createdAt updatedAt } } }";
const PRODUCT_ATTRIBUTES_QUERY: &str = "query ProductAdminAttributes($tenantId: UUID!, $locale: String!) { productAttributes(tenantId: $tenantId, locale: $locale) { total items { id code valueType isLocalized isFilterable isSearchable isSortable showOnStorefront label } } }";
const CATALOG_CATEGORIES_QUERY: &str = "query ProductAdminCatalogCategories($tenantId: UUID!, $locale: String!) { catalogCategories(tenantId: $tenantId, locale: $locale) { total items { id parentId code slug path kind name } } }";
const ATTRIBUTE_SCHEMAS_QUERY: &str = "query ProductAdminAttributeSchemas($tenantId: UUID!, $locale: String!) { productAttributeSchemas(tenantId: $tenantId, locale: $locale) { total items { id code name } } }";
const EFFECTIVE_FORM_QUERY: &str = "query ProductAdminEffectiveForm($tenantId: UUID!, $productId: UUID, $categoryId: UUID, $locale: String!) { productEffectiveForm(tenantId: $tenantId, productId: $productId, categoryId: $categoryId, locale: $locale) { categoryId detachedAttributeIds attributes { attributeId code label valueType isLocalized options { id code label position } groupCode groupLabel isRequired isDisabled position source variantAxisPolicy defaultVariantAxis } } }";
const ATTRIBUTE_VALUES_QUERY: &str = "query ProductAdminAttributeValues($tenantId: UUID!, $productId: UUID!, $locale: String!) { productAttributeValues(tenantId: $tenantId, productId: $productId, locale: $locale) { attributeId kind text integer decimal boolean date datetime optionId optionIds json detached } }";

#[derive(Debug, Deserialize)]
struct BootstrapResponse {
    #[serde(rename = "currentTenant")]
    current_tenant: crate::model::CurrentTenant,
    me: crate::model::CurrentUser,
}

#[derive(Debug, Deserialize)]
struct ProductsResponse {
    products: ProductList,
}

#[derive(Debug, Deserialize)]
struct ProductResponse {
    product: Option<ProductDetail>,
}

#[derive(Debug, Deserialize)]
struct ProductPricingResponse {
    #[serde(rename = "adminPricingProduct")]
    product_pricing: Option<ProductPricingDetail>,
}

#[derive(Debug, Deserialize)]
struct ShippingProfilesResponse {
    #[serde(rename = "shippingProfiles")]
    shipping_profiles: ShippingProfileList,
}

#[derive(Debug, Deserialize)]
struct ProductAttributesResponse {
    #[serde(rename = "productAttributes")]
    product_attributes: ProductAttributeList,
}

#[derive(Debug, Deserialize)]
struct CatalogCategoriesResponse {
    #[serde(rename = "catalogCategories")]
    catalog_categories: CatalogCategoryList,
}

#[derive(Debug, Deserialize)]
struct ProductAttributeSchemasResponse {
    #[serde(rename = "productAttributeSchemas")]
    product_attribute_schemas: ProductAttributeSchemaList,
}

#[derive(Debug, Deserialize)]
struct EffectiveFormResponse {
    #[serde(rename = "productEffectiveForm")]
    product_effective_form: Option<ProductEffectiveForm>,
}

#[derive(Debug, Deserialize)]
struct AttributeValuesResponse {
    #[serde(rename = "productAttributeValues")]
    product_attribute_values: Vec<ProductAttributeValueItem>,
}

#[derive(Debug, Serialize)]
struct TenantScopedVariables<T> {
    #[serde(rename = "tenantId")]
    tenant_id: String,
    #[serde(flatten)]
    extra: T,
}

#[derive(Debug, Serialize)]
struct ProductsVariables {
    locale: Option<String>,
    filter: ProductsFilter,
}

#[derive(Debug, Serialize)]
struct ProductVariables {
    id: String,
    locale: Option<String>,
}

#[derive(Debug, Serialize)]
struct ProductPricingVariables {
    id: String,
    locale: Option<String>,
    #[serde(rename = "currencyCode")]
    currency_code: Option<String>,
    quantity: Option<i32>,
}

#[derive(Debug, Serialize)]
struct ShippingProfilesVariables {
    filter: ShippingProfilesFilter,
}

#[derive(Debug, Serialize)]
struct LocaleVariables {
    locale: String,
}

#[derive(Debug, Serialize)]
struct EffectiveFormVariables {
    #[serde(rename = "productId")]
    product_id: Option<String>,
    #[serde(rename = "categoryId")]
    category_id: Option<String>,
    locale: String,
}

#[derive(Debug, Serialize)]
struct AttributeValuesVariables {
    #[serde(rename = "productId")]
    product_id: String,
    locale: String,
}

#[derive(Debug, Serialize)]
struct ProductsFilter {
    status: Option<String>,
    vendor: Option<String>,
    search: Option<String>,
    page: Option<u64>,
    #[serde(rename = "perPage")]
    per_page: Option<u64>,
}

#[derive(Debug, Serialize)]
struct ShippingProfilesFilter {
    active: Option<bool>,
    search: Option<String>,
    page: Option<u64>,
    #[serde(rename = "perPage")]
    per_page: Option<u64>,
}

async fn request<V, T>(
    query: &str,
    variables: Option<V>,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<T, ApiError>
where
    V: Serialize,
    T: for<'de> Deserialize<'de>,
{
    execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(query, variables),
        token,
        tenant_slug,
        None,
    )
    .await
}

// Schema-authoring writes are owned by `product_schema_graphql`, which forwards the caller
// idempotency key minted once per logical invocation by `transport`. The same key reaches the
// native owner call, so the native-first fallbacks in `transport` share one owner receipt.
pub(super) use crate::product_schema_graphql::{
    bind_category_attribute, bind_schema_attribute, clear_detached_product_attribute_values,
    create_attribute_schema, create_catalog_category, create_category_attribute_group,
    create_product_attribute, create_product_attribute_option,
    create_product_attribute_schema_group, save_product_attribute_values, set_category_schema_mode,
};

pub(super) async fn fetch_bootstrap(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<ProductAdminBootstrap, ApiError> {
    let response: BootstrapResponse =
        request::<serde_json::Value, BootstrapResponse>(BOOTSTRAP_QUERY, None, token, tenant_slug)
            .await?;
    Ok(ProductAdminBootstrap {
        current_tenant: response.current_tenant,
        me: response.me,
    })
}

pub(super) async fn fetch_products(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    locale: Option<String>,
    search: Option<String>,
    status: Option<String>,
) -> Result<ProductList, ApiError> {
    let response: ProductsResponse = request(
        PRODUCTS_QUERY,
        Some(TenantScopedVariables {
            tenant_id,
            extra: ProductsVariables {
                locale,
                filter: ProductsFilter {
                    status,
                    vendor: None,
                    search,
                    page: Some(1),
                    per_page: Some(24),
                },
            },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.products)
}

pub(super) async fn fetch_product(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
    locale: Option<String>,
) -> Result<Option<ProductDetail>, ApiError> {
    let response: ProductResponse = request(
        PRODUCT_QUERY,
        Some(TenantScopedVariables {
            tenant_id,
            extra: ProductVariables { id, locale },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.product)
}

pub(super) async fn fetch_product_pricing(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
    locale: Option<String>,
    currency_code: Option<String>,
) -> Result<Option<ProductPricingDetail>, ApiError> {
    let response: ProductPricingResponse = request(
        PRODUCT_PRICING_QUERY,
        Some(TenantScopedVariables {
            tenant_id,
            extra: ProductPricingVariables {
                id,
                locale,
                currency_code,
                quantity: Some(1),
            },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.product_pricing)
}

pub(super) async fn fetch_shipping_profiles(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
) -> Result<ShippingProfileList, ApiError> {
    let response: ShippingProfilesResponse = request(
        SHIPPING_PROFILES_QUERY,
        Some(TenantScopedVariables {
            tenant_id,
            extra: ShippingProfilesVariables {
                filter: ShippingProfilesFilter {
                    active: Some(true),
                    search: None,
                    page: Some(1),
                    per_page: Some(100),
                },
            },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.shipping_profiles)
}

pub(super) async fn fetch_product_attributes(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    locale: String,
) -> Result<ProductAttributeList, ApiError> {
    let response: ProductAttributesResponse = request(
        PRODUCT_ATTRIBUTES_QUERY,
        Some(TenantScopedVariables {
            tenant_id,
            extra: LocaleVariables { locale },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.product_attributes)
}

pub(super) async fn fetch_catalog_categories(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    locale: String,
) -> Result<CatalogCategoryList, ApiError> {
    let response: CatalogCategoriesResponse = request(
        CATALOG_CATEGORIES_QUERY,
        Some(TenantScopedVariables {
            tenant_id,
            extra: LocaleVariables { locale },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.catalog_categories)
}

pub(super) async fn fetch_attribute_schemas(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    locale: String,
) -> Result<ProductAttributeSchemaList, ApiError> {
    let response: ProductAttributeSchemasResponse = request(
        ATTRIBUTE_SCHEMAS_QUERY,
        Some(TenantScopedVariables {
            tenant_id,
            extra: LocaleVariables { locale },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.product_attribute_schemas)
}

pub(super) async fn fetch_effective_product_form(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    product_id: Option<String>,
    category_id: Option<String>,
    locale: String,
) -> Result<Option<ProductEffectiveForm>, ApiError> {
    let response: EffectiveFormResponse = request(
        EFFECTIVE_FORM_QUERY,
        Some(TenantScopedVariables {
            tenant_id,
            extra: EffectiveFormVariables {
                product_id,
                category_id,
                locale,
            },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.product_effective_form)
}

pub(super) async fn fetch_product_attribute_values(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    product_id: String,
    locale: String,
) -> Result<Vec<ProductAttributeValueItem>, ApiError> {
    let response: AttributeValuesResponse = request(
        ATTRIBUTE_VALUES_QUERY,
        Some(TenantScopedVariables {
            tenant_id,
            extra: AttributeValuesVariables { product_id, locale },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.product_attribute_values)
}
