use rustok_fulfillment::FulfillmentService;
use rustok_fulfillment::dto::{
    CreateFulfillmentInput, CreateFulfillmentItemInput, DeliverFulfillmentInput,
    FulfillmentItemQuantityInput, ReopenFulfillmentInput, ShipFulfillmentInput,
};
use rustok_fulfillment::migrations;
use rustok_test_utils::db::setup_test_db;
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

mod support;

async fn setup() -> (sea_orm::DatabaseConnection, FulfillmentService) {
    let db = setup_test_db().await;
    support::ensure_fulfillment_schema(&db).await;

    let manager = SchemaManager::new(&db);
    let serialization = migrations::migrations()
        .pop()
        .expect("fulfillment progress serialization migration should be registered last");
    serialization
        .up(&manager)
        .await
        .expect("fulfillment progress serialization migration should install on SQLite");

    (db.clone(), FulfillmentService::new(db))
}

#[tokio::test]
async fn partial_progress_is_allowed_but_stale_item_writes_are_rejected() {
    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();

    let created = service
        .create_fulfillment(
            tenant_id,
            CreateFulfillmentInput {
                order_id: Uuid::new_v4(),
                shipping_option_id: None,
                customer_id: None,
                carrier: None,
                tracking_number: None,
                items: Some(vec![CreateFulfillmentItemInput {
                    order_line_item_id: Uuid::new_v4(),
                    quantity: 3,
                    metadata: serde_json::json!({}),
                }]),
                metadata: serde_json::json!({"source":"progress-serialization-test"}),
            },
        )
        .await
        .expect("fulfillment should be created");
    let item_id = created.items[0].id;

    let shipped = service
        .ship_fulfillment(
            tenant_id,
            created.id,
            ShipFulfillmentInput {
                carrier: "manual".to_string(),
                tracking_number: "SERIAL-1".to_string(),
                items: Some(vec![FulfillmentItemQuantityInput {
                    fulfillment_item_id: item_id,
                    quantity: 3,
                }]),
                metadata: serde_json::json!({"step":"ship"}),
            },
        )
        .await
        .expect("fulfillment should ship");
    assert_eq!(shipped.status, "shipped");
    assert_eq!(shipped.items[0].shipped_quantity, 3);

    let partially_delivered = service
        .deliver_fulfillment(
            tenant_id,
            created.id,
            DeliverFulfillmentInput {
                delivered_note: Some("partial".to_string()),
                items: Some(vec![FulfillmentItemQuantityInput {
                    fulfillment_item_id: item_id,
                    quantity: 1,
                }]),
                metadata: serde_json::json!({"step":"partial-delivery"}),
            },
        )
        .await
        .expect("partial delivery should preserve shipped status");
    assert_eq!(partially_delivered.status, "shipped");
    assert_eq!(partially_delivered.items[0].delivered_quantity, 1);
    assert_eq!(
        partially_delivered.metadata["audit"]["events"]
            .as_array()
            .expect("audit events should be an array")
            .len(),
        2
    );

    let stale_progress = db
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE fulfillment_items
             SET delivered_quantity = 1, updated_at = CURRENT_TIMESTAMP
             WHERE id = ?",
            vec![item_id.into()],
        ))
        .await;
    assert!(
        stale_progress.is_err(),
        "a stale writer must not overwrite the already committed item progress"
    );

    let delivered = service
        .deliver_fulfillment(
            tenant_id,
            created.id,
            DeliverFulfillmentInput {
                delivered_note: Some("complete".to_string()),
                items: Some(vec![FulfillmentItemQuantityInput {
                    fulfillment_item_id: item_id,
                    quantity: 2,
                }]),
                metadata: serde_json::json!({"step":"complete-delivery"}),
            },
        )
        .await
        .expect("remaining quantity should be deliverable");
    assert_eq!(delivered.status, "delivered");
    assert_eq!(delivered.items[0].delivered_quantity, 3);

    let reopened = service
        .reopen_fulfillment(
            tenant_id,
            created.id,
            ReopenFulfillmentInput {
                items: Some(vec![FulfillmentItemQuantityInput {
                    fulfillment_item_id: item_id,
                    quantity: 1,
                }]),
                metadata: serde_json::json!({"step":"reopen"}),
            },
        )
        .await
        .expect("delivered progress should be reopenable through the owner service");
    assert_eq!(reopened.status, "shipped");
    assert_eq!(reopened.items[0].shipped_quantity, 3);
    assert_eq!(reopened.items[0].delivered_quantity, 2);
}
