use rust_decimal::Decimal;
use rustok_order::dto::{
    CreateOrderInput, CreateOrderLineItemInput, CreateOrderReturnInput, ListOrderReturnsInput,
};
use rustok_order::services::OrderService;
use rustok_test_utils::mock_transactional_event_bus;
use sea_orm::Database;
use uuid::Uuid;

mod support;

#[tokio::test]
async fn commerce_test_schema_supports_order_returns_filters() {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    support::ensure_commerce_schema(&db).await;

    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let service = OrderService::new(db.clone(), mock_transactional_event_bus());

    let order = service
        .create_order(
            tenant_id,
            actor_id,
            CreateOrderInput {
                customer_id: Some(Uuid::new_v4()),
                currency_code: "usd".to_string(),
                shipping_total: Decimal::ZERO,
                line_items: vec![CreateOrderLineItemInput {
                    product_id: None,
                    variant_id: None,
                    shipping_profile_slug: "default".to_string(),
                    seller_id: None,
                    sku: Some("RET-SKU-1".to_string()),
                    title: "Return Candidate".to_string(),
                    quantity: 1,
                    unit_price: Decimal::new(2500, 2),
                    metadata: serde_json::json!({"slot":1}),
                }],
                adjustments: Vec::new(),
                tax_lines: Vec::new(),
                metadata: serde_json::json!({"source":"commerce-order-returns-bridge-test"}),
            },
        )
        .await
        .unwrap();

    let created = service
        .create_return(
            tenant_id,
            order.id,
            CreateOrderReturnInput {
                reason: Some("damaged".to_string()),
                note: None,
                items: Vec::new(),
                metadata: serde_json::json!({"source":"commerce-order-returns-bridge-test"}),
            },
        )
        .await
        .unwrap();

    let (filtered, filtered_total) = service
        .list_returns(
            tenant_id,
            ListOrderReturnsInput {
                page: 1,
                per_page: 20,
                order_id: Some(order.id),
                status: Some("pending".to_string()),
            },
        )
        .await
        .unwrap();

    assert_eq!(filtered_total, 1);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].id, created.id);
    assert_eq!(filtered[0].order_id, order.id);
    assert_eq!(filtered[0].status, "pending");
    assert_eq!(filtered[0].reason.as_deref(), Some("damaged"));
}

#[tokio::test]
async fn commerce_order_returns_listing_ignores_blank_status_filter() {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    support::ensure_commerce_schema(&db).await;

    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let service = OrderService::new(db.clone(), mock_transactional_event_bus());

    let order = service
        .create_order(
            tenant_id,
            actor_id,
            CreateOrderInput {
                customer_id: Some(Uuid::new_v4()),
                currency_code: "usd".to_string(),
                shipping_total: Decimal::ZERO,
                line_items: vec![CreateOrderLineItemInput {
                    product_id: None,
                    variant_id: None,
                    shipping_profile_slug: "default".to_string(),
                    seller_id: None,
                    sku: Some("RET-SKU-2".to_string()),
                    title: "Return Candidate 2".to_string(),
                    quantity: 1,
                    unit_price: Decimal::new(1500, 2),
                    metadata: serde_json::json!({"slot":2}),
                }],
                adjustments: Vec::new(),
                tax_lines: Vec::new(),
                metadata: serde_json::json!({"source":"commerce-order-returns-blank-filter-test"}),
            },
        )
        .await
        .unwrap();

    let created = service
        .create_return(
            tenant_id,
            order.id,
            CreateOrderReturnInput {
                reason: Some("wrong-size".to_string()),
                note: None,
                items: Vec::new(),
                metadata: serde_json::json!({"source":"commerce-order-returns-blank-filter-test"}),
            },
        )
        .await
        .unwrap();

    let (rows, total) = service
        .list_returns(
            tenant_id,
            ListOrderReturnsInput {
                page: 1,
                per_page: 20,
                order_id: Some(order.id),
                status: Some("   ".to_string()),
            },
        )
        .await
        .unwrap();

    assert_eq!(total, 1);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, created.id);
}

