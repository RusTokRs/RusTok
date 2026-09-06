use std::time::{SystemTime, UNIX_EPOCH};

use super::{
    PaymentCollectionCreateRequest, PaymentCollectionFetchRequest, PaymentTransportError,
    RefundSummaryFetchRequest,
};

const PAYMENT_STOREFRONT_NATIVE_CLIENT_OWNER: &str = "rustok_payment.storefront";
const PAYMENT_STOREFRONT_NATIVE_CLIENT_BOUNDARY: &str =
    "payment_storefront_native_client_transport";
const PAYMENT_STOREFRONT_NATIVE_CLIENT_PUBLIC_MESSAGE: &str =
    "Payment storefront request could not be completed";

pub(super) struct NativeClientErrorContext {
    operation: &'static str,
    correlation_id: String,
    cart_id_length: Option<usize>,
    order_id_length: Option<usize>,
    command_metadata_present: bool,
}

impl NativeClientErrorContext {
    pub(super) fn create_payment_collection(request: &PaymentCollectionCreateRequest) -> Self {
        Self {
            operation: "create_storefront_payment_collection",
            correlation_id: native_client_correlation_id("create_storefront_payment_collection"),
            cart_id_length: Some(request.cart_id.chars().count()),
            order_id_length: None,
            command_metadata_present: true,
        }
    }

    pub(super) fn fetch_payment_collection(request: &PaymentCollectionFetchRequest) -> Self {
        Self {
            operation: "read_storefront_payment_collection",
            correlation_id: native_client_correlation_id("read_storefront_payment_collection"),
            cart_id_length: Some(request.cart_id.chars().count()),
            order_id_length: None,
            command_metadata_present: false,
        }
    }

    pub(super) fn fetch_refund_summary(request: &RefundSummaryFetchRequest) -> Self {
        Self {
            operation: "read_storefront_order_refunds",
            correlation_id: native_client_correlation_id("read_storefront_order_refunds"),
            cart_id_length: None,
            order_id_length: Some(request.order_id.chars().count()),
            command_metadata_present: false,
        }
    }

    pub(super) fn map_error(&self, error: PaymentTransportError) -> PaymentTransportError {
        match error {
            PaymentTransportError::Validation(message) => {
                PaymentTransportError::Validation(message)
            }
            error => {
                tracing::error!(
                    raw_error = ?error,
                    owner = PAYMENT_STOREFRONT_NATIVE_CLIENT_OWNER,
                    owner_operation = self.operation,
                    correlation_id = %self.correlation_id,
                    cart_id_present = self.cart_id_length.is_some(),
                    cart_id_length = ?self.cart_id_length,
                    order_id_present = self.order_id_length.is_some(),
                    order_id_length = ?self.order_id_length,
                    command_metadata_present = self.command_metadata_present,
                    code = "payment.storefront_native_client_transport_failed",
                    boundary = PAYMENT_STOREFRONT_NATIVE_CLIENT_BOUNDARY,
                    "payment storefront native client transport request failed"
                );

                PaymentTransportError::ServerFn(
                    PAYMENT_STOREFRONT_NATIVE_CLIENT_PUBLIC_MESSAGE.to_string(),
                )
            }
        }
    }
}

fn native_client_correlation_id(operation: &'static str) -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("payment-storefront-native-client:{operation}:{timestamp}")
}
