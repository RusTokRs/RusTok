use async_graphql::{EmptySubscription, Request, Schema};
use rust_decimal::Decimal;
use rustok_api::Permission;
use rustok_api::{AuthContext, RequestContext, TenantContext};
use rustok_cart::CartService;
use rustok_cart::dto::{AddCartLineItemInput, CreateCartInput, SetCartAdjustmentInput};
use rustok_commerce::dto::{CompleteCheckoutInput, ShippingProfileTranslationInput};
use rustok_commerce::graphql::{CommerceMutation, CommerceQuery};
use rustok_commerce::{CheckoutService, ShippingProfileService};
use rustok_customer::CustomerService;
use rustok_customer::dto::CreateCustomerInput;
use rustok_fulfillment::FulfillmentService;
use rustok_fulfillment::dto::{
    CreateFulfillmentInput, CreateShippingOptionInput, DeliverFulfillmentInput,
    ShipFulfillmentInput, ShippingOptionTranslationInput,
};
use rustok_order::OrderService;
use rustok_order::dto::{CreateOrderInput, CreateOrderLineItemInput};
use rustok_outbox::{OutboxTransport, SysEventsMigration, TransactionalEventBus};
use rustok_payment::PaymentRefundCreationService;
use rustok_payment::PaymentService;
use rustok_payment::dto::{
    AuthorizePaymentInput, CapturePaymentInput, CreatePaymentCollectionInput, CreateRefundInput,
};
use rustok_pricing::PricingService;
use rustok_product::CatalogService;
use rustok_product::dto::{
    CreateProductInput, CreateVariantInput, PriceInput, ProductTranslationInput,
};
use rustok_region::dto::{CreateRegionInput, RegionTranslationInput};
use rustok_region::services::RegionService;
use rustok_test_utils::{db::setup_test_db, helpers::unique_slug, mock_transactional_event_bus};
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement};
use sea_orm_migration::{MigrationTrait, prelude::SchemaManager};
use serde_json::Value;
use std::str::FromStr;
use std::sync::Arc;
use uuid::Uuid;

#[path = "../support.rs"]
mod support;

type CommerceSchema = Schema<CommerceQuery, CommerceMutation, EmptySubscription>;

const STOREFRONT_QUERY_TEMPLATE: &str = r#"
query {
  storefrontProducts(locale: "de") {
    total
    items { title handle }
  }
  storefrontProduct(locale: "de", handle: "__HANDLE__") {
    translations { locale title handle }
  }
}
"#;

async fn outbox_event_bus(db: &DatabaseConnection) -> TransactionalEventBus {
    SysEventsMigration
        .up(&SchemaManager::new(db))
        .await
        .expect("sys events migration should run");
    TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())))
}

async fn setup() -> (DatabaseConnection, CatalogService, CartService) {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let event_bus = outbox_event_bus(&db).await;
    (
        db.clone(),
        CatalogService::new(db.clone(), event_bus),
        CartService::new(db),
    )
}

async fn setup_checkout() -> (
    DatabaseConnection,
    CatalogService,
    CartService,
    CheckoutService,
    FulfillmentService,
) {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let event_bus = outbox_event_bus(&db).await;
    (
        db.clone(),
        CatalogService::new(db.clone(), event_bus.clone()),
        CartService::new(db.clone()),
        CheckoutService::new(
            db.clone(),
            event_bus.clone(),
            std::sync::Arc::new(rustok_region::RegionService::new(db.clone())),
            std::sync::Arc::new(rustok_cart::CartService::new(db.clone())),
            std::sync::Arc::new(rustok_inventory::InventoryService::new(
                db.clone(),
                event_bus.clone(),
            )),
            std::sync::Arc::new(rustok_product::CatalogService::new(db.clone(), event_bus)),
        ),
        FulfillmentService::new(db),
    )
}

async fn capture_payment_collection_for_refund(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    collection_id: Uuid,
    amount: Decimal,
) {
    let payment_service = PaymentService::new(db.clone());
    payment_service
        .authorize_collection(
            tenant_id,
            collection_id,
            AuthorizePaymentInput {
                provider_id: Some("manual".to_string()),
                provider_payment_id: Some(format!("test-payment-{collection_id}")),
                amount: Some(amount),
                metadata: serde_json::json!({ "source": "graphql-runtime-refund-capture" }),
            },
        )
        .await
        .expect("payment collection should be authorized before refund");
    payment_service
        .capture_collection(
            tenant_id,
            collection_id,
            CapturePaymentInput {
                amount: Some(amount),
                metadata: serde_json::json!({ "source": "graphql-runtime-refund-capture" }),
            },
        )
        .await
        .expect("payment collection should be captured before refund");
}

