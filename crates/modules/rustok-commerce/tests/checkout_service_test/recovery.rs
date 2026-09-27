use super::*;

#[tokio::test]
async fn repeated_complete_checkout_recovers_existing_result() {
    let (db, cart_service, checkout, fulfillment) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    let region = RegionService::new(db.clone())
        .create_region(
            tenant_id,
            CreateRegionInput {
                translations: vec![RegionTranslationInput {
                    locale: "en".to_string(),
                    name: "Europe".to_string(),
                }],
                currency_code: "usd".to_string(),
                tax_provider_id: None,
                tax_rate: Decimal::from_str("20.00").expect("valid decimal"),
                tax_included: true,
                country_tax_policies: None,
                countries: vec!["de".to_string()],
                metadata: serde_json::json!({ "source": "checkout-retry-test" }),
            },
        )
        .await
        .unwrap();
    let shipping_option = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "Standard".to_string(),
                }],
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: None,
                metadata: serde_json::json!({ "source": "checkout-retry-test" }),
            },
        )
        .await
        .unwrap();

    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: Some(Uuid::new_v4()),
                email: Some("buyer@example.com".to_string()),
                region_id: Some(region.id),
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: Some(shipping_option.id),
                currency_code: "usd".to_string(),
                metadata: serde_json::json!({ "source": "checkout-retry-test" }),
            },
        )
        .await
        .unwrap();
    let cart = cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: None,
                variant_id: None,
                fulfillment_requirement: CartLineFulfillmentRequirement::Physical,
                shipping_profile_slug: None,
                sku: Some("CHK-RETRY-1".to_string()),
                title: "Checkout Retry Product".to_string(),
                quantity: 1,
                unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                metadata: serde_json::json!({ "slot": 1 }),
            },
        )
        .await
        .unwrap();

    let first = checkout
        .complete_checkout(
            tenant_id,
            actor_id,
            CompleteCheckoutInput {
                cart_id: cart.id,
                shipping_option_id: None,
                shipping_selections: None,
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: true,
                metadata: serde_json::json!({ "flow": "checkout-retry-test" }),
            },
        )
        .await
        .unwrap();

    let second = checkout
        .complete_checkout(
            tenant_id,
            actor_id,
            CompleteCheckoutInput {
                cart_id: cart.id,
                shipping_option_id: None,
                shipping_selections: None,
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: true,
                metadata: serde_json::json!({ "flow": "checkout-retry-test" }),
            },
        )
        .await
        .unwrap();

    assert_eq!(first.cart.id, second.cart.id);
    assert_eq!(first.order.id, second.order.id);
    assert_eq!(first.payment_collection.id, second.payment_collection.id);
    assert_eq!(
        first.fulfillment.as_ref().map(|value| value.id),
        second.fulfillment.as_ref().map(|value| value.id)
    );
}

#[tokio::test]
async fn complete_checkout_recovers_stuck_checking_out_cart_when_paid_artifacts_exist() {
    let (db, cart_service, checkout, fulfillment) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    let region = RegionService::new(db.clone())
        .create_region(
            tenant_id,
            CreateRegionInput {
                translations: vec![RegionTranslationInput {
                    locale: "en".to_string(),
                    name: "Europe".to_string(),
                }],
                currency_code: "usd".to_string(),
                tax_provider_id: None,
                tax_rate: Decimal::from_str("20.00").expect("valid decimal"),
                tax_included: true,
                country_tax_policies: None,
                countries: vec!["de".to_string()],
                metadata: serde_json::json!({ "source": "checkout-recovery-test" }),
            },
        )
        .await
        .unwrap();
    let shipping_option = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "Standard".to_string(),
                }],
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: None,
                metadata: serde_json::json!({ "source": "checkout-recovery-test" }),
            },
        )
        .await
        .unwrap();

    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: Some(Uuid::new_v4()),
                email: Some("buyer@example.com".to_string()),
                region_id: Some(region.id),
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: Some(shipping_option.id),
                currency_code: "usd".to_string(),
                metadata: serde_json::json!({ "source": "checkout-recovery-test" }),
            },
        )
        .await
        .unwrap();
    let cart = cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: None,
                variant_id: None,
                fulfillment_requirement: CartLineFulfillmentRequirement::Physical,
                shipping_profile_slug: None,
                sku: Some("CHK-RECOVER-1".to_string()),
                title: "Checkout Recovery Product".to_string(),
                quantity: 1,
                unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                metadata: serde_json::json!({ "slot": 1 }),
            },
        )
        .await
        .unwrap();

    let first = checkout
        .complete_checkout(
            tenant_id,
            actor_id,
            CompleteCheckoutInput {
                cart_id: cart.id,
                shipping_option_id: None,
                shipping_selections: None,
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: true,
                metadata: serde_json::json!({ "flow": "checkout-recovery-test" }),
            },
        )
        .await
        .unwrap();

    db.execute_raw(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "UPDATE carts SET status = ?, completed_at = NULL WHERE id = ? AND tenant_id = ?",
        vec!["checking_out".into(), cart.id.into(), tenant_id.into()],
    ))
    .await
    .unwrap();

    let recovered = checkout
        .complete_checkout(
            tenant_id,
            actor_id,
            CompleteCheckoutInput {
                cart_id: cart.id,
                shipping_option_id: None,
                shipping_selections: None,
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: true,
                metadata: serde_json::json!({ "flow": "checkout-recovery-test" }),
            },
        )
        .await
        .unwrap();

    assert_eq!(recovered.cart.status, "completed");
    assert!(recovered.cart.completed_at.is_some());
    assert_eq!(first.cart.id, recovered.cart.id);
    assert_eq!(first.order.id, recovered.order.id);
    assert_eq!(first.payment_collection.id, recovered.payment_collection.id);
    assert_eq!(
        first.fulfillment.as_ref().map(|value| value.id),
        recovered.fulfillment.as_ref().map(|value| value.id)
    );
}

