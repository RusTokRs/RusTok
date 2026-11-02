//! Cart admission must use the Inventory owner, never Product's descriptive global count.
//! Source regressions only; this continuation does not execute these async tests.

use super::*;
use async_trait::async_trait;
use rustok_inventory::{
    InventoryAvailabilitySnapshot, InventoryReservationReleaseRequest,
    InventoryReservationReleaseSnapshot, InventoryReservationRequest, InventoryReservationSnapshot,
};
use std::sync::Mutex;

#[derive(Clone, Copy)]
enum Reply {
    Available(bool),
    WrongSku,
    WrongQuantity,
    Unavailable,
    Pending,
}

struct Owner {
    reply: Reply,
    requests: Mutex<Vec<InventoryAvailabilityRequest>>,
}

impl Owner {
    fn new(reply: Reply) -> Self {
        Self {
            reply,
            requests: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl InventoryReservationPort for Owner {
    async fn check_availability(
        &self,
        context: PortContext,
        request: InventoryAvailabilityRequest,
    ) -> Result<InventoryAvailabilitySnapshot, PortError> {
        assert_eq!(context.channel.as_deref(), Some("web"));
        assert_eq!(request.channel_slug, context.channel);
        self.requests.lock().unwrap().push(request.clone());
        if matches!(self.reply, Reply::Pending) {
            return std::future::pending().await;
        }
        if matches!(self.reply, Reply::Unavailable) {
            return Err(PortError::unavailable(
                "inventory.test_unavailable",
                "private storage cause must not reach Cart",
            ));
        }
        Ok(InventoryAvailabilitySnapshot {
            variant_id: if matches!(self.reply, Reply::WrongSku) {
                Uuid::new_v4()
            } else {
                request.variant_id
            },
            requested_quantity: if matches!(self.reply, Reply::WrongQuantity) {
                request.requested_quantity - 1
            } else {
                request.requested_quantity
            },
            available: !matches!(self.reply, Reply::Available(false)),
        })
    }

    async fn reserve_inventory(
        &self,
        _context: PortContext,
        _request: InventoryReservationRequest,
    ) -> Result<InventoryReservationSnapshot, PortError> {
        panic!("Cart addition observes stock; it must not reserve");
    }

    async fn release_inventory_reservation(
        &self,
        _context: PortContext,
        _request: InventoryReservationReleaseRequest,
    ) -> Result<InventoryReservationReleaseSnapshot, PortError> {
        panic!("Cart addition must not release another consumer's stock");
    }
}

fn fixture() -> (CartResponse, rustok_product::dto::ProductResponse) {
    let (mut cart, product) = crate::services::checkout_catalog::tests::fixture();
    cart.status = "active".to_string();
    cart.channel_id = Some(Uuid::from_u128(8));
    cart.channel_slug = Some("web".to_string());
    (cart, product)
}

fn context(cart: &CartResponse) -> PortContext {
    PortContext::new(
        cart.tenant_id.to_string(),
        rustok_api::PortActor::service("cart-inventory-test"),
        "en-GB",
        "cart-inventory-test",
    )
    .with_channel("web")
    .with_deadline(std::time::Duration::from_secs(2))
}

#[tokio::test]
async fn global_product_quantity_cannot_overrule_a_scoped_inventory_refusal() {
    let (cart, mut product) = fixture();
    let variant_id = product.variants[0].id;
    product.variants[0].inventory_quantity = 100;
    product.variants[0].in_stock = true;
    let owner = Owner::new(Reply::Available(false));
    let error = require_cart_inventory(&owner, context(&cart), &cart, variant_id, 3, None)
        .await
        .unwrap_err();
    assert_eq!(error.kind, rustok_api::PortErrorKind::Conflict);
    assert_eq!(error.code, "commerce.cart_inventory_insufficient");
    assert!(product.variants[0].inventory_quantity >= 5);
    let requests = owner.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].variant_id, variant_id);
    assert_eq!(requests[0].requested_quantity, 5);
    assert_eq!(requests[0].channel_slug.as_deref(), Some("web"));
}

#[tokio::test]
async fn owner_admission_does_not_reimplement_backorder_or_use_stale_product_numbers() {
    let (cart, mut product) = fixture();
    product.variants[0].inventory_quantity = 0;
    product.variants[0].in_stock = false;
    let owner = Owner::new(Reply::Available(true));
    // Fresh owner stock or its canonical backorder policy can admit the request. The
    // Product projection is deliberately stale and is not an input to stock admission.
    require_cart_inventory(
        &owner,
        context(&cart),
        &cart,
        product.variants[0].id,
        3,
        None,
    )
    .await
    .unwrap();
    assert_eq!(owner.requests.lock().unwrap()[0].requested_quantity, 5);
    assert_eq!(cart.line_items.len(), 1);
    assert_eq!(cart.line_items[0].quantity, 2);
}

#[tokio::test]
async fn available_for_another_sku_or_quantity_cannot_authorize_the_cart_demand() {
    let (cart, product) = fixture();
    for reply in [Reply::WrongSku, Reply::WrongQuantity] {
        let owner = Owner::new(reply);
        let error = require_cart_inventory(
            &owner,
            context(&cart),
            &cart,
            product.variants[0].id,
            3,
            None,
        )
        .await
        .unwrap_err();
        assert_eq!(error.kind, rustok_api::PortErrorKind::InvariantViolation);
        assert_eq!(error.code, "commerce.cart_inventory_refused");
        assert_eq!(owner.requests.lock().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn inventory_unavailability_is_not_a_catalog_stock_fallback_or_retry() {
    let (cart, product) = fixture();
    let owner = Owner::new(Reply::Unavailable);
    let error = require_cart_inventory(
        &owner,
        context(&cart),
        &cart,
        product.variants[0].id,
        3,
        None,
    )
    .await
    .unwrap_err();
    assert_eq!(error.kind, rustok_api::PortErrorKind::Unavailable);
    assert!(error.retryable);
    assert!(!error.message.contains("private storage cause"));
    assert_eq!(owner.requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn combined_cart_quantity_overflow_refuses_before_inventory_io() {
    let (mut cart, product) = fixture();
    cart.line_items[0].quantity = i32::MAX;
    let owner = Owner::new(Reply::Available(true));
    let error = require_cart_inventory(
        &owner,
        context(&cart),
        &cart,
        product.variants[0].id,
        1,
        None,
    )
    .await
    .unwrap_err();
    assert_eq!(error.kind, rustok_api::PortErrorKind::Validation);
    assert!(owner.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn inventory_wait_is_bounded_without_a_catalog_fallback() {
    let (cart, product) = fixture();
    let owner = Owner::new(Reply::Pending);
    let context = context(&cart).with_deadline(std::time::Duration::from_millis(10));
    let error = require_cart_inventory(&owner, context, &cart, product.variants[0].id, 3, None)
        .await
        .unwrap_err();
    assert_eq!(error.kind, rustok_api::PortErrorKind::Timeout);
    assert_eq!(error.code, "commerce.cart_inventory_timeout");
    assert_eq!(owner.requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn quantity_replacement_counts_siblings_without_adding_the_old_target_twice() {
    let (mut cart, product) = fixture();
    let target = cart.line_items[0].id;
    let mut sibling = cart.line_items[0].clone();
    sibling.id = Uuid::new_v4();
    sibling.quantity = 3;
    cart.line_items.push(sibling);
    let owner = Owner::new(Reply::Available(true));
    require_cart_inventory(
        &owner,
        context(&cart),
        &cart,
        product.variants[0].id,
        4,
        Some(target),
    )
    .await
    .unwrap();
    assert_eq!(owner.requests.lock().unwrap()[0].requested_quantity, 7);
    assert_eq!(cart.line_items[0].quantity, 2);
    assert_eq!(cart.line_items[1].quantity, 3);
}

#[tokio::test]
async fn replacement_of_an_unknown_or_other_sku_line_is_refused_before_inventory_io() {
    let (cart, product) = fixture();
    let owner = Owner::new(Reply::Available(true));
    assert!(
        require_cart_inventory(
            &owner,
            context(&cart),
            &cart,
            product.variants[0].id,
            4,
            Some(Uuid::new_v4())
        )
        .await
        .is_err()
    );
    assert!(owner.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn replacement_demand_overflow_refuses_before_any_owner_call() {
    let (mut cart, product) = fixture();
    let target = cart.line_items[0].id;
    let mut sibling = cart.line_items[0].clone();
    sibling.id = Uuid::new_v4();
    sibling.quantity = i32::MAX;
    cart.line_items.push(sibling);
    let owner = Owner::new(Reply::Available(true));
    assert!(
        require_cart_inventory(
            &owner,
            context(&cart),
            &cart,
            product.variants[0].id,
            1,
            Some(target)
        )
        .await
        .is_err()
    );
    assert!(owner.requests.lock().unwrap().is_empty());
}
