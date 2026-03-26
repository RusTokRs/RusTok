use rust_decimal::Decimal;
use rustok_commerce::dto::{AuthorizePaymentInput, CancelPaymentInput, CreateCartInput};
use rustok_commerce::{CartService, PaymentService};
use rustok_test_utils::db::setup_test_db;
use std::str::FromStr;
use uuid::Uuid;

mod support;

async fn setup() -> (CartService, PaymentService) {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    (CartService::new(db.clone()), PaymentService::new(db))
}

#[tokio::test]
async fn payment_collection_reuse_by_cart_matches_storefront_retry_semantics() {
    let (cart_service, payment_service) = setup().await;
    let tenant_id = Uuid::new_v4();

    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: Some(Uuid::new_v4()),
                email: Some("buyer@example.com".to_string()),
                region_id: None,
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: None,
                currency_code: "eur".to_string(),
                metadata: serde_json::json!({ "source": "payment-retry-test" }),
            },
        )
        .await
        .unwrap();

    let pending = payment_service
        .create_collection(
            tenant_id,
            rustok_commerce::CreatePaymentCollectionInput {
                cart_id: Some(cart.id),
                order_id: None,
                customer_id: cart.customer_id,
                currency_code: cart.currency_code.clone(),
                amount: Decimal::from_str("19.99").expect("valid decimal"),
                metadata: serde_json::json!({ "source": "payment-retry-test" }),
            },
        )
        .await
        .unwrap();
    let reusable_pending = payment_service
        .find_reusable_collection_by_cart(tenant_id, cart.id)
        .await
        .unwrap()
        .expect("pending payment collection should be reusable");
    assert_eq!(reusable_pending.id, pending.id);
    assert_eq!(reusable_pending.status, "pending");

    let authorized = payment_service
        .authorize_collection(
            tenant_id,
            pending.id,
            AuthorizePaymentInput {
                provider_id: None,
                provider_payment_id: None,
                amount: None,
                metadata: serde_json::json!({ "authorized": true }),
            },
        )
        .await
        .unwrap();
    let reusable_authorized = payment_service
        .find_reusable_collection_by_cart(tenant_id, cart.id)
        .await
        .unwrap()
        .expect("authorized payment collection should stay reusable");
    assert_eq!(reusable_authorized.id, authorized.id);
    assert_eq!(reusable_authorized.status, "authorized");

    let cancelled = payment_service
        .cancel_collection(
            tenant_id,
            authorized.id,
            CancelPaymentInput {
                reason: Some("customer restarted checkout".to_string()),
                metadata: serde_json::json!({ "cancelled": true }),
            },
        )
        .await
        .unwrap();
    assert_eq!(cancelled.status, "cancelled");
    assert!(
        payment_service
            .find_reusable_collection_by_cart(tenant_id, cart.id)
            .await
            .unwrap()
            .is_none(),
        "cancelled payment collection must not be reused for storefront retries"
    );
}
