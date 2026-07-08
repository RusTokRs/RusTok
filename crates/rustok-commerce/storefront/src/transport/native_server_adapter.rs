use leptos::prelude::*;

#[cfg(feature = "ssr")]
use super::shared_adapter;
use super::shared_adapter::ApiError;
use crate::core::FetchCommerceRequest;
#[cfg(feature = "ssr")]
use crate::model::StorefrontCheckoutWorkspace;
use crate::model::StorefrontCommerceData;

pub async fn fetch_storefront_commerce(
    request: FetchCommerceRequest,
) -> Result<StorefrontCommerceData, ApiError> {
    storefront_commerce_native(request.selected_cart_id, request.locale)
        .await
        .map_err(|error| ApiError::ServerFn(error.to_string()))
}

#[server(prefix = "/api/fn", endpoint = "commerce/storefront-data")]
async fn storefront_commerce_native(
    selected_cart_id: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontCommerceData, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let normalized_locale = shared_adapter::resolve_requested_locale(
            locale,
            Some(request_context.locale.as_str()),
            tenant.default_locale.as_str(),
        );
        let mut data = StorefrontCommerceData {
            effective_locale: normalized_locale,
            tenant_slug: Some(tenant.slug),
            tenant_default_locale: tenant.default_locale,
            channel_slug: request_context.channel_slug.clone(),
            channel_resolution_source: request_context
                .channel_resolution_source
                .as_ref()
                .map(|source| source.as_str().to_string()),
            selected_cart_id: None,
            checkout: None,
        };

        let Some((normalized_cart_id, _)) = shared_adapter::parse_cart_id(selected_cart_id)
            .map_err(|err| ServerFnError::new(err.to_string()))?
        else {
            return Ok(data);
        };
        let cart_data = rustok_cart_storefront::transport::fetch_cart(
            rustok_cart_storefront::core::build_cart_fetch_request(
                Some(normalized_cart_id.clone()),
                Some(data.effective_locale.clone()),
            ),
        )
        .await
        .map_err(|err| ServerFnError::new(err.to_string()))?;
        let payment_collection = if cart_data.cart.is_some() {
            rustok_payment_storefront::transport::fetch_payment_collection(
                rustok_payment_storefront::transport::build_payment_collection_fetch_request(
                    normalized_cart_id.clone(),
                ),
            )
            .await
            .map_err(|err| ServerFnError::new(err.to_string()))?
        } else {
            None
        };

        data.selected_cart_id = Some(normalized_cart_id);
        data.checkout = Some(StorefrontCheckoutWorkspace {
            cart: cart_data.cart.map(shared_adapter::map_cart_checkout_cart),
            payment_collection,
        });
        Ok(data)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (selected_cart_id, locale);
        Err(ServerFnError::new(
            "commerce/storefront-data requires the `ssr` feature",
        ))
    }
}
