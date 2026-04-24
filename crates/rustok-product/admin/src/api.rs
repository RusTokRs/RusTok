#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use leptos_graphql::{execute as execute_graphql, GraphqlHttpError, GraphqlRequest};
use serde::{Deserialize, Serialize};

use crate::model::{
    ProductAdminBootstrap, ProductDetail, ProductDraft, ProductList, ProductPricingDetail,
    ShippingProfileList,
};

pub type ApiError = GraphqlHttpError;

const BOOTSTRAP_QUERY: &str =
    "query ProductAdminBootstrap { currentTenant { id slug name } me { id email name } }";
const PRODUCTS_QUERY: &str = "query ProductAdminProducts($tenantId: UUID!, $locale: String, $filter: ProductsFilter) { products(tenantId: $tenantId, locale: $locale, filter: $filter) { total page perPage hasNext items { id status title handle sellerId vendor productType shippingProfileSlug tags createdAt publishedAt } } }";
const PRODUCT_QUERY: &str = "query ProductAdminProduct($tenantId: UUID!, $id: UUID!, $locale: String) { product(tenantId: $tenantId, id: $id, locale: $locale) { id status sellerId vendor productType shippingProfileSlug tags createdAt updatedAt publishedAt translations { locale title handle description metaTitle metaDescription } variants { id sku barcode shippingProfileSlug title option1 option2 option3 inventoryQuantity inventoryPolicy inStock prices { currencyCode amount compareAtAmount onSale } } options { id name values position } } }";
const PRODUCT_PRICING_QUERY: &str = "query ProductAdminPricingProduct($tenantId: UUID!, $id: UUID!, $locale: String, $currencyCode: String, $quantity: Int) { adminPricingProduct(tenantId: $tenantId, id: $id, locale: $locale, currencyCode: $currencyCode, quantity: $quantity) { variants { id prices { currencyCode amount compareAtAmount discountPercent onSale } effectivePrice { currencyCode amount compareAtAmount discountPercent onSale priceListId channelId channelSlug } } } }";
const SHIPPING_PROFILES_QUERY: &str = "query ProductAdminShippingProfiles($tenantId: UUID!, $filter: ShippingProfilesFilter) { shippingProfiles(tenantId: $tenantId, filter: $filter) { total page perPage hasNext items { id tenantId slug name description active metadata createdAt updatedAt } } }";
const CREATE_PRODUCT_MUTATION: &str = "mutation ProductAdminCreateProduct($tenantId: UUID!, $userId: UUID!, $input: CreateProductInput!) { createProduct(tenantId: $tenantId, userId: $userId, input: $input) { id status sellerId vendor productType shippingProfileSlug tags createdAt updatedAt publishedAt translations { locale title handle description metaTitle metaDescription } variants { id sku barcode shippingProfileSlug title option1 option2 option3 inventoryQuantity inventoryPolicy inStock prices { currencyCode amount compareAtAmount onSale } } options { id name values position } } }";
const UPDATE_PRODUCT_MUTATION: &str = "mutation ProductAdminUpdateProduct($tenantId: UUID!, $userId: UUID!, $id: UUID!, $input: UpdateProductInput!) { updateProduct(tenantId: $tenantId, userId: $userId, id: $id, input: $input) { id status sellerId vendor productType shippingProfileSlug tags createdAt updatedAt publishedAt translations { locale title handle description metaTitle metaDescription } variants { id sku barcode shippingProfileSlug title option1 option2 option3 inventoryQuantity inventoryPolicy inStock prices { currencyCode amount compareAtAmount onSale } } options { id name values position } } }";
const DELETE_PRODUCT_MUTATION: &str = "mutation ProductAdminDeleteProduct($tenantId: UUID!, $userId: UUID!, $id: UUID!) { deleteProduct(tenantId: $tenantId, userId: $userId, id: $id) }";

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
struct ProductIdVariables {
    id: String,
}

#[derive(Debug, Serialize)]
struct CreateProductVariables {
    input: CreateProductInput,
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

pub async fn fetch_bootstrap(
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

pub async fn fetch_products(
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

pub async fn fetch_product(
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

pub async fn fetch_product_pricing(
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

pub async fn fetch_shipping_profiles(
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

pub async fn create_product(
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

pub async fn update_product(
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

pub async fn change_product_status(
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

pub async fn delete_product(
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

fn optional_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