fn create_product_input() -> CreateProductInput {
    CreateProductInput {
        translations: vec![
            ProductTranslationInput {
                locale: "en".to_string(),
                title: "Parity Product".to_string(),
                description: Some("English description".to_string()),
                handle: Some(unique_slug("parity-product")),
                meta_title: None,
                meta_description: None,
            },
            ProductTranslationInput {
                locale: "de".to_string(),
                title: "Paritaet Produkt".to_string(),
                description: Some("German description".to_string()),
                handle: Some(unique_slug("paritaet-produkt")),
                meta_title: None,
                meta_description: None,
            },
        ],
        options: vec![],
        variants: vec![CreateVariantInput {
            sku: Some("PARITY-SKU-1".to_string()),
            barcode: None,
            shipping_profile_slug: None,
            option1: Some("Default".to_string()),
            option2: None,
            option3: None,
            prices: vec![PriceInput {
                currency_code: "EUR".to_string(),
                channel_id: None,
                channel_slug: None,
                amount: Decimal::from_str("19.99").expect("valid decimal"),
                compare_at_amount: None,
            }],
            inventory_quantity: 5,
            inventory_policy: "deny".to_string(),
            weight: None,
            weight_unit: None,
        }],
        seller_id: None,
        vendor: Some("Parity Vendor".to_string()),
        product_type: Some("physical".to_string()),
        shipping_profile_slug: None,
        primary_category_id: None,
        tags: vec![],
        publish: false,
        metadata: serde_json::json!({}),
    }
}

fn tenant_context(tenant_id: Uuid) -> TenantContext {
    TenantContext {
        id: tenant_id,
        name: "Parity Tenant".to_string(),
        slug: "parity-tenant".to_string(),
        domain: None,
        settings: serde_json::json!({}),
        default_locale: "en".to_string(),
        is_active: true,
    }
}

fn request_context(tenant_id: Uuid, locale: &str) -> RequestContext {
    RequestContext {
        tenant_id,
        user_id: None,
        channel_id: None,
        channel_slug: None,
        channel_resolution_source: None,
        locale: locale.to_string(),
    }
}

fn request_context_with_channel(
    tenant_id: Uuid,
    locale: &str,
    channel_id: Uuid,
    channel_slug: &str,
) -> RequestContext {
    RequestContext {
        tenant_id,
        user_id: None,
        channel_id: Some(channel_id),
        channel_slug: Some(channel_slug.to_string()),
        channel_resolution_source: None,
        locale: locale.to_string(),
    }
}

async fn seed_channel_binding(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    channel_id: Uuid,
    channel_slug: &str,
    is_enabled: bool,
) {
    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "INSERT INTO channels (id, tenant_id, slug, name, is_active, is_default, status, settings, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        vec![
            channel_id.into(),
            tenant_id.into(),
            channel_slug.into(),
            format!("Channel {channel_slug}").into(),
            true.into(),
            false.into(),
            "active".into(),
            serde_json::json!({}).to_string().into(),
        ],
    ))
    .await
    .expect("channel should be inserted");

    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "INSERT INTO channel_module_bindings (id, channel_id, module_slug, is_enabled, settings, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        vec![
            Uuid::new_v4().into(),
            channel_id.into(),
            "commerce".into(),
            is_enabled.into(),
            serde_json::json!({}).to_string().into(),
        ],
    ))
    .await
    .expect("channel binding should be inserted");
}

async fn seed_active_price_list(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    name: &str,
    channel_id: Option<Uuid>,
    channel_slug: Option<&str>,
    adjustment_percent: Option<&str>,
) -> Uuid {
    seed_active_price_list_with_window(
        db,
        tenant_id,
        name,
        channel_id,
        channel_slug,
        adjustment_percent,
        None,
        None,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn seed_active_price_list_with_window(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    name: &str,
    channel_id: Option<Uuid>,
    channel_slug: Option<&str>,
    adjustment_percent: Option<&str>,
    starts_at: Option<chrono::DateTime<chrono::Utc>>,
    ends_at: Option<chrono::DateTime<chrono::Utc>>,
) -> Uuid {
    let price_list_id = Uuid::new_v4();
    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "INSERT INTO price_lists (id, tenant_id, type, status, channel_id, channel_slug, rule_kind, adjustment_percent, starts_at, ends_at, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        vec![
            price_list_id.into(),
            tenant_id.into(),
            "sale".into(),
            "active".into(),
            channel_id.into(),
            channel_slug
                .map(|value| value.to_ascii_lowercase())
                .into(),
            adjustment_percent
                .map(|_| "percentage_discount".to_string())
                .into(),
            adjustment_percent.map(|value| value.to_string()).into(),
            starts_at.into(),
            ends_at.into(),
        ],
    ))
    .await
    .expect("active price list should be inserted");

    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "INSERT INTO price_list_translations (id, price_list_id, locale, name, description)
         VALUES (?, ?, ?, ?, ?)",
        vec![
            Uuid::new_v4().into(),
            price_list_id.into(),
            "en".into(),
            name.into(),
            Some(format!("GraphQL pricing helper {name}")).into(),
        ],
    ))
    .await
    .expect("price list translation should be inserted");

    price_list_id
}

