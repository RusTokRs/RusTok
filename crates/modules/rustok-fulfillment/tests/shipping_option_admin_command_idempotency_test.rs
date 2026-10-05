use std::time::Duration;

use rust_decimal::Decimal;
use rustok_api::{PortActor, PortContext, PortErrorKind};
use rustok_fulfillment::{
    CreateAdminShippingOptionRequest, CreateShippingOptionInput, DeactivateAdminShippingOptionRequest,
    FulfillmentService, ReactivateAdminShippingOptionRequest, ShippingOptionAdminCommandPort,
    ShippingOptionTranslationInput, UpdateAdminShippingOptionRequest, UpdateShippingOptionInput,
    in_process_shipping_option_admin_command_port,
};
use rustok_test_utils::db::setup_test_db;
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::SchemaManager;
use std::str::FromStr;
use uuid::Uuid;

mod support;

async fn setup() -> (sea_orm::DatabaseConnection, rustok_fulfillment::ShippingOptionAdminCommandRuntime) {
    let db = setup_test_db().await;
    support::ensure_fulfillment_schema(&db).await;
    rustok_outbox::migration::create_owner_operation_receipts_table(&SchemaManager::new(&db))
        .await
        .expect("owner operation receipt schema should be created");
    let port = in_process_shipping_option_admin_command_port(db.clone());
    (db, rustok_fulfillment::ShippingOptionAdminCommandRuntime::new(port))
}

fn create_request(amount: &str) -> CreateAdminShippingOptionRequest {
    CreateAdminShippingOptionRequest {
        input: CreateShippingOptionInput {
            translations: vec![ShippingOptionTranslationInput {
                locale: "en".to_string(),
                name: "Express".to_string(),
            }],
            currency_code: "usd".to_string(),
            amount: Decimal::from_str(amount).expect("valid amount"),
            provider_id: None,
            allowed_shipping_profile_slugs: None,
            metadata: serde_json::json!({"source": "admin-create-idempotency-test"}),
        },
    }
}


fn update_request(amount: &str) -> UpdateAdminShippingOptionRequest {
    UpdateAdminShippingOptionRequest {
        shipping_option_id: Uuid::nil(),
        input: UpdateShippingOptionInput {
            translations: None,
            expected_translation_revision: None,
            currency_code: Some("usd".to_string()),
            amount: Some(Decimal::from_str(amount).expect("valid amount")),
            provider_id: None,
            allowed_shipping_profile_slugs: None,
            metadata: Some(serde_json::json!({"updated": true})),
        },
    }
}

fn command_context(tenant_id: Uuid, correlation_id: &str, key: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        correlation_id,
    )
    .with_idempotency_key(key)
    .with_deadline(Duration::from_secs(5))
}

#[tokio::test]
async fn admin_shipping_option_create_replays_atomically_and_binds_key_to_request() {
    let (db, runtime) = setup().await;
    let tenant_id = Uuid::new_v4();
    let request = create_request("12.50");

    let first = runtime
        .command_port()
        .create_shipping_option(
            command_context(tenant_id, "shipping-option-create-1", "shipping-option-key-1"),
            request.clone(),
        )
        .await
        .expect("first create should succeed");

    let replay = runtime
        .command_port()
        .create_shipping_option(
            command_context(tenant_id, "shipping-option-create-replay", "shipping-option-key-1"),
            request,
        )
        .await
        .expect("same-key retry should replay");

    assert_eq!(first.id, replay.id);
    assert_eq!(first.translation_revision, replay.translation_revision);
    assert_eq!(first.updated_at, replay.updated_at);

    let owner = FulfillmentService::new(db.clone());
    let options = owner
        .list_all_shipping_options(tenant_id, Some("en"), None)
        .await
        .expect("tenant shipping options should be readable");
    assert_eq!(options.len(), 1, "same-key retry must not create a duplicate");

    let reused = runtime
        .command_port()
        .create_shipping_option(
            command_context(tenant_id, "shipping-option-create-conflict", "shipping-option-key-1"),
            create_request("19.00"),
        )
        .await
        .expect_err("reusing a key with a changed request must fail");

    assert_eq!(reused.kind, PortErrorKind::Conflict);

    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS count FROM owner_operation_receipts WHERE tenant_id = ? AND owner_slug = ? AND idempotency_key = ? AND operation = ? AND status = ?",
            vec![
                tenant_id.into(),
                "fulfillment".to_string().into(),
                "shipping-option-key-1".to_string().into(),
                "create_admin_shipping_option".to_string().into(),
                "completed".to_string().into(),
            ],
        ))
        .await
        .expect("completed receipt query should succeed")
        .expect("completed receipt count row should exist");
    let receipt_count: i64 = row.try_get("", "count").expect("count should be an integer");
    assert_eq!(receipt_count, 1);
}

