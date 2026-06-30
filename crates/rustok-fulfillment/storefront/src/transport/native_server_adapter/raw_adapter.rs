use leptos::prelude::*;
#[cfg(feature = "ssr")]
use uuid::Uuid;

#[cfg(feature = "ssr")]
use super::super::build_shipping_selection_updates;
use super::super::{SelectShippingOptionRequest, ShippingSelectionTransportError};

pub async fn select_shipping_option_server(
    request: SelectShippingOptionRequest,
) -> Result<(), ShippingSelectionTransportError> {
    storefront_fulfillment_select_shipping_option_native(request)
        .await
        .map_err(|error| ShippingSelectionTransportError::ServerFn(error.to_string()))
}

#[server(prefix = "/api/fn", endpoint = "fulfillment/select-shipping-option")]
async fn storefront_fulfillment_select_shipping_option_native(
    request: SelectShippingOptionRequest,
) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_commerce::storefront_checkout_runtime::{
            self, StorefrontShippingSelectionCommand, StorefrontShippingSelectionUpdateInput,
        };

        let app_ctx = expect_context::<AppContext>();
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let auth = leptos_axum::extract::<rustok_api::OptionalAuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .ok();
        let cart_id = Uuid::parse_str(request.cart_id.trim())
            .map_err(|_| ServerFnError::new("cart_id must be a valid UUID"))?;

        let shipping_selections = build_shipping_selection_updates(&request)
            .map_err(|error| ServerFnError::new(error.message().to_string()))?
            .into_iter()
            .map(|selection| {
                Ok(StorefrontShippingSelectionUpdateInput {
                    shipping_profile_slug: selection.shipping_profile_slug,
                    seller_id: selection.seller_id,
                    selected_shipping_option_id: parse_optional_uuid(
                        selection.selected_shipping_option_id,
                        "selected_shipping_option_id",
                    )?,
                })
            })
            .collect::<Result<Vec<_>, ServerFnError>>()?;

        storefront_checkout_runtime::select_storefront_shipping_option(
            &app_ctx,
            &tenant,
            request_context.as_ref(),
            auth,
            StorefrontShippingSelectionCommand {
                cart_id,
                shipping_selections,
            },
        )
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = request;
        Err(ServerFnError::new(
            "fulfillment/select-shipping-option requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
fn parse_optional_uuid(
    value: Option<String>,
    field_name: &str,
) -> Result<Option<Uuid>, ServerFnError> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            Uuid::parse_str(value.trim())
                .map_err(|_| ServerFnError::new(format!("{field_name} must be a valid UUID")))
        })
        .transpose()
}
