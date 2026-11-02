//! Source regressions for the real staged preflight. These doubles are test-only owners;
//! no mounted, database or remote-provider certification is implied by their presence.

use super::*;
use async_trait::async_trait;
use rustok_inventory::{
    InventoryAvailabilitySnapshot, InventoryReservationReleaseRequest,
    InventoryReservationReleaseSnapshot, InventoryReservationRequest, InventoryReservationSnapshot,
};
use rustok_product::{PublishedProductsRequest, StorefrontProductList, dto::ProductResponse};
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};

use crate::services::checkout_catalog::tests::fixture;

struct ProductOwner {
    product: ProductResponse,
    reads: AtomicUsize,
}

#[async_trait]
impl ProductCatalogReadPort for ProductOwner {
    async fn read_product_projection(
        &self,
        context: PortContext,
        request: ProductProjectionRequest,
    ) -> Result<ProductResponse, PortError> {
        assert_eq!(request.product_id, self.product.id);
        assert!(rustok_api::locale_tags_match(&context.locale, "en-GB"));
        assert_eq!(context.deadline_ms, Some(2000));
        assert!(context.channel.is_none());
        self.reads.fetch_add(1, Ordering::SeqCst);
        Ok(self.product.clone())
    }

    async fn read_variant_product_projection(
        &self,
        _context: PortContext,
        _request: VariantProductProjectionRequest,
    ) -> Result<ProductResponse, PortError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        Ok(self.product.clone())
    }

    async fn list_published_products(
        &self,
        _context: PortContext,
        _request: PublishedProductsRequest,
    ) -> Result<StorefrontProductList, PortError> {
        Err(PortError::unavailable(
            "test.unexpected_list",
            "Checkout must not discover a substitute",
        ))
    }
}

struct InventoryOwner {
    stock: i32,
    requests: Mutex<Vec<InventoryAvailabilityRequest>>,
    mismatched_response: bool,
}

#[async_trait]
impl InventoryReservationPort for InventoryOwner {
    async fn check_availability(
        &self,
        _context: PortContext,
        request: InventoryAvailabilityRequest,
    ) -> Result<InventoryAvailabilitySnapshot, PortError> {
        let result = InventoryAvailabilitySnapshot {
            variant_id: if self.mismatched_response {
                Uuid::new_v4()
            } else {
                request.variant_id
            },
            requested_quantity: request.requested_quantity,
            available: request.requested_quantity <= self.stock,
        };
        self.requests.lock().unwrap().push(request);
        Ok(result)
    }

    async fn reserve_inventory(
        &self,
        _context: PortContext,
        _request: InventoryReservationRequest,
    ) -> Result<InventoryReservationSnapshot, PortError> {
        Err(PortError::unavailable(
            "test.unexpected_reserve",
            "Preflight is not a reservation",
        ))
    }

    async fn release_inventory_reservation(
        &self,
        _context: PortContext,
        _request: InventoryReservationReleaseRequest,
    ) -> Result<InventoryReservationReleaseSnapshot, PortError> {
        Err(PortError::unavailable(
            "test.unexpected_release",
            "Preflight must be read-only",
        ))
    }
}

async fn builder(
    product: ProductResponse,
    stock: i32,
    mismatched_response: bool,
) -> (CheckoutPlanBuilder, Arc<ProductOwner>, Arc<InventoryOwner>) {
    let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
    let product = Arc::new(ProductOwner {
        product,
        reads: AtomicUsize::new(0),
    });
    let inventory = Arc::new(InventoryOwner {
        stock,
        requests: Mutex::new(Vec::new()),
        mismatched_response,
    });
    let builder = CheckoutPlanBuilder::new(
        db.clone(),
        rustok_region::in_process_region_read_port(db),
        inventory.clone(),
        product.clone(),
    );
    (builder, product, inventory)
}

#[tokio::test]
async fn repeated_uuid_is_checked_once_for_the_total_before_any_reservation() {
    let (mut cart, product) = fixture();
    let mut line = cart.line_items[0].clone();
    line.id = Uuid::new_v4();
    line.quantity = 3;
    cart.line_items.push(line);
    let before = serde_json::to_value(&cart).unwrap();
    let (builder, product, inventory) = builder(product, 4, false).await;
    let result = builder
        .validate_cart_inventory(cart.tenant_id, Uuid::new_v4(), &cart)
        .await;
    assert!(matches!(result, Err(CheckoutError::Validation(_))));
    assert_eq!(product.reads.load(Ordering::SeqCst), 1);
    let requests = inventory.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].requested_quantity, 5);
    assert_eq!(
        requests[0].variant_id,
        cart.line_items[0].variant_id.unwrap()
    );
    assert_eq!(serde_json::to_value(&cart).unwrap(), before);
}

#[tokio::test]
async fn available_total_preserves_separate_line_snapshots() {
    let (mut cart, product) = fixture();
    let mut line = cart.line_items[0].clone();
    line.id = Uuid::new_v4();
    line.quantity = 3;
    cart.line_items.push(line);
    let (builder, product, inventory) = builder(product, 5, false).await;
    assert!(
        builder
            .validate_cart_inventory(cart.tenant_id, Uuid::new_v4(), &cart)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(product.reads.load(Ordering::SeqCst), 1);
    assert_eq!(inventory.requests.lock().unwrap()[0].requested_quantity, 5);
    assert_ne!(cart.line_items[0].id, cart.line_items[1].id);
}

#[tokio::test]
async fn missing_selected_uuid_refuses_before_inventory_without_first_sku_fallback() {
    let (cart, mut product) = fixture();
    product.variants[0].id = Uuid::new_v4();
    let (builder, _, inventory) = builder(product, 100, false).await;
    assert!(
        builder
            .validate_cart_inventory(cart.tenant_id, Uuid::new_v4(), &cart)
            .await
            .is_err()
    );
    assert!(inventory.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn wrong_sku_availability_cannot_authorize_the_original_cart() {
    let (cart, product) = fixture();
    let (builder, _, _) = builder(product, 100, true).await;
    match builder
        .validate_cart_inventory(cart.tenant_id, Uuid::new_v4(), &cart)
        .await
        .unwrap_err()
    {
        CheckoutError::BoundaryFailure {
            stage, kind, code, ..
        } => {
            assert_eq!(stage, "check_inventory_availability");
            assert_eq!(kind, PortErrorKind::InvariantViolation);
            assert_eq!(code, "commerce.inventory_observation_mismatch");
        }
        other => panic!("expected owner response correlation refusal, got {other:?}"),
    }
}
