use super::super::fulfillment_orchestration::FulfillmentOrchestrationError;
use super::*;
use async_trait::async_trait;
use rustok_api::{PortActor, PortError};
use rustok_fulfillment::{
    CancelAdminFulfillmentRequest, DeliverAdminFulfillmentRequest, ReopenAdminFulfillmentRequest,
    ReshipAdminFulfillmentRequest, RetryLocalFulfillmentPersistenceRequest,
    ShipAdminFulfillmentRequest,
};
use std::sync::Mutex;

#[derive(Default)]
struct SelectedOwner {
    calls: Mutex<Vec<(PortContext, Uuid)>>,
}
fn selected_refusal() -> PortError {
    PortError::unavailable(
        "selected_fulfillment.offline",
        "selected owner is unavailable",
    )
    .with_retryable(false)
}
#[async_trait]
impl FulfillmentAdminCommandPort for SelectedOwner {
    async fn ship_fulfillment(
        &self,
        _: PortContext,
        _: ShipAdminFulfillmentRequest,
    ) -> Result<FulfillmentResponse, PortError> {
        Err(selected_refusal())
    }
    async fn deliver_fulfillment(
        &self,
        _: PortContext,
        _: DeliverAdminFulfillmentRequest,
    ) -> Result<FulfillmentResponse, PortError> {
        Err(selected_refusal())
    }
    async fn reopen_fulfillment(
        &self,
        _: PortContext,
        _: ReopenAdminFulfillmentRequest,
    ) -> Result<FulfillmentResponse, PortError> {
        Err(selected_refusal())
    }
    async fn reship_fulfillment(
        &self,
        _: PortContext,
        _: ReshipAdminFulfillmentRequest,
    ) -> Result<FulfillmentResponse, PortError> {
        Err(selected_refusal())
    }
    async fn cancel_fulfillment(
        &self,
        _: PortContext,
        _: CancelAdminFulfillmentRequest,
    ) -> Result<FulfillmentResponse, PortError> {
        Err(selected_refusal())
    }
    async fn retry_local_persistence(
        &self,
        context: PortContext,
        request: RetryLocalFulfillmentPersistenceRequest,
    ) -> Result<FulfillmentResponse, PortError> {
        self.calls
            .lock()
            .expect("calls")
            .push((context, request.operation_id));
        Err(selected_refusal())
    }
}

fn current_context(tenant: Uuid) -> PortContext {
    PortContext::new(
        tenant.to_string(),
        PortActor::user(Uuid::new_v4().to_string()),
        "fr",
        "actual-request",
    )
    .with_claim("fulfillments:update")
    .with_role("operator")
    .with_channel("backoffice")
    .with_causation_id("retry-request")
    .with_traceparent("00-0123456789abcdef0123456789abcdef-0123456789abcdef-01")
    .with_deadline(std::time::Duration::from_secs(17))
}

#[tokio::test]
async fn retry_local_persistence_rejects_nil_uuids_before_owner_io() {
    let db = sea_orm::Database::connect("sqlite::memory:")
        .await
        .expect("db without native owner tables");
    let service = FulfillmentReconciliationService::new(db);
    for (tenant, operation) in [(Uuid::nil(), Uuid::new_v4()), (Uuid::new_v4(), Uuid::nil())] {
        assert!(matches!(
            service
                .retry_local_persistence(current_context(tenant), operation)
                .await,
            Err(FulfillmentOrchestrationError::Validation(_))
        ));
    }
}

#[tokio::test]
async fn selected_recovery_keeps_real_context_and_never_reconstructs_a_native_owner() {
    let db = sea_orm::Database::connect("sqlite::memory:")
        .await
        .expect("db without native owner tables");
    let selected = Arc::new(SelectedOwner::default());
    let service = FulfillmentReconciliationService::new(db.clone())
        .with_fulfillment_admin_command_port(selected.clone())
        .with_shipment_stock_port(Arc::new(rustok_fulfillment::UnconfiguredShipmentStockPort))
        .with_order_read_port(rustok_order::in_process_order_read_port(
            db,
            rustok_test_utils::mock_transactional_event_bus(),
        ));
    let context = current_context(Uuid::new_v4());
    let operation = Uuid::new_v4();
    let error = service
        .retry_local_persistence(context.clone(), operation)
        .await
        .expect_err("selected outage is not native fallback");
    assert!(
        matches!(error, FulfillmentOrchestrationError::Owner(cause) if cause.code == "selected_fulfillment.offline" && !cause.retryable)
    );
    assert_eq!(
        *selected.calls.lock().expect("calls"),
        vec![(context.clone(), operation)]
    );
    let literal = context.with_idempotency_key("fulfillment:saved-legacy-provider-key");
    assert!(
        service
            .retry_local_persistence(literal.clone(), operation)
            .await
            .is_err()
    );
    assert_eq!(
        selected.calls.lock().expect("calls")[1],
        (literal, operation)
    );
}
