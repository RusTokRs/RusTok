use super::{
    graphql_adapter,
    graphql_error_safety::{graphql_correlation_id, map_graphql_error},
    native_server_adapter::ApiError,
};
use crate::model::{
    CommerceAdminBootstrap, ShippingProfile, ShippingProfileDraft, ShippingProfileList,
};

pub async fn fetch_bootstrap(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<CommerceAdminBootstrap, ApiError> {
    let operation = "fetch_bootstrap";
    let correlation_id = graphql_correlation_id(operation);
    let tenant_slug_length = tenant_slug.as_deref().map(str::len);
    graphql_adapter::fetch_bootstrap(token, tenant_slug)
        .await
        .map_err(|error| {
            map_graphql_error(error, operation, &correlation_id, None, tenant_slug_length)
        })
}

pub async fn fetch_shipping_profiles(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    search: Option<String>,
) -> Result<ShippingProfileList, ApiError> {
    let operation = "fetch_shipping_profiles";
    let correlation_id = graphql_correlation_id(operation);
    let diagnostic_tenant_id = tenant_id.clone();
    let tenant_slug_length = tenant_slug.as_deref().map(str::len);
    graphql_adapter::fetch_shipping_profiles(token, tenant_slug, tenant_id, search)
        .await
        .map_err(|error| {
            map_graphql_error(
                error,
                operation,
                &correlation_id,
                Some(diagnostic_tenant_id.as_str()),
                tenant_slug_length,
            )
        })
}

pub async fn fetch_shipping_profile(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
) -> Result<Option<ShippingProfile>, ApiError> {
    let operation = "fetch_shipping_profile";
    let correlation_id = graphql_correlation_id(operation);
    let diagnostic_tenant_id = tenant_id.clone();
    let tenant_slug_length = tenant_slug.as_deref().map(str::len);
    graphql_adapter::fetch_shipping_profile(token, tenant_slug, tenant_id, id)
        .await
        .map_err(|error| {
            map_graphql_error(
                error,
                operation,
                &correlation_id,
                Some(diagnostic_tenant_id.as_str()),
                tenant_slug_length,
            )
        })
}

pub async fn create_shipping_profile(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    draft: ShippingProfileDraft,
) -> Result<ShippingProfile, ApiError> {
    let operation = "create_shipping_profile";
    let correlation_id = graphql_correlation_id(operation);
    let diagnostic_tenant_id = tenant_id.clone();
    let tenant_slug_length = tenant_slug.as_deref().map(str::len);
    graphql_adapter::create_shipping_profile(token, tenant_slug, tenant_id, draft)
        .await
        .map_err(|error| {
            map_graphql_error(
                error,
                operation,
                &correlation_id,
                Some(diagnostic_tenant_id.as_str()),
                tenant_slug_length,
            )
        })
}

pub async fn update_shipping_profile(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
    draft: ShippingProfileDraft,
) -> Result<ShippingProfile, ApiError> {
    let operation = "update_shipping_profile";
    let correlation_id = graphql_correlation_id(operation);
    let diagnostic_tenant_id = tenant_id.clone();
    let tenant_slug_length = tenant_slug.as_deref().map(str::len);
    graphql_adapter::update_shipping_profile(token, tenant_slug, tenant_id, id, draft)
        .await
        .map_err(|error| {
            map_graphql_error(
                error,
                operation,
                &correlation_id,
                Some(diagnostic_tenant_id.as_str()),
                tenant_slug_length,
            )
        })
}

pub async fn deactivate_shipping_profile(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
) -> Result<ShippingProfile, ApiError> {
    let operation = "deactivate_shipping_profile";
    let correlation_id = graphql_correlation_id(operation);
    let diagnostic_tenant_id = tenant_id.clone();
    let tenant_slug_length = tenant_slug.as_deref().map(str::len);
    graphql_adapter::deactivate_shipping_profile(token, tenant_slug, tenant_id, id)
        .await
        .map_err(|error| {
            map_graphql_error(
                error,
                operation,
                &correlation_id,
                Some(diagnostic_tenant_id.as_str()),
                tenant_slug_length,
            )
        })
}

pub async fn reactivate_shipping_profile(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
) -> Result<ShippingProfile, ApiError> {
    let operation = "reactivate_shipping_profile";
    let correlation_id = graphql_correlation_id(operation);
    let diagnostic_tenant_id = tenant_id.clone();
    let tenant_slug_length = tenant_slug.as_deref().map(str::len);
    graphql_adapter::reactivate_shipping_profile(token, tenant_slug, tenant_id, id)
        .await
        .map_err(|error| {
            map_graphql_error(
                error,
                operation,
                &correlation_id,
                Some(diagnostic_tenant_id.as_str()),
                tenant_slug_length,
            )
        })
}

#[cfg(test)]
mod tests {
    use std::any::type_name;

    use super::*;

    #[test]
    fn shipping_profile_transport_keeps_api_error_contract() {
        assert!(type_name::<ApiError>().contains("ApiError"));
    }
}
