use rustok_graphql::{GraphqlRequest, execute, graphql_url};
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

    if response.updated_cart.cart.id.trim().is_empty() {
        return Err(ShippingSelectionTransportError::Graphql(
            "storefront cart response missing cart identity".to_string(),
        ));
    }
    Ok(())
}

fn parse_required_uuid(
    value: &str,
    field_name: &str,
) -> Result<Uuid, ShippingSelectionTransportError> {
    let parsed = Uuid::parse_str(value.trim()).map_err(|_| {
        ShippingSelectionTransportError::Validation(format!("{field_name} must be a valid UUID"))
    })?;
    if parsed.is_nil() {
        return Err(ShippingSelectionTransportError::Validation(format!(
            "{field_name} must not be a nil UUID"
        )));
    }
    Ok(parsed)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_required_uuid_validates_and_rejects_nil() {
        let valid = Uuid::new_v4();
        assert_eq!(
            parse_required_uuid(&valid.to_string(), "cart_id").unwrap(),
            valid
        );

        let err_invalid = parse_required_uuid("not-a-uuid", "cart_id").unwrap_err();
        assert!(
            matches!(err_invalid, ShippingSelectionTransportError::Validation(msg) if msg.contains("must be a valid UUID"))
        );

        let err_nil = parse_required_uuid(&Uuid::nil().to_string(), "cart_id").unwrap_err();
        assert!(
            matches!(err_nil, ShippingSelectionTransportError::Validation(msg) if msg.contains("must not be a nil UUID"))
        );
    }

    #[test]
    fn parse_optional_uuid_handles_none_empty_and_valid() {
        assert_eq!(parse_optional_uuid(None, "option_id").unwrap(), None);
        assert_eq!(
            parse_optional_uuid(Some("   ".into()), "option_id").unwrap(),
            None
        );

        let valid = Uuid::new_v4();
        assert_eq!(
            parse_optional_uuid(Some(valid.to_string()), "option_id").unwrap(),
            Some(valid)
        );

        let err_nil = parse_optional_uuid(Some(Uuid::nil().to_string()), "option_id").unwrap_err();
        assert!(
            matches!(err_nil, ShippingSelectionTransportError::Validation(msg) if msg.contains("must not be a nil UUID"))
        );
    }
}
