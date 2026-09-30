use chrono::Utc;
use rustok_fulfillment::entities::fulfillment;
use rustok_test_utils::db::setup_test_db;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use sea_orm_migration::MigrationTrait;
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

mod support;

async fn apply_checkout_identity_migration(db: &sea_orm::DatabaseConnection) {
    let migration = rustok_fulfillment::migrations::migrations()
        .into_iter()
        .nth(12)
        .expect("checkout identity migration should exist");
    migration
        .up(&SchemaManager::new(db))
        .await
        .expect("checkout identity migration should install");
}

async fn insert_fulfillment(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    metadata: serde_json::Value,
) -> Result<fulfillment::Model, sea_orm::DbErr> {
    let now = Utc::now().fixed_offset();
    fulfillment::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        order_id: Set(Uuid::new_v4()),
        shipping_option_id: Set(None),
        customer_id: Set(None),
        checkout_operation_id: Set(None),
        checkout_fulfillment_index: Set(None),
        checkout_plan_hash: Set(None),
        status: Set("pending".to_string()),
        carrier: Set(None),
        tracking_number: Set(None),
        delivered_note: Set(None),
        cancellation_reason: Set(None),
        metadata: Set(metadata),
        created_at: Set(now),
        updated_at: Set(now),
        shipped_at: Set(None),
        delivered_at: Set(None),
        cancelled_at: Set(None),
    }
    .insert(db)
    .await
}

#[tokio::test]
async fn checkout_identity_rejects_insert_without_operation_id() {
    let db = setup_test_db().await;
    support::ensure_fulfillment_schema(&db).await;
    apply_checkout_identity_migration(&db).await;

    let tenant_id = Uuid::new_v4();
    let result = insert_fulfillment(
        &db,
        tenant_id,
        serde_json::json!({
            "checkout": {
                "fulfillment_key": "checkout:operation:fulfillment:0"
            }
        }),
    )
    .await;

    assert!(
        result.is_err(),
        "checkout identity must require operation_id when fulfillment_key is present"
    );
}

#[tokio::test]
async fn checkout_identity_rejects_update_that_removes_operation_id() {
    let db = setup_test_db().await;
    support::ensure_fulfillment_schema(&db).await;
    apply_checkout_identity_migration(&db).await;

    let tenant_id = Uuid::new_v4();
    let fulfillment = insert_fulfillment(
        &db,
        tenant_id,
        serde_json::json!({
            "checkout": {
                "fulfillment_key": "checkout:operation:fulfillment:0",
                "operation_id": Uuid::new_v4().to_string()
            }
        }),
    )
    .await
    .expect("valid checkout identity should be inserted");

    let mut active: fulfillment::ActiveModel = fulfillment.into();
    active.metadata = Set(serde_json::json!({
        "checkout": {
            "fulfillment_key": "checkout:operation:fulfillment:0"
        }
    }));

    let result = active.update(&db).await;
    assert!(
        result.is_err(),
        "checkout identity must not lose operation_id while retaining fulfillment_key"
    );
}

#[tokio::test]
async fn checkout_identity_allows_unrelated_metadata_updates() {
    let db = setup_test_db().await;
    support::ensure_fulfillment_schema(&db).await;
    apply_checkout_identity_migration(&db).await;

    let tenant_id = Uuid::new_v4();
    let operation_id = Uuid::new_v4();
    let fulfillment = insert_fulfillment(
        &db,
        tenant_id,
        serde_json::json!({
            "checkout": {
                "fulfillment_key": "checkout:operation:fulfillment:0",
                "operation_id": operation_id.to_string()
            },
            "operator_note": "before"
        }),
    )
    .await
    .expect("valid checkout identity should be inserted");

    let mut active: fulfillment::ActiveModel = fulfillment.into();
    active.metadata = Set(serde_json::json!({
        "checkout": {
            "fulfillment_key": "checkout:operation:fulfillment:0",
            "operation_id": operation_id.to_string()
        },
        "operator_note": "after"
    }));

    let updated = active
        .update(&db)
        .await
        .expect("unrelated metadata update should remain allowed");

    assert_eq!(
        updated.metadata["operator_note"].as_str(),
        Some("after")
    );
    assert_eq!(
        updated.metadata["checkout"]["fulfillment_key"].as_str(),
        Some("checkout:operation:fulfillment:0")
    );
    assert_eq!(
        updated.metadata["checkout"]["operation_id"],
        serde_json::Value::String(operation_id.to_string())
    );
}