#[tokio::test]
async fn complete_checkout_rejects_reentry_for_checking_out_cart_without_artifacts() {
    let (db, cart_service, checkout, fulfillment) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    let region = RegionService::new(db.clone())
        .create_region(
            tenant_id,
            CreateRegionInput {
                translations: vec![RegionTranslationInput {
                    locale: "en".to_string(),
                    name: "Europe".to_string(),
                }],
                currency_code: "usd".to_string(),
                tax_provider_id: None,
                tax_rate: Decimal::from_str("20.00").expect("valid decimal"),
                tax_included: true,
                country_tax_policies: None,
                countries: vec!["de".to_string()],
                metadata: serde_json::json!({ "source": "checkout-reentry-guard-test" }),
            },
        )
        .await
        .unwrap();
    let shipping_option = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "Standard".to_string(),
                }],
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: None,
                metadata: serde_json::json!({ "source": "checkout-reentry-guard-test" }),
            },
        )
        .await
        .unwrap();

    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: Some(Uuid::new_v4()),
                email: Some("buyer@example.com".to_string()),
                region_id: Some(region.id),
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: Some(shipping_option.id),
                currency_code: "usd".to_string(),
                metadata: serde_json::json!({ "source": "checkout-reentry-guard-test" }),
            },
        )
        .await
        .unwrap();
    let cart = cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: None,
                variant_id: None,
                fulfillment_requirement: CartLineFulfillmentRequirement::Physical,
                shipping_profile_slug: None,
                sku: Some("CHK-REENTRY-1".to_string()),
                title: "Checkout Reentry Guard Product".to_string(),
                quantity: 1,
                unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                metadata: serde_json::json!({ "slot": 1 }),
            },
        )
        .await
        .unwrap();

    let checking_out = cart_service
        .begin_checkout(tenant_id, cart.id)
        .await
        .unwrap();
    assert_eq!(checking_out.status, "checking_out");

    let error = checkout
        .complete_checkout(
            tenant_id,
            actor_id,
            CompleteCheckoutInput {
                cart_id: cart.id,
                shipping_option_id: None,
                shipping_selections: None,
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: true,
                metadata: serde_json::json!({ "flow": "checkout-reentry-guard-test" }),
            },
        )
        .await
        .expect_err("re-entry from checking_out without artifacts must fail");

    match error {
        CheckoutError::CheckoutInProgress(cart_id) => {
            assert_eq!(cart_id, cart.id);
        }
        other => panic!("expected checkout-in-progress guard, got {other:?}"),
    }

    let cart_after = cart_service.get_cart(tenant_id, cart.id).await.unwrap();
    assert_eq!(cart_after.status, "checking_out");
    assert!(cart_after.completed_at.is_none());
}