async fn set_stock_location_channel_visibility(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    allowed_channel_slugs: &[&str],
) {
    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "UPDATE stock_locations SET metadata = ? WHERE tenant_id = ?",
        vec![
            serde_json::json!({
                "channel_visibility": {
                    "allowed_channel_slugs": allowed_channel_slugs
                }
            })
            .to_string()
            .into(),
            tenant_id.into(),
        ],
    ))
    .await
    .expect("stock location visibility should be updated");
}

fn auth_context(tenant_id: Uuid) -> AuthContext {
    AuthContext {
        user_id: Uuid::new_v4(),
        session_id: Uuid::new_v4(),
        tenant_id,
        permissions: vec![Permission::PRODUCTS_READ, Permission::PRODUCTS_LIST],
        client_id: None,
        scopes: vec![],
        grant_type: "direct".to_string(),
    }
}

fn pricing_update_auth_context(tenant_id: Uuid) -> AuthContext {
    AuthContext {
        user_id: Uuid::new_v4(),
        session_id: Uuid::new_v4(),
        tenant_id,
        permissions: vec![
            Permission::PRODUCTS_READ,
            Permission::PRODUCTS_LIST,
            Permission::PRODUCTS_UPDATE,
        ],
        client_id: None,
        scopes: vec![],
        grant_type: "direct".to_string(),
    }
}

fn admin_order_auth_context(tenant_id: Uuid) -> AuthContext {
    AuthContext {
        user_id: Uuid::new_v4(),
        session_id: Uuid::new_v4(),
        tenant_id,
        permissions: vec![
            Permission::ORDERS_READ,
            Permission::ORDERS_LIST,
            Permission::ORDERS_UPDATE,
            Permission::PAYMENTS_READ,
            Permission::PAYMENTS_UPDATE,
            Permission::FULFILLMENTS_READ,
            Permission::FULFILLMENTS_UPDATE,
        ],
        client_id: None,
        scopes: vec![],
        grant_type: "direct".to_string(),
    }
}

fn admin_fulfillment_auth_context(tenant_id: Uuid) -> AuthContext {
    AuthContext {
        user_id: Uuid::new_v4(),
        session_id: Uuid::new_v4(),
        tenant_id,
        permissions: vec![
            Permission::FULFILLMENTS_READ,
            Permission::FULFILLMENTS_CREATE,
            Permission::FULFILLMENTS_UPDATE,
        ],
        client_id: None,
        scopes: vec![],
        grant_type: "direct".to_string(),
    }
}

fn customer_auth_context(tenant_id: Uuid, user_id: Uuid) -> AuthContext {
    AuthContext {
        user_id,
        session_id: Uuid::new_v4(),
        tenant_id,
        permissions: vec![],
        client_id: None,
        scopes: vec![],
        grant_type: "direct".to_string(),
    }
}

fn build_schema(
    db: &DatabaseConnection,
    tenant: TenantContext,
    request_context: RequestContext,
    auth: Option<AuthContext>,
) -> CommerceSchema {
    let event_bus = mock_transactional_event_bus();
    let mut builder = Schema::build(
        CommerceQuery,
        CommerceMutation::default(),
        EmptySubscription,
    )
    .data(db.clone())
    .data(event_bus)
    .data(tenant)
    .data(request_context);

    if let Some(auth) = auth {
        builder = builder.data(auth);
    }

    builder.finish()
}

fn storefront_query(handle: &str) -> String {
    STOREFRONT_QUERY_TEMPLATE.replace("__HANDLE__", handle)
}

fn admin_query(tenant_id: Uuid, product_id: Uuid) -> String {
    format!(
        r#"
        query {{
          products(tenantId: "{tenant_id}", locale: "en", filter: {{ page: 1, perPage: 20 }}) {{
            total
            items {{ title handle }}
          }}
          product(tenantId: "{tenant_id}", id: "{product_id}", locale: "en") {{
            translations {{ locale title handle }}
          }}
        }}
        "#
    )
}

