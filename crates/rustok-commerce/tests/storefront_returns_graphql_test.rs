use async_graphql::{EmptySubscription, Request, Schema};
use rust_decimal::Decimal;
use rustok_api::{AuthContext, RequestContext, TenantContext};
use rustok_commerce::dto::{CreateCustomerInput, CreateOrderInput, CreateOrderLineItemInput};
use rustok_commerce::graphql::{CommerceMutation, CommerceQuery};
use rustok_commerce::{CustomerService, OrderService};
use rustok_test_utils::{db::setup_test_db, mock_transactional_event_bus};
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement};
use serde_json::Value;
use std::str::FromStr;
use uuid::Uuid;

mod support;

type CommerceSchema = Schema<CommerceQuery, CommerceMutation, EmptySubscription>;

fn tenant_context(tenant_id: Uuid) -> TenantContext {
    TenantContext {
        id: tenant_id,
        name: "Returns Tenant".to_string(),
        slug: "returns-tenant".to_string(),
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
    let mut builder = Schema::build(CommerceQuery, CommerceMutation, EmptySubscription)
        .data(db.clone())
        .data(event_bus)
        .data(tenant)
        .data(request_context);

    if let Some(auth) = auth {
        builder = builder.data(auth);
    }

    builder.finish()
}

async fn seed_tenant_context(db: &DatabaseConnection, tenant_id: Uuid) {
    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "INSERT INTO tenants (id, name, slug, domain, settings, default_locale, is_active, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        vec![
            tenant_id.into(),
            "Returns Tenant".into(),
            "returns-tenant".into(),
            sea_orm::Value::String(None),
            serde_json::json!({}).to_string().into(),
            "en".into(),
            true.into(),
        ],
    ))
    .await
    .expect("tenant should insert");

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
        .expect("tenant locale should insert");
    }

    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "INSERT INTO tenant_modules (id, tenant_id, module_slug, enabled, settings, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        vec![
            Uuid::new_v4().into(),
            tenant_id.into(),
            "commerce".into(),
            true.into(),
            serde_json::json!({}).to_string().into(),
        ],
    ))
    .await
    .expect("commerce module should be enabled");
}

fn storefront_create_return_mutation(
    tenant_id: Uuid,
    order_id: Uuid,
    line_item_id: Uuid,
) -> String {
    format!(
        r#"
        mutation {{
          createStorefrontOrderReturn(
            tenantId: "{tenant_id}",
            orderId: "{order_id}",
            input: {{
              reason: "damaged",
              note: "customer-visible return",
              metadata: "{{\"source\":\"storefront-graphql-return\"}}",
              items: [{{
                lineItemId: "{line_item_id}",
                quantity: 1,
                reason: "damaged-package",
                metadata: "{{\"condition\":\"opened\"}}"
              }}]
            }}
          ) {{
            id
            orderId
            status
            reason
            note
            metadata
            items {{ lineItemId quantity reason metadata }}
          }}
        }}
        "#
    )
}

fn storefront_returns_query(tenant_id: Uuid, order_id: Uuid) -> String {
    format!(
        r#"
        query {{
          storefrontReturns(
            tenantId: "{tenant_id}",
            orderId: "{order_id}",
            filter: {{ page: 1, perPage: 20, status: "PENDING" }}
          ) {{
            total
            items {{
              id
              orderId
              status
              reason
              items {{ lineItemId quantity reason }}
            }}
          }}
        }}
        "#
    )
}

async fn create_customer_order(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    user_id: Uuid,
    email: &str,
) -> (
    rustok_commerce::dto::CustomerResponse,
    rustok_commerce::dto::OrderResponse,
) {
    let customer = CustomerService::new(db.clone())
        .create_customer(
            tenant_id,
            CreateCustomerInput {
                user_id: Some(user_id),
                email: email.to_string(),
                first_name: Some("Return".to_string()),
                last_name: Some("Buyer".to_string()),
                phone: None,
                locale: Some("de".to_string()),
                metadata: serde_json::json!({ "source": "storefront-graphql-return" }),
            },
        )
        .await
        .expect("customer should be created");

    let order = OrderService::new(db.clone(), mock_transactional_event_bus())
        .create_order(
            tenant_id,
            user_id,
            CreateOrderInput {
                customer_id: Some(customer.id),
                currency_code: "eur".to_string(),
                shipping_total: Decimal::ZERO,
                line_items: vec![CreateOrderLineItemInput {
                    product_id: Some(Uuid::new_v4()),
                    variant_id: Some(Uuid::new_v4()),
                    shipping_profile_slug: "default".to_string(),
                    seller_id: None,
                    sku: Some("STOREFRONT-RETURN-1".to_string()),
                    title: "Storefront Returnable Order".to_string(),
                    quantity: 2,
                    unit_price: Decimal::from_str("30.00").expect("valid decimal"),
                    metadata: serde_json::json!({ "source": "storefront-graphql-return" }),
                }],
                adjustments: Vec::new(),
                tax_lines: Vec::new(),
                metadata: serde_json::json!({ "source": "storefront-graphql-return" }),
            },
        )
        .await
        .expect("order should be created");

    (customer, order)
}