#[tokio::test]
async fn checkout_failure_releases_cart_back_to_active() {
    let (db, cart_service, checkout, _) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    let region = RegionService::new(db.clone())
        .create_region(
            tenant_id,
            CreateRegionInput {
                translations: vec![RegionTranslationInput {
                    locale: "en".to_string(),
                    name: "Europe".to_string(),
                }],
                currency_code: "usd".to_string(),
                tax_provider_id: None,
                tax_rate: Decimal::from_str("20.00").expect("valid decimal"),
                tax_included: true,
                country_tax_policies: None,
                countries: vec!["de".to_string()],
                metadata: serde_json::json!({ "source": "checkout-lock-release-test" }),
            },
        )
        .await
        .unwrap();

    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: Some(Uuid::new_v4()),
                email: Some("buyer@example.com".to_string()),
                region_id: Some(region.id),
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: None,
                currency_code: "usd".to_string(),
                metadata: serde_json::json!({ "source": "checkout-lock-release-test" }),
            },
        )
        .await
        .unwrap();
    let cart = cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: None,
                variant_id: None,
                fulfillment_requirement: CartLineFulfillmentRequirement::Physical,
                shipping_profile_slug: None,
                sku: Some("CHK-LOCK-1".to_string()),
                title: "Checkout Lock Product".to_string(),
                quantity: 1,
                unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                metadata: serde_json::json!({ "slot": 1 }),
            },
        )
        .await
        .unwrap();

    let error = checkout
        .complete_checkout(
            tenant_id,
            actor_id,
            CompleteCheckoutInput {
                cart_id: cart.id,
                shipping_option_id: Some(Uuid::new_v4()),
                shipping_selections: None,
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: true,
                metadata: serde_json::json!({ "flow": "checkout-lock-release-test" }),
            },
        )
        .await
        .expect_err("invalid shipping option must fail checkout");

    match error {
        CheckoutError::StageFailure { stage, .. } => {
            assert_eq!(stage, "load_shipping_option");
        }
        other => panic!("expected stage failure, got {other:?}"),
    }

    let cart_after = cart_service.get_cart(tenant_id, cart.id).await.unwrap();
    assert_eq!(cart_after.status, "active");
    assert!(cart_after.completed_at.is_none());
}

#[tokio::test]
async fn checkout_preflight_failure_does_not_create_payment_or_order_artifacts() {
    let (db, cart_service, checkout, _) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    let region = RegionService::new(db.clone())
        .create_region(
            tenant_id,
            CreateRegionInput {
                translations: vec![RegionTranslationInput {
                    locale: "en".to_string(),
                    name: "Europe".to_string(),
                }],
                currency_code: "usd".to_string(),
                tax_provider_id: None,
                tax_rate: Decimal::from_str("20.00").expect("valid decimal"),
                tax_included: true,
                country_tax_policies: None,
                countries: vec!["de".to_string()],
                metadata: serde_json::json!({ "source": "checkout-compensation-test" }),
            },
        )
        .await
        .unwrap();

    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: Some(Uuid::new_v4()),
                email: Some("buyer@example.com".to_string()),
                region_id: Some(region.id),
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: None,
                currency_code: "usd".to_string(),
                metadata: serde_json::json!({ "source": "checkout-compensation-test" }),
            },
        )
        .await
        .unwrap();
    let cart = cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: None,
                variant_id: None,
                fulfillment_requirement: CartLineFulfillmentRequirement::Physical,
                shipping_profile_slug: None,
                sku: Some("CHK-COMP-1".to_string()),
                title: "Checkout Compensation Product".to_string(),
                quantity: 1,
                unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                metadata: serde_json::json!({ "slot": 1 }),
            },
        )
        .await
        .unwrap();

    let error = checkout
        .complete_checkout(
            tenant_id,
            actor_id,
            CompleteCheckoutInput {
                cart_id: cart.id,
                shipping_option_id: Some(Uuid::new_v4()),
                shipping_selections: None,
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: true,
                metadata: serde_json::json!({ "flow": "checkout-compensation-test" }),
            },
        )
        .await
        .expect_err("invalid shipping option must trigger compensation");

    match error {
        CheckoutError::StageFailure { stage, .. } => assert_eq!(stage, "load_shipping_option"),
        other => panic!("expected stage failure, got {other:?}"),
    }

    let payment_collection = PaymentService::new(db)
        .find_latest_collection_by_cart(tenant_id, cart.id)
        .await
        .unwrap();
    assert!(
        payment_collection.is_none(),
        "preflight checkout failure should not create payment artifacts"
    );
}