#[tokio::test]
async fn commerce_post_order_decision_creates_return_bound_refund() {
    use rustok_commerce::{
        CreateReturnDecisionInput, PostOrderOrchestrationService, ReturnDecisionInput,
        ReturnRefundDecisionInput,
    };
    use rustok_payment::dto::{
        AuthorizePaymentInput, CapturePaymentInput, CreatePaymentCollectionInput,
    };
    use rustok_payment::services::PaymentService;

    let db = Database::connect("sqlite::memory:").await.unwrap();
    support::ensure_commerce_schema(&db).await;

    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let order_service = OrderService::new(db.clone(), mock_transactional_event_bus());
    let payment_service = PaymentService::new(db.clone());

    let order = order_service
        .create_order(
            tenant_id,
            actor_id,
            CreateOrderInput {
                customer_id: Some(Uuid::new_v4()),
                currency_code: "usd".to_string(),
                shipping_total: Decimal::ZERO,
                line_items: vec![CreateOrderLineItemInput {
                    product_id: None,
                    variant_id: None,
                    shipping_profile_slug: "default".to_string(),
                    seller_id: None,
                    sku: Some("RET-REFUND-1".to_string()),
                    title: "Refundable Return Candidate".to_string(),
                    quantity: 1,
                    unit_price: Decimal::new(4200, 2),
                    metadata: serde_json::json!({"slot":3}),
                }],
                adjustments: Vec::new(),
                tax_lines: Vec::new(),
                metadata: serde_json::json!({"source":"commerce-return-refund-decision-test"}),
            },
        )
        .await
        .unwrap();

    let collection = payment_service
        .create_collection(
            tenant_id,
            CreatePaymentCollectionInput {
                cart_id: None,
                order_id: Some(order.id),
                customer_id: order.customer_id,
                currency_code: "usd".to_string(),
                amount: order.total_amount,
                metadata: serde_json::json!({"source":"commerce-return-refund-decision-test"}),
            },
        )
        .await
        .unwrap();
    payment_service
        .authorize_collection(
            tenant_id,
            collection.id,
            AuthorizePaymentInput {
                provider_id: Some("manual".to_string()),
                provider_payment_id: Some("ret-refund-payment".to_string()),
                amount: Some(order.total_amount),
                metadata: serde_json::json!({}),
            },
        )
        .await
        .unwrap();
    payment_service
        .capture_collection(
            tenant_id,
            collection.id,
            CapturePaymentInput {
                amount: Some(order.total_amount),
                metadata: serde_json::json!({}),
            },
        )
        .await
        .unwrap();

    let decision = PostOrderOrchestrationService::new(db.clone(), mock_transactional_event_bus())
        .create_return_decision(
            tenant_id,
            actor_id,
            order.id,
            CreateReturnDecisionInput {
                return_request: CreateOrderReturnInput {
                    reason: Some("damaged".to_string()),
                    note: Some("refund the returned item".to_string()),
                    items: Vec::new(),
                    metadata: serde_json::json!({"source":"commerce-return-refund-decision-test"}),
                },
                decision: ReturnDecisionInput {
                    action: "refund".to_string(),
                    refund: Some(ReturnRefundDecisionInput {
                        payment_collection_id: Some(collection.id),
                        amount: Some(Decimal::new(4200, 2)),
                        reason: Some("damaged".to_string()),
                        metadata: serde_json::json!({"operator":"returns-desk"}),
                    }),
                    exchange: None,
                    claim: None,
                    metadata: serde_json::json!({"flow":"refund"}),
                },
            },
        )
        .await
        .unwrap();

    assert_eq!(decision.action, "refund");
    assert_eq!(decision.order_return.order_id, order.id);
    assert_eq!(decision.order_return.status, "completed");
    assert_eq!(
        decision.order_return.resolution_type.as_deref(),
        Some("refund")
    );
    let refund = decision.refund.expect("refund should be created");
    assert_eq!(refund.payment_collection_id, collection.id);
    assert_eq!(decision.order_return.refund_id, Some(refund.id));
    assert_eq!(refund.amount, Decimal::new(4200, 2));
    assert_eq!(
        refund.metadata["order_return_id"],
        serde_json::json!(decision.order_return.id.to_string())
    );
    assert!(decision.order_change.is_none());
}

