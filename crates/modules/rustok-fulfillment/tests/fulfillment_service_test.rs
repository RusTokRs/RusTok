use rust_decimal::Decimal;
use rustok_fulfillment::dto::{
    CancelFulfillmentInput, CreateFulfillmentInput, CreateFulfillmentItemInput,
    CreateShippingOptionInput, DeliverFulfillmentInput, FulfillmentItemQuantityInput,
    ReopenFulfillmentInput, ReshipFulfillmentInput, ShipFulfillmentInput,
    ShippingOptionTranslationInput, UpdateShippingOptionInput,
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
        translations: vec![ShippingOptionTranslationInput {
            locale: "en".to_string(),
            name: "Standard Shipping".to_string(),
        }],
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

    let listed = service
        .list_shipping_options(tenant_id, Some("en"), Some("en"))
        .await
        .unwrap();
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
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "Bulky Freight".to_string(),
                }],
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
                translations: Some(vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "Freight".to_string(),
                }]),
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
    assert!(
        service
            .list_shipping_options(tenant_id, Some("en"), Some("en"))
            .await
            .expect("active shipping options should load")
            .is_empty()
    );
    let all_options = service
        .list_all_shipping_options(tenant_id, Some("en"), Some("en"))
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
            .list_shipping_options(tenant_id, Some("en"), Some("en"))
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
                items: None,
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
                items: None,
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
                items: None,
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
                items: None,
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
                items: None,
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
                items: None,
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
                items: None,
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
                items: None,
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

#[tokio::test]
async fn create_fulfillment_persists_typed_items() {
    let service = setup().await;
    let tenant_id = Uuid::new_v4();
    let order_line_item_id = Uuid::new_v4();

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
                    order_line_item_id,
                    quantity: 2,
                    metadata: serde_json::json!({
                        "source_cart_line_item_id": Uuid::new_v4(),
                        "shipping_profile_slug": "default"
                    }),
                }]),
                metadata: serde_json::json!({ "source": "typed-fulfillment-items" }),
            },
        )
        .await
        .expect("fulfillment should be created");

    assert_eq!(created.items.len(), 1);
    assert_eq!(created.items[0].order_line_item_id, order_line_item_id);
    assert_eq!(created.items[0].quantity, 2);
    assert_eq!(created.items[0].shipped_quantity, 0);
    assert_eq!(created.items[0].delivered_quantity, 0);
}

#[tokio::test]
async fn ship_and_deliver_fulfillment_support_partial_item_progress_and_audit() {
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
                items: Some(vec![CreateFulfillmentItemInput {
                    order_line_item_id: Uuid::new_v4(),
                    quantity: 3,
                    metadata: serde_json::json!({ "source": "partial-progress" }),
                }]),
                metadata: serde_json::json!({ "source": "partial-progress" }),
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
                tracking_number: "PARTIAL-1".to_string(),
                items: Some(vec![FulfillmentItemQuantityInput {
                    fulfillment_item_id: item_id,
                    quantity: 3,
                }]),
                metadata: serde_json::json!({ "step": "partial-ship" }),
            },
        )
        .await
        .expect("fulfillment should ship partially");
    assert_eq!(shipped.status, "shipped");
    assert_eq!(shipped.items[0].shipped_quantity, 3);
    assert_eq!(shipped.items[0].delivered_quantity, 0);
    assert_eq!(
        shipped.metadata["audit"]["events"][0]["type"],
        serde_json::json!("ship")
    );
    assert_eq!(
        shipped.items[0].metadata["audit"]["events"][0]["quantity"],
        serde_json::json!(3)
    );

    let delivered_partial = service
        .deliver_fulfillment(
            tenant_id,
            created.id,
            DeliverFulfillmentInput {
                delivered_note: Some("partial".to_string()),
                items: Some(vec![FulfillmentItemQuantityInput {
                    fulfillment_item_id: item_id,
                    quantity: 1,
                }]),
                metadata: serde_json::json!({ "step": "partial-deliver" }),
            },
        )
        .await
        .expect("fulfillment should deliver partially");
    assert_eq!(delivered_partial.status, "shipped");
    assert_eq!(delivered_partial.items[0].shipped_quantity, 3);
    assert_eq!(delivered_partial.items[0].delivered_quantity, 1);

    let delivered_full = service
        .deliver_fulfillment(
            tenant_id,
            created.id,
            DeliverFulfillmentInput {
                delivered_note: Some("final".to_string()),
                items: Some(vec![FulfillmentItemQuantityInput {
                    fulfillment_item_id: item_id,
                    quantity: 2,
                }]),
                metadata: serde_json::json!({ "step": "final-deliver" }),
            },
        )
        .await
        .expect("remaining shipped quantity should be deliverable");
    assert_eq!(delivered_full.status, "delivered");
    assert_eq!(delivered_full.items[0].delivered_quantity, 3);
    assert_eq!(
        delivered_full.metadata["audit"]["events"][2]["type"],
        serde_json::json!("deliver")
    );
    assert!(
        delivered_full.metadata["audit"]["events"][2]
            .get("delivered_note")
            .is_none()
    );
}

