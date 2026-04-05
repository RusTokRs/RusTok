use leptos_graphql::{execute as execute_graphql, GraphqlHttpError, GraphqlRequest};
use serde::{Deserialize, Serialize};

use crate::model::{
    CommerceAdminBootstrap, ProductDetail, ProductDraft, ProductList, ShippingOption,
    ShippingOptionDraft, ShippingOptionList, ShippingProfile, ShippingProfileDraft,
    ShippingProfileList,
};

pub type ApiError = GraphqlHttpError;

const BOOTSTRAP_QUERY: &str =
    "query CommerceAdminBootstrap { currentTenant { id slug name } me { id email name } }";
const PRODUCTS_QUERY: &str = "query CommerceProducts($tenantId: UUID!, $locale: String, $filter: ProductsFilter) { products(tenantId: $tenantId, locale: $locale, filter: $filter) { total page perPage hasNext items { id status title handle vendor productType shippingProfileSlug tags createdAt publishedAt } } }";
const PRODUCT_QUERY: &str = "query CommerceProduct($tenantId: UUID!, $id: UUID!, $locale: String) { product(tenantId: $tenantId, id: $id, locale: $locale) { id status vendor productType shippingProfileSlug tags createdAt updatedAt publishedAt translations { locale title handle description metaTitle metaDescription } options { id name values position } variants { id sku barcode shippingProfileSlug title option1 option2 option3 inventoryQuantity inventoryPolicy inStock prices { currencyCode amount compareAtAmount onSale } } } }";
const SHIPPING_OPTIONS_QUERY: &str = "query CommerceShippingOptions($tenantId: UUID!, $filter: ShippingOptionsFilter) { shippingOptions(tenantId: $tenantId, filter: $filter) { total page perPage hasNext items { id tenantId name currencyCode amount providerId active allowedShippingProfileSlugs metadata createdAt updatedAt } } }";
const SHIPPING_OPTION_QUERY: &str = "query CommerceShippingOption($tenantId: UUID!, $id: UUID!) { shippingOption(tenantId: $tenantId, id: $id) { id tenantId name currencyCode amount providerId active allowedShippingProfileSlugs metadata createdAt updatedAt } }";
const SHIPPING_PROFILES_QUERY: &str = "query CommerceShippingProfiles($tenantId: UUID!, $filter: ShippingProfilesFilter) { shippingProfiles(tenantId: $tenantId, filter: $filter) { total page perPage hasNext items { id tenantId slug name description active metadata createdAt updatedAt } } }";
const SHIPPING_PROFILE_QUERY: &str = "query CommerceShippingProfile($tenantId: UUID!, $id: UUID!) { shippingProfile(tenantId: $tenantId, id: $id) { id tenantId slug name description active metadata createdAt updatedAt } }";
const CREATE_PRODUCT_MUTATION: &str = "mutation CommerceCreateProduct($tenantId: UUID!, $userId: UUID!, $input: CreateProductInput!) { createProduct(tenantId: $tenantId, userId: $userId, input: $input) { id status vendor productType shippingProfileSlug tags createdAt updatedAt publishedAt translations { locale title handle description metaTitle metaDescription } options { id name values position } variants { id sku barcode shippingProfileSlug title option1 option2 option3 inventoryQuantity inventoryPolicy inStock prices { currencyCode amount compareAtAmount onSale } } } }";
const UPDATE_PRODUCT_MUTATION: &str = "mutation CommerceUpdateProduct($tenantId: UUID!, $userId: UUID!, $id: UUID!, $input: UpdateProductInput!) { updateProduct(tenantId: $tenantId, userId: $userId, id: $id, input: $input) { id status vendor productType shippingProfileSlug tags createdAt updatedAt publishedAt translations { locale title handle description metaTitle metaDescription } options { id name values position } variants { id sku barcode shippingProfileSlug title option1 option2 option3 inventoryQuantity inventoryPolicy inStock prices { currencyCode amount compareAtAmount onSale } } } }";
const PUBLISH_PRODUCT_MUTATION: &str = "mutation CommercePublishProduct($tenantId: UUID!, $userId: UUID!, $id: UUID!) { publishProduct(tenantId: $tenantId, userId: $userId, id: $id) { id status vendor productType shippingProfileSlug tags createdAt updatedAt publishedAt translations { locale title handle description metaTitle metaDescription } options { id name values position } variants { id sku barcode shippingProfileSlug title option1 option2 option3 inventoryQuantity inventoryPolicy inStock prices { currencyCode amount compareAtAmount onSale } } } }";
const DELETE_PRODUCT_MUTATION: &str = "mutation CommerceDeleteProduct($tenantId: UUID!, $userId: UUID!, $id: UUID!) { deleteProduct(tenantId: $tenantId, userId: $userId, id: $id) }";
const CREATE_SHIPPING_OPTION_MUTATION: &str = "mutation CommerceCreateShippingOption($tenantId: UUID!, $input: CreateShippingOptionInput!) { createShippingOption(tenantId: $tenantId, input: $input) { id tenantId name currencyCode amount providerId active allowedShippingProfileSlugs metadata createdAt updatedAt } }";
const UPDATE_SHIPPING_OPTION_MUTATION: &str = "mutation CommerceUpdateShippingOption($tenantId: UUID!, $id: UUID!, $input: UpdateShippingOptionInput!) { updateShippingOption(tenantId: $tenantId, id: $id, input: $input) { id tenantId name currencyCode amount providerId active allowedShippingProfileSlugs metadata createdAt updatedAt } }";
const DEACTIVATE_SHIPPING_OPTION_MUTATION: &str = "mutation CommerceDeactivateShippingOption($tenantId: UUID!, $id: UUID!) { deactivateShippingOption(tenantId: $tenantId, id: $id) { id tenantId name currencyCode amount providerId active allowedShippingProfileSlugs metadata createdAt updatedAt } }";
const REACTIVATE_SHIPPING_OPTION_MUTATION: &str = "mutation CommerceReactivateShippingOption($tenantId: UUID!, $id: UUID!) { reactivateShippingOption(tenantId: $tenantId, id: $id) { id tenantId name currencyCode amount providerId active allowedShippingProfileSlugs metadata createdAt updatedAt } }";
const CREATE_SHIPPING_PROFILE_MUTATION: &str = "mutation CommerceCreateShippingProfile($tenantId: UUID!, $input: CreateShippingProfileInput!) { createShippingProfile(tenantId: $tenantId, input: $input) { id tenantId slug name description active metadata createdAt updatedAt } }";
const UPDATE_SHIPPING_PROFILE_MUTATION: &str = "mutation CommerceUpdateShippingProfile($tenantId: UUID!, $id: UUID!, $input: UpdateShippingProfileInput!) { updateShippingProfile(tenantId: $tenantId, id: $id, input: $input) { id tenantId slug name description active metadata createdAt updatedAt } }";
const DEACTIVATE_SHIPPING_PROFILE_MUTATION: &str = "mutation CommerceDeactivateShippingProfile($tenantId: UUID!, $id: UUID!) { deactivateShippingProfile(tenantId: $tenantId, id: $id) { id tenantId slug name description active metadata createdAt updatedAt } }";
const REACTIVATE_SHIPPING_PROFILE_MUTATION: &str = "mutation CommerceReactivateShippingProfile($tenantId: UUID!, $id: UUID!) { reactivateShippingProfile(tenantId: $tenantId, id: $id) { id tenantId slug name description active metadata createdAt updatedAt } }";

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
struct ShippingOptionsResponse {
    #[serde(rename = "shippingOptions")]
    shipping_options: ShippingOptionList,
}

