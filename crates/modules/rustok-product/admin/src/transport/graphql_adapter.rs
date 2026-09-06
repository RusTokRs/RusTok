#![allow(dead_code)]

#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use rustok_graphql::{GraphqlHttpError, GraphqlRequest, execute as execute_graphql};
use rustok_ui_core::normalize_ui_text as optional_text;
use serde::{Deserialize, Serialize};

use crate::model::{
    BindCategoryAttributeDraft, BindSchemaAttributeDraft, CatalogCategoryDraft,
    CatalogCategoryList, CategoryAttributeGroupDraft, ProductAdminBootstrap, ProductAttributeDraft,
    ProductAttributeList, ProductAttributeOptionDraft, ProductAttributeSchemaDraft,
    ProductAttributeSchemaGroupDraft, ProductAttributeSchemaList, ProductAttributeValueItem,
    ProductAttributeValuePatchDraft, ProductDetail, ProductDraft, ProductEffectiveForm,
    ProductList, ProductPricingDetail, SetCategorySchemaModeDraft, ShippingProfileList,
};

pub type ApiError = GraphqlHttpError;

const BOOTSTRAP_QUERY: &str =
    "query ProductAdminBootstrap { currentTenant { id slug name } me { id email name } }";
const PRODUCTS_QUERY: &str = "query ProductAdminProducts($tenantId: UUID!, $locale: String, $filter: ProductsFilter) { products(tenantId: $tenantId, locale: $locale, filter: $filter) { total page perPage hasNext items { id status title handle sellerId vendor productType shippingProfileSlug tags createdAt publishedAt } } }";
const PRODUCT_QUERY: &str = "query ProductAdminProduct($tenantId: UUID!, $id: UUID!, $locale: String) { product(tenantId: $tenantId, id: $id, locale: $locale) { id status sellerId vendor productType shippingProfileSlug primaryCategoryId tags createdAt updatedAt publishedAt translations { locale title handle description metaTitle metaDescription } variants { id sku barcode shippingProfileSlug title option1 option2 option3 inventoryQuantity inventoryPolicy inStock prices { currencyCode amount compareAtAmount onSale } } options { id name values position } } }";
const PRODUCT_PRICING_QUERY: &str = "query ProductAdminPricingProduct($tenantId: UUID!, $id: UUID!, $locale: String, $currencyCode: String, $quantity: Int) { adminPricingProduct(tenantId: $tenantId, id: $id, locale: $locale, currencyCode: $currencyCode, quantity: $quantity) { variants { id prices { currencyCode amount compareAtAmount discountPercent onSale } effectivePrice { currencyCode amount compareAtAmount discountPercent onSale priceListId channelId channelSlug } } } }";
const SHIPPING_PROFILES_QUERY: &str = "query ProductAdminShippingProfiles($tenantId: UUID!, $filter: ShippingProfilesFilter) { shippingProfiles(tenantId: $tenantId, filter: $filter) { total page perPage hasNext items { id tenantId slug name description active metadata createdAt updatedAt } } }";
const PRODUCT_ATTRIBUTES_QUERY: &str = "query ProductAdminAttributes($tenantId: UUID!, $locale: String!) { productAttributes(tenantId: $tenantId, locale: $locale) { total items { id code valueType isLocalized isFilterable isSearchable isSortable showOnStorefront label } } }";
const CATALOG_CATEGORIES_QUERY: &str = "query ProductAdminCatalogCategories($tenantId: UUID!, $locale: String!) { catalogCategories(tenantId: $tenantId, locale: $locale) { total items { id parentId code slug path kind name } } }";
const ATTRIBUTE_SCHEMAS_QUERY: &str = "query ProductAdminAttributeSchemas($tenantId: UUID!, $locale: String!) { productAttributeSchemas(tenantId: $tenantId, locale: $locale) { total items { id code name } } }";
const EFFECTIVE_FORM_QUERY: &str = "query ProductAdminEffectiveForm($tenantId: UUID!, $productId: UUID, $categoryId: UUID, $locale: String!) { productEffectiveForm(tenantId: $tenantId, productId: $productId, categoryId: $categoryId, locale: $locale) { categoryId detachedAttributeIds attributes { attributeId code label valueType isLocalized options { id code label position } groupCode groupLabel isRequired isDisabled position source } } }";
const ATTRIBUTE_VALUES_QUERY: &str = "query ProductAdminAttributeValues($tenantId: UUID!, $productId: UUID!, $locale: String!) { productAttributeValues(tenantId: $tenantId, productId: $productId, locale: $locale) { attributeId kind text integer decimal boolean date datetime optionId optionIds json detached } }";
const CREATE_PRODUCT_MUTATION: &str = "mutation ProductAdminCreateProduct($input: CreateProductInput!) { createProduct(input: $input) { id status sellerId vendor productType shippingProfileSlug primaryCategoryId tags createdAt updatedAt publishedAt translations { locale title handle description metaTitle metaDescription } variants { id sku barcode shippingProfileSlug title option1 option2 option3 inventoryQuantity inventoryPolicy inStock prices { currencyCode amount compareAtAmount onSale } } options { id name values position } } }";
const UPDATE_PRODUCT_MUTATION: &str = "mutation ProductAdminUpdateProduct($id: UUID!, $input: UpdateProductInput!) { updateProduct(id: $id, input: $input) { id status sellerId vendor productType shippingProfileSlug primaryCategoryId tags createdAt updatedAt publishedAt translations { locale title handle description metaTitle metaDescription } variants { id sku barcode shippingProfileSlug title option1 option2 option3 inventoryQuantity inventoryPolicy inStock prices { currencyCode amount compareAtAmount onSale } } options { id name values position } } }";
const DELETE_PRODUCT_MUTATION: &str =
    "mutation ProductAdminDeleteProduct($id: UUID!) { deleteProduct(id: $id) }";
