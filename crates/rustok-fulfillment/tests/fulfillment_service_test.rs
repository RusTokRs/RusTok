use rust_decimal::Decimal;
use rustok_fulfillment::dto::{
    CancelFulfillmentInput, CreateFulfillmentInput, CreateShippingOptionInput,
    DeliverFulfillmentInput, ShipFulfillmentInput, UpdateShippingOptionInput,
};
use rustok_fulfillment::error::FulfillmentError;
use rustok_fulfillment::services::FulfillmentService;
use rustok_test_utils::db::setup_test_db;
use std::str::FromStr;
use uuid::Uuid;

mod support;

async fn setup() -> FulfillmentService {
    let db = setup_test_db().await;
    support::ensure_fulfillment_schema(&db).await;
    FulfillmentService::new(db)
}

fn create_shipping_option_input() -> CreateShippingOptionInput {
    CreateShippingOptionInput {
        name: "Standard Shipping".to_string(),
        currency_code: "usd".to_string(),
        amount: Decimal::from_str("9.99").expect("valid decimal"),
        provider_id: None,
        allowed_shipping_profile_slugs: None,
        metadata: serde_json::json!({ "source": "fulfillment-test" }),
    }
}

#[tokio::test]
async fn create_and_list_shipping_options() {
    let service = setup().await;
    let tenant_id = Uuid::new_v4();

    let created = service
        .create_shipping_option(tenant_id, create_shipping_option_input())
        .await
        .unwrap();
    assert_eq!(created.name, "Standard Shipping");

    let listed = service.list_shipping_options(tenant_id).await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, created.id);
}

#[tokio::test]
async fn create_shipping_option_normalizes_allowed_shipping_profile_slugs() {
    let service = setup().await;
    let tenant_id = Uuid::new_v4();

    let created = service
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                name: "Bulky Freight".to_string(),
                currency_code: "eur".to_string(),
                amount: Decimal::from_str("29.99").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: Some(vec![
                    " Bulky ".to_string(),
                    "cold-chain".to_string(),
                    "bulky".to_string(),
                ]),
                metadata: serde_json::json!({ "source": "typed-shipping-profiles" }),
            },
        )
        .await
        .expect("shipping option should be created");

    assert_eq!(
        created.allowed_shipping_profile_slugs,
        Some(vec!["bulky".to_string(), "cold-chain".to_string()])
    );
    assert_eq!(
        created.metadata["shipping_profiles"]["allowed_slugs"],
        serde_json::json!(["bulky", "cold-chain"])
    );
}

#[tokio::test]
async fn update_shipping_option_normalizes_allowed_shipping_profile_slugs() {
    let service = setup().await;
    let tenant_id = Uuid::new_v4();
    let created = service
        .create_shipping_option(tenant_id, create_shipping_option_input())
        .await
        .expect("shipping option should be created");

    let updated = service
        .update_shipping_option(
            tenant_id,
            created.id,
            UpdateShippingOptionInput {
                name: Some("Freight".to_string()),
                currency_code: Some("eur".to_string()),
                amount: Some(Decimal::from_str("14.99").expect("valid decimal")),
                provider_id: Some(" custom-provider ".to_string()),
                allowed_shipping_profile_slugs: Some(vec![
                    " bulky ".to_string(),
                    "cold-chain".to_string(),
                    "bulky".to_string(),
                ]),
                metadata: Some(serde_json::json!({ "updated": true })),
            },
        )
        .await
        .expect("shipping option should be updated");

    assert_eq!(updated.name, "Freight");
    assert_eq!(updated.currency_code, "EUR");
    assert_eq!(
        updated.amount,
        Decimal::from_str("14.99").expect("valid decimal")
    );
    assert_eq!(updated.provider_id, "custom-provider");
    assert_eq!(
        updated.allowed_shipping_profile_slugs,
        Some(vec!["bulky".to_string(), "cold-chain".to_string()])
    );
    assert_eq!(updated.metadata["updated"], serde_json::json!(true));
    assert_eq!(
        updated.metadata["shipping_profiles"]["allowed_slugs"],
        serde_json::json!(["bulky", "cold-chain"])
    );
}