#[tokio::test]
async fn admin_shipping_option_update_replays_and_rejects_changed_payload() {
    let (db, runtime) = setup().await;
    let tenant_id = Uuid::new_v4();
    let created = runtime
        .command_port()
        .create_shipping_option(
            command_context(tenant_id, "shipping-option-update-create", "shipping-option-create-2"),
            create_request("12.50"),
        )
        .await
        .expect("create should succeed");

    let mut request = update_request("19.00");
    request.shipping_option_id = created.id;

    let first = runtime
        .command_port()
        .update_shipping_option(
            command_context(tenant_id, "shipping-option-update-1", "shipping-option-update-key-1"),
            request.clone(),
        )
        .await
        .expect("update should succeed");
    let replay = runtime
        .command_port()
        .update_shipping_option(
            command_context(tenant_id, "shipping-option-update-replay", "shipping-option-update-key-1"),
            request,
        )
        .await
        .expect("same-key update should replay");

    assert_eq!(first.id, replay.id);
    assert_eq!(first.amount, replay.amount);
    assert_eq!(first.updated_at, replay.updated_at);

    let mut conflict_request = update_request("21.00");
    conflict_request.shipping_option_id = created.id;
    let error = runtime
        .command_port()
        .update_shipping_option(
            command_context(tenant_id, "shipping-option-update-conflict", "shipping-option-update-key-1"),
            conflict_request,
        )
        .await
        .expect_err("changed update payload must conflict on the reused key");
    assert_eq!(error.kind, PortErrorKind::Conflict);

    let owner = FulfillmentService::new(db);
    let current = owner
        .get_shipping_option(tenant_id, created.id, Some("en"), None)
        .await
        .expect("updated shipping option should exist");
    assert_eq!(current.amount, Decimal::from_str("19.00").unwrap());
}

#[tokio::test]
async fn admin_shipping_option_state_commands_replay_and_cross_operation_key_reuse_conflicts() {
    let (_db, runtime) = setup().await;
    let tenant_id = Uuid::new_v4();
    let created = runtime
        .command_port()
        .create_shipping_option(
            command_context(tenant_id, "shipping-option-state-create", "shipping-option-create-3"),
            create_request("10.00"),
        )
        .await
        .expect("create should succeed");

    let deactivate = DeactivateAdminShippingOptionRequest {
        shipping_option_id: created.id,
    };
    let first_deactivate = runtime
        .command_port()
        .deactivate_shipping_option(
            command_context(tenant_id, "shipping-option-deactivate-1", "shipping-option-state-key-1"),
            deactivate.clone(),
        )
        .await
        .expect("deactivation should succeed");
    let replay_deactivate = runtime
        .command_port()
        .deactivate_shipping_option(
            command_context(tenant_id, "shipping-option-deactivate-replay", "shipping-option-state-key-1"),
            deactivate,
        )
        .await
        .expect("same-key deactivation should replay");
    assert!(!first_deactivate.active);
    assert_eq!(first_deactivate.id, replay_deactivate.id);
    assert_eq!(first_deactivate.updated_at, replay_deactivate.updated_at);
    assert!(!replay_deactivate.active);

    let cross_operation = runtime
        .command_port()
        .reactivate_shipping_option(
            command_context(tenant_id, "shipping-option-cross-operation", "shipping-option-state-key-1"),
            ReactivateAdminShippingOptionRequest {
                shipping_option_id: created.id,
            },
        )
        .await
        .expect_err("one idempotency key must not cross operation identities");
    assert_eq!(cross_operation.kind, PortErrorKind::Conflict);

    let reactivate = ReactivateAdminShippingOptionRequest {
        shipping_option_id: created.id,
    };
    let first_reactivate = runtime
        .command_port()
        .reactivate_shipping_option(
            command_context(tenant_id, "shipping-option-reactivate-1", "shipping-option-state-key-2"),
            reactivate.clone(),
        )
        .await
        .expect("reactivation should succeed");
    let replay_reactivate = runtime
        .command_port()
        .reactivate_shipping_option(
            command_context(tenant_id, "shipping-option-reactivate-replay", "shipping-option-state-key-2"),
            reactivate,
        )
        .await
        .expect("same-key reactivation should replay");
    assert!(first_reactivate.active);
    assert_eq!(first_reactivate.id, replay_reactivate.id);
    assert_eq!(first_reactivate.updated_at, replay_reactivate.updated_at);
    assert!(replay_reactivate.active);
}
