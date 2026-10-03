use chrono::Utc;
use rustok_fulfillment::entities::{checkout_identity, fulfillment};
use rustok_test_utils::db::setup_test_db;
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, DbBackend, EntityTrait, QueryFilter, Set, Statement,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

mod support;

fn migration_named(name: &str) -> Box<dyn MigrationTrait> {
    rustok_fulfillment::migrations::migrations()
        .into_iter()
        .find(|migration| migration.name() == name)
        .unwrap_or_else(|| panic!("migration {name} should exist"))
}

async fn apply_typed_checkout_identity_migration(db: &sea_orm::DatabaseConnection) {
    migration_named("m20260925_000119_type_checkout_fulfillment_identity")
        .up(&SchemaManager::new(db))
        .await
        .expect("typed checkout identity migration should install");
}

async fn apply_checkout_identity_anchor_migration(db: &sea_orm::DatabaseConnection) {
    migration_named("m20261003_000120_create_checkout_identity_anchor")
        .up(&SchemaManager::new(db))
        .await
        .expect("checkout identity anchor migration should install");
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

async fn insert_legacy_fulfillment(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    metadata: serde_json::Value,
) -> Result<Uuid, sea_orm::DbErr> {
    let id = Uuid::new_v4();
    let now = Utc::now().fixed_offset();
    let active = fulfillment::ActiveModel {
        id: Set(id),
        tenant_id: Set(tenant_id),
        order_id: Set(Uuid::new_v4()),
        status: Set("pending".to_string()),
        metadata: Set(metadata),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    active.insert(db).await.map(|_| id)
}

#[tokio::test]
async fn typed_checkout_identity_rollback_restores_current_legacy_sqlite_guards() {
    let db = setup_test_db().await;
    support::ensure_fulfillment_schema(&db).await;

    let migrations = rustok_fulfillment::migrations::migrations();
    let manager = SchemaManager::new(&db);
    db.execute_unprepared(
        "ALTER TABLE fulfillments DROP COLUMN checkout_plan_hash;\
         ALTER TABLE fulfillments DROP COLUMN checkout_fulfillment_index;\
         ALTER TABLE fulfillments DROP COLUMN checkout_operation_id;",
    )
    .await
    .expect("drop typed columns to prepare pre-migration schema");

    migration_named("m20260713_000117_enforce_checkout_fulfillment_identity")
        .up(&manager)
        .await
        .expect("legacy checkout identity migration should install");
    migration_named("m20260925_000119_type_checkout_fulfillment_identity")
        .up(&manager)
        .await
        .expect("typed checkout identity migration should install");

    let tenant_id = Uuid::new_v4();
    let operation_id = Uuid::new_v4();
    let now = Utc::now().fixed_offset();
    let valid = fulfillment::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        order_id: Set(Uuid::new_v4()),
        shipping_option_id: Set(None),
        customer_id: Set(None),
        checkout_operation_id: Set(Some(operation_id)),
        checkout_fulfillment_index: Set(Some(0)),
        checkout_plan_hash: Set(Some("a".repeat(64))),
        status: Set("pending".to_string()),
        carrier: Set(None),
        tracking_number: Set(None),
        delivered_note: Set(None),
        cancellation_reason: Set(None),
        metadata: Set(serde_json::json!({
            "operator_note": "typed"
        })),
        created_at: Set(now),
        updated_at: Set(now),
        shipped_at: Set(None),
        delivered_at: Set(None),
        cancelled_at: Set(None),
    }
    .insert(&db)
    .await
    .expect("typed checkout fulfillment should be inserted");

    assert_eq!(valid.checkout_operation_id, Some(operation_id));

    migration_named("m20260925_000119_type_checkout_fulfillment_identity")
        .down(&manager)
        .await
        .expect("typed checkout identity rollback should restore legacy schema");

    let triggers = db
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT group_concat(sql, char(10)) FROM sqlite_master WHERE type = 'trigger' AND tbl_name = 'fulfillments'".to_owned(),
        ))
        .await
        .expect("sqlite trigger catalog should be queryable")
        .expect("sqlite trigger catalog row should exist");
    let trigger_sql: Option<String> = triggers
        .try_get_by_index(0)
        .expect("trigger sql should be text");
    let trigger_sql = trigger_sql.unwrap_or_default();
    assert!(
        trigger_sql.contains("fulfillments_checkout_identity_guard_insert"),
        "rollback must restore the current legacy INSERT guard"
    );
    assert!(
        trigger_sql.contains("invalid fulfillment checkout identity"),
        "rollback must restore legacy identity-pair validation"
    );

    let invalid_insert = insert_legacy_fulfillment(
        &db,
        tenant_id,
        serde_json::json!({
            "checkout": {
                "fulfillment_key": "checkout:missing-operation:fulfillment:0"
            }
        }),
    )
    .await;
    assert!(
        invalid_insert.is_err(),
        "rollback must retain the current legacy INSERT guard"
    );

    let valid_legacy = insert_legacy_fulfillment(
        &db,
        tenant_id,
        serde_json::json!({
            "checkout": {
                "fulfillment_key": format!("checkout:{operation_id}:fulfillment:1"),
                "operation_id": operation_id.to_string()
            }
        }),
    )
    .await
    .expect("valid legacy identity should be inserted after rollback");

    let invalid_update = db
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE fulfillments SET metadata = ? WHERE id = ?",
            vec![
                serde_json::json!({
                    "checkout": {
                        "fulfillment_key": format!("checkout:{operation_id}:fulfillment:1")
                    }
                })
                .into(),
                valid_legacy.into(),
            ],
        ))
        .await;
    assert!(
        invalid_update.is_err(),
        "rollback must retain legacy identity-pair validation on UPDATE"
    );
}

