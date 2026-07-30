use std::time::{SystemTime, UNIX_EPOCH};

use super::native_server_adapter::ApiError;

const COMMERCE_ADMIN_PROMOTION_CLIENT_OWNER: &str =
    "rustok_commerce.admin_promotion_transport";
const COMMERCE_ADMIN_PROMOTION_CLIENT_BOUNDARY: &str =
    "commerce_admin_promotion_client_transport";
const COMMERCE_ADMIN_PROMOTION_CLIENT_PUBLIC_MESSAGE: &str =
    "Commerce admin promotion request could not be completed";

pub(super) struct PromotionClientErrorContext {
    operation: &'static str,
    correlation_id: String,
    cart_id_length: usize,
    payload_present: bool,
}

impl PromotionClientErrorContext {
    pub(super) fn for_preview(cart_id: &str) -> Self {
        Self::new("preview_cart_promotion", cart_id)
    }

    pub(super) fn for_apply(cart_id: &str) -> Self {
        Self::new("apply_cart_promotion", cart_id)
    }

    fn new(operation: &'static str, cart_id: &str) -> Self {
        Self {
            operation,
            correlation_id: promotion_client_correlation_id(operation),
            cart_id_length: cart_id.chars().count(),
            payload_present: true,
        }
    }

    pub(super) fn map_error(&self, error: ApiError) -> ApiError {
        tracing::error!(
            raw_error = ?error,
            owner = COMMERCE_ADMIN_PROMOTION_CLIENT_OWNER,
            owner_operation = self.operation,
            correlation_id = %self.correlation_id,
            cart_id_present = self.cart_id_length > 0,
            cart_id_length = self.cart_id_length,
            payload_present = self.payload_present,
            code = "commerce.admin_promotion_client_transport_failed",
            boundary = COMMERCE_ADMIN_PROMOTION_CLIENT_BOUNDARY,
            "commerce admin promotion client transport request failed"
        );

        ApiError::ServerFn(COMMERCE_ADMIN_PROMOTION_CLIENT_PUBLIC_MESSAGE.to_string())
    }
}

fn promotion_client_correlation_id(operation: &'static str) -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("commerce-admin-promotion-client:{operation}:{timestamp}")
}