fn admin_order_mutation(
    tenant_id: Uuid,
    actor_id: Uuid,
    order_id: Uuid,
    payment_collection_id: Uuid,
    fulfillment_id: Uuid,
) -> String {
    format!(
        r#"
        mutation {{
          authorizePaymentCollection(
            tenantId: "{tenant_id}",
            id: "{payment_collection_id}",
            input: {{
              providerId: "manual"
              providerPaymentId: "graphql-pay-1"
              amount: "25.00"
              metadata: "{{\"source\":\"graphql-admin-order-parity\",\"step\":\"authorize\"}}"
            }}
          ) {{
            status
            authorizedAmount
          }}
          capturePaymentCollection(
            tenantId: "{tenant_id}",
            id: "{payment_collection_id}",
            input: {{
              amount: "25.00"
              metadata: "{{\"source\":\"graphql-admin-order-parity\",\"step\":\"capture\"}}"
            }}
          ) {{
            status
            capturedAmount
          }}
          markOrderPaid(
            tenantId: "{tenant_id}",
            userId: "{actor_id}",
            id: "{order_id}",
            input: {{
              paymentId: "graphql-pay-1"
              paymentMethod: "manual"
            }}
          ) {{
            status
            paymentId
            paymentMethod
          }}
          shipFulfillment(
            tenantId: "{tenant_id}",
            id: "{fulfillment_id}",
            input: {{
              carrier: "manual"
              trackingNumber: "TRACK-789"
              metadata: "{{\"source\":\"graphql-admin-order-parity\",\"step\":\"ship-fulfillment\"}}"
            }}
          ) {{
            status
            trackingNumber
          }}
          deliverFulfillment(
            tenantId: "{tenant_id}",
            id: "{fulfillment_id}",
            input: {{
              deliveredNote: "Left at reception"
              metadata: "{{\"source\":\"graphql-admin-order-parity\",\"step\":\"deliver-fulfillment\"}}"
            }}
          ) {{
            status
            deliveredNote
          }}
          shipOrder(
            tenantId: "{tenant_id}",
            userId: "{actor_id}",
            id: "{order_id}",
            input: {{
              trackingNumber: "TRACK-789"
              carrier: "manual"
            }}
          ) {{
            status
            trackingNumber
            carrier
          }}
          deliverOrder(
            tenantId: "{tenant_id}",
            userId: "{actor_id}",
            id: "{order_id}",
            input: {{
              deliveredSignature: "signed-by-customer"
            }}
          ) {{
            status
            deliveredSignature
          }}
        }}
        "#
    )
}

fn admin_order_parity_query(
    tenant_id: Uuid,
    order_id: Uuid,
    payment_collection_id: Uuid,
    fulfillment_id: Uuid,
) -> String {
    format!(
        r#"
        query {{
          order(tenantId: "{tenant_id}", id: "{order_id}") {{
            order {{
              id
              status
              totalAmount
              taxTotal
              taxIncluded
              taxLines {{
                providerId
              }}
              paymentId
              paymentMethod
              trackingNumber
              carrier
              deliveredSignature
            }}
            paymentCollection {{
              id
              status
              authorizedAmount
              capturedAmount
            }}
            fulfillment {{
              id
              status
              trackingNumber
              deliveredNote
            }}
          }}
          orders(tenantId: "{tenant_id}", filter: {{ page: 1, perPage: 20, status: "delivered" }}) {{
            total
            items {{
              id
              status
              totalAmount
              taxTotal
              taxIncluded
              taxLines {{
                providerId
              }}
              trackingNumber
              deliveredSignature
            }}
          }}
          paymentCollection(tenantId: "{tenant_id}", id: "{payment_collection_id}") {{
            id
            status
            providerId
            authorizedAmount
            capturedAmount
            payments {{
              providerPaymentId
              status
              capturedAmount
            }}
          }}
          fulfillment(tenantId: "{tenant_id}", id: "{fulfillment_id}") {{
            id
            status
            trackingNumber
            deliveredNote
          }}
          paymentCollections(
            tenantId: "{tenant_id}",
            filter: {{ page: 1, perPage: 20, orderId: "{order_id}", status: "captured" }}
          ) {{
            total
            items {{
              id
              status
              orderId
            }}
          }}
          fulfillments(
            tenantId: "{tenant_id}",
            filter: {{ page: 1, perPage: 20, orderId: "{order_id}", status: "delivered" }}
          ) {{
            total
            items {{
              id
              status
              orderId
              trackingNumber
            }}
          }}
        }}
        "#
    )
}