#[tokio::test]
async fn deliver_fulfillment_rejects_quantity_beyond_shipped_progress() {
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
                items: Some(vec![CreateFulfillmentItemInput {
                    order_line_item_id: Uuid::new_v4(),
                    quantity: 3,
                    metadata: serde_json::json!({ "source": "deliver-overrun" }),
                }]),
                metadata: serde_json::json!({ "source": "deliver-overrun" }),
            },
        )
        .await
        .expect("fulfillment should be created");
    let item_id = created.items[0].id;

    service
        .ship_fulfillment(
            tenant_id,
            created.id,
            ShipFulfillmentInput {
                carrier: "manual".to_string(),
                tracking_number: "PARTIAL-2".to_string(),
                items: Some(vec![FulfillmentItemQuantityInput {
                    fulfillment_item_id: item_id,
                    quantity: 1,
                }]),
                metadata: serde_json::json!({ "step": "ship" }),
            },
        )
        .await
        .expect("fulfillment should ship");

    let error = service
        .deliver_fulfillment(
            tenant_id,
            created.id,
            DeliverFulfillmentInput {
                delivered_note: None,
                items: Some(vec![FulfillmentItemQuantityInput {
                    fulfillment_item_id: item_id,
                    quantity: 2,
                }]),
                metadata: serde_json::json!({ "step": "too-much-deliver" }),
            },
        )
        .await
        .unwrap_err();

    match error {
        FulfillmentError::Validation(message) => {
            assert!(message.contains("exceeds remaining quantity"));
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[tokio::test]
async fn reopen_delivered_fulfillment_restores_shipped_progress_and_audit() {
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
                items: Some(vec![CreateFulfillmentItemInput {
                    order_line_item_id: Uuid::new_v4(),
                    quantity: 3,
                    metadata: serde_json::json!({ "source": "reopen-progress" }),
                }]),
                metadata: serde_json::json!({ "source": "reopen-progress" }),
            },
        )
        .await
        .expect("fulfillment should be created");
    let item_id = created.items[0].id;

    let delivered = service
        .ship_fulfillment(
            tenant_id,
            created.id,
            ShipFulfillmentInput {
                carrier: "manual".to_string(),
                tracking_number: "REOPEN-1".to_string(),
                items: Some(vec![FulfillmentItemQuantityInput {
                    fulfillment_item_id: item_id,
                    quantity: 3,
                }]),
                metadata: serde_json::json!({ "step": "ship" }),
            },
        )
        .await
        .expect("fulfillment should ship");
    assert_eq!(delivered.status, "shipped");

    let delivered = service
        .deliver_fulfillment(
            tenant_id,
            created.id,
            DeliverFulfillmentInput {
                delivered_note: Some("done".to_string()),
                items: Some(vec![FulfillmentItemQuantityInput {
                    fulfillment_item_id: item_id,
                    quantity: 3,
                }]),
                metadata: serde_json::json!({ "step": "deliver" }),
            },
        )
        .await
        .expect("fulfillment should deliver");
    assert_eq!(delivered.status, "delivered");

    let reopened = service
        .reopen_fulfillment(
            tenant_id,
            created.id,
            ReopenFulfillmentInput {
                items: Some(vec![FulfillmentItemQuantityInput {
                    fulfillment_item_id: item_id,
                    quantity: 1,
                }]),
                metadata: serde_json::json!({ "step": "reopen" }),
            },
        )
        .await
        .expect("fulfillment should reopen");

    assert_eq!(reopened.status, "shipped");
    assert_eq!(reopened.items[0].shipped_quantity, 3);
    assert_eq!(reopened.items[0].delivered_quantity, 2);
    assert!(reopened.delivered_note.is_none());
    assert!(reopened.delivered_at.is_none());
    assert_eq!(
        reopened.metadata["audit"]["events"][2]["type"],
        serde_json::json!("reopen")
    );
    assert_eq!(
        reopened.items[0].metadata["audit"]["events"][2]["type"],
        serde_json::json!("reopen")
    );
}