const CREATE_PRODUCT_ATTRIBUTE_MUTATION: &str = "mutation ProductAdminCreateAttribute($locale: String!, $input: CreateProductAttributeInput!) { createProductAttribute(locale: $locale, input: $input) }";
const CREATE_PRODUCT_ATTRIBUTE_OPTION_MUTATION: &str = "mutation ProductAdminCreateAttributeOption($locale: String!, $input: CreateProductAttributeOptionInput!) { createProductAttributeOption(locale: $locale, input: $input) }";
const CREATE_CATALOG_CATEGORY_MUTATION: &str = "mutation ProductAdminCreateCatalogCategory($locale: String!, $input: CreateCatalogCategoryInput!) { createCatalogCategory(locale: $locale, input: $input) }";
const CREATE_ATTRIBUTE_SCHEMA_MUTATION: &str = "mutation ProductAdminCreateAttributeSchema($locale: String!, $input: CreateProductAttributeSchemaInput!) { createProductAttributeSchema(locale: $locale, input: $input) }";
const CREATE_SCHEMA_GROUP_MUTATION: &str = "mutation ProductAdminCreateSchemaGroup($locale: String!, $input: CreateProductAttributeSchemaGroupInput!) { createProductAttributeSchemaGroup(locale: $locale, input: $input) }";
const CREATE_CATEGORY_GROUP_MUTATION: &str = "mutation ProductAdminCreateCategoryGroup($locale: String!, $input: CreateCategoryAttributeGroupInput!) { createCatalogCategoryAttributeGroup(locale: $locale, input: $input) }";
const SET_CATEGORY_SCHEMA_MODE_MUTATION: &str = "mutation ProductAdminSetCategorySchemaMode($input: SetCategorySchemaModeInput!) { setCatalogCategorySchemaMode(input: $input) }";
const BIND_SCHEMA_ATTRIBUTE_MUTATION: &str = "mutation ProductAdminBindSchemaAttribute($input: BindSchemaAttributeInput!) { bindProductAttributeSchemaAttribute(input: $input) }";
const BIND_CATEGORY_ATTRIBUTE_MUTATION: &str = "mutation ProductAdminBindCategoryAttribute($input: BindCategoryAttributeInput!) { bindCatalogCategoryAttribute(input: $input) }";
const SAVE_ATTRIBUTE_VALUES_MUTATION: &str = "mutation ProductAdminSaveAttributeValues($productId: UUID!, $locale: String!, $patches: [ProductAttributeValuePatchInput!]!) { saveProductAttributeValues(productId: $productId, locale: $locale, patches: $patches) { attributeId kind text integer decimal boolean date datetime optionId optionIds json detached } }";
const CLEAR_DETACHED_ATTRIBUTE_VALUES_MUTATION: &str = "mutation ProductAdminClearDetachedAttributeValues($productId: UUID!, $locale: String!, $attributeIds: [UUID!]!) { clearDetachedProductAttributeValues(productId: $productId, locale: $locale, attributeIds: $attributeIds) { attributeId kind text integer decimal boolean date datetime optionId optionIds json detached } }";

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