fn admin_create_refund_mutation(
    tenant_id: Uuid,
    payment_collection_id: Uuid,
    amount: &str,
    reason: &str,
    step: &str,
) -> String {
    format!(
        r#"
        mutation {{
          createRefund(
            tenantId: "{tenant_id}",
            paymentCollectionId: "{payment_collection_id}",
            idempotencyKey: "graphql-refund-{step}",
            input: {{
              amount: "{amount}"
              reason: "{reason}"
              metadata: "{{\"source\":\"graphql-refund\",\"step\":\"{step}\"}}"
            }}
          ) {{
            id
            status
            amount
          }}
        }}
        "#
    )
}

fn admin_complete_refund_mutation(tenant_id: Uuid, refund_id: Uuid) -> String {
    format!(
        r#"
        mutation {{
          completeRefund(
            tenantId: "{tenant_id}",
            id: "{refund_id}",
            input: {{
              metadata: "{{\"source\":\"graphql-refund\",\"step\":\"complete-1\"}}"
            }}
          ) {{
            id
            status
            refundedAt
          }}
        }}
        "#
    )
}

fn admin_cancel_refund_mutation(tenant_id: Uuid, refund_id: Uuid) -> String {
    format!(
        r#"
        mutation {{
          cancelRefund(
            tenantId: "{tenant_id}",
            id: "{refund_id}",
            input: {{
              reason: "review-failed"
              metadata: "{{\"source\":\"graphql-refund\",\"step\":\"cancel-2\"}}"
            }}
          ) {{
            id
            status
            cancelledAt
            reason
          }}
        }}
        "#
    )
}

fn admin_refund_query(tenant_id: Uuid, refund_id: Uuid, payment_collection_id: Uuid) -> String {
    format!(
        r#"
        query {{
          refund(tenantId: "{tenant_id}", id: "{refund_id}") {{
            id
            status
            amount
            reason
          }}
          refunds(
            tenantId: "{tenant_id}",
            filter: {{ page: 1, perPage: 20, paymentCollectionId: "{payment_collection_id}" }}
          ) {{
            total
            items {{
              id
              status
              paymentCollectionId
            }}
          }}
          paymentCollection(tenantId: "{tenant_id}", id: "{payment_collection_id}") {{
            id
            status
            refundedAmount
            refunds {{
              id
              status
            }}
          }}
        }}
        "#
    )
}

fn admin_refunds_list_query(tenant_id: Uuid, payment_collection_id: Uuid) -> String {
    format!(
        r#"
        query {{
          refunds(
            tenantId: "{tenant_id}",
            filter: {{ page: 1, perPage: 20, paymentCollectionId: "{payment_collection_id}" }}
          ) {{
            total
            items {{
              id
              status
              paymentCollectionId
            }}
          }}
        }}
        "#
    )
}

fn admin_refunds_list_query_with_status(
    tenant_id: Uuid,
    payment_collection_id: Uuid,
    status: &str,
) -> String {
    format!(
        r#"
        query {{
          refunds(
            tenantId: "{tenant_id}",
            filter: {{
              page: 1,
              perPage: 20,
              paymentCollectionId: "{payment_collection_id}",
              status: "{status}"
            }}
          ) {{
            total
            items {{
              id
              status
              paymentCollectionId
            }}
          }}
        }}
        "#
    )
}

fn admin_refunds_list_query_with_order(tenant_id: Uuid, order_id: Uuid) -> String {
    format!(
        r#"
        query {{
          refunds(
            tenantId: "{tenant_id}",
            filter: {{
              page: 1,
              perPage: 20,
              orderId: "{order_id}"
            }}
          ) {{
            total
            items {{
              id
              status
              paymentCollectionId
            }}
          }}
        }}
        "#
    )
}

fn storefront_refunds_query(tenant_id: Uuid, order_id: Uuid) -> String {
    format!(
        r#"
        query {{
          storefrontRefunds(
            tenantId: "{tenant_id}",
            orderId: "{order_id}",
            filter: {{ page: 1, perPage: 20 }}
          ) {{
            total
            items {{
              id
              status
              paymentCollectionId
              amount
            }}
          }}
        }}
        "#
    )
}

fn storefront_refunds_query_with_paging(
    tenant_id: Uuid,
    order_id: Uuid,
    page: u64,
    per_page: u64,
) -> String {
    format!(
        r#"
        query {{
          storefrontRefunds(
            tenantId: "{tenant_id}",
            orderId: "{order_id}",
            filter: {{ page: {page}, perPage: {per_page} }}
          ) {{
            total
            page
            perPage
            items {{
              id
            }}
          }}
        }}
        "#
    )
}