#[tokio::test]
async fn retry_after_preflight_failure_creates_checkout_artifacts() {
    let (db, cart_service, checkout, fulfillment) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;
    let region = RegionService::new(db.clone())
        .create_region(
            tenant_id,
            CreateRegionInput {
                translations: vec![RegionTranslationInput {
                    locale: "en".to_string(),
                    name: "Europe".to_string(),
                }],
                currency_code: "usd".to_string(),
                tax_provider_id: None,
                tax_rate: Decimal::from_str("20.00").expect("valid decimal"),
                tax_included: true,
                country_tax_policies: None,
                countries: vec!["de".to_string()],
                metadata: serde_json::json!({ "source": "checkout-retry-after-failure-test" }),
            },
        )
        .await
        .unwrap();
    let shipping_option = fulfillment
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "Standard".to_string(),
                }],
                currency_code: "usd".to_string(),
                amount: Decimal::from_str("9.99").expect("valid decimal"),
                provider_id: None,
                allowed_shipping_profile_slugs: None,
                metadata: serde_json::json!({ "source": "checkout-retry-after-failure-test" }),
            },
        )
        .await
        .unwrap();

    let cart = cart_service
        .create_cart(
            tenant_id,
            CreateCartInput {
                customer_id: Some(Uuid::new_v4()),
                email: Some("buyer@example.com".to_string()),
                region_id: Some(region.id),
                country_code: Some("de".to_string()),
                locale_code: Some("de".to_string()),
                selected_shipping_option_id: None,
                currency_code: "usd".to_string(),
                metadata: serde_json::json!({ "source": "checkout-retry-after-failure-test" }),
            },
        )
        .await
        .unwrap();
    let cart = cart_service
        .add_line_item(
            tenant_id,
            cart.id,
            AddCartLineItemInput {
                product_id: None,
                variant_id: None,
                fulfillment_requirement: CartLineFulfillmentRequirement::Physical,
                shipping_profile_slug: None,
                sku: Some("CHK-RETRY-AFTER-FAIL-1".to_string()),
                title: "Checkout Retry After Failure Product".to_string(),
                quantity: 1,
                unit_price: Decimal::from_str("25.00").expect("valid decimal"),
                metadata: serde_json::json!({ "slot": 1 }),
            },
        )
        .await
        .unwrap();

    let first_error = checkout
        .complete_checkout(
            tenant_id,
            actor_id,
            CompleteCheckoutInput {
                cart_id: cart.id,
                shipping_option_id: Some(Uuid::new_v4()),
                shipping_selections: None,
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: true,
                metadata: serde_json::json!({ "flow": "checkout-retry-after-failure-test" }),
            },
        )
        .await
        .expect_err("first checkout must fail on invalid shipping option");

    match first_error {
        CheckoutError::StageFailure { stage, .. } => assert_eq!(stage, "load_shipping_option"),
        other => panic!("expected stage failure, got {other:?}"),
    }

    let failed_collection = PaymentService::new(db.clone())
        .find_latest_collection_by_cart(tenant_id, cart.id)
        .await
        .unwrap();
    assert!(
        failed_collection.is_none(),
        "preflight checkout failure should not create payment artifacts"
    );

    let retried = checkout
        .complete_checkout(
            tenant_id,
            actor_id,
            CompleteCheckoutInput {
                cart_id: cart.id,
                shipping_option_id: Some(shipping_option.id),
                shipping_selections: None,
                region_id: None,
                country_code: None,
                locale: None,
                create_fulfillment: true,
                metadata: serde_json::json!({ "flow": "checkout-retry-after-failure-test" }),
            },
        )
        .await
        .unwrap();

    assert_eq!(retried.cart.status, "completed");
    assert_eq!(retried.order.status, "paid");
    assert_eq!(retried.payment_collection.status, "captured");
}

