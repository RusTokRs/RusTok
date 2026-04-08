use leptos_graphql::{execute as execute_graphql, GraphqlHttpError, GraphqlRequest};
use serde::{Deserialize, Serialize};

use crate::model::{
    CommerceAdminBootstrap, ShippingProfile, ShippingProfileDraft, ShippingProfileList,
};

pub type ApiError = GraphqlHttpError;

const BOOTSTRAP_QUERY: &str = "query CommerceAdminBootstrap { currentTenant { id slug name } }";
const SHIPPING_PROFILES_QUERY: &str = "query CommerceShippingProfiles($tenantId: UUID!, $filter: ShippingProfilesFilter) { shippingProfiles(tenantId: $tenantId, filter: $filter) { total page perPage hasNext items { id tenantId slug name description active metadata createdAt updatedAt } } }";
const SHIPPING_PROFILE_QUERY: &str = "query CommerceShippingProfile($tenantId: UUID!, $id: UUID!) { shippingProfile(tenantId: $tenantId, id: $id) { id tenantId slug name description active metadata createdAt updatedAt } }";
const CREATE_SHIPPING_PROFILE_MUTATION: &str = "mutation CommerceCreateShippingProfile($tenantId: UUID!, $input: CreateShippingProfileInput!) { createShippingProfile(tenantId: $tenantId, input: $input) { id tenantId slug name description active metadata createdAt updatedAt } }";
const UPDATE_SHIPPING_PROFILE_MUTATION: &str = "mutation CommerceUpdateShippingProfile($tenantId: UUID!, $id: UUID!, $input: UpdateShippingProfileInput!) { updateShippingProfile(tenantId: $tenantId, id: $id, input: $input) { id tenantId slug name description active metadata createdAt updatedAt } }";
const DEACTIVATE_SHIPPING_PROFILE_MUTATION: &str = "mutation CommerceDeactivateShippingProfile($tenantId: UUID!, $id: UUID!) { deactivateShippingProfile(tenantId: $tenantId, id: $id) { id tenantId slug name description active metadata createdAt updatedAt } }";
const REACTIVATE_SHIPPING_PROFILE_MUTATION: &str = "mutation CommerceReactivateShippingProfile($tenantId: UUID!, $id: UUID!) { reactivateShippingProfile(tenantId: $tenantId, id: $id) { id tenantId slug name description active metadata createdAt updatedAt } }";

#[derive(Debug, Deserialize)]
struct BootstrapResponse {
    #[serde(rename = "currentTenant")]
    current_tenant: crate::model::CurrentTenant,
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
struct ShippingProfileVariables {
    id: String,
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
struct ShippingProfilesFilter {
    active: Option<bool>,
    search: Option<String>,
    page: Option<u64>,
    #[serde(rename = "perPage")]
    per_page: Option<u64>,
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
    })
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

fn optional_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
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