#[tokio::test]
async fn reship_delivered_fulfillment_reopens_delivery_with_new_tracking() {
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
                items: Some(vec![CreateFulfillmentItemInput {
                    order_line_item_id: Uuid::new_v4(),
                    quantity: 2,
                    metadata: serde_json::json!({ "source": "reship-progress" }),
                }]),
                metadata: serde_json::json!({ "source": "reship-progress" }),
            },
        )
        .await
        .expect("fulfillment should be created");
    let item_id = created.items[0].id;

    service
        .ship_fulfillment(
            tenant_id,
            created.id,
            ShipFulfillmentInput {
                carrier: "manual".to_string(),
                tracking_number: "RESHIP-OLD".to_string(),
                items: Some(vec![FulfillmentItemQuantityInput {
                    fulfillment_item_id: item_id,
                    quantity: 2,
                }]),
                metadata: serde_json::json!({ "step": "ship" }),
            },
        )
        .await
        .expect("fulfillment should ship");
    service
        .deliver_fulfillment(
            tenant_id,
            created.id,
            DeliverFulfillmentInput {
                delivered_note: Some("done".to_string()),
                items: Some(vec![FulfillmentItemQuantityInput {
                    fulfillment_item_id: item_id,
                    quantity: 2,
                }]),
                metadata: serde_json::json!({ "step": "deliver" }),
            },
        )
        .await
        .expect("fulfillment should deliver");

    let reshipped = service
        .reship_fulfillment(
            tenant_id,
            created.id,
            ReshipFulfillmentInput {
                carrier: "manual".to_string(),
                tracking_number: "RESHIP-NEW".to_string(),
                items: Some(vec![FulfillmentItemQuantityInput {
                    fulfillment_item_id: item_id,
                    quantity: 2,
                }]),
                metadata: serde_json::json!({ "step": "reship" }),
            },
        )
        .await
        .expect("fulfillment should reship");

    assert_eq!(reshipped.status, "shipped");
    assert_eq!(reshipped.tracking_number.as_deref(), Some("RESHIP-NEW"));
    assert_eq!(reshipped.items[0].shipped_quantity, 2);
    assert_eq!(reshipped.items[0].delivered_quantity, 0);
    assert!(reshipped.delivered_note.is_none());
    assert!(reshipped.delivered_at.is_none());
    assert_eq!(
        reshipped.metadata["audit"]["events"][2]["type"],
        serde_json::json!("reship")
    );
    assert_eq!(
        reshipped.items[0].metadata["audit"]["events"][2]["type"],
        serde_json::json!("reship")
    );
}

#[tokio::test]
async fn reopen_cancelled_fulfillment_restores_pending_or_shipped_state() {
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
                items: Some(vec![CreateFulfillmentItemInput {
                    order_line_item_id: Uuid::new_v4(),
                    quantity: 1,
                    metadata: serde_json::json!({ "source": "cancelled-reopen" }),
                }]),
                metadata: serde_json::json!({ "source": "cancelled-reopen" }),
            },
        )
        .await
        .expect("fulfillment should be created");
    let item_id = created.items[0].id;

    service
        .ship_fulfillment(
            tenant_id,
            created.id,
            ShipFulfillmentInput {
                carrier: "manual".to_string(),
                tracking_number: "CANCELLED-REOPEN".to_string(),
                items: Some(vec![FulfillmentItemQuantityInput {
                    fulfillment_item_id: item_id,
                    quantity: 1,
                }]),
                metadata: serde_json::json!({ "step": "ship" }),
            },
        )
        .await
        .expect("fulfillment should ship");
    let cancelled = service
        .cancel_fulfillment(
            tenant_id,
            created.id,
            CancelFulfillmentInput {
                reason: Some("manual".to_string()),
                metadata: serde_json::json!({ "step": "cancel" }),
            },
        )
        .await
        .expect("fulfillment should cancel");
    assert_eq!(cancelled.status, "cancelled");

    let reopened = service
        .reopen_fulfillment(
            tenant_id,
            created.id,
            ReopenFulfillmentInput {
                items: None,
                metadata: serde_json::json!({ "step": "reopen" }),
            },
        )
        .await
        .expect("cancelled fulfillment should reopen");

    assert_eq!(reopened.status, "shipped");
    assert!(reopened.cancellation_reason.is_none());
    assert!(reopened.cancelled_at.is_none());
    assert_eq!(
        reopened.metadata["audit"]["events"][2]["type"],
        serde_json::json!("reopen")
    );
}
