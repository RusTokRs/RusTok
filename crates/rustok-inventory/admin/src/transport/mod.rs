mod native_server_adapter;

use leptos::prelude::*;
use std::fmt::{Display, Formatter};

use crate::core::{
    InventoryAdjustQuantityRequest, InventoryAvailabilityCheckRequest, InventoryProductRequest,
    InventoryProductsRequest, InventoryReleaseReservationRequest, InventoryReserveQuantityRequest,
    InventorySetQuantityRequest, normalized_adjust_quantity_input,
    normalized_availability_check_input, normalized_release_reservation_input,
    normalized_reserve_quantity_input, normalized_set_quantity_input,
};
use crate::model::{
    InventoryAdminBootstrap, InventoryAvailabilityCheckResult, InventoryProductDetail,
    InventoryProductList, InventoryQuantityWriteResult, InventoryReservationReleaseWriteResult,
    InventoryReservationWriteResult,
};

#[derive(Debug, Clone)]
pub enum InventoryTransportError {
    ServerFn(String),
}

impl Display for InventoryTransportError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ServerFn(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for InventoryTransportError {}

impl From<ServerFnError> for InventoryTransportError {
    fn from(value: ServerFnError) -> Self {
        Self::ServerFn(value.to_string())
    }
}

fn products_request(
    tenant_id: String,
    locale: Option<String>,
    search: Option<String>,
    status: Option<String>,
) -> InventoryProductsRequest {
    InventoryProductsRequest {
        tenant_id,
        locale,
        search,
        status,
    }
}

fn product_request(
    tenant_id: String,
    id: String,
    locale: Option<String>,
) -> InventoryProductRequest {
    InventoryProductRequest {
        tenant_id,
        id,
        locale,
    }
}

fn set_quantity_request(
    tenant_id: String,
    variant_id: String,
    quantity: i32,
) -> InventorySetQuantityRequest {
    let input = normalized_set_quantity_input(tenant_id, variant_id, quantity);
    InventorySetQuantityRequest {
        tenant_id: input.tenant_id,
        variant_id: input.variant_id,
        quantity: input.quantity,
    }
}

fn adjust_quantity_request(
    tenant_id: String,
    variant_id: String,
    adjustment: i32,
) -> InventoryAdjustQuantityRequest {
    let input = normalized_adjust_quantity_input(tenant_id, variant_id, adjustment);
    InventoryAdjustQuantityRequest {
        tenant_id: input.tenant_id,
        variant_id: input.variant_id,
        adjustment: input.adjustment,
    }
}

fn reserve_quantity_request(
    tenant_id: String,
    variant_id: String,
    quantity: i32,
) -> InventoryReserveQuantityRequest {
    let input = normalized_reserve_quantity_input(tenant_id, variant_id, quantity);
    InventoryReserveQuantityRequest {
        tenant_id: input.tenant_id,
        variant_id: input.variant_id,
        quantity: input.quantity,
    }
}

fn availability_check_request(
    tenant_id: String,
    variant_id: String,
    requested_quantity: i32,
) -> InventoryAvailabilityCheckRequest {
    let input = normalized_availability_check_input(tenant_id, variant_id, requested_quantity);
    InventoryAvailabilityCheckRequest {
        tenant_id: input.tenant_id,
        variant_id: input.variant_id,
        requested_quantity: input.requested_quantity,
    }
}

fn release_reservation_request(
    tenant_id: String,
    variant_id: String,
    quantity: i32,
) -> InventoryReleaseReservationRequest {
    let input = normalized_release_reservation_input(tenant_id, variant_id, quantity);
    InventoryReleaseReservationRequest {
        tenant_id: input.tenant_id,
        variant_id: input.variant_id,
        quantity: input.quantity,
    }
}

pub async fn fetch_bootstrap() -> Result<InventoryAdminBootstrap, InventoryTransportError> {
    native_server_adapter::fetch_bootstrap()
        .await
        .map_err(Into::into)
}

pub async fn fetch_products(
    tenant_id: String,
    locale: Option<String>,
    search: Option<String>,
    status: Option<String>,
) -> Result<InventoryProductList, InventoryTransportError> {
    let request = products_request(tenant_id, locale, search, status);
    native_server_adapter::fetch_products(
        request.tenant_id,
        request.locale,
        request.search,
        request.status,
    )
    .await
    .map_err(Into::into)
}

pub async fn fetch_product(
    tenant_id: String,
    id: String,
    locale: Option<String>,
) -> Result<Option<InventoryProductDetail>, InventoryTransportError> {
    let request = product_request(tenant_id, id, locale);
    native_server_adapter::fetch_product(request.tenant_id, request.id, request.locale)
        .await
        .map_err(Into::into)
}

pub async fn set_variant_quantity(
    tenant_id: String,
    variant_id: String,
    quantity: i32,
) -> Result<InventoryQuantityWriteResult, InventoryTransportError> {
    let request = set_quantity_request(tenant_id, variant_id, quantity);
    native_server_adapter::set_variant_quantity(
        request.tenant_id,
        request.variant_id,
        request.quantity,
    )
    .await
    .map_err(Into::into)
}

pub async fn adjust_variant_quantity(
    tenant_id: String,
    variant_id: String,
    adjustment: i32,
) -> Result<InventoryQuantityWriteResult, InventoryTransportError> {
    let request = adjust_quantity_request(tenant_id, variant_id, adjustment);
    native_server_adapter::adjust_variant_quantity(
        request.tenant_id,
        request.variant_id,
        request.adjustment,
    )
    .await
    .map_err(Into::into)
}

pub async fn reserve_variant_quantity(
    tenant_id: String,
    variant_id: String,
    quantity: i32,
) -> Result<InventoryReservationWriteResult, InventoryTransportError> {
    let request = reserve_quantity_request(tenant_id, variant_id, quantity);
    native_server_adapter::reserve_variant_quantity(
        request.tenant_id,
        request.variant_id,
        request.quantity,
    )
    .await
    .map_err(Into::into)
}

pub async fn check_variant_availability(
    tenant_id: String,
    variant_id: String,
    requested_quantity: i32,
) -> Result<InventoryAvailabilityCheckResult, InventoryTransportError> {
    let request = availability_check_request(tenant_id, variant_id, requested_quantity);
    native_server_adapter::check_variant_availability(
        request.tenant_id,
        request.variant_id,
        request.requested_quantity,
    )
    .await
    .map_err(Into::into)
}

pub async fn release_reservation_quantity(
    tenant_id: String,
    variant_id: String,
    quantity: i32,
) -> Result<InventoryReservationReleaseWriteResult, InventoryTransportError> {
    let request = release_reservation_request(tenant_id, variant_id, quantity);
    native_server_adapter::release_reservation_quantity(
        request.tenant_id,
        request.variant_id,
        request.quantity,
    )
    .await
    .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::{
        adjust_quantity_request, availability_check_request, product_request, products_request,
        release_reservation_request, reserve_quantity_request, set_quantity_request,
    };

    #[test]
    fn adjust_quantity_request_normalizes_inventory_write_facade_context() {
        let request =
            adjust_quantity_request(" tenant-id ".to_string(), " variant-id ".to_string(), -2);

        assert_eq!(request.tenant_id, "tenant-id");
        assert_eq!(request.variant_id, "variant-id");
        assert_eq!(request.adjustment, -2);
    }

    #[test]
    fn set_quantity_request_normalizes_inventory_write_facade_context() {
        let request =
            set_quantity_request(" tenant-id ".to_string(), " variant-id ".to_string(), 12);

        assert_eq!(request.tenant_id, "tenant-id");
        assert_eq!(request.variant_id, "variant-id");
        assert_eq!(request.quantity, 12);
    }

    #[test]
    fn release_reservation_request_normalizes_inventory_write_facade_context() {
        let request =
            release_reservation_request(" tenant-id ".to_string(), " variant-id ".to_string(), 2);

        assert_eq!(request.tenant_id, "tenant-id");
        assert_eq!(request.variant_id, "variant-id");
        assert_eq!(request.quantity, 2);
    }

    #[test]
    fn availability_check_request_normalizes_inventory_facade_context() {
        let request =
            availability_check_request(" tenant-id ".to_string(), " variant-id ".to_string(), 5);

        assert_eq!(request.tenant_id, "tenant-id");
        assert_eq!(request.variant_id, "variant-id");
        assert_eq!(request.requested_quantity, 5);
    }

    #[test]
    fn reserve_quantity_request_normalizes_inventory_write_facade_context() {
        let request =
            reserve_quantity_request(" tenant-id ".to_string(), " variant-id ".to_string(), 3);

        assert_eq!(request.tenant_id, "tenant-id");
        assert_eq!(request.variant_id, "variant-id");
        assert_eq!(request.quantity, 3);
    }

    #[test]
    fn products_request_preserves_inventory_facade_context() {
        let request = products_request(
            "tenant-id".to_string(),
            Some("en".to_string()),
            Some("boots".to_string()),
            Some("ACTIVE".to_string()),
        );

        assert_eq!(request.tenant_id, "tenant-id");
        assert_eq!(request.locale.as_deref(), Some("en"));
        assert_eq!(request.search.as_deref(), Some("boots"));
        assert_eq!(request.status.as_deref(), Some("ACTIVE"));
    }

    #[test]
    fn product_request_preserves_inventory_facade_context() {
        let request = product_request(
            "tenant-id".to_string(),
            "product-id".to_string(),
            Some("de".to_string()),
        );

        assert_eq!(request.tenant_id, "tenant-id");
        assert_eq!(request.id, "product-id");
        assert_eq!(request.locale.as_deref(), Some("de"));
    }
}