fn storefront_refunds_query_with_status(tenant_id: Uuid, order_id: Uuid, status: &str) -> String {
    format!(
        r#"
        query {{
          storefrontRefunds(
            tenantId: "{tenant_id}",
            orderId: "{order_id}",
            filter: {{ page: 1, perPage: 20, status: "{status}" }}
          ) {{
            total
            items {{ id status }}
          }}
        }}
        "#
    )
}

fn admin_return_claim_decision_mutation(
    tenant_id: Uuid,
    order_id: Uuid,
    line_item_id: Uuid,
) -> String {
    format!(
        r#"
        mutation {{
          createOrderReturnDecision(
            tenantId: "{tenant_id}"
            orderId: "{order_id}"
            input: {{
              returnRequest: {{
                reason: "damaged"
                note: "claim reviewed through GraphQL"
                items: [{{
                  lineItemId: "{line_item_id}"
                  quantity: 1
                  reason: "damaged"
                  metadata: "{{\"source\":\"graphql-return-claim-decision\",\"scope\":\"item\"}}"
                }}]
                metadata: "{{\"source\":\"graphql-return-claim-decision\",\"scope\":\"return\"}}"
              }}
              decision: {{
                action: "claim"
                claim: {{
                  description: "Operator claim review"
                  preview: "{{\"claim_type\":\"damaged_item\",\"resolution\":\"review\"}}"
                  metadata: "{{\"operator\":\"claims-desk\"}}"
                }}
                metadata: "{{\"flow\":\"claim\"}}"
              }}
            }}
          ) {{
            action
            metadata
            orderReturn {{
              id
              orderId
              status
              resolutionType
              orderChangeId
              metadata
            }}
            orderChange {{
              id
              orderId
              changeType
              description
              preview
              metadata
            }}
            refund {{ id }}
          }}
        }}
        "#
    )
}

fn admin_complete_order_return_with_exchange_mutation(
    tenant_id: Uuid,
    return_id: Uuid,
    preview_json: &str,
) -> String {
    format!(
        r#"
        mutation {{
          completeOrderReturn(
            tenantId: "{tenant_id}"
            id: "{return_id}"
            input: {{
              resolutionType: "exchange"
              exchange: {{
                description: "GraphQL exchange completion"
                preview: "{preview_json}"
                metadata: "{{\"operator\":\"exchange-desk\"}}"
              }}
              metadata: "{{\"source\":\"graphql-complete-exchange\"}}"
            }}
          ) {{
            id
            status
            resolutionType
            orderChangeId
            metadata
          }}
        }}
        "#,
        preview_json = preview_json.replace("\"", "\\\"")
    )
}

fn admin_complete_order_return_with_claim_mutation(
    tenant_id: Uuid,
    return_id: Uuid,
    preview_json: &str,
) -> String {
    format!(
        r#"
        mutation {{
          completeOrderReturn(
            tenantId: "{tenant_id}"
            id: "{return_id}"
            input: {{
              resolutionType: "claim"
              claim: {{
                description: "GraphQL claim completion"
                preview: "{preview_json}"
                metadata: "{{\"operator\":\"claim-desk\"}}"
              }}
              metadata: "{{\"source\":\"graphql-complete-claim\"}}"
            }}
          ) {{
            id
            status
            resolutionType
            orderChangeId
            metadata
          }}
        }}
        "#,
        preview_json = preview_json.replace("\"", "\\\"")
    )
}

fn admin_create_fulfillment_mutation(
    tenant_id: Uuid,
    order_id: Uuid,
    order_line_item_id: Uuid,
) -> String {
    format!(
        r#"
        mutation {{
          createFulfillment(
            tenantId: "{tenant_id}",
            input: {{
              orderId: "{order_id}"
              shippingOptionId: null
              customerId: null
              carrier: null
              trackingNumber: null
              items: [{{
                orderLineItemId: "{order_line_item_id}"
                quantity: 2
                metadata: "{{\"source\":\"graphql-manual-fulfillment\"}}"
              }}]
              metadata: "{{\"source\":\"graphql-manual-fulfillment\"}}"
            }}
          ) {{
            id
            orderId
            customerId
            status
            items {{
              orderLineItemId
              quantity
            }}
            metadata
          }}
        }}
        "#
    )
}

