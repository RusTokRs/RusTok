use uuid::Uuid;

use super::super::CompleteCheckoutRequest;

const ORDER_STOREFRONT_NATIVE_CLIENT_OWNER: &str = "rustok_order.storefront";
const ORDER_STOREFRONT_NATIVE_CLIENT_OPERATION: &str = "complete_storefront_checkout";
const ORDER_STOREFRONT_NATIVE_CLIENT_BOUNDARY: &str = "order_storefront_native_client_transport";

pub(super) struct NativeClientDiagnosticContext {
    correlation_id: String,
    cart_id_length: usize,
    idempotency_key_length: usize,
    source_module_length: usize,
    source_surface_length: usize,
    command_length: usize,
    owner_module_length: usize,
}

impl NativeClientDiagnosticContext {
    pub(super) fn new(request: &CompleteCheckoutRequest) -> Self {
        Self {
            correlation_id: format!(
                "order-storefront-native-client:{}:{}",
                ORDER_STOREFRONT_NATIVE_CLIENT_OPERATION,
                Uuid::new_v4()
            ),
            cart_id_length: request.cart_id.chars().count(),
            idempotency_key_length: request.idempotency_key.chars().count(),
            source_module_length: request.metadata.source_module.chars().count(),
            source_surface_length: request.metadata.source_surface.chars().count(),
            command_length: request.metadata.command.chars().count(),
            owner_module_length: request.metadata.owner_module.chars().count(),
        }
    }

    pub(super) fn record_error<E: std::fmt::Debug>(&self, error: &E) {
        tracing::error!(
            raw_error = ?error,
            owner = ORDER_STOREFRONT_NATIVE_CLIENT_OWNER,
            owner_operation = ORDER_STOREFRONT_NATIVE_CLIENT_OPERATION,
            correlation_id = %self.correlation_id,
            cart_id_length = self.cart_id_length,
            idempotency_key_length = self.idempotency_key_length,
            source_module_length = self.source_module_length,
            source_surface_length = self.source_surface_length,
            command_length = self.command_length,
            owner_module_length = self.owner_module_length,
            command_metadata_present = true,
            code = "order.storefront_native_client_transport_failed",
            boundary = ORDER_STOREFRONT_NATIVE_CLIENT_BOUNDARY,
            "order storefront native client transport request failed"
        );
    }
}
