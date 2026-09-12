use std::time::Duration;

use rustok_api::{PortActor, PortContext, PortErrorKind};
use rustok_pricing::{CreatePriceListOwnerInput, in_process_price_list_owner_port};
use sea_orm::Database;
use uuid::Uuid;

fn input() -> CreatePriceListOwnerInput {
    CreatePriceListOwnerInput {
        translations: Vec::new(),
        list_type: "sale".to_string(),
        status: "active".to_string(),
        channel_id: None,
        channel_slug: None,
        starts_at: None,
        ends_at: None,
    }
}

fn context(tenant_id: impl Into<String>, correlation_id: &str) -> PortContext {
    PortContext::new(
        tenant_id,
        PortActor::system(),
        "en",
        correlation_id,
    )
}

#[tokio::test]
async fn price_list_owner_port_requires_idempotency_before_owner_access() {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("failed to create owner-port runtime database");
    let port = in_process_price_list_owner_port(db);

    let error = port
        .create_price_list(
            context(Uuid::new_v4().to_string(), "missing-idempotency")
                .with_deadline(Duration::from_secs(2)),
            input(),
        )
        .await
        .expect_err("write owner port must require idempotency semantics");

    assert_eq!(error.kind, PortErrorKind::Validation);
    assert_eq!(error.code, "port.idempotency_key_required");
}

#[tokio::test]
async fn price_list_owner_port_requires_deadline_before_owner_access() {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("failed to create owner-port runtime database");
    let port = in_process_price_list_owner_port(db);

    let error = port
        .create_price_list(
            context(Uuid::new_v4().to_string(), "missing-deadline")
                .with_idempotency_key("price-list-create"),
            input(),
        )
        .await
        .expect_err("write owner port must require deadline semantics");

    assert_eq!(error.kind, PortErrorKind::Timeout);
    assert_eq!(error.code, "port.deadline_required");
}

#[tokio::test]
async fn price_list_owner_port_rejects_invalid_tenant_before_owner_access() {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("failed to create owner-port runtime database");
    let port = in_process_price_list_owner_port(db);

    let error = port
        .create_price_list(
            context("not-a-uuid", "invalid-tenant")
                .with_idempotency_key("price-list-create")
                .with_deadline(Duration::from_secs(2)),
            input(),
        )
        .await
        .expect_err("owner port must reject a non-UUID tenant");

    assert_eq!(error.kind, PortErrorKind::Validation);
    assert_eq!(error.code, "pricing.price_list_owner_tenant_id_invalid");
}

#[tokio::test]
async fn price_list_owner_port_delegates_owner_validation() {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("failed to create owner-port runtime database");
    let port = in_process_price_list_owner_port(db);

    let error = port
        .create_price_list(
            context(Uuid::new_v4().to_string(), "owner-validation")
                .with_idempotency_key("price-list-create")
                .with_deadline(Duration::from_secs(2)),
            input(),
        )
        .await
        .expect_err("empty localized copy must be rejected by the owner service");

    assert_eq!(error.kind, PortErrorKind::Validation);
    assert_eq!(error.code, "pricing.price_list_owner_validation");
}
