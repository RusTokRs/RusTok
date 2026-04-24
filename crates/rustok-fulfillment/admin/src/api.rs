#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use leptos_graphql::{execute as execute_graphql, GraphqlHttpError, GraphqlRequest};
use serde::{Deserialize, Serialize};

use crate::model::{
    FulfillmentAdminBootstrap, ShippingOption, ShippingOptionDraft, ShippingOptionList,
    ShippingProfile, ShippingProfileList,
};

pub type ApiError = GraphqlHttpError;

const BOOTSTRAP_QUERY: &str = "query FulfillmentAdminBootstrap { currentTenant { id slug name } }";
const SHIPPING_OPTIONS_QUERY: &str = "query FulfillmentAdminShippingOptions($tenantId: UUID!, $filter: ShippingOptionsFilter) { shippingOptions(tenantId: $tenantId, filter: $filter) { total page perPage hasNext items { id tenantId name currencyCode amount providerId active allowedShippingProfileSlugs metadata createdAt updatedAt } } }";
const SHIPPING_OPTION_QUERY: &str = "query FulfillmentAdminShippingOption($tenantId: UUID!, $id: UUID!) { shippingOption(tenantId: $tenantId, id: $id) { id tenantId name currencyCode amount providerId active allowedShippingProfileSlugs metadata createdAt updatedAt } }";
const SHIPPING_PROFILES_QUERY: &str = "query FulfillmentAdminShippingProfiles($tenantId: UUID!, $filter: ShippingProfilesFilter) { shippingProfiles(tenantId: $tenantId, filter: $filter) { total page perPage hasNext items { id tenantId slug name description active } } }";
const CREATE_SHIPPING_OPTION_MUTATION: &str = "mutation FulfillmentAdminCreateShippingOption($tenantId: UUID!, $input: CreateShippingOptionInput!) { createShippingOption(tenantId: $tenantId, input: $input) { id tenantId name currencyCode amount providerId active allowedShippingProfileSlugs metadata createdAt updatedAt } }";
const UPDATE_SHIPPING_OPTION_MUTATION: &str = "mutation FulfillmentAdminUpdateShippingOption($tenantId: UUID!, $id: UUID!, $input: UpdateShippingOptionInput!) { updateShippingOption(tenantId: $tenantId, id: $id, input: $input) { id tenantId name currencyCode amount providerId active allowedShippingProfileSlugs metadata createdAt updatedAt } }";
const DEACTIVATE_SHIPPING_OPTION_MUTATION: &str = "mutation FulfillmentAdminDeactivateShippingOption($tenantId: UUID!, $id: UUID!) { deactivateShippingOption(tenantId: $tenantId, id: $id) { id tenantId name currencyCode amount providerId active allowedShippingProfileSlugs metadata createdAt updatedAt } }";
const REACTIVATE_SHIPPING_OPTION_MUTATION: &str = "mutation FulfillmentAdminReactivateShippingOption($tenantId: UUID!, $id: UUID!) { reactivateShippingOption(tenantId: $tenantId, id: $id) { id tenantId name currencyCode amount providerId active allowedShippingProfileSlugs metadata createdAt updatedAt } }";

#[derive(Debug, Deserialize)]
struct BootstrapResponse {
    #[serde(rename = "currentTenant")]
    current_tenant: crate::model::CurrentTenant,
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

#[derive(Debug, Serialize)]
struct TenantScopedVariables<T> {
    #[serde(rename = "tenantId")]
    tenant_id: String,
    #[serde(flatten)]
    extra: T,
}

#[derive(Debug, Serialize)]
struct ShippingOptionVariables {
    id: String,
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
struct ShippingOptionsVariables {
    filter: ShippingOptionsFilter,
}

#[derive(Debug, Serialize)]
struct ShippingProfilesVariables {
    filter: ShippingProfilesFilter,
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
struct CreateShippingOptionInput {
    translations: Vec<ShippingOptionTranslationInput>,
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
    translations: Option<Vec<ShippingOptionTranslationInput>>,
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
struct ShippingOptionTranslationInput {
    locale: String,
    name: String,
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
) -> Result<FulfillmentAdminBootstrap, ApiError> {
    let response: BootstrapResponse =
        request::<serde_json::Value, BootstrapResponse>(BOOTSTRAP_QUERY, None, token, tenant_slug)
            .await?;
    Ok(FulfillmentAdminBootstrap {
        current_tenant: response.current_tenant,
    })
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
) -> Result<Vec<ShippingProfile>, ApiError> {
    let response: ShippingProfilesResponse = request(
        SHIPPING_PROFILES_QUERY,
        Some(TenantScopedVariables {
            tenant_id,
            extra: ShippingProfilesVariables {
                filter: ShippingProfilesFilter {
                    active: None,
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
    Ok(response.shipping_profiles.items)
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

fn build_create_shipping_option_input(draft: ShippingOptionDraft) -> CreateShippingOptionInput {
    CreateShippingOptionInput {
        translations: vec![ShippingOptionTranslationInput {
            locale: draft.locale.trim().to_string(),
            name: draft.name.trim().to_string(),
        }],
        currency_code: normalize_currency_code(draft.currency_code.as_str()),
        amount: normalize_amount(draft.amount.as_str()),
        provider_id: optional_text(draft.provider_id.as_str()),
        allowed_shipping_profile_slugs: vec_or_none(draft.allowed_shipping_profile_slugs),
        metadata: optional_json_text(draft.metadata_json.as_str()),
    }
}

fn build_update_shipping_option_input(draft: ShippingOptionDraft) -> UpdateShippingOptionInput {
    UpdateShippingOptionInput {
        translations: optional_text(draft.name.as_str()).map(|name| {
            vec![ShippingOptionTranslationInput {
                locale: draft.locale.trim().to_string(),
                name,
            }]
        }),
        currency_code: optional_text(draft.currency_code.as_str())
            .map(|value| normalize_currency_code(value.as_str())),
        amount: optional_text(draft.amount.as_str()).map(|value| normalize_amount(value.as_str())),
        provider_id: optional_text(draft.provider_id.as_str()),
        allowed_shipping_profile_slugs: Some(vec_or_empty(draft.allowed_shipping_profile_slugs)),
        metadata: optional_json_text(draft.metadata_json.as_str()),
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