fn admin_partial_fulfillment_progress_mutation(
    tenant_id: Uuid,
    fulfillment_id: Uuid,
    fulfillment_item_id: Uuid,
) -> String {
    format!(
        r#"
        mutation {{
          shipFulfillment(
            tenantId: "{tenant_id}",
            id: "{fulfillment_id}",
            input: {{
              carrier: "manual"
              trackingNumber: "GRAPHQL-PARTIAL"
              items: [{{
                fulfillmentItemId: "{fulfillment_item_id}"
                quantity: 2
              }}]
              metadata: "{{\"source\":\"graphql-partial-ship\"}}"
            }}
          ) {{
            status
            items {{
              id
              quantity
              shippedQuantity
              deliveredQuantity
            }}
          }}
          deliverFulfillment(
            tenantId: "{tenant_id}",
            id: "{fulfillment_id}",
            input: {{
              deliveredNote: "partial"
              items: [{{
                fulfillmentItemId: "{fulfillment_item_id}"
                quantity: 1
              }}]
              metadata: "{{\"source\":\"graphql-partial-deliver\"}}"
            }}
          ) {{
            status
            items {{
              id
              quantity
              shippedQuantity
              deliveredQuantity
            }}
            metadata
          }}
        }}
        "#
    )
}

fn admin_reopen_fulfillment_mutation(
    tenant_id: Uuid,
    fulfillment_id: Uuid,
    fulfillment_item_id: Uuid,
) -> String {
    format!(
        r#"
        mutation {{
          reopenFulfillment(
            tenantId: "{tenant_id}",
            id: "{fulfillment_id}",
            input: {{
              items: [{{
                fulfillmentItemId: "{fulfillment_item_id}"
                quantity: 1
              }}]
              metadata: "{{\"source\":\"graphql-reopen\"}}"
            }}
          ) {{
            status
            deliveredNote
            items {{
              id
              quantity
              shippedQuantity
              deliveredQuantity
            }}
            metadata
          }}
        }}
        "#
    )
}

fn admin_reship_fulfillment_mutation(
    tenant_id: Uuid,
    fulfillment_id: Uuid,
    fulfillment_item_id: Uuid,
) -> String {
    format!(
        r#"
        mutation {{
          reshipFulfillment(
            tenantId: "{tenant_id}",
            id: "{fulfillment_id}",
            input: {{
              carrier: "manual"
              trackingNumber: "GRAPHQL-RESHIP"
              items: [{{
                fulfillmentItemId: "{fulfillment_item_id}"
                quantity: 2
              }}]
              metadata: "{{\"source\":\"graphql-reship\"}}"
            }}
          ) {{
            status
            trackingNumber
            deliveredNote
            items {{
              id
              quantity
              shippedQuantity
              deliveredQuantity
            }}
            metadata
          }}
        }}
        "#
    )
}

fn storefront_customer_order_query(tenant_id: Uuid, order_id: Uuid) -> String {
    format!(
        r#"
        query {{
          storefrontMe(tenantId: "{tenant_id}") {{
            id
            email
            locale
          }}
          storefrontOrder(tenantId: "{tenant_id}", id: "{order_id}") {{
            id
            customerId
            status
            currencyCode
            taxTotal
            taxIncluded
            taxLines {{
              providerId
              lineItemId
              shippingOptionId
            }}
            totalAmount
            lineItems {{
              title
              quantity
              currencyCode
            }}
          }}
        }}
        "#
    )
}

fn storefront_checkout_mutation(tenant_id: Uuid, cart_id: Uuid) -> String {
    format!(
        r#"
        mutation {{
          createStorefrontPaymentCollection(
            tenantId: "{tenant_id}",
            input: {{
              cartId: "{cart_id}"
              metadata: "{{\"source\":\"storefront-graphql-checkout\",\"step\":\"payment\"}}"
            }}
          ) {{
            id
            status
            amount
          }}
          completeStorefrontCheckout(
            tenantId: "{tenant_id}",
            input: {{
              cartId: "{cart_id}"
              createFulfillment: true
              metadata: "{{\"source\":\"storefront-graphql-checkout\",\"step\":\"complete\"}}"
            }}
          ) {{
            cart {{
              id
              status
              selectedShippingOptionId
              deliveryGroups {{
                shippingProfileSlug
                selectedShippingOptionId
                lineItemIds
              }}
            }}
            order {{ id status }}
            paymentCollection {{ id status cartId orderId }}
            fulfillment {{ id status }}
            fulfillments {{ id status shippingOptionId }}
            context {{ locale currencyCode }}
          }}
        }}
        "#
    )
}

fn storefront_cart_flow_mutation(tenant_id: Uuid) -> String {
    format!(
        r#"
        mutation {{
          createStorefrontCart(
            tenantId: "{tenant_id}",
            input: {{
              email: "guest-cart@example.com"
              currencyCode: "eur"
              countryCode: "de"
              locale: "de"
              metadata: "{{\"source\":\"storefront-graphql-cart\",\"step\":\"create\"}}"
            }}
          ) {{
            cart {{ id status currencyCode email }}
            context {{ locale currencyCode }}
          }}
        }}
        "#,
    )
}