#[tokio::test]
async fn storefront_graphql_create_and_list_returns_use_customer_order_boundary() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let customer_user_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let (_customer, order) =
        create_customer_order(&db, tenant_id, customer_user_id, "return-buyer@example.com").await;

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "de"),
        Some(customer_auth_context(tenant_id, customer_user_id)),
    );
    let created_response = schema
        .execute(Request::new(storefront_create_return_mutation(
            tenant_id,
            order.id,
            order.line_items[0].id,
        )))
        .await;
    assert!(
        created_response.errors.is_empty(),
        "unexpected storefront return mutation errors: {:?}",
        created_response.errors
    );
    let created_json = created_response
        .data
        .into_json()
        .expect("created return response should serialize");
    assert_eq!(
        created_json["createStorefrontOrderReturn"]["orderId"],
        Value::from(order.id.to_string())
    );
    assert_eq!(
        created_json["createStorefrontOrderReturn"]["status"],
        Value::from("pending")
    );
    assert_eq!(
        created_json["createStorefrontOrderReturn"]["items"][0]["lineItemId"],
        Value::from(order.line_items[0].id.to_string())
    );
    assert_eq!(
        created_json["createStorefrontOrderReturn"]["items"][0]["quantity"],
        Value::from(1)
    );

    let list_response = schema
        .execute(Request::new(storefront_returns_query(tenant_id, order.id)))
        .await;
    assert!(
        list_response.errors.is_empty(),
        "unexpected storefront returns query errors: {:?}",
        list_response.errors
    );
    let list_json = list_response
        .data
        .into_json()
        .expect("returns list response should serialize");
    assert_eq!(list_json["storefrontReturns"]["total"], Value::from(1));
    assert_eq!(
        list_json["storefrontReturns"]["items"][0]["orderId"],
        Value::from(order.id.to_string())
    );
    assert_eq!(
        list_json["storefrontReturns"]["items"][0]["items"][0]["lineItemId"],
        Value::from(order.line_items[0].id.to_string())
    );
}

#[tokio::test]
async fn storefront_graphql_create_return_rejects_foreign_customer_order() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    let owner_user_id = Uuid::new_v4();
    let foreign_user_id = Uuid::new_v4();
    seed_tenant_context(&db, tenant_id).await;

    let (_owner, order) =
        create_customer_order(&db, tenant_id, owner_user_id, "return-owner@example.com").await;
    CustomerService::new(db.clone())
        .create_customer(
            tenant_id,
            CreateCustomerInput {
                user_id: Some(foreign_user_id),
                email: "return-foreign@example.com".to_string(),
                first_name: Some("Foreign".to_string()),
                last_name: None,
                phone: None,
                locale: Some("de".to_string()),
                metadata: serde_json::json!({ "source": "storefront-graphql-return-forbidden" }),
            },
        )
        .await
        .expect("foreign customer should be created");

    let schema = build_schema(
        &db,
        tenant_context(tenant_id),
        request_context(tenant_id, "de"),
        Some(customer_auth_context(tenant_id, foreign_user_id)),
    );
    let response = schema
        .execute(Request::new(storefront_create_return_mutation(
            tenant_id,
            order.id,
            order.line_items[0].id,
        )))
        .await;

    assert_eq!(response.errors.len(), 1, "expected ownership error");
    assert!(
        response.errors[0]
            .message
            .contains("Order does not belong to the current customer"),
        "unexpected ownership error: {}",
        response.errors[0].message
    );
}
