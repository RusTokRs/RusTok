use uuid::Uuid;

use super::super::{
    SelectShippingOptionRequest, ShippingSelectionTransportError, build_shipping_selection_updates,
};

const FULFILLMENT_STOREFRONT_NATIVE_CLIENT_OWNER: &str = "rustok_fulfillment.storefront";
const FULFILLMENT_STOREFRONT_NATIVE_CLIENT_OPERATION: &str = "select_storefront_shipping_option";
const FULFILLMENT_STOREFRONT_NATIVE_CLIENT_BOUNDARY: &str =
    "fulfillment_storefront_native_client_transport";
const FULFILLMENT_STOREFRONT_NATIVE_CLIENT_PUBLIC_MESSAGE: &str =
    "Shipping selection request could not be completed";

pub(super) struct NativeClientErrorContext {
    correlation_id: String,
    cart_id_length: usize,
    delivery_group_count: usize,
    shipping_profile_slug_length: usize,
    seller_id_present: bool,
    shipping_option_id_present: bool,
    available_shipping_option_count: usize,
}

impl NativeClientErrorContext {
    pub(super) fn validate_and_new(
        request: &SelectShippingOptionRequest,
    ) -> Result<Self, ShippingSelectionTransportError> {
        Uuid::parse_str(request.cart_id.trim()).map_err(|_| {
            ShippingSelectionTransportError::Validation("cart_id must be a valid UUID".to_string())
        })?;

        let updates = build_shipping_selection_updates(request)?;
        for update in updates {
            if let Some(option_id) = update
                .selected_shipping_option_id
                .as_deref()
                .filter(|value| !value.trim().is_empty())
            {
                Uuid::parse_str(option_id.trim()).map_err(|_| {
                    ShippingSelectionTransportError::Validation(
                        "selected_shipping_option_id must be a valid UUID".to_string(),
                    )
                })?;
            }
        }

        Ok(Self {
            correlation_id: format!(
                "fulfillment-storefront-native-client:{}:{}",
                FULFILLMENT_STOREFRONT_NATIVE_CLIENT_OPERATION,
                Uuid::new_v4()
            ),
            cart_id_length: request.cart_id.chars().count(),
            delivery_group_count: request.delivery_groups.len(),
            shipping_profile_slug_length: request.shipping_profile_slug.chars().count(),
            seller_id_present: request.seller_id.is_some(),
            shipping_option_id_present: request.shipping_option_id.is_some(),
            available_shipping_option_count: request
                .delivery_groups
                .iter()
                .map(|group| group.available_shipping_option_ids.len())
                .sum(),
        })
    }

    pub(super) fn map_error(
        &self,
        error: ShippingSelectionTransportError,
    ) -> ShippingSelectionTransportError {
        match error {
            ShippingSelectionTransportError::Validation(message) => {
                ShippingSelectionTransportError::Validation(message)
            }
            error => {
                tracing::error!(
                    raw_error = ?error,
                    owner = FULFILLMENT_STOREFRONT_NATIVE_CLIENT_OWNER,
                    owner_operation = FULFILLMENT_STOREFRONT_NATIVE_CLIENT_OPERATION,
                    correlation_id = %self.correlation_id,
                    cart_id_length = self.cart_id_length,
                    delivery_group_count = self.delivery_group_count,
                    shipping_profile_slug_length = self.shipping_profile_slug_length,
                    seller_id_present = self.seller_id_present,
                    shipping_option_id_present = self.shipping_option_id_present,
                    available_shipping_option_count = self.available_shipping_option_count,
                    code = "fulfillment.storefront_native_client_transport_failed",
                    boundary = FULFILLMENT_STOREFRONT_NATIVE_CLIENT_BOUNDARY,
                    "fulfillment storefront native client transport request failed"
                );

                ShippingSelectionTransportError::ServerFn(
                    FULFILLMENT_STOREFRONT_NATIVE_CLIENT_PUBLIC_MESSAGE.to_string(),
                )
            }
        }
    }
}