#[tokio::test]
async fn deactivate_and_reactivate_shipping_option_changes_admin_visibility() {
    let service = setup().await;
    let tenant_id = Uuid::new_v4();
    let created = service
        .create_shipping_option(tenant_id, create_shipping_option_input())
        .await
        .expect("shipping option should be created");

    let deactivated = service
        .deactivate_shipping_option(tenant_id, created.id)
        .await
        .expect("shipping option should be deactivated");
    assert!(!deactivated.active);
    assert!(service
        .list_shipping_options(tenant_id)
        .await
        .expect("active shipping options should load")
        .is_empty());
    let all_options = service
        .list_all_shipping_options(tenant_id)
        .await
        .expect("all shipping options should load");
    assert_eq!(all_options.len(), 1);
    assert!(!all_options[0].active);

    let reactivated = service
        .reactivate_shipping_option(tenant_id, created.id)
        .await
        .expect("shipping option should be reactivated");
    assert!(reactivated.active);
    assert_eq!(
        service
            .list_shipping_options(tenant_id)
            .await
            .expect("active shipping options should load")
            .len(),
        1
    );
}

#[tokio::test]
async fn create_ship_and_deliver_fulfillment() {
    let service = setup().await;
    let tenant_id = Uuid::new_v4();
    let shipping_option = service
        .create_shipping_option(tenant_id, create_shipping_option_input())
        .await
        .unwrap();

    let created = service
        .create_fulfillment(
            tenant_id,
            CreateFulfillmentInput {
                order_id: Uuid::new_v4(),
                shipping_option_id: Some(shipping_option.id),
                customer_id: Some(Uuid::new_v4()),
                carrier: None,
                tracking_number: None,
                metadata: serde_json::json!({ "step": "created" }),
            },
        )
        .await
        .unwrap();
    assert_eq!(created.status, "pending");

    let shipped = service
        .ship_fulfillment(
            tenant_id,
            created.id,
            ShipFulfillmentInput {
                carrier: "dhl".to_string(),
                tracking_number: "trk_123".to_string(),
                metadata: serde_json::json!({ "step": "shipped" }),
            },
        )
        .await
        .unwrap();
    assert_eq!(shipped.status, "shipped");

    let delivered = service
        .deliver_fulfillment(
            tenant_id,
            created.id,
            DeliverFulfillmentInput {
                delivered_note: Some("front-desk".to_string()),
                metadata: serde_json::json!({ "step": "delivered" }),
            },
        )
        .await
        .unwrap();
    assert_eq!(delivered.status, "delivered");
    assert_eq!(delivered.delivered_note.as_deref(), Some("front-desk"));
}

#[tokio::test]
async fn cancel_pending_fulfillment() {
    let service = setup().await;
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
                metadata: serde_json::json!({}),
            },
        )
        .await
        .unwrap();
    let cancelled = service
        .cancel_fulfillment(
            tenant_id,
            created.id,
            CancelFulfillmentInput {
                reason: Some("buyer-requested".to_string()),
                metadata: serde_json::json!({ "step": "cancelled" }),
            },
        )
        .await
        .unwrap();
    assert_eq!(cancelled.status, "cancelled");
}

#[tokio::test]
async fn deliver_requires_shipped_state() {
    let service = setup().await;
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
                metadata: serde_json::json!({}),
            },
        )
        .await
        .unwrap();
    let error = service
        .deliver_fulfillment(
            tenant_id,
            created.id,
            DeliverFulfillmentInput {
                delivered_note: None,
                metadata: serde_json::json!({}),
            },
        )
        .await
        .unwrap_err();

    match error {
        FulfillmentError::InvalidTransition { from, to } => {
            assert_eq!(from, "pending");
            assert_eq!(to, "delivered");
        }
        other => panic!("expected invalid transition, got {other:?}"),
    }
}

#[tokio::test]
async fn find_by_order_returns_latest_fulfillment() {
    let service = setup().await;
    let tenant_id = Uuid::new_v4();
    let order_id = Uuid::new_v4();

    let first = service
        .create_fulfillment(
            tenant_id,
            CreateFulfillmentInput {
                order_id,
                shipping_option_id: None,
                customer_id: Some(Uuid::new_v4()),
                carrier: None,
                tracking_number: None,
                metadata: serde_json::json!({ "attempt": 1 }),
            },
        )
        .await
        .unwrap();
    let second = service
        .create_fulfillment(
            tenant_id,
            CreateFulfillmentInput {
                order_id,
                shipping_option_id: None,
                customer_id: Some(Uuid::new_v4()),
                carrier: None,
                tracking_number: None,
                metadata: serde_json::json!({ "attempt": 2 }),
            },
        )
        .await
        .unwrap();

    let found = service
        .find_by_order(tenant_id, order_id)
        .await
        .unwrap()
        .expect("expected fulfillment");
    assert_eq!(found.id, second.id);
    assert_ne!(found.id, first.id);
}