#[derive(Debug, Deserialize)]
struct SaveAttributeValuesResponse {
    #[serde(rename = "saveProductAttributeValues")]
    save_product_attribute_values: Vec<ProductAttributeValueItem>,
}

#[derive(Debug, Deserialize)]
struct ClearDetachedAttributeValuesResponse {
    #[serde(rename = "clearDetachedProductAttributeValues")]
    clear_detached_product_attribute_values: Vec<ProductAttributeValueItem>,
}

#[derive(Debug, Deserialize)]
struct CreateProductResponse {
    #[serde(rename = "createProduct")]
    create_product: ProductDetail,
}

#[derive(Debug, Deserialize)]
struct UpdateProductResponse {
    #[serde(rename = "updateProduct")]
    update_product: ProductDetail,
}

#[derive(Debug, Deserialize)]
struct DeleteProductResponse {
    #[serde(rename = "deleteProduct")]
    delete_product: bool,
}

#[derive(Debug, Deserialize)]
struct BoolMutationResponse {
    #[serde(rename = "createProductAttribute")]
    create_product_attribute: Option<bool>,
    #[serde(rename = "createProductAttributeOption")]
    create_product_attribute_option: Option<bool>,
    #[serde(rename = "createCatalogCategory")]
    create_catalog_category: Option<bool>,
    #[serde(rename = "createProductAttributeSchema")]
    create_product_attribute_schema: Option<bool>,
    #[serde(rename = "createProductAttributeSchemaGroup")]
    create_product_attribute_schema_group: Option<bool>,
    #[serde(rename = "createCatalogCategoryAttributeGroup")]
    create_catalog_category_attribute_group: Option<bool>,
    #[serde(rename = "setCatalogCategorySchemaMode")]
    set_catalog_category_schema_mode: Option<bool>,
    #[serde(rename = "bindProductAttributeSchemaAttribute")]
    bind_product_attribute_schema_attribute: Option<bool>,
    #[serde(rename = "bindCatalogCategoryAttribute")]
    bind_catalog_category_attribute: Option<bool>,
}

#[derive(Debug, Serialize)]
struct TenantScopedVariables<T> {
    #[serde(rename = "tenantId")]
    tenant_id: String,
    #[serde(flatten)]
    extra: T,
}