#[tokio::test]
async fn commerce_post_order_decision_creates_return_bound_exchange_change() {
    use rustok_commerce::{
        CreateReturnDecisionInput, PostOrderOrchestrationService, ReturnDecisionInput,
        ReturnExchangeDecisionInput,
    };

    let db = Database::connect("sqlite::memory:").await.unwrap();
    support::ensure_commerce_schema(&db).await;

    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let order_service = OrderService::new(db.clone(), mock_transactional_event_bus());

    let order = order_service
        .create_order(
            tenant_id,
            actor_id,
            CreateOrderInput {
                customer_id: Some(Uuid::new_v4()),
                currency_code: "usd".to_string(),
                shipping_total: Decimal::ZERO,
                line_items: vec![CreateOrderLineItemInput {
                    product_id: None,
                    variant_id: None,
                    shipping_profile_slug: "default".to_string(),
                    seller_id: None,
                    sku: Some("RET-EXCHANGE-1".to_string()),
                    title: "Exchange Return Candidate".to_string(),
                    quantity: 1,
                    unit_price: Decimal::new(3300, 2),
                    metadata: serde_json::json!({"slot":4}),
                }],
                adjustments: Vec::new(),
                tax_lines: Vec::new(),
                metadata: serde_json::json!({"source":"commerce-return-exchange-decision-test"}),
            },
        )
        .await
        .unwrap();

    let decision = PostOrderOrchestrationService::new(db.clone(), mock_transactional_event_bus())
        .create_return_decision(
            tenant_id,
            actor_id,
            order.id,
            CreateReturnDecisionInput {
                return_request: CreateOrderReturnInput {
                    reason: Some("wrong-size".to_string()),
                    note: None,
                    items: Vec::new(),
                    metadata: serde_json::json!({"source":"commerce-return-exchange-decision-test"}),
                },
                decision: ReturnDecisionInput {
                    action: "exchange".to_string(),
                    refund: None,
                    exchange: Some(ReturnExchangeDecisionInput {
                        description: Some("Exchange for another size".to_string()),
                        preview: serde_json::json!({"remove":["old-line"],"add":["new-line"]}),
                        metadata: serde_json::json!({"operator":"returns-desk"}),
                    }),
                    claim: None,
                    metadata: serde_json::json!({"flow":"exchange"}),
                },
            },
        )
        .await
        .unwrap();

    assert_eq!(decision.action, "exchange");
    assert_eq!(decision.order_return.status, "completed");
    assert_eq!(
        decision.order_return.resolution_type.as_deref(),
        Some("exchange")
    );
    assert!(decision.refund.is_none());
    let change = decision
        .order_change
        .expect("exchange change should be created");
    assert_eq!(change.order_id, order.id);
    assert_eq!(change.change_type, "exchange");
    assert_eq!(decision.order_return.order_change_id, Some(change.id));
    assert_eq!(
        change.metadata["order_return_id"],
        serde_json::json!(decision.order_return.id.to_string())
    );
    assert_eq!(
        change.preview["order_return_id"],
        serde_json::json!(decision.order_return.id.to_string())
    );
    assert_eq!(
        change.preview["return_decision_action"],
        serde_json::json!("exchange")
    );
    assert_eq!(
        change.metadata["return_decision_source"],
        serde_json::json!("rustok-commerce")
    );
}

