use std::sync::Arc;

use sea_orm::{
    ColumnTrait, ConnectOptions, Database, DatabaseConnection, EntityTrait, PaginatorTrait,
    QueryFilter,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

use crate::command_receipts::{admit, complete, replay_existing, ListingCommandAdmission};
use crate::dto::{
    MarketplaceListingApprovalStatus, MarketplaceListingResponse, MarketplaceListingStatus,
    MarketplaceListingTermsResponse,
};
use crate::entities::listing_command_receipt;
use crate::error::MarketplaceListingError;

#[tokio::test]
async fn completed_receipt_commits_one_contract_event_and_replay_adds_none() {
    let db = setup_database(true).await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let key = "create-listing-once";
    let hash = "request-hash";
    let receipt = new_receipt(&db, tenant_id, actor_id, key, "create_listing", hash).await;
    let response = listing_response(tenant_id);

    let completed = complete(receipt, &response).await.unwrap();
    assert_eq!(completed.id, response.id);

    let receipts = listing_command_receipt::Entity::find()
        .filter(listing_command_receipt::Column::TenantId.eq(tenant_id))
        .all(&db)
        .await
        .unwrap();
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].status, "completed");

    let outbox = rustok_outbox::SysEvents::find().all(&db).await.unwrap();
    assert_eq!(outbox.len(), 1);
    assert_eq!(outbox[0].event_type, "marketplace.listing.created");
    let payload = outbox[0].payload.to_string();
    for forbidden in ["\"note\"", "\"reason\"", "\"metadata\"", "owner_private"] {
        assert!(
            !payload.contains(forbidden),
            "outbox payload leaked {forbidden}"
        );
    }

    let replayed =
        replay_existing::<MarketplaceListingResponse>(&db, tenant_id, key, "create_listing", hash)
            .await
            .unwrap()
            .expect("completed receipt must replay");
    assert_eq!(replayed.id, response.id);
    assert_eq!(
        rustok_outbox::SysEvents::find().count(&db).await.unwrap(),
        1
    );
}

#[tokio::test]
async fn missing_outbox_storage_rolls_back_the_pending_receipt() {
    let db = setup_database(false).await;
    let tenant_id = Uuid::new_v4();
    let receipt = new_receipt(
        &db,
        tenant_id,
        Uuid::new_v4(),
        "outbox-failure",
        "create_listing",
        "request-hash",
    )
    .await;

    let error = complete(receipt, &listing_response(tenant_id))
        .await
        .expect_err("missing outbox table must fail the owner transaction");
    assert!(matches!(
        error,
        MarketplaceListingError::EventPublicationUnavailable
    ));

    let receipts = listing_command_receipt::Entity::find()
        .filter(listing_command_receipt::Column::TenantId.eq(tenant_id))
        .all(&db)
        .await
        .unwrap();
    assert!(
        receipts.is_empty(),
        "pending receipt must roll back with outbox failure"
    );
}

#[tokio::test]
async fn receipt_completion_failure_rolls_back_the_inserted_outbox_event() {
    let db = setup_database(true).await;
    let tenant_id = Uuid::new_v4();
    let receipt = new_receipt(
        &db,
        tenant_id,
        Uuid::new_v4(),
        "receipt-failure",
        "create_listing",
        "request-hash",
    )
    .await;
    listing_command_receipt::Entity::delete_by_id(receipt.receipt_id)
        .exec(&receipt.transaction)
        .await
        .unwrap();

    let error = complete(receipt, &listing_response(tenant_id))
        .await
        .expect_err("missing pending receipt must fail completion");
    assert!(matches!(
        error,
        MarketplaceListingError::CommandReceiptCorrupt
    ));
    assert_eq!(
        rustok_outbox::SysEvents::find().count(&db).await.unwrap(),
        0,
        "outbox insert must roll back when receipt completion fails"
    );
    assert_eq!(
        listing_command_receipt::Entity::find()
            .count(&db)
            .await
            .unwrap(),
        0
    );
}

async fn new_receipt(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    actor_id: Uuid,
    key: &str,
    command_kind: &str,
    hash: &str,
) -> crate::command_receipts::NewListingCommandReceipt {
    let event_bus = rustok_outbox::TransactionalEventBus::new(Arc::new(
        rustok_outbox::OutboxTransport::new(db.clone()),
    ));
    match admit(
        db,
        event_bus,
        tenant_id,
        actor_id,
        key.to_string(),
        command_kind,
        hash,
    )
    .await
    .unwrap()
    {
        ListingCommandAdmission::New(receipt) => receipt,
        ListingCommandAdmission::Replay(_) => panic!("first admission must be new"),
    }
}

async fn setup_database(with_outbox: bool) -> DatabaseConnection {
    let url = format!(
        "sqlite:file:marketplace_listing_outbox_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await.unwrap();
    let manager = SchemaManager::new(&db);
    if with_outbox {
        rustok_outbox::SysEventsMigration
            .up(&manager)
            .await
            .unwrap();
    }
    for migration in crate::migrations::migrations() {
        migration.up(&manager).await.unwrap();
    }
    db
}

fn listing_response(tenant_id: Uuid) -> MarketplaceListingResponse {
    let now = chrono::Utc::now().fixed_offset();
    let listing_id = Uuid::new_v4();
    MarketplaceListingResponse {
        id: listing_id,
        tenant_id,
        seller_id: Uuid::new_v4(),
        master_product_id: Uuid::new_v4(),
        master_variant_id: Uuid::new_v4(),
        seller_sku: "seller-sku".to_string(),
        market_slug: "primary-market".to_string(),
        channel_slug: "web".to_string(),
        status: MarketplaceListingStatus::Draft,
        approval_status: MarketplaceListingApprovalStatus::Draft,
        current_terms_version: 1,
        current_terms: MarketplaceListingTermsResponse {
            id: Uuid::new_v4(),
            listing_id,
            version: 1,
            pricing_reference: Some("price-list".to_string()),
            inventory_reference: Some("inventory-item".to_string()),
            fulfillment_profile_slug: Some("standard".to_string()),
            metadata: serde_json::json!({"owner_private": true}),
            created_at: now.clone(),
        },
        metadata: serde_json::json!({"owner_private": true}),
        published_at: None,
        approved_at: None,
        created_at: now.clone(),
        updated_at: now,
    }
}
