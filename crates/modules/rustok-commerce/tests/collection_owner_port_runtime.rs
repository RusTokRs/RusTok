use std::{sync::Arc, time::Duration};

use rustok_api::{PortActor, PortContext, PortErrorKind};
use rustok_core::events::MemoryTransport;
use rustok_commerce::{
    CollectionOwnerPort, CreateCollectionOwnerInput, in_process_collection_owner_port,
};
use sea_orm::Database;
use uuid::Uuid;

fn input() -> CreateCollectionOwnerInput {
    CreateCollectionOwnerInput {
        collection_type: "manual".to_string(),
        conditions: None,
        metadata: serde_json::json!({}),
        translations: Vec::new(),
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

async fn port() -> Arc<dyn CollectionOwnerPort> {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("failed to create Collection owner-port runtime database");
    in_process_collection_owner_port(
        db,
        rustok_outbox::TransactionalEventBus::new(Arc::new(MemoryTransport::new())),
    )
}

#[tokio::test]
async fn collection_owner_port_requires_idempotency_before_owner_access() {
    let error = port()
        .await
        .create_collection(
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
async fn collection_owner_port_requires_deadline_before_owner_access() {
    let error = port()
        .await
        .create_collection(
            context(Uuid::new_v4().to_string(), "missing-deadline")
                .with_idempotency_key("collection-create"),
            input(),
        )
        .await
        .expect_err("write owner port must require deadline semantics");
    assert_eq!(error.kind, PortErrorKind::Timeout);
    assert_eq!(error.code, "port.deadline_required");
}

#[tokio::test]
async fn collection_owner_port_rejects_invalid_tenant_before_owner_access() {
    let error = port()
        .await
        .create_collection(
            context("not-a-uuid", "invalid-tenant")
                .with_idempotency_key("collection-create")
                .with_deadline(Duration::from_secs(2)),
            input(),
        )
        .await
        .expect_err("owner port must reject a non-UUID tenant");
    assert_eq!(error.kind, PortErrorKind::Validation);
    assert_eq!(error.code, "commerce.collection_owner_tenant_id_invalid");
}

#[tokio::test]
async fn collection_owner_port_delegates_owner_validation() {
    let error = port()
        .await
        .create_collection(
            context(Uuid::new_v4().to_string(), "owner-validation")
                .with_idempotency_key("collection-create")
                .with_deadline(Duration::from_secs(2)),
            input(),
        )
        .await
        .expect_err("empty localized copy must be rejected by the owner service");
    assert_eq!(error.kind, PortErrorKind::Validation);
    assert_eq!(error.code, "commerce.collection_owner_validation");
}