#[tokio::test]
async fn commerce_post_order_decision_creates_return_bound_claim_change() {
    use rustok_commerce::{
        CreateReturnDecisionInput, PostOrderOrchestrationService, ReturnClaimDecisionInput,
        ReturnDecisionInput,
    };

    let db = Database::connect("sqlite::memory:").await.unwrap();
    support::ensure_commerce_schema(&db).await;

    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let order_service = OrderService::new(db.clone(), mock_transactional_event_bus());

    let order = order_service
        .create_order(
            tenant_id,
            actor_id,
            CreateOrderInput {
                customer_id: Some(Uuid::new_v4()),
                currency_code: "usd".to_string(),
                shipping_total: Decimal::ZERO,
                line_items: vec![CreateOrderLineItemInput {
                    product_id: None,
                    variant_id: None,
                    shipping_profile_slug: "default".to_string(),
                    seller_id: None,
                    sku: Some("RET-CLAIM-1".to_string()),
                    title: "Claim Return Candidate".to_string(),
                    quantity: 1,
                    unit_price: Decimal::new(2700, 2),
                    metadata: serde_json::json!({"slot":5}),
                }],
                adjustments: Vec::new(),
                tax_lines: Vec::new(),
                metadata: serde_json::json!({"source":"commerce-return-claim-decision-test"}),
            },
        )
        .await
        .unwrap();

    let decision = PostOrderOrchestrationService::new(db.clone(), mock_transactional_event_bus())
        .create_return_decision(
            tenant_id,
            actor_id,
            order.id,
            CreateReturnDecisionInput {
                return_request: CreateOrderReturnInput {
                    reason: Some("damaged".to_string()),
                    note: Some("claim for damaged inbound item".to_string()),
                    items: Vec::new(),
                    metadata: serde_json::json!({"source":"commerce-return-claim-decision-test"}),
                },
                decision: ReturnDecisionInput {
                    action: "claim".to_string(),
                    refund: None,
                    exchange: None,
                    claim: Some(ReturnClaimDecisionInput {
                        description: Some("Operator claim review".to_string()),
                        preview: serde_json::json!({"claim_type":"damaged_item","resolution":"review"}),
                        metadata: serde_json::json!({"operator":"claims-desk"}),
                    }),
                    metadata: serde_json::json!({"flow":"claim"}),
                },
            },
        )
        .await
        .unwrap();

    assert_eq!(decision.action, "claim");
    assert_eq!(decision.order_return.status, "completed");
    assert_eq!(
        decision.order_return.resolution_type.as_deref(),
        Some("claim")
    );
    assert!(decision.refund.is_none());
    let change = decision
        .order_change
        .expect("claim change should be created");
    assert_eq!(change.order_id, order.id);
    assert_eq!(change.change_type, "claim");
    assert_eq!(decision.order_return.order_change_id, Some(change.id));
    assert_eq!(
        change.metadata["order_return_id"],
        serde_json::json!(decision.order_return.id.to_string())
    );
    assert_eq!(
        change.preview["order_return_id"],
        serde_json::json!(decision.order_return.id.to_string())
    );
}

#[tokio::test]
async fn commerce_apply_exchange_order_change_with_explicit_refund() {
    use rustok_commerce::{
        CreateReturnDecisionInput, ExchangeDifferenceRefundInput, PostOrderOrchestrationService,
        ReturnDecisionInput, ReturnExchangeDecisionInput,
    };
    use rustok_payment::dto::{
        AuthorizePaymentInput, CapturePaymentInput, CreatePaymentCollectionInput,
    };
    use rustok_payment::services::PaymentService;

    let db = Database::connect("sqlite::memory:").await.unwrap();
    support::ensure_commerce_schema(&db).await;

    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let order_service = OrderService::new(db.clone(), mock_transactional_event_bus());
    let payment_service = PaymentService::new(db.clone());

    let order = order_service
        .create_order(
            tenant_id,
            actor_id,
            CreateOrderInput {
                customer_id: Some(Uuid::new_v4()),
                currency_code: "usd".to_string(),
                shipping_total: Decimal::ZERO,
                line_items: vec![CreateOrderLineItemInput {
                    product_id: None,
                    variant_id: None,
                    shipping_profile_slug: "default".to_string(),
                    seller_id: None,
                    sku: Some("RET-EXCHANGE-2".to_string()),
                    title: "Exchange Return Candidate 2".to_string(),
                    quantity: 1,
                    unit_price: Decimal::new(3300, 2),
                    metadata: serde_json::json!({"slot":4}),
                }],
                adjustments: Vec::new(),
                tax_lines: Vec::new(),
                metadata: serde_json::json!({"source":"commerce-apply-exchange-test"}),
            },
        )
        .await
        .unwrap();

    let collection = payment_service
        .create_collection(
            tenant_id,
            CreatePaymentCollectionInput {
                cart_id: None,
                order_id: Some(order.id),
                customer_id: order.customer_id,
                currency_code: "usd".to_string(),
                amount: order.total_amount,
                metadata: serde_json::json!({"source":"commerce-apply-exchange-test"}),
            },
        )
        .await
        .unwrap();

    payment_service
        .authorize_collection(
            tenant_id,
            collection.id,
            AuthorizePaymentInput {
                provider_id: Some("manual".to_string()),
                provider_payment_id: Some("exchange-refund-payment".to_string()),
                amount: Some(order.total_amount),
                metadata: serde_json::json!({}),
            },
        )
        .await
        .unwrap();

    payment_service
        .capture_collection(
            tenant_id,
            collection.id,
            CapturePaymentInput {
                amount: Some(order.total_amount),
                metadata: serde_json::json!({}),
            },
        )
        .await
        .unwrap();

    let orchestration =
        PostOrderOrchestrationService::new(db.clone(), mock_transactional_event_bus());
    let decision = orchestration
        .create_return_decision(
            tenant_id,
            actor_id,
            order.id,
            CreateReturnDecisionInput {
                return_request: CreateOrderReturnInput {
                    reason: Some("wrong-size".to_string()),
                    note: None,
                    items: Vec::new(),
                    metadata: serde_json::json!({"source":"commerce-apply-exchange-test"}),
                },
                decision: ReturnDecisionInput {
                    action: "exchange".to_string(),
                    refund: None,
                    exchange: Some(ReturnExchangeDecisionInput {
                        description: Some("Exchange for another size".to_string()),
                        preview: serde_json::json!({"remove":["old-line"],"add":["new-line"]}),
                        metadata: serde_json::json!({"operator":"returns-desk"}),
                    }),
                    claim: None,
                    metadata: serde_json::json!({"flow":"exchange"}),
                },
            },
        )
        .await
        .unwrap();

    let change = decision.order_change.expect("change created");

    let result = orchestration
        .apply_exchange_order_change(
            tenant_id,
            order.id,
            change.id,
            Some(ExchangeDifferenceRefundInput {
                amount: Decimal::new(1000, 2),
                reason: Some("Exchange size price diff".to_string()),
                metadata: serde_json::json!({}),
            }),
            serde_json::json!({"operator":"returns-desk"}),
        )
        .await
        .unwrap();

    assert_eq!(result.order_change.status, "applied");
    let refund = result.refund.expect("refund created");
    assert_eq!(refund.amount, Decimal::new(1000, 2));
    assert_eq!(refund.reason.as_deref(), Some("Exchange size price diff"));
}