#[derive(Debug, Serialize)]
struct TenantUserScopedVariables<T> {
    #[serde(rename = "tenantId")]
    tenant_id: String,
    #[serde(rename = "userId")]
    user_id: String,
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
struct SaveAttributeValuesVariables {
    #[serde(rename = "productId")]
    product_id: String,
    locale: String,
    patches: Vec<ProductAttributeValuePatchDraft>,
}

#[derive(Serialize)]
struct ClearDetachedAttributeValuesVariables {
    #[serde(rename = "productId")]
    product_id: String,
    locale: String,
    #[serde(rename = "attributeIds")]
    attribute_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ProductIdVariables {
    id: String,
}

#[derive(Debug, Serialize)]
struct CreateProductVariables {
    input: CreateProductInput,
}

#[derive(Debug, Serialize)]
struct LocaleMutationVariables<T> {
    locale: String,
    input: T,
}

#[derive(Debug, Serialize)]
struct InputVariables<T> {
    input: T,
}

#[derive(Debug, Serialize)]
struct UpdateProductVariables {
    id: String,
    input: UpdateProductInput,
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

#[derive(Debug, Serialize)]
struct CreateProductInput {
    translations: Vec<ProductTranslationInput>,
    options: Vec<ProductOptionInput>,
    variants: Vec<CreateVariantInput>,
    #[serde(rename = "sellerId")]
    seller_id: Option<String>,
    vendor: Option<String>,
    #[serde(rename = "productType")]
    product_type: Option<String>,
    #[serde(rename = "shippingProfileSlug")]
    shipping_profile_slug: Option<String>,
    #[serde(rename = "primaryCategoryId")]
    primary_category_id: Option<String>,
    publish: Option<bool>,
}

#[derive(Debug, Serialize)]
struct UpdateProductInput {
    translations: Option<Vec<ProductTranslationInput>>,
    #[serde(rename = "sellerId")]
    seller_id: Option<String>,
    vendor: Option<String>,
    #[serde(rename = "productType")]
    product_type: Option<String>,
    #[serde(rename = "shippingProfileSlug")]
    shipping_profile_slug: Option<String>,
    #[serde(rename = "primaryCategoryId")]
    primary_category_id: Option<String>,
    status: Option<String>,
}

#[derive(Debug, Serialize)]
struct ProductTranslationInput {
    locale: String,
    title: String,
    handle: Option<String>,
    description: Option<String>,
    #[serde(rename = "metaTitle")]
    meta_title: Option<String>,
    #[serde(rename = "metaDescription")]
    meta_description: Option<String>,
}

#[derive(Debug, Serialize)]
struct ProductOptionInput {
    translations: Vec<ProductOptionTranslationInput>,
}

#[derive(Debug, Serialize)]
struct ProductOptionTranslationInput {
    locale: String,
    name: String,
    values: Vec<String>,
}

#[derive(Debug, Serialize)]
struct CreateVariantInput {
    sku: Option<String>,
    barcode: Option<String>,
    #[serde(rename = "shippingProfileSlug")]
    shipping_profile_slug: Option<String>,
    option1: Option<String>,
    option2: Option<String>,
    option3: Option<String>,
    prices: Vec<PriceInput>,
    #[serde(rename = "inventoryQuantity")]
    inventory_quantity: Option<i32>,
    #[serde(rename = "inventoryPolicy")]
    inventory_policy: Option<String>,
}

#[derive(Debug, Serialize)]
struct PriceInput {
    #[serde(rename = "currencyCode")]
    currency_code: String,
    amount: String,
    #[serde(rename = "compareAtAmount")]
    compare_at_amount: Option<String>,
}

fn graphql_url() -> String {
    if let Some(url) = option_env!("RUSTOK_GRAPHQL_URL") {
        return url.to_string();
    }

    #[cfg(target_arch = "wasm32")]
    {
        let origin = web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:5150".to_string());
        format!("{origin}/api/graphql")
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let base =
            std::env::var("RUSTOK_API_URL").unwrap_or_else(|_| "http://localhost:5150".to_string());
        format!("{base}/api/graphql")
    }
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

pub(super) async fn create_product(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    draft: ProductDraft,
) -> Result<ProductDetail, ApiError> {
    let response: CreateProductResponse = request(
        CREATE_PRODUCT_MUTATION,
        Some(TenantUserScopedVariables {
            tenant_id,
            user_id,
            extra: CreateProductVariables {
                input: build_create_product_input(draft),
            },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.create_product)
}

pub(super) async fn create_product_attribute(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    locale: String,
    draft: ProductAttributeDraft,
) -> Result<bool, ApiError> {
    let response: BoolMutationResponse = request(
        CREATE_PRODUCT_ATTRIBUTE_MUTATION,
        Some(TenantUserScopedVariables {
            tenant_id,
            user_id,
            extra: LocaleMutationVariables {
                locale,
                input: draft,
            },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.create_product_attribute.unwrap_or(false))
}

pub(super) async fn create_product_attribute_option(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    locale: String,
    draft: ProductAttributeOptionDraft,
) -> Result<bool, ApiError> {
    let response: BoolMutationResponse = request(
        CREATE_PRODUCT_ATTRIBUTE_OPTION_MUTATION,
        Some(TenantUserScopedVariables {
            tenant_id,
            user_id,
            extra: LocaleMutationVariables {
                locale,
                input: draft,
            },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.create_product_attribute_option.unwrap_or(false))
}

pub(super) async fn create_catalog_category(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    locale: String,
    draft: CatalogCategoryDraft,
) -> Result<bool, ApiError> {
    let response: BoolMutationResponse = request(
        CREATE_CATALOG_CATEGORY_MUTATION,
        Some(TenantUserScopedVariables {
            tenant_id,
            user_id,
            extra: LocaleMutationVariables {
                locale,
                input: draft,
            },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.create_catalog_category.unwrap_or(false))
}

pub(super) async fn create_attribute_schema(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    locale: String,
    draft: ProductAttributeSchemaDraft,
) -> Result<bool, ApiError> {
    let response: BoolMutationResponse = request(
        CREATE_ATTRIBUTE_SCHEMA_MUTATION,
        Some(TenantUserScopedVariables {
            tenant_id,
            user_id,
            extra: LocaleMutationVariables {
                locale,
                input: draft,
            },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.create_product_attribute_schema.unwrap_or(false))
}

pub(super) async fn create_product_attribute_schema_group(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    locale: String,
    draft: ProductAttributeSchemaGroupDraft,
) -> Result<bool, ApiError> {
    let response: BoolMutationResponse = request(
        CREATE_SCHEMA_GROUP_MUTATION,
        Some(TenantUserScopedVariables {
            tenant_id,
            user_id,
            extra: LocaleMutationVariables {
                locale,
                input: draft,
            },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response
        .create_product_attribute_schema_group
        .unwrap_or(false))
}

pub(super) async fn create_category_attribute_group(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    locale: String,
    draft: CategoryAttributeGroupDraft,
) -> Result<bool, ApiError> {
    let response: BoolMutationResponse = request(
        CREATE_CATEGORY_GROUP_MUTATION,
        Some(TenantUserScopedVariables {
            tenant_id,
            user_id,
            extra: LocaleMutationVariables {
                locale,
                input: draft,
            },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response
        .create_catalog_category_attribute_group
        .unwrap_or(false))
}

pub(super) async fn set_category_schema_mode(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    draft: SetCategorySchemaModeDraft,
) -> Result<bool, ApiError> {
    let response: BoolMutationResponse = request(
        SET_CATEGORY_SCHEMA_MODE_MUTATION,
        Some(TenantUserScopedVariables {
            tenant_id,
            user_id,
            extra: InputVariables { input: draft },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.set_catalog_category_schema_mode.unwrap_or(false))
}

pub(super) async fn bind_schema_attribute(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    draft: BindSchemaAttributeDraft,
) -> Result<bool, ApiError> {
    let response: BoolMutationResponse = request(
        BIND_SCHEMA_ATTRIBUTE_MUTATION,
        Some(TenantUserScopedVariables {
            tenant_id,
            user_id,
            extra: InputVariables { input: draft },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response
        .bind_product_attribute_schema_attribute
        .unwrap_or(false))
}

pub(super) async fn bind_category_attribute(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    draft: BindCategoryAttributeDraft,
) -> Result<bool, ApiError> {
    let response: BoolMutationResponse = request(
        BIND_CATEGORY_ATTRIBUTE_MUTATION,
        Some(TenantUserScopedVariables {
            tenant_id,
            user_id,
            extra: InputVariables { input: draft },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.bind_catalog_category_attribute.unwrap_or(false))
}

pub(super) async fn save_product_attribute_values(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    product_id: String,
    locale: String,
    mut patches: Vec<ProductAttributeValuePatchDraft>,
) -> Result<Vec<ProductAttributeValueItem>, ApiError> {
    for patch in &mut patches {
        patch.kind = patch.kind.trim().to_ascii_uppercase();
    }
    let response: SaveAttributeValuesResponse = request(
        SAVE_ATTRIBUTE_VALUES_MUTATION,
        Some(TenantUserScopedVariables {
            tenant_id,
            user_id,
            extra: SaveAttributeValuesVariables {
                product_id,
                locale,
                patches,
            },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.save_product_attribute_values)
}

pub(super) async fn clear_detached_product_attribute_values(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    product_id: String,
    locale: String,
    attribute_ids: Vec<String>,
) -> Result<Vec<ProductAttributeValueItem>, ApiError> {
    let response: ClearDetachedAttributeValuesResponse = request(
        CLEAR_DETACHED_ATTRIBUTE_VALUES_MUTATION,
        Some(TenantUserScopedVariables {
            tenant_id,
            user_id,
            extra: ClearDetachedAttributeValuesVariables {
                product_id,
                locale,
                attribute_ids,
            },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.clear_detached_product_attribute_values)
}

pub(super) async fn update_product(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    id: String,
    draft: ProductDraft,
) -> Result<ProductDetail, ApiError> {
    let response: UpdateProductResponse = request(
        UPDATE_PRODUCT_MUTATION,
        Some(TenantUserScopedVariables {
            tenant_id,
            user_id,
            extra: UpdateProductVariables {
                id,
                input: UpdateProductInput {
                    translations: Some(vec![build_translation_input(&draft)]),
                    seller_id: optional_text(draft.seller_id.as_str()),
                    vendor: optional_text(draft.vendor.as_str()),
                    product_type: optional_text(draft.product_type.as_str()),
                    shipping_profile_slug: draft.shipping_profile_slug.clone(),
                    primary_category_id: draft.primary_category_id.clone(),
                    status: None,
                },
            },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.update_product)
}

pub(super) async fn change_product_status(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    id: String,
    status: &str,
) -> Result<ProductDetail, ApiError> {
    let response: UpdateProductResponse = request(
        UPDATE_PRODUCT_MUTATION,
        Some(TenantUserScopedVariables {
            tenant_id,
            user_id,
            extra: UpdateProductVariables {
                id,
                input: UpdateProductInput {
                    translations: None,
                    seller_id: None,
                    vendor: None,
                    product_type: None,
                    shipping_profile_slug: None,
                    primary_category_id: None,
                    status: Some(status.to_string()),
                },
            },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.update_product)
}

pub(super) async fn delete_product(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    id: String,
) -> Result<bool, ApiError> {
    let response: DeleteProductResponse = request(
        DELETE_PRODUCT_MUTATION,
        Some(TenantUserScopedVariables {
            tenant_id,
            user_id,
            extra: ProductIdVariables { id },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.delete_product)
}

fn build_create_product_input(draft: ProductDraft) -> CreateProductInput {
    CreateProductInput {
        translations: vec![build_translation_input(&draft)],
        options: Vec::new(),
        variants: vec![CreateVariantInput {
            sku: optional_text(draft.sku.as_str()),
            barcode: optional_text(draft.barcode.as_str()),
            shipping_profile_slug: None,
            option1: None,
            option2: None,
            option3: None,
            prices: vec![PriceInput {
                currency_code: if draft.currency_code.trim().is_empty() {
                    "USD".to_string()
                } else {
                    draft.currency_code.trim().to_uppercase()
                },
                amount: if draft.amount.trim().is_empty() {
                    "0.00".to_string()
                } else {
                    draft.amount.trim().to_string()
                },
                compare_at_amount: optional_text(draft.compare_at_amount.as_str()),
            }],
            inventory_quantity: Some(draft.inventory_quantity),
            inventory_policy: Some("deny".to_string()),
        }],
        seller_id: optional_text(draft.seller_id.as_str()),
        vendor: optional_text(draft.vendor.as_str()),
        product_type: optional_text(draft.product_type.as_str()),
        shipping_profile_slug: draft.shipping_profile_slug,
        primary_category_id: draft.primary_category_id,
        publish: Some(draft.publish_now),
    }
}

fn build_translation_input(draft: &ProductDraft) -> ProductTranslationInput {
    ProductTranslationInput {
        locale: draft.locale.clone(),
        title: draft.title.trim().to_string(),
        handle: optional_text(draft.handle.as_str()),
        description: optional_text(draft.description.as_str()),
        meta_title: None,
        meta_description: None,
    }
}
