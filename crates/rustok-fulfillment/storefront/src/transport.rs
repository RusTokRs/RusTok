use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShippingSelectionDeliveryGroup {
    pub shipping_profile_slug: String,
    pub seller_id: Option<String>,
    pub seller_scope: Option<String>,
    pub selected_shipping_option_id: Option<String>,
    pub available_shipping_option_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectShippingOptionRequest {
    pub cart_id: String,
    pub delivery_groups: Vec<ShippingSelectionDeliveryGroup>,
    pub shipping_profile_slug: String,
    pub seller_id: Option<String>,
    pub seller_scope: Option<String>,
    pub shipping_option_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShippingSelectionUpdate {
    pub shipping_profile_slug: String,
    pub seller_id: Option<String>,
    pub seller_scope: Option<String>,
    pub selected_shipping_option_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShippingSelectionTransportError {
    Graphql(String),
    ServerFn(String),
    Validation(String),
}

impl ShippingSelectionTransportError {
    pub fn message(&self) -> &str {
        match self {
            Self::Graphql(message) | Self::ServerFn(message) | Self::Validation(message) => message,
        }
    }

    pub fn should_fallback_to_graphql(&self) -> bool {
        match self {
            Self::ServerFn(server_error) => {
                server_error.contains("MissingServer")
                    || server_error.contains("missing server")
                    || server_error.contains("not available on this target")
            }
            _ => false,
        }
    }
}

pub async fn select_shipping_option_with_fallback<N, NFut, G, GFut>(
    request: SelectShippingOptionRequest,
    native: N,
    graphql: G,
) -> Result<(), ShippingSelectionTransportError>
where
    N: FnOnce(SelectShippingOptionRequest) -> NFut,
    NFut: std::future::Future<Output = Result<(), ShippingSelectionTransportError>>,
    G: FnOnce(SelectShippingOptionRequest) -> GFut,
    GFut: std::future::Future<Output = Result<(), ShippingSelectionTransportError>>,
{
    match native(request.clone()).await {
        Ok(()) => Ok(()),
        Err(error) if error.should_fallback_to_graphql() => graphql(request).await,
        Err(error) => Err(error),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShippingSelectionError {
    MissingDeliveryGroup {
        shipping_profile_slug: String,
        seller_id: Option<String>,
        seller_scope: Option<String>,
    },
    UnavailableShippingOption {
        shipping_profile_slug: String,
        shipping_option_id: String,
    },
}

pub fn build_select_shipping_option_request(
    cart_id: String,
    delivery_groups: Vec<ShippingSelectionDeliveryGroup>,
    shipping_profile_slug: String,
    seller_id: Option<String>,
    seller_scope: Option<String>,
    shipping_option_id: Option<String>,
) -> SelectShippingOptionRequest {
    SelectShippingOptionRequest {
        cart_id: normalize_required(cart_id),
        delivery_groups,
        shipping_profile_slug: normalize_required(shipping_profile_slug),
        seller_id: normalize_optional(seller_id),
        seller_scope: normalize_optional(seller_scope),
        shipping_option_id: normalize_optional(shipping_option_id),
    }
}

pub fn build_shipping_selection_plan(
    request: &SelectShippingOptionRequest,
) -> Result<Vec<ShippingSelectionUpdate>, ShippingSelectionError> {
    let mut matched_target = false;
    let mut selections = Vec::with_capacity(request.delivery_groups.len());

    for group in &request.delivery_groups {
        let group_matches = group.shipping_profile_slug == request.shipping_profile_slug
            && if let Some(seller_id) = request.seller_id.as_deref() {
                group.seller_id.as_deref() == Some(seller_id)
            } else {
                group.seller_id.is_none() && group.seller_scope == request.seller_scope
            };
        let selected_shipping_option_id = if group_matches {
            matched_target = true;
            if let Some(shipping_option_id) = request.shipping_option_id.as_deref() {
                let is_available = group
                    .available_shipping_option_ids
                    .iter()
                    .any(|option_id| option_id == shipping_option_id);
                if !is_available {
                    return Err(ShippingSelectionError::UnavailableShippingOption {
                        shipping_profile_slug: group.shipping_profile_slug.clone(),
                        shipping_option_id: shipping_option_id.to_string(),
                    });
                }
            }
            request.shipping_option_id.clone()
        } else {
            group.selected_shipping_option_id.clone()
        };

        selections.push(ShippingSelectionUpdate {
            shipping_profile_slug: group.shipping_profile_slug.clone(),
            seller_id: group.seller_id.clone(),
            seller_scope: group.seller_scope.clone(),
            selected_shipping_option_id,
        });
    }

    if !matched_target {
        return Err(ShippingSelectionError::MissingDeliveryGroup {
            shipping_profile_slug: request.shipping_profile_slug.clone(),
            seller_id: request.seller_id.clone(),
            seller_scope: request.seller_scope.clone(),
        });
    }

    Ok(selections)
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn normalize_required(value: String) -> String {
    value.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_request_normalizes_ids() {
        let request = build_select_shipping_option_request(
            " cart-1 ".into(),
            Vec::new(),
            " default ".into(),
            Some(" seller-1 ".into()),
            Some(" ".into()),
            Some(" ship-1 ".into()),
        );

        assert_eq!(request.cart_id, "cart-1");
        assert_eq!(request.shipping_profile_slug, "default");
        assert_eq!(request.seller_id.as_deref(), Some("seller-1"));
        assert_eq!(request.seller_scope, None);
        assert_eq!(request.shipping_option_id.as_deref(), Some("ship-1"));
    }

    #[test]
    fn selection_plan_preserves_existing_groups_and_updates_target() {
        let request = build_select_shipping_option_request(
            "cart-1".into(),
            vec![
                ShippingSelectionDeliveryGroup {
                    shipping_profile_slug: "default".into(),
                    seller_id: Some("seller-1".into()),
                    seller_scope: None,
                    selected_shipping_option_id: Some("old".into()),
                    available_shipping_option_ids: vec!["ship-1".into()],
                },
                ShippingSelectionDeliveryGroup {
                    shipping_profile_slug: "digital".into(),
                    seller_id: None,
                    seller_scope: Some("platform".into()),
                    selected_shipping_option_id: Some("keep".into()),
                    available_shipping_option_ids: vec!["keep".into()],
                },
            ],
            "default".into(),
            Some("seller-1".into()),
            None,
            Some("ship-1".into()),
        );

        let plan = build_shipping_selection_plan(&request).expect("selection plan should build");

        assert_eq!(
            plan[0].selected_shipping_option_id.as_deref(),
            Some("ship-1")
        );
        assert_eq!(plan[1].selected_shipping_option_id.as_deref(), Some("keep"));
    }

    #[test]
    fn server_function_missing_error_can_fallback_to_graphql() {
        assert!(
            ShippingSelectionTransportError::ServerFn("MissingServerFunction".into())
                .should_fallback_to_graphql()
        );
        assert!(ShippingSelectionTransportError::ServerFn(
            "server function is not available on this target".into()
        )
        .should_fallback_to_graphql());
        assert!(
            !ShippingSelectionTransportError::Validation("bad cart".into())
                .should_fallback_to_graphql()
        );
        assert!(!ShippingSelectionTransportError::Graphql("network".into())
            .should_fallback_to_graphql());
    }

    #[test]
    fn selection_plan_rejects_unavailable_target_option() {
        let request = build_select_shipping_option_request(
            "cart-1".into(),
            vec![ShippingSelectionDeliveryGroup {
                shipping_profile_slug: "default".into(),
                seller_id: None,
                seller_scope: None,
                selected_shipping_option_id: None,
                available_shipping_option_ids: vec!["ship-1".into()],
            }],
            "default".into(),
            None,
            None,
            Some("missing".into()),
        );

        assert!(matches!(
            build_shipping_selection_plan(&request),
            Err(ShippingSelectionError::UnavailableShippingOption { .. })
        ));
    }
}