#[tokio::test]
async fn commerce_apply_exchange_order_change_with_automated_refund() {
    use rustok_commerce::{
        CreateReturnDecisionInput, PostOrderOrchestrationService, ReturnDecisionInput,
        ReturnExchangeDecisionInput,
    };
    use rustok_payment::dto::{
        AuthorizePaymentInput, CapturePaymentInput, CreatePaymentCollectionInput,
    };
    use rustok_payment::services::PaymentService;

    let db = Database::connect("sqlite::memory:").await.unwrap();
    support::ensure_commerce_schema(&db).await;

    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let order_service = OrderService::new(db.clone(), mock_transactional_event_bus());
    let payment_service = PaymentService::new(db.clone());

    let order = order_service
        .create_order(
            tenant_id,
            actor_id,
            CreateOrderInput {
                customer_id: Some(Uuid::new_v4()),
                currency_code: "usd".to_string(),
                shipping_total: Decimal::ZERO,
                line_items: vec![CreateOrderLineItemInput {
                    product_id: None,
                    variant_id: None,
                    shipping_profile_slug: "default".to_string(),
                    seller_id: None,
                    sku: Some("RET-EXCHANGE-3".to_string()),
                    title: "Exchange Return Candidate 3".to_string(),
                    quantity: 1,
                    unit_price: Decimal::new(3300, 2),
                    metadata: serde_json::json!({"slot":4}),
                }],
                adjustments: Vec::new(),
                tax_lines: Vec::new(),
                metadata: serde_json::json!({"source":"commerce-auto-exchange-test"}),
            },
        )
        .await
        .unwrap();

    let collection = payment_service
        .create_collection(
            tenant_id,
            CreatePaymentCollectionInput {
                cart_id: None,
                order_id: Some(order.id),
                customer_id: order.customer_id,
                currency_code: "usd".to_string(),
                amount: order.total_amount,
                metadata: serde_json::json!({"source":"commerce-auto-exchange-test"}),
            },
        )
        .await
        .unwrap();

    payment_service
        .authorize_collection(
            tenant_id,
            collection.id,
            AuthorizePaymentInput {
                provider_id: Some("manual".to_string()),
                provider_payment_id: Some("exchange-auto-refund-payment".to_string()),
                amount: Some(order.total_amount),
                metadata: serde_json::json!({}),
            },
        )
        .await
        .unwrap();

    payment_service
        .capture_collection(
            tenant_id,
            collection.id,
            CapturePaymentInput {
                amount: Some(order.total_amount),
                metadata: serde_json::json!({}),
            },
        )
        .await
        .unwrap();

    let orchestration =
        PostOrderOrchestrationService::new(db.clone(), mock_transactional_event_bus());
    let decision = orchestration
        .create_return_decision(
            tenant_id,
            actor_id,
            order.id,
            CreateReturnDecisionInput {
                return_request: CreateOrderReturnInput {
                    reason: Some("wrong-size".to_string()),
                    note: None,
                    items: Vec::new(),
                    metadata: serde_json::json!({"source":"commerce-auto-exchange-test"}),
                },
                decision: ReturnDecisionInput {
                    action: "exchange".to_string(),
                    refund: None,
                    exchange: Some(ReturnExchangeDecisionInput {
                        description: Some("Exchange for another size".to_string()),
                        preview: serde_json::json!({
                            "remove":["old-line"],
                            "add":["new-line"],
                            "difference_refund_amount": "15.50",
                            "difference_refund_reason": "Exchange automated size price diff"
                        }),
                        metadata: serde_json::json!({"operator":"returns-desk"}),
                    }),
                    claim: None,
                    metadata: serde_json::json!({"flow":"exchange"}),
                },
            },
        )
        .await
        .unwrap();

    let change = decision.order_change.expect("change created");

    let result = orchestration
        .apply_exchange_order_change(
            tenant_id,
            order.id,
            change.id,
            None, // Use automated extraction!
            serde_json::json!({"operator":"returns-desk"}),
        )
        .await
        .unwrap();

    assert_eq!(result.order_change.status, "applied");
    let refund = result.refund.expect("automated refund created");
    assert_eq!(refund.amount, Decimal::new(1550, 2));
    assert_eq!(
        refund.reason.as_deref(),
        Some("Exchange automated size price diff")
    );
}

