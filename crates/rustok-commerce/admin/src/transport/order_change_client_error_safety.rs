use std::time::{SystemTime, UNIX_EPOCH};

use super::native_server_adapter::ApiError;

const COMMERCE_ADMIN_ORDER_CHANGE_CLIENT_OWNER: &str =
    "rustok_commerce.admin_order_change_transport";
const COMMERCE_ADMIN_ORDER_CHANGE_CLIENT_BOUNDARY: &str =
    "commerce_admin_order_change_client_transport";
const COMMERCE_ADMIN_ORDER_CHANGE_CLIENT_PUBLIC_MESSAGE: &str =
    "Commerce admin order-change request could not be completed";

pub(super) struct OrderChangeClientErrorContext {
    operation: &'static str,
    correlation_id: String,
    token_present: bool,
    tenant_slug_length: Option<usize>,
    tenant_id_length: usize,
    order_id_length: Option<usize>,
    order_change_id_length: Option<usize>,
    status_length: Option<usize>,
    payload_present: bool,
}

impl OrderChangeClientErrorContext {
    pub(super) fn for_fetch(
        token: Option<&str>,
        tenant_slug: Option<&str>,
        tenant_id: &str,
        order_id: Option<&str>,
        status: Option<&str>,
    ) -> Self {
        Self {
            operation: "fetch_order_changes",
            correlation_id: order_change_client_correlation_id("fetch_order_changes"),
            token_present: token.is_some(),
            tenant_slug_length: tenant_slug.map(str::chars).map(Iterator::count),
            tenant_id_length: tenant_id.chars().count(),
            order_id_length: order_id.map(str::chars).map(Iterator::count),
            order_change_id_length: None,
            status_length: status.map(str::chars).map(Iterator::count),
            payload_present: false,
        }
    }

    pub(super) fn for_apply(
        token: Option<&str>,
        tenant_slug: Option<&str>,
        tenant_id: &str,
        order_change_id: &str,
    ) -> Self {
        Self::for_write(
            "apply_order_change",
            token,
            tenant_slug,
            tenant_id,
            order_change_id,
        )
    }

    pub(super) fn for_cancel(
        token: Option<&str>,
        tenant_slug: Option<&str>,
        tenant_id: &str,
        order_change_id: &str,
    ) -> Self {
        Self::for_write(
            "cancel_order_change",
            token,
            tenant_slug,
            tenant_id,
            order_change_id,
        )
    }

    fn for_write(
        operation: &'static str,
        token: Option<&str>,
        tenant_slug: Option<&str>,
        tenant_id: &str,
        order_change_id: &str,
    ) -> Self {
        Self {
            operation,
            correlation_id: order_change_client_correlation_id(operation),
            token_present: token.is_some(),
            tenant_slug_length: tenant_slug.map(str::chars).map(Iterator::count),
            tenant_id_length: tenant_id.chars().count(),
            order_id_length: None,
            order_change_id_length: Some(order_change_id.chars().count()),
            status_length: None,
            payload_present: true,
        }
    }

    pub(super) fn map_error(&self, error: ApiError) -> ApiError {
        tracing::error!(
            raw_error = ?error,
            owner = COMMERCE_ADMIN_ORDER_CHANGE_CLIENT_OWNER,
            owner_operation = self.operation,
            correlation_id = %self.correlation_id,
            token_present = self.token_present,
            tenant_slug_present = self.tenant_slug_length.is_some(),
            tenant_slug_length = ?self.tenant_slug_length,
            tenant_id_present = self.tenant_id_length > 0,
            tenant_id_length = self.tenant_id_length,
            order_id_present = self.order_id_length.is_some(),
            order_id_length = ?self.order_id_length,
            order_change_id_present = self.order_change_id_length.is_some(),
            order_change_id_length = ?self.order_change_id_length,
            status_present = self.status_length.is_some(),
            status_length = ?self.status_length,
            payload_present = self.payload_present,
            code = "commerce.admin_order_change_client_transport_failed",
            boundary = COMMERCE_ADMIN_ORDER_CHANGE_CLIENT_BOUNDARY,
            "commerce admin order-change client transport request failed"
        );

        ApiError::ServerFn(COMMERCE_ADMIN_ORDER_CHANGE_CLIENT_PUBLIC_MESSAGE.to_string())
    }
}

fn order_change_client_correlation_id(operation: &'static str) -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("commerce-admin-order-change-client:{operation}:{timestamp}")
}