#[tokio::test]
async fn checkout_operation_lease_renewal_and_fencing_prevents_expired_executor_side_effects() {
    let (db, _, _, _) = setup().await;
    let tenant_id = Uuid::new_v4();
    let cart_id = Uuid::new_v4();
    let journal = rustok_commerce::services::CheckoutOperationJournal::new(db.clone());

    let op = journal
        .begin(rustok_commerce::services::BeginCheckoutOperation {
            tenant_id,
            cart_id,
            idempotency_key: "lease-fencing-test".to_string(),
            request_hash: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            snapshot_hash: Some("abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789".to_string()),
        })
        .await
        .expect("operation created");

    // Executor A claims execution with 10 seconds lease
    let claimed_a = journal
        .claim_execution(tenant_id, op.id, "executor-A", 10)
        .await
        .expect("claim succeeds")
        .expect("claimed by A");
    assert_eq!(claimed_a.lease_owner.as_deref(), Some("executor-A"));

    // Executor A successfully renews lease while valid
    let renewed_a = journal
        .renew_lease(tenant_id, op.id, "executor-A", 30)
        .await
        .expect("A can renew valid lease");
    assert_eq!(renewed_a.lease_owner.as_deref(), Some("executor-A"));

    // Foreign executor B cannot renew A's lease
    let foreign_err = journal
        .renew_lease(tenant_id, op.id, "executor-B", 30)
        .await;
    assert!(foreign_err.is_err(), "executor B must not be able to renew executor A's lease");

    // Simulate lease expiration by setting lease_expires_at in the past
    use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};
    rustok_commerce::entities::checkout_operation::Entity::update_many()
        .col_expr(
            rustok_commerce::entities::checkout_operation::Column::LeaseExpiresAt,
            sea_orm::sea_query::Expr::value(chrono::Utc::now().fixed_offset() - chrono::Duration::seconds(10)),
        )
        .filter(rustok_commerce::entities::checkout_operation::Column::Id.eq(op.id))
        .exec(&db)
        .await
        .expect("lease expired");

    // Executor A's renew_lease now FAILS because the lease has expired (fencing)
    let expired_err = journal
        .renew_lease(tenant_id, op.id, "executor-A", 30)
        .await;
    assert!(expired_err.is_err(), "expired lease renewal MUST be rejected (fencing)");

    // New executor B can now reclaim execution
    let claimed_b = journal
        .claim_execution(tenant_id, op.id, "executor-B", 30)
        .await
        .expect("reclaim succeeds")
        .expect("claimed by B");
    assert_eq!(claimed_b.lease_owner.as_deref(), Some("executor-B"));

    // Stale executor A is completely locked out
    let stale_err = journal
        .renew_lease(tenant_id, op.id, "executor-A", 30)
        .await;
    assert!(stale_err.is_err(), "stale executor A is fenced from renewing lease");
}

#[tokio::test]
async fn expired_lease_executor_is_fenced_from_order_creation_without_ghost_orders() {
    let (db, _, _, _) = setup().await;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let cart_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let journal = rustok_commerce::services::CheckoutOperationJournal::new(db.clone());
    let op = journal
        .begin(rustok_commerce::services::BeginCheckoutOperation {
            tenant_id,
            cart_id,
            idempotency_key: "lease-fencing-order-test".to_string(),
            request_hash: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            snapshot_hash: Some("abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789".to_string()),
        })
        .await
        .expect("operation created");

    // Executor A claims execution
    journal
        .claim_execution(tenant_id, op.id, "executor-A", 10)
        .await
        .expect("claim succeeds");

    // Transition to inventory_reserved
    journal
        .checkpoint(rustok_commerce::services::CheckoutOperationCheckpoint {
            tenant_id,
            operation_id: op.id,
            lease_owner: "executor-A",
            expected_stage: rustok_commerce::services::CheckoutOperationStage::Executing,
            next_stage: rustok_commerce::services::CheckoutOperationStage::InventoryReserved,
            snapshot_hash: None,
            order_id: None,
            payment_collection_id: None,
            fulfillment_shipment_id: None,
            last_error: None,
            lease_seconds: 10,
        })
        .await
        .expect("checkpoint succeeds");

    // Expire executor A's lease in DB
    use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
    rustok_commerce::entities::checkout_operation::Entity::update_many()
        .col_expr(
            rustok_commerce::entities::checkout_operation::Column::LeaseExpiresAt,
            sea_orm::sea_query::Expr::value(chrono::Utc::now().fixed_offset() - chrono::Duration::seconds(10)),
        )
        .filter(rustok_commerce::entities::checkout_operation::Column::Id.eq(op.id))
        .exec(&db)
        .await
        .expect("lease expired");

    // Executor A attempts create_pending_and_adopt with expired lease
    let event_bus = rustok_outbox::TransactionalEventBus::new(db.clone());
    let creation_executor = rustok_commerce::services::CheckoutOrderCreationExecutor::new(db.clone(), event_bus);

    let create_input = rustok_order::CreateOrderInput {
        customer_id: None,
        currency_code: "USD".to_string(),
        shipping_total: rust_decimal::Decimal::ZERO,
        line_items: vec![],
        adjustments: vec![],
        tax_lines: vec![],
        metadata: serde_json::json!({}),
    };

    let result = creation_executor
        .create_pending_and_adopt(
            tenant_id,
            actor_id,
            op.id,
            "executor-A",
            create_input,
            None,
            None,
            "en",
            None,
        )
        .await;

    assert!(result.is_err(), "stale executor A must be fenced by renew_lease before creating order");

    // Verify zero rows in orders were created
    let orders_count = rustok_order::entities::order::Entity::find()
        .filter(rustok_order::entities::order::Column::TenantId.eq(tenant_id))
        .count(&db)
        .await
        .expect("count orders");
    assert_eq!(orders_count, 0, "no ghost order must exist in orders table");
}

