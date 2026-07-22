use rustok_graphql::{GraphqlRequest, execute};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    SelectShippingOptionRequest, ShippingSelectionTransportError, build_shipping_selection_updates,
};

const SELECT_STOREFRONT_SHIPPING_OPTION_MUTATION: &str = "mutation SelectStorefrontShippingOption($cartId: UUID!, $input: UpdateStorefrontCartContextInput!) { updateStorefrontCartContext(cartId: $cartId, input: $input) { cart { id } } }";

#[derive(Debug, Deserialize)]
struct SelectStorefrontShippingOptionResponse {
    #[serde(rename = "updateStorefrontCartContext")]
    updated_cart: GraphqlStorefrontCartContextUpdate,
}

#[derive(Debug, Deserialize)]
struct GraphqlStorefrontCartContextUpdate {
    cart: GraphqlCartMutationPayload,
}

#[derive(Debug, Deserialize)]
struct GraphqlCartMutationPayload {
    id: String,
}

#[derive(Debug, Serialize)]
struct SelectStorefrontShippingOptionVariables {
    #[serde(rename = "cartId")]
    cart_id: Uuid,
    input: UpdateStorefrontCartContextInput,
}

#[derive(Debug, Serialize)]
struct UpdateStorefrontCartContextInput {
    #[serde(rename = "shippingSelections")]
    shipping_selections: Vec<StorefrontShippingSelectionInput>,
}

#[derive(Debug, Serialize)]
struct StorefrontShippingSelectionInput {
    #[serde(rename = "shippingProfileSlug")]
    shipping_profile_slug: String,
    #[serde(rename = "sellerId")]
    seller_id: Option<String>,
    #[serde(rename = "selectedShippingOptionId")]
    selected_shipping_option_id: Option<Uuid>,
}

pub(super) async fn select_shipping_option(
    request: SelectShippingOptionRequest,
) -> Result<(), ShippingSelectionTransportError> {
    let cart_id = parse_required_uuid(&request.cart_id, "cart_id")?;
    let shipping_selections = build_shipping_selection_updates(&request)?
        .into_iter()
        .map(|selection| {
            Ok(StorefrontShippingSelectionInput {
                shipping_profile_slug: selection.shipping_profile_slug,
                seller_id: selection.seller_id,
                selected_shipping_option_id: parse_optional_uuid(
                    selection.selected_shipping_option_id,
                    "selected_shipping_option_id",
                )?,
            })
        })
        .collect::<Result<Vec<_>, ShippingSelectionTransportError>>()?;

    let response: SelectStorefrontShippingOptionResponse = execute(
        &graphql_url(),
        GraphqlRequest::new(
            SELECT_STOREFRONT_SHIPPING_OPTION_MUTATION,
            Some(SelectStorefrontShippingOptionVariables {
                cart_id,
                input: UpdateStorefrontCartContextInput {
                    shipping_selections,
                },
            }),
        ),
        None,
        configured_tenant_slug(),
        None,
    )
    .await
    .map_err(|error| ShippingSelectionTransportError::Graphql(error.to_string()))?;

    let _ = response.updated_cart.cart.id;
    Ok(())
}

fn parse_required_uuid(
    value: &str,
    field_name: &str,
) -> Result<Uuid, ShippingSelectionTransportError> {
    Uuid::parse_str(value.trim()).map_err(|_| {
        ShippingSelectionTransportError::Validation(format!("{field_name} must be a valid UUID"))
    })
}

fn parse_optional_uuid(
    value: Option<String>,
    field_name: &str,
) -> Result<Option<Uuid>, ShippingSelectionTransportError> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(|value| parse_required_uuid(&value, field_name))
        .transpose()
}

fn configured_tenant_slug() -> Option<String> {
    [
        "RUSTOK_TENANT_SLUG",
        "NEXT_PUBLIC_TENANT_SLUG",
        "NEXT_PUBLIC_DEFAULT_TENANT_SLUG",
    ]
    .into_iter()
    .find_map(|key| {
        std::env::var(key).ok().and_then(|value| {
            let value = value.trim().to_string();
            (!value.is_empty()).then_some(value)
        })
    })
}

fn graphql_url() -> String {
    if let Ok(url) = std::env::var("RUSTOK_GRAPHQL_URL") {
        return url;
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
