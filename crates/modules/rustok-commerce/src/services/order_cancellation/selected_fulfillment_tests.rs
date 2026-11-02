use super::*;
use rustok_api::{PortActor, PortErrorKind};
use rustok_fulfillment::{
    FindLatestFulfillmentByOrderProjectionRequest, FulfillmentProjectionPage,
    ReadFulfillmentProjectionRequest,
};
use std::sync::Mutex;

struct Pages {
    pages: Vec<(u64, Vec<FulfillmentResponse>)>,
    requests: Mutex<Vec<(PortContext, ListFulfillmentProjectionsRequest)>>,
}
#[async_trait]
impl FulfillmentReadPort for Pages {
    async fn read_fulfillment_projection(
        &self,
        _: PortContext,
        _: ReadFulfillmentProjectionRequest,
    ) -> Result<FulfillmentResponse, PortError> {
        Err(PortError::unavailable(
            "fixture.unexpected_read",
            "unexpected detail read",
        ))
    }
    async fn find_latest_fulfillment_by_order_projection(
        &self,
        _: PortContext,
        _: FindLatestFulfillmentByOrderProjectionRequest,
    ) -> Result<Option<FulfillmentResponse>, PortError> {
        Err(PortError::unavailable(
            "fixture.unexpected_read",
            "unexpected latest read",
        ))
    }
    async fn list_fulfillment_projections(
        &self,
        context: PortContext,
        request: ListFulfillmentProjectionsRequest,
    ) -> Result<FulfillmentProjectionPage, PortError> {
        let page = request.page;
        self.requests
            .lock()
            .expect("requests")
            .push((context, request));
        let (total, items) = self
            .pages
            .get(page as usize - 1)
            .cloned()
            .ok_or_else(|| PortError::unavailable("fixture.extra_page", "unexpected page"))?;
        Ok(FulfillmentProjectionPage { total, items })
    }
}

fn row(tenant: Uuid, order: Uuid) -> FulfillmentResponse {
    let now = chrono::Utc::now();
    let id = Uuid::new_v4();
    FulfillmentResponse {
        id,
        tenant_id: tenant,
        order_id: order,
        shipping_option_id: None,
        customer_id: None,
        status: "pending".into(),
        carrier: None,
        tracking_number: None,
        delivered_note: None,
        cancellation_reason: None,
        items: vec![rustok_fulfillment::dto::FulfillmentItemResponse {
            id: Uuid::new_v4(),
            fulfillment_id: id,
            order_line_item_id: Uuid::new_v4(),
            quantity: 3,
            shipped_quantity: 0,
            delivered_quantity: 0,
            metadata: json!({}),
            created_at: now,
            updated_at: now,
        }],
        metadata: json!({}),
        created_at: now,
        updated_at: now,
        shipped_at: None,
        delivered_at: None,
        cancelled_at: None,
    }
}
fn ctx(tenant: Uuid) -> PortContext {
    PortContext::new(
        tenant.to_string(),
        PortActor::user(Uuid::new_v4().to_string()),
        "fr",
        "actual-cancellation",
    )
    .with_claim("orders:update")
    .with_role("operator")
    .with_channel("backoffice")
    .with_idempotency_key("cancel-order")
    .with_deadline(std::time::Duration::from_secs(30))
}
async fn service() -> OrderCancellationOrchestrator {
    OrderCancellationOrchestrator::new(
        sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("no native F tables"),
        rustok_test_utils::mock_transactional_event_bus(),
    )
}
async fn read(
    pages: Vec<(u64, Vec<FulfillmentResponse>)>,
    tenant: Uuid,
    order: Uuid,
) -> Result<Vec<FulfillmentResponse>, PortError> {
    let pages: Arc<dyn FulfillmentReadPort> = Arc::new(Pages {
        pages,
        requests: Mutex::new(vec![]),
    });
    service()
        .await
        .read_fulfillments(&pages, &ctx(tenant), tenant, order)
        .await
}

#[tokio::test]
async fn incomplete_selected_pair_refuses_instead_of_reading_native_fulfillment_tables() {
    let service = service().await;
    let commands = FulfillmentAdminCommandRuntime::in_process(
        service.db.clone(),
        FulfillmentProviderRegistry::new(),
    )
    .command_port();
    let error = service
        .clone()
        .with_fulfillment_admin_command_port(commands)
        .fulfillment_ports()
        .err()
        .expect("reader missing");
    assert_eq!(error.code, "order_cancellation.fulfillment_reads_missing");
    assert!(!error.retryable);
    let pages = Arc::new(Pages {
        pages: vec![(0, vec![])],
        requests: Mutex::new(vec![]),
    });
    let error = service
        .with_fulfillment_read_port(pages.clone())
        .fulfillment_ports()
        .err()
        .expect("writer missing");
    assert_eq!(
        error.code,
        "order_cancellation.fulfillment_commands_missing"
    );
    assert!(!error.retryable);
    assert!(pages.requests.lock().expect("requests").is_empty());
}

