mod graphql_adapter;
mod native_server_adapter;

use std::fmt::{Display, Formatter};

use rustok_ui_core::{normalize_optional_ui_text, normalize_required_ui_text};
use rustok_ui_transport::{UiTransportError, UiTransportPath, execute_selected_transport};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShippingSelectionDeliveryGroup {
    pub shipping_profile_slug: String,
    pub seller_id: Option<String>,
    pub selected_shipping_option_id: Option<String>,
    pub available_shipping_option_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectShippingOptionRequest {
    pub cart_id: String,
    pub delivery_groups: Vec<ShippingSelectionDeliveryGroup>,
    pub shipping_profile_slug: String,
    pub seller_id: Option<String>,
    pub shipping_option_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShippingSelectionUpdate {
    pub shipping_profile_slug: String,
    pub seller_id: Option<String>,
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
}

impl Display for ShippingSelectionTransportError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message())
    }
}

impl std::error::Error for ShippingSelectionTransportError {}

impl ShippingSelectionError {
    pub fn message(&self) -> String {
        match self {
            Self::MissingDeliveryGroup {
                shipping_profile_slug,
                seller_id,
            } => format!(
                "delivery group `{shipping_profile_slug}`/{seller_id:?} is not present in the checkout cart"
            ),
            Self::UnavailableShippingOption {
                shipping_profile_slug,
                shipping_option_id,
            } => format!(
                "shipping option {shipping_option_id} is not available for shipping profile {shipping_profile_slug}"
            ),
        }
    }
}

pub async fn select_shipping_option(
    request: SelectShippingOptionRequest,
) -> Result<(), UiTransportError> {
    let native_request = request.clone();
    execute_selected_transport(
        "fulfillment",
        selected_transport_path(),
        move || native_server_adapter::select_shipping_option(native_request),
        move || graphql_adapter::select_shipping_option(request),
    )
    .await
}

fn selected_transport_path() -> UiTransportPath {
    #[cfg(any(feature = "ssr", feature = "hydrate"))]
    {
        UiTransportPath::NativeServer
    }
    #[cfg(not(any(feature = "ssr", feature = "hydrate")))]
    {
        UiTransportPath::Graphql
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShippingSelectionError {
    MissingDeliveryGroup {
        shipping_profile_slug: String,
        seller_id: Option<String>,
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
    shipping_option_id: Option<String>,
) -> SelectShippingOptionRequest {
    SelectShippingOptionRequest {
        cart_id: normalize_required_ui_text(cart_id),
        delivery_groups,
        shipping_profile_slug: normalize_required_ui_text(shipping_profile_slug),
        seller_id: normalize_optional_ui_text(seller_id),
        shipping_option_id: normalize_optional_ui_text(shipping_option_id),
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
                group.seller_id.is_none()
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
            selected_shipping_option_id,
        });
    }

    if !matched_target {
        return Err(ShippingSelectionError::MissingDeliveryGroup {
            shipping_profile_slug: request.shipping_profile_slug.clone(),
            seller_id: request.seller_id.clone(),
        });
    }

    Ok(selections)
}

pub fn build_shipping_selection_updates(
    request: &SelectShippingOptionRequest,
) -> Result<Vec<ShippingSelectionUpdate>, ShippingSelectionTransportError> {
    build_shipping_selection_plan(request)
        .map_err(|error| ShippingSelectionTransportError::Validation(error.message()))
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
            Some(" ship-1 ".into()),
        );

        assert_eq!(request.cart_id, "cart-1");
        assert_eq!(request.shipping_profile_slug, "default");
        assert_eq!(request.seller_id.as_deref(), Some("seller-1"));
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
                    selected_shipping_option_id: Some("old".into()),
                    available_shipping_option_ids: vec!["ship-1".into()],
                },
                ShippingSelectionDeliveryGroup {
                    shipping_profile_slug: "digital".into(),
                    seller_id: None,
                    selected_shipping_option_id: Some("keep".into()),
                    available_shipping_option_ids: vec!["keep".into()],
                },
            ],
            "default".into(),
            Some("seller-1".into()),
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
    fn default_test_profile_uses_graphql_transport_without_native_fallback() {
        assert_eq!(selected_transport_path(), UiTransportPath::Graphql);
    }

    #[test]
    fn selection_plan_rejects_unavailable_target_option() {
        let request = build_select_shipping_option_request(
            "cart-1".into(),
            vec![ShippingSelectionDeliveryGroup {
                shipping_profile_slug: "default".into(),
                seller_id: None,
                selected_shipping_option_id: None,
                available_shipping_option_ids: vec!["ship-1".into()],
            }],
            "default".into(),
            None,
            Some("missing".into()),
        );

        assert!(matches!(
            build_shipping_selection_plan(&request),
            Err(ShippingSelectionError::UnavailableShippingOption { .. })
        ));
    }

    #[test]
    fn selection_plan_requires_canonical_seller_id() {
        let request = build_select_shipping_option_request(
            "cart-1".into(),
            vec![ShippingSelectionDeliveryGroup {
                shipping_profile_slug: "default".into(),
                seller_id: Some("seller-1".into()),
                selected_shipping_option_id: Some("old".into()),
                available_shipping_option_ids: vec!["ship-1".into()],
            }],
            "default".into(),
            None,
            Some("ship-1".into()),
        );

        assert!(matches!(
            build_shipping_selection_plan(&request),
            Err(ShippingSelectionError::MissingDeliveryGroup { .. })
        ));
    }
}
