use rustok_cart::{AtomicCartCheckoutHandle, in_process_cart_checkout_port};
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use rustok_payment::providers::PaymentProviderRegistry;
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

use crate::dto::{CompleteCheckoutInput, CompleteCheckoutResponse};

use super::{
    CheckoutOperationError, CheckoutPlanBuilder, CheckoutService, CheckoutStagePipeline,
    DEFAULT_CHECKOUT_LEASE_SECONDS, RecoveringStagedCheckoutError, RecoveringStagedCheckoutService,
    StagedCheckoutService,
};

#[derive(Debug, Error)]
pub enum JournaledCheckoutError {
    #[error(transparent)]
    Operation(#[from] CheckoutOperationError),
    #[error(transparent)]
    Recovering(#[from] RecoveringStagedCheckoutError),
}

pub type JournaledCheckoutResult<T> = Result<T, JournaledCheckoutError>;

/// Compatibility adapter retained for GraphQL and external callers that still
/// construct the historical journal wrapper. Execution is fully delegated to
/// the same staged checkout and compensation pipeline used by storefront REST.
pub struct JournaledCheckoutService {
    db: sea_orm::DatabaseConnection,
    atomic_cart_checkout: Option<AtomicCartCheckoutHandle>,
    payment_provider_registry: PaymentProviderRegistry,
    lease_seconds: i64,
}

impl JournaledCheckoutService {
    pub fn new(_legacy_checkout: CheckoutService, db: sea_orm::DatabaseConnection) -> Self {
        Self {
            db,
            atomic_cart_checkout: None,
            payment_provider_registry: PaymentProviderRegistry::with_manual_provider(),
            lease_seconds: DEFAULT_CHECKOUT_LEASE_SECONDS,
        }
    }

    pub fn with_lease_seconds(mut self, lease_seconds: i64) -> Self {
        self.lease_seconds = lease_seconds;
        self
    }

    pub fn with_atomic_cart_checkout_handle(
        mut self,
        atomic_cart_checkout: AtomicCartCheckoutHandle,
    ) -> Self {
        self.atomic_cart_checkout = Some(atomic_cart_checkout);
        self
    }

    pub fn with_payment_provider_registry(
        mut self,
        payment_provider_registry: PaymentProviderRegistry,
    ) -> Self {
        self.payment_provider_registry = payment_provider_registry;
        self
    }

    pub async fn complete_checkout(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        idempotency_key: impl Into<String>,
        input: CompleteCheckoutInput,
    ) -> JournaledCheckoutResult<CompleteCheckoutResponse> {
        let atomic_cart_checkout = self.atomic_cart_checkout.clone().ok_or_else(|| {
            CheckoutOperationError::Validation(
                "journaled checkout requires an atomic cart checkout handle".to_string(),
            )
        })?;
        if atomic_cart_checkout.cart_id() != input.cart_id {
            return Err(CheckoutOperationError::Validation(format!(
                "atomic cart checkout is bound to cart {}, not {}",
                atomic_cart_checkout.cart_id(),
                input.cart_id
            ))
            .into());
        }

        let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(self.db.clone())));
        let inventory_availability = Arc::new(rustok_inventory::InventoryService::new(
            self.db.clone(),
            event_bus.clone(),
        ));
        let reservation_port =
            rustok_inventory::in_process_inventory_reservation_identity_port(self.db.clone());
        let cart_port = in_process_cart_checkout_port(self.db.clone());
        let plan_builder = CheckoutPlanBuilder::new(
            self.db.clone(),
            Arc::new(rustok_region::RegionService::new(self.db.clone())),
            inventory_availability,
            Arc::new(rustok_product::CatalogService::new(
                self.db.clone(),
                event_bus.clone(),
            )),
        );
        let pipeline = CheckoutStagePipeline::new(
            self.db.clone(),
            event_bus.clone(),
            reservation_port.clone(),
            cart_port.clone(),
        )
        .with_payment_provider_registry(self.payment_provider_registry.clone());
        let staged = StagedCheckoutService::new(
            plan_builder,
            pipeline,
            atomic_cart_checkout,
            self.db.clone(),
        )
        .with_lease_seconds(self.lease_seconds);
        let compensation = super::CheckoutCompensationService::new(
            self.db.clone(),
            event_bus,
            reservation_port,
            cart_port,
        )
        .with_payment_provider_registry(self.payment_provider_registry.clone())
        .with_lease_seconds(self.lease_seconds);

        RecoveringStagedCheckoutService::new(staged, compensation)
            .complete_checkout(tenant_id, actor_id, idempotency_key, input)
            .await
            .map_err(Into::into)
    }
}