#[tokio::test]
async fn selected_page_total_is_frozen_before_any_cancellation_effect() {
    let (tenant, order) = (Uuid::new_v4(), Uuid::new_v4());
    for changed_total in [100, 102] {
        let first = (0..100).map(|_| row(tenant, order)).collect();
        let error = read(
            vec![(101, first), (changed_total, vec![row(tenant, order)])],
            tenant,
            order,
        )
        .await
        .expect_err("total changed");
        assert_eq!(error.kind, PortErrorKind::InvariantViolation);
        assert!(!error.retryable);
    }
}

#[tokio::test]
async fn selected_short_pages_duplicate_parents_and_incomplete_coverage_refuse() {
    let (tenant, order) = (Uuid::new_v4(), Uuid::new_v4());
    assert!(
        read(vec![(101, vec![row(tenant, order)])], tenant, order)
            .await
            .is_err()
    );
    let duplicate = row(tenant, order);
    assert!(
        read(vec![(2, vec![duplicate.clone(), duplicate])], tenant, order)
            .await
            .is_err()
    );
    assert!(read(vec![(1, vec![])], tenant, order).await.is_err());
    assert!(
        read(vec![(0, vec![row(tenant, order)])], tenant, order)
            .await
            .is_err()
    );
    assert!(read(vec![(1025, vec![])], tenant, order).await.is_err());
}

#[tokio::test]
async fn selected_children_require_unique_identity_scope_saved_lines_and_valid_pending_counters() {
    let (tenant, order) = (Uuid::new_v4(), Uuid::new_v4());
    let valid = row(tenant, order);
    let mut invalid = Vec::new();
    let mut foreign = valid.clone();
    foreign.tenant_id = Uuid::new_v4();
    invalid.push(foreign);
    let mut wrong_order = valid.clone();
    wrong_order.order_id = Uuid::new_v4();
    invalid.push(wrong_order);
    let mut nil_line = valid.clone();
    nil_line.items[0].order_line_item_id = Uuid::nil();
    invalid.push(nil_line);
    let mut wrong_parent = valid.clone();
    wrong_parent.items[0].fulfillment_id = Uuid::new_v4();
    invalid.push(wrong_parent);
    let mut negative = valid.clone();
    negative.items[0].shipped_quantity = -1;
    invalid.push(negative);
    let mut overdelivered = valid.clone();
    overdelivered.items[0].delivered_quantity = 1;
    invalid.push(overdelivered);
    let mut overshipped = valid.clone();
    overshipped.items[0].shipped_quantity = 4;
    invalid.push(overshipped);
    let mut contradictory = valid.clone();
    contradictory.items[0].shipped_quantity = 1;
    invalid.push(contradictory);
    let mut duplicate_line = valid.clone();
    let mut child = duplicate_line.items[0].clone();
    child.id = Uuid::new_v4();
    duplicate_line.items.push(child);
    invalid.push(duplicate_line);
    for row in invalid {
        assert!(read(vec![(1, vec![row])], tenant, order).await.is_err());
    }
    let mut other = row(tenant, order);
    other.items[0].id = valid.items[0].id;
    assert!(
        read(vec![(2, vec![valid, other])], tenant, order)
            .await
            .is_err(),
        "one child UUID cannot be owned by two parents"
    );
}

#[tokio::test]
async fn complete_selected_pages_preserve_real_context_and_include_the_last_page() {
    let (tenant, order) = (Uuid::new_v4(), Uuid::new_v4());
    let pages = Arc::new(Pages {
        pages: vec![
            (101, (0..100).map(|_| row(tenant, order)).collect()),
            (101, vec![row(tenant, order)]),
        ],
        requests: Mutex::new(vec![]),
    });
    let port: Arc<dyn FulfillmentReadPort> = pages.clone();
    let context = ctx(tenant);
    let result = service()
        .await
        .read_fulfillments(&port, &context, tenant, order)
        .await
        .expect("complete bounded page set");
    assert_eq!(result.len(), 101);
    let requests = pages.requests.lock().expect("requests").clone();
    assert_eq!(requests.len(), 2);
    for (index, (seen, request)) in requests.iter().enumerate() {
        assert_eq!(*seen, context);
        assert_eq!(request.page, index as u64 + 1);
        assert_eq!(request.per_page, 100);
        assert_eq!(request.order_id, Some(order));
        assert!(request.status.is_none() && request.customer_id.is_none());
    }
    assert!(
        read(vec![(0, vec![])], tenant, order)
            .await
            .expect("empty owner set")
            .is_empty()
    );
}