#[derive(Debug, Deserialize)]
struct ShippingOptionResponse {
    #[serde(rename = "shippingOption")]
    shipping_option: Option<ShippingOption>,
}

#[derive(Debug, Deserialize)]
struct ShippingProfilesResponse {
    #[serde(rename = "shippingProfiles")]
    shipping_profiles: ShippingProfileList,
}

#[derive(Debug, Deserialize)]
struct ShippingProfileResponse {
    #[serde(rename = "shippingProfile")]
    shipping_profile: Option<ShippingProfile>,
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
struct PublishProductResponse {
    #[serde(rename = "publishProduct")]
    publish_product: ProductDetail,
}

#[derive(Debug, Deserialize)]
struct DeleteProductResponse {
    #[serde(rename = "deleteProduct")]
    delete_product: bool,
}

#[derive(Debug, Deserialize)]
struct CreateShippingOptionResponse {
    #[serde(rename = "createShippingOption")]
    create_shipping_option: ShippingOption,
}

#[derive(Debug, Deserialize)]
struct UpdateShippingOptionResponse {
    #[serde(rename = "updateShippingOption")]
    update_shipping_option: ShippingOption,
}

#[derive(Debug, Deserialize)]
struct DeactivateShippingOptionResponse {
    #[serde(rename = "deactivateShippingOption")]
    deactivate_shipping_option: ShippingOption,
}

#[derive(Debug, Deserialize)]
struct ReactivateShippingOptionResponse {
    #[serde(rename = "reactivateShippingOption")]
    reactivate_shipping_option: ShippingOption,
}

#[derive(Debug, Deserialize)]
struct CreateShippingProfileResponse {
    #[serde(rename = "createShippingProfile")]
    create_shipping_profile: ShippingProfile,
}

#[derive(Debug, Deserialize)]
struct UpdateShippingProfileResponse {
    #[serde(rename = "updateShippingProfile")]
    update_shipping_profile: ShippingProfile,
}

#[derive(Debug, Deserialize)]
struct DeactivateShippingProfileResponse {
    #[serde(rename = "deactivateShippingProfile")]
    deactivate_shipping_profile: ShippingProfile,
}

#[derive(Debug, Deserialize)]
struct ReactivateShippingProfileResponse {
    #[serde(rename = "reactivateShippingProfile")]
    reactivate_shipping_profile: ShippingProfile,
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
struct ShippingOptionVariables {
    id: String,
}

#[derive(Debug, Serialize)]
struct ShippingProfileVariables {
    id: String,
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
struct CreateShippingOptionVariables {
    input: CreateShippingOptionInput,
}

#[derive(Debug, Serialize)]
struct UpdateShippingOptionVariables {
    id: String,
    input: UpdateShippingOptionInput,
}

#[derive(Debug, Serialize)]
struct ShippingProfilesVariables {
    filter: ShippingProfilesFilter,
}

#[derive(Debug, Serialize)]
struct CreateShippingProfileVariables {
    input: CreateShippingProfileInput,
}

#[derive(Debug, Serialize)]
struct UpdateShippingProfileVariables {
    id: String,
    input: UpdateShippingProfileInput,
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
struct ShippingOptionsVariables {
    filter: ShippingOptionsFilter,
}

#[derive(Debug, Serialize)]
struct ShippingOptionsFilter {
    active: Option<bool>,
    #[serde(rename = "currencyCode")]
    currency_code: Option<String>,
    #[serde(rename = "providerId")]
    provider_id: Option<String>,
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
    vendor: Option<String>,
    #[serde(rename = "productType")]
    product_type: Option<String>,
    #[serde(rename = "shippingProfileSlug")]
    shipping_profile_slug: Option<String>,
    status: Option<String>,
}

#[derive(Debug, Serialize)]
struct CreateShippingOptionInput {
    name: String,
    #[serde(rename = "currencyCode")]
    currency_code: String,
    amount: String,
    #[serde(rename = "providerId")]
    provider_id: Option<String>,
    #[serde(rename = "allowedShippingProfileSlugs")]
    allowed_shipping_profile_slugs: Option<Vec<String>>,
    metadata: Option<String>,
}

#[derive(Debug, Serialize)]
struct UpdateShippingOptionInput {
    name: Option<String>,
    #[serde(rename = "currencyCode")]
    currency_code: Option<String>,
    amount: Option<String>,
    #[serde(rename = "providerId")]
    provider_id: Option<String>,
    #[serde(rename = "allowedShippingProfileSlugs")]
    allowed_shipping_profile_slugs: Option<Vec<String>>,
    metadata: Option<String>,
}

#[derive(Debug, Serialize)]
struct CreateShippingProfileInput {
    slug: String,
    name: String,
    description: Option<String>,
    metadata: Option<String>,
}

#[derive(Debug, Serialize)]
struct UpdateShippingProfileInput {
    slug: Option<String>,
    name: Option<String>,
    description: Option<String>,
    metadata: Option<String>,
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
) -> Result<CommerceAdminBootstrap, ApiError> {
    let response: BootstrapResponse =
        request::<serde_json::Value, BootstrapResponse>(BOOTSTRAP_QUERY, None, token, tenant_slug)
            .await?;
    Ok(CommerceAdminBootstrap {
        current_tenant: response.current_tenant,
        me: response.me,
    })
}

pub async fn fetch_products(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    locale: String,
    search: Option<String>,
    status: Option<String>,
) -> Result<ProductList, ApiError> {
    let response: ProductsResponse = request(
        PRODUCTS_QUERY,
        Some(TenantScopedVariables {
            tenant_id,
            extra: ProductsVariables {
                locale: Some(locale),
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
    locale: String,
) -> Result<Option<ProductDetail>, ApiError> {
    let response: ProductResponse = request(
        PRODUCT_QUERY,
        Some(TenantScopedVariables {
            tenant_id,
            extra: ProductVariables {
                id,
                locale: Some(locale),
            },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.product)
}

pub async fn fetch_shipping_options(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    search: Option<String>,
    currency_code: Option<String>,
    provider_id: Option<String>,
) -> Result<ShippingOptionList, ApiError> {
    let response: ShippingOptionsResponse = request(
        SHIPPING_OPTIONS_QUERY,
        Some(TenantScopedVariables {
            tenant_id,
            extra: ShippingOptionsVariables {
                filter: ShippingOptionsFilter {
                    active: None,
                    currency_code,
                    provider_id,
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
    Ok(response.shipping_options)
}

pub async fn fetch_shipping_option(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
) -> Result<Option<ShippingOption>, ApiError> {
    let response: ShippingOptionResponse = request(
        SHIPPING_OPTION_QUERY,
        Some(TenantScopedVariables {
            tenant_id,
            extra: ShippingOptionVariables { id },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.shipping_option)
}

pub async fn fetch_shipping_profiles(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    search: Option<String>,
) -> Result<ShippingProfileList, ApiError> {
    let response: ShippingProfilesResponse = request(
        SHIPPING_PROFILES_QUERY,
        Some(TenantScopedVariables {
            tenant_id,
            extra: ShippingProfilesVariables {
                filter: ShippingProfilesFilter {
                    active: None,
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
    Ok(response.shipping_profiles)
}

pub async fn fetch_shipping_profile(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
) -> Result<Option<ShippingProfile>, ApiError> {
    let response: ShippingProfileResponse = request(
        SHIPPING_PROFILE_QUERY,
        Some(TenantScopedVariables {
            tenant_id,
            extra: ShippingProfileVariables { id },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.shipping_profile)
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

pub async fn publish_product(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    id: String,
) -> Result<ProductDetail, ApiError> {
    let response: PublishProductResponse = request(
        PUBLISH_PRODUCT_MUTATION,
        Some(TenantUserScopedVariables {
            tenant_id,
            user_id,
            extra: ProductIdVariables { id },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.publish_product)
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

pub async fn create_shipping_option(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    draft: ShippingOptionDraft,
) -> Result<ShippingOption, ApiError> {
    let response: CreateShippingOptionResponse = request(
        CREATE_SHIPPING_OPTION_MUTATION,
        Some(TenantScopedVariables {
            tenant_id,
            extra: CreateShippingOptionVariables {
                input: build_create_shipping_option_input(draft),
            },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.create_shipping_option)
}

pub async fn update_shipping_option(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
    draft: ShippingOptionDraft,
) -> Result<ShippingOption, ApiError> {
    let response: UpdateShippingOptionResponse = request(
        UPDATE_SHIPPING_OPTION_MUTATION,
        Some(TenantScopedVariables {
            tenant_id,
            extra: UpdateShippingOptionVariables {
                id,
                input: build_update_shipping_option_input(draft),
            },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.update_shipping_option)
}

pub async fn deactivate_shipping_option(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
) -> Result<ShippingOption, ApiError> {
    let response: DeactivateShippingOptionResponse = request(
        DEACTIVATE_SHIPPING_OPTION_MUTATION,
        Some(TenantScopedVariables {
            tenant_id,
            extra: ShippingOptionVariables { id },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.deactivate_shipping_option)
}

pub async fn reactivate_shipping_option(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
) -> Result<ShippingOption, ApiError> {
    let response: ReactivateShippingOptionResponse = request(
        REACTIVATE_SHIPPING_OPTION_MUTATION,
        Some(TenantScopedVariables {
            tenant_id,
            extra: ShippingOptionVariables { id },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.reactivate_shipping_option)
}

pub async fn create_shipping_profile(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    draft: ShippingProfileDraft,
) -> Result<ShippingProfile, ApiError> {
    let response: CreateShippingProfileResponse = request(
        CREATE_SHIPPING_PROFILE_MUTATION,
        Some(TenantScopedVariables {
            tenant_id,
            extra: CreateShippingProfileVariables {
                input: build_create_shipping_profile_input(draft),
            },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.create_shipping_profile)
}

pub async fn update_shipping_profile(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
    draft: ShippingProfileDraft,
) -> Result<ShippingProfile, ApiError> {
    let response: UpdateShippingProfileResponse = request(
        UPDATE_SHIPPING_PROFILE_MUTATION,
        Some(TenantScopedVariables {
            tenant_id,
            extra: UpdateShippingProfileVariables {
                id,
                input: build_update_shipping_profile_input(draft),
            },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.update_shipping_profile)
}

pub async fn deactivate_shipping_profile(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
) -> Result<ShippingProfile, ApiError> {
    let response: DeactivateShippingProfileResponse = request(
        DEACTIVATE_SHIPPING_PROFILE_MUTATION,
        Some(TenantScopedVariables {
            tenant_id,
            extra: ShippingProfileVariables { id },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.deactivate_shipping_profile)
}

pub async fn reactivate_shipping_profile(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
) -> Result<ShippingProfile, ApiError> {
    let response: ReactivateShippingProfileResponse = request(
        REACTIVATE_SHIPPING_PROFILE_MUTATION,
        Some(TenantScopedVariables {
            tenant_id,
            extra: ShippingProfileVariables { id },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.reactivate_shipping_profile)
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
        vendor: optional_text(draft.vendor.as_str()),
        product_type: optional_text(draft.product_type.as_str()),
        shipping_profile_slug: draft.shipping_profile_slug,
        publish: Some(draft.publish_now),
    }
}

fn build_create_shipping_option_input(draft: ShippingOptionDraft) -> CreateShippingOptionInput {
    CreateShippingOptionInput {
        name: draft.name.trim().to_string(),
        currency_code: normalize_currency_code(draft.currency_code.as_str()),
        amount: normalize_amount(draft.amount.as_str()),
        provider_id: optional_text(draft.provider_id.as_str()),
        allowed_shipping_profile_slugs: vec_or_none(draft.allowed_shipping_profile_slugs),
        metadata: optional_json_text(draft.metadata_json.as_str()),
    }
}

fn build_update_shipping_option_input(draft: ShippingOptionDraft) -> UpdateShippingOptionInput {
    UpdateShippingOptionInput {
        name: optional_text(draft.name.as_str()),
        currency_code: optional_text(draft.currency_code.as_str())
            .map(|value| normalize_currency_code(value.as_str())),
        amount: optional_text(draft.amount.as_str()).map(|value| normalize_amount(value.as_str())),
        provider_id: optional_text(draft.provider_id.as_str()),
        allowed_shipping_profile_slugs: Some(vec_or_empty(draft.allowed_shipping_profile_slugs)),
        metadata: optional_json_text(draft.metadata_json.as_str()),
    }
}

fn build_create_shipping_profile_input(draft: ShippingProfileDraft) -> CreateShippingProfileInput {
    CreateShippingProfileInput {
        slug: draft.slug.trim().to_string(),
        name: draft.name.trim().to_string(),
        description: optional_text(draft.description.as_str()),
        metadata: optional_json_text(draft.metadata_json.as_str()),
    }
}

fn build_update_shipping_profile_input(draft: ShippingProfileDraft) -> UpdateShippingProfileInput {
    UpdateShippingProfileInput {
        slug: optional_text(draft.slug.as_str()),
        name: optional_text(draft.name.as_str()),
        description: optional_text(draft.description.as_str()),
        metadata: optional_json_text(draft.metadata_json.as_str()),
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

fn vec_or_none(value: Vec<String>) -> Option<Vec<String>> {
    let items = vec_or_empty(value);
    if items.is_empty() {
        None
    } else {
        Some(items)
    }
}

fn vec_or_empty(value: Vec<String>) -> Vec<String> {
    value
        .into_iter()
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

fn normalize_currency_code(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "USD".to_string()
    } else {
        trimmed.to_uppercase()
    }
}

fn normalize_amount(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "0.00".to_string()
    } else {
        trimmed.to_string()
    }
}

fn optional_json_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