async fn insert_typed_fulfillment(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    operation_id: Uuid,
    index: u32,
    order_id: Uuid,
    customer_id: Option<Uuid>,
    plan_hash: &str,
) -> fulfillment::Model {
    let now = Utc::now().fixed_offset();
    fulfillment::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        order_id: Set(order_id),
        shipping_option_id: Set(None),
        customer_id: Set(customer_id),
        checkout_operation_id: Set(Some(operation_id)),
        checkout_fulfillment_index: Set(Some(i64::from(index))),
        checkout_plan_hash: Set(Some(plan_hash.to_string())),
        status: Set("pending".to_string()),
        carrier: Set(None),
        tracking_number: Set(None),
        delivered_note: Set(None),
        cancellation_reason: Set(None),
        metadata: Set(serde_json::json!({})),
        created_at: Set(now),
        updated_at: Set(now),
        shipped_at: Set(None),
        delivered_at: Set(None),
        cancelled_at: Set(None),
    }
    .insert(db)
    .await
    .expect("typed fulfillment should be inserted")
}

#[tokio::test]
async fn checkout_identity_anchor_migration_rejects_divergent_operation_bindings() {
    let db = setup_test_db().await;
    support::ensure_fulfillment_schema(&db).await;
    db.execute_unprepared("DROP TABLE fulfillment_checkout_identities")
        .await
        .expect("anchor table should be droppable before migration");

    let tenant_id = Uuid::new_v4();
    let operation_id = Uuid::new_v4();
    let customer_id = Some(Uuid::new_v4());
    let plan_hash = "a".repeat(64);

    insert_typed_fulfillment(
        &db,
        tenant_id,
        operation_id,
        0,
        Uuid::new_v4(),
        customer_id,
        &plan_hash,
    )
    .await;
    insert_typed_fulfillment(
        &db,
        tenant_id,
        operation_id,
        1,
        Uuid::new_v4(),
        customer_id,
        &plan_hash,
    )
    .await;

    let error = migration_named("m20261003_000120_create_checkout_identity_anchor")
        .up(&SchemaManager::new(&db))
        .await
        .expect_err("divergent order identity must fail migration");
    assert!(
        error.to_string().contains("UNIQUE") || error.to_string().contains("constraint"),
        "migration should fail at the anchor primary key, got: {error}"
    );
}

#[tokio::test]
async fn checkout_identity_anchor_migration_collapses_consistent_operation_bindings() {
    let db = setup_test_db().await;
    support::ensure_fulfillment_schema(&db).await;
    db.execute_unprepared("DROP TABLE fulfillment_checkout_identities")
        .await
        .expect("anchor table should be droppable before migration");

    let tenant_id = Uuid::new_v4();
    let operation_id = Uuid::new_v4();
    let order_id = Uuid::new_v4();
    let customer_id = Some(Uuid::new_v4());

    for index in [0, 1] {
        insert_typed_fulfillment(
            &db,
            tenant_id,
            operation_id,
            index,
            order_id,
            customer_id,
            "A".repeat(64).as_str(),
        )
        .await;
    }

    apply_checkout_identity_anchor_migration(&db).await;

    let anchor = checkout_identity::Entity::find()
        .filter(checkout_identity::Column::TenantId.eq(tenant_id))
        .filter(checkout_identity::Column::CheckoutOperationId.eq(operation_id))
        .one(&db)
        .await
        .expect("anchor query should succeed")
        .expect("consistent operation should have one anchor");

    assert_eq!(anchor.order_id, order_id);
    assert_eq!(anchor.customer_id, customer_id);
    assert_eq!(anchor.plan_hash, "a".repeat(64));
}

#[tokio::test]
async fn checkout_identity_rejects_insert_without_operation_id() {
    let db = setup_test_db().await;
    support::ensure_fulfillment_schema(&db).await;
    apply_typed_checkout_identity_migration(&db).await;

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
    apply_typed_checkout_identity_migration(&db).await;

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
    apply_typed_checkout_identity_migration(&db).await;

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

    assert_eq!(updated.metadata["operator_note"].as_str(), Some("after"));
    assert_eq!(
        updated.metadata["checkout"]["fulfillment_key"].as_str(),
        Some("checkout:operation:fulfillment:0")
    );
    assert_eq!(
        updated.metadata["checkout"]["operation_id"],
        serde_json::Value::String(operation_id.to_string())
    );
}