fn storefront_cart_add_line_item_mutation(
    tenant_id: Uuid,
    cart_id: Uuid,
    variant_id: Uuid,
) -> String {
    format!(
        r#"
        mutation {{
          addStorefrontCartLineItem(
            tenantId: "{tenant_id}",
            cartId: "{cart_id}",
            input: {{
              variantId: "{variant_id}"
              quantity: 2
              metadata: "{{\"source\":\"storefront-graphql-cart\",\"step\":\"add\"}}"
            }}
          ) {{
            id
            status
            totalAmount
            lineItems {{ id title quantity totalPrice currencyCode }}
          }}
        }}
        "#
    )
}

fn storefront_cart_update_line_item_mutation(
    tenant_id: Uuid,
    cart_id: Uuid,
    line_id: Uuid,
) -> String {
    format!(
        r#"
        mutation {{
          updateStorefrontCartLineItem(
            tenantId: "{tenant_id}",
            cartId: "{cart_id}",
            lineId: "{line_id}",
            input: {{ quantity: 3 }}
          ) {{
            id
            totalAmount
            lineItems {{ id quantity totalPrice }}
          }}
        }}
        "#
    )
}

fn storefront_cart_remove_line_item_mutation(
    tenant_id: Uuid,
    cart_id: Uuid,
    line_id: Uuid,
) -> String {
    format!(
        r#"
        mutation {{
          removeStorefrontCartLineItem(
            tenantId: "{tenant_id}",
            cartId: "{cart_id}",
            lineId: "{line_id}"
          ) {{
            id
            totalAmount
            lineItems {{ id }}
          }}
        }}
        "#
    )
}

fn storefront_cart_query(tenant_id: Uuid, cart_id: Uuid) -> String {
    format!(
        r#"
        query {{
          storefrontCart(tenantId: "{tenant_id}", id: "{cart_id}") {{
            id
            email
            status
            currencyCode
            totalAmount
            lineItems {{ id title quantity totalPrice currencyCode }}
          }}
        }}
        "#
    )
}

fn storefront_cart_context_update_mutation(
    tenant_id: Uuid,
    cart_id: Uuid,
    region_id: Uuid,
    shipping_option_id: Uuid,
) -> String {
    format!(
        r#"
        mutation {{
          updateStorefrontCartContext(
            tenantId: "{tenant_id}",
            cartId: "{cart_id}",
            input: {{
              email: null
              regionId: "{region_id}"
              selectedShippingOptionId: "{shipping_option_id}"
            }}
          ) {{
            cart {{
              id
              email
              regionId
              countryCode
              localeCode
              selectedShippingOptionId
            }}
            context {{
              locale
              currencyCode
              region {{ id }}
            }}
          }}
        }}
        "#
    )
}

fn storefront_discovery_query(tenant_id: Uuid, cart_id: Uuid) -> String {
    format!(
        r#"
        query {{
          storefrontRegions(tenantId: "{tenant_id}") {{
            id
            name
            currencyCode
          }}
          storefrontShippingOptions(
            tenantId: "{tenant_id}",
            filter: {{
              cartId: "{cart_id}"
              currencyCode: "usd"
            }}
          ) {{
            id
            name
            currencyCode
            amount
          }}
        }}
        "#
    )
}

async fn seed_tenant_context(db: &DatabaseConnection, tenant_id: Uuid) {
    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "INSERT INTO tenants (id, name, slug, domain, settings, default_locale, is_active, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        vec![
            tenant_id.into(),
            "Parity Tenant".into(),
            "parity-tenant".into(),
            sea_orm::Value::String(None),
            serde_json::json!({}).to_string().into(),
            "en".into(),
            true.into(),
        ],
    ))
    .await
    .unwrap();

    for (locale, name, native_name, is_default) in [
        ("en", "English", "English", true),
        ("de", "German", "Deutsch", false),
    ] {
        db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO tenant_locales (id, tenant_id, locale, name, native_name, is_default, is_enabled, fallback_locale, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)",
            vec![
                Uuid::new_v4().into(),
                tenant_id.into(),
                locale.into(),
                name.into(),
                native_name.into(),
                is_default.into(),
                true.into(),
                sea_orm::Value::String(None),
            ],
        ))
        .await
        .unwrap();
    }

    for module_slug in ["commerce", "product"] {
        db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO tenant_modules (id, tenant_id, module_slug, enabled, settings, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            vec![
                Uuid::new_v4().into(),
                tenant_id.into(),
                module_slug.into(),
                true.into(),
                serde_json::json!({}).to_string().into(),
            ],
        ))
        .await
        .unwrap();
    }
}

pub mod cart;
pub mod catalog;
pub mod checkout;
pub mod pricing;
pub mod shipping;
