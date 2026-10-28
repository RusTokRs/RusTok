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

#[derive(Debug)]
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
        let cart_uuid = Uuid::parse_str(request.cart_id.trim()).map_err(|_| {
            ShippingSelectionTransportError::Validation("cart_id must be a valid UUID".to_string())
        })?;
        if cart_uuid.is_nil() {
            return Err(ShippingSelectionTransportError::Validation(
                "cart_id must not be a nil UUID".to_string(),
            ));
        }

        let updates = build_shipping_selection_updates(request)?;
        for update in updates {
            if let Some(option_id) = update
                .selected_shipping_option_id
                .as_deref()
                .filter(|value| !value.trim().is_empty())
            {
                let option_uuid = Uuid::parse_str(option_id.trim()).map_err(|_| {
                    ShippingSelectionTransportError::Validation(
                        "selected_shipping_option_id must be a valid UUID".to_string(),
                    )
                })?;
                if option_uuid.is_nil() {
                    return Err(ShippingSelectionTransportError::Validation(
                        "selected_shipping_option_id must not be a nil UUID".to_string(),
                    ));
                }
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
        let (error_kind, raw_message_present, raw_message_length) = match &error {
            ShippingSelectionTransportError::Validation(message) => {
                return ShippingSelectionTransportError::Validation(message.clone());
            }
            ShippingSelectionTransportError::Graphql(message) => (
                "graphql",
                !message.trim().is_empty(),
                message.chars().count(),
            ),
            ShippingSelectionTransportError::ServerFn(message) => (
                "server_fn",
                !message.trim().is_empty(),
                message.chars().count(),
            ),
        };

        tracing::error!(
            error_kind,
            raw_message_present,
            raw_message_length,
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

#[cfg(test)]
mod tests {
    use super::{FULFILLMENT_STOREFRONT_NATIVE_CLIENT_PUBLIC_MESSAGE, NativeClientErrorContext};
    use crate::transport::ShippingSelectionTransportError;

    #[test]
    fn native_client_error_does_not_return_raw_transport_detail() {
        let context = NativeClientErrorContext {
            correlation_id: "test-correlation".to_string(),
            cart_id_length: 36,
            delivery_group_count: 1,
            shipping_profile_slug_length: 7,
            seller_id_present: false,
            shipping_option_id_present: true,
            available_shipping_option_count: 1,
        };

        let error = context.map_error(ShippingSelectionTransportError::Graphql(
            "database password: do-not-return".to_string(),
        ));

        assert_eq!(
            error,
            ShippingSelectionTransportError::ServerFn(
                FULFILLMENT_STOREFRONT_NATIVE_CLIENT_PUBLIC_MESSAGE.to_string()
            )
        );
        assert!(!error.message().contains("database password"));
    }

    #[test]
    fn validate_and_new_rejects_nil_cart_id() {
        let request = crate::transport::SelectShippingOptionRequest {
            cart_id: uuid::Uuid::nil().to_string(),
            delivery_groups: vec![],
            shipping_profile_slug: "default".into(),
            seller_id: None,
            shipping_option_id: None,
        };

        let err = NativeClientErrorContext::validate_and_new(&request).unwrap_err();
        assert!(
            matches!(err, ShippingSelectionTransportError::Validation(msg) if msg.contains("cart_id must not be a nil UUID"))
        );
    }

    #[test]
    fn validate_and_new_rejects_nil_shipping_option_id() {
        let opt_id = uuid::Uuid::new_v4().to_string();
        let request = crate::transport::SelectShippingOptionRequest {
            cart_id: uuid::Uuid::new_v4().to_string(),
            delivery_groups: vec![crate::transport::ShippingSelectionDeliveryGroup {
                shipping_profile_slug: "default".into(),
                seller_id: None,
                selected_shipping_option_id: None,
                available_shipping_option_ids: vec![opt_id.clone()],
            }],
            shipping_profile_slug: "default".into(),
            seller_id: None,
            shipping_option_id: Some(uuid::Uuid::nil().to_string()),
        };

        // Note: build_shipping_selection_updates will check availability first if the option is not in available list.
        // If nil UUID is available or passed, it must fail with nil UUID validation.
        let mut request_with_nil_available = request;
        request_with_nil_available.delivery_groups[0].available_shipping_option_ids =
            vec![uuid::Uuid::nil().to_string()];
        let err =
            NativeClientErrorContext::validate_and_new(&request_with_nil_available).unwrap_err();
        assert!(
            matches!(err, ShippingSelectionTransportError::Validation(msg) if msg.contains("selected_shipping_option_id must not be a nil UUID"))
        );
    }
}