#[tokio::test]
async fn commerce_apply_claim_order_change() {
    use rustok_commerce::{
        CreateReturnDecisionInput, PostOrderOrchestrationService, ReturnClaimDecisionInput,
        ReturnDecisionInput,
    };

    let db = Database::connect("sqlite::memory:").await.unwrap();
    support::ensure_commerce_schema(&db).await;

    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let order_service = OrderService::new(db.clone(), mock_transactional_event_bus());

    let order = order_service
        .create_order(
            tenant_id,
            actor_id,
            CreateOrderInput {
                customer_id: Some(Uuid::new_v4()),
                currency_code: "usd".to_string(),
                shipping_total: Decimal::ZERO,
                line_items: vec![CreateOrderLineItemInput {
                    product_id: None,
                    variant_id: None,
                    shipping_profile_slug: "default".to_string(),
                    seller_id: None,
                    sku: Some("RET-CLAIM-2".to_string()),
                    title: "Claim Return Candidate 2".to_string(),
                    quantity: 1,
                    unit_price: Decimal::new(2700, 2),
                    metadata: serde_json::json!({"slot":5}),
                }],
                adjustments: Vec::new(),
                tax_lines: Vec::new(),
                metadata: serde_json::json!({"source":"commerce-apply-claim-test"}),
            },
        )
        .await
        .unwrap();

    let orchestration =
        PostOrderOrchestrationService::new(db.clone(), mock_transactional_event_bus());
    let decision = orchestration
        .create_return_decision(
            tenant_id,
            actor_id,
            order.id,
            CreateReturnDecisionInput {
                return_request: CreateOrderReturnInput {
                    reason: Some("damaged".to_string()),
                    note: Some("claim for damaged item".to_string()),
                    items: Vec::new(),
                    metadata: serde_json::json!({"source":"commerce-apply-claim-test"}),
                },
                decision: ReturnDecisionInput {
                    action: "claim".to_string(),
                    refund: None,
                    exchange: None,
                    claim: Some(ReturnClaimDecisionInput {
                        description: Some("Claim free replacement".to_string()),
                        preview: serde_json::json!({"claim_type":"damaged_item"}),
                        metadata: serde_json::json!({"operator":"claims-desk"}),
                    }),
                    metadata: serde_json::json!({"flow":"claim"}),
                },
            },
        )
        .await
        .unwrap();

    let change = decision.order_change.expect("claim change created");

    let result = orchestration
        .apply_claim_order_change(
            tenant_id,
            change.id,
            serde_json::json!({"operator":"claims-desk"}),
        )
        .await
        .unwrap();

    assert_eq!(result.order_change.status, "applied");
    assert!(result.refund.is_none());
}
