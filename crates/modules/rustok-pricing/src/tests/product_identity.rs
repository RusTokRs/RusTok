//! Selected Product identity/correlation regressions. Source only until the maintainer runs tests.
//! The positive fixture creates Pricing tables but deliberately no Product storage.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use rust_decimal::Decimal;
use rustok_api::{PortActor, PortContext, PortError, PortErrorKind};
use rustok_core::events::MemoryTransport;
use rustok_outbox::TransactionalEventBus;
use rustok_product::{
    ProductCatalogReadPort, ProductCatalogReadProfile, ProductCatalogReadRuntime,
    ProductProjectionRequest, ProductVariantIdentitiesRequest, ProductVariantIdentityListRequest,
    ProductVariantIdentityReadPort, ProductVariantIdentitySnapshot, PublishedProductsRequest,
    VariantProductProjectionRequest,
};
use sea_orm::{
    ActiveModelTrait, ConnectOptions, ConnectionTrait, Database, DatabaseBackend,
    DatabaseConnection, Schema, Set, Statement,
};
use serde_json::json;
use uuid::Uuid;

use crate::{
    AdminProductPricingProjectionRequest, PreviewVariantDiscountRequest, PriceResolutionContext,
    PricingReadPort, PricingService, ResolveProductPricesRequest,
    StorefrontProductPricingListRequest, StorefrontProductPricingProjectionRequest,
    UpsertVariantPriceRequest, in_process_pricing_read_port, in_process_pricing_write_port,
};

const SECRET: &str = "private-provider-sql-token@example.invalid";

#[derive(Clone)]
enum IdentityReply {
    Exact,
    Fixed(Vec<ProductVariantIdentitySnapshot>),
    Missing,
    Unavailable { retryable: bool },
    Pending,
}

struct SelectedProduct {
    product: rustok_product::dto::ProductResponse,
    reply: IdentityReply,
    reads: Mutex<Vec<(PortContext, ProductVariantIdentitiesRequest)>>,
    lists: Mutex<Vec<(PortContext, ProductVariantIdentityListRequest)>>,
    catalog_reads: Mutex<Vec<(&'static str, PortContext)>>,
}

impl SelectedProduct {
    fn new(tenant_id: Uuid, product_id: Uuid, variant_ids: &[Uuid], reply: IdentityReply) -> Self {
        let time = "2026-10-30T00:00:00Z";
        let variants = variant_ids
            .iter()
            .map(|id| {
                json!({
                    "id": id, "product_id": product_id, "title": "Selected owner SKU",
                    "prices": [], "inventory_quantity": 0, "inventory_policy": "deny",
                    "in_stock": false, "position": 0
                })
            })
            .collect::<Vec<_>>();
        let product = serde_json::from_value(json!({
            "id": product_id, "tenant_id": tenant_id, "status": "active",
            "fulfillment_requirement": "digital", "product_type": "digital",
            "tags": [], "metadata": {}, "created_at": time, "updated_at": time,
            "published_at": time, "revision": 1, "images": [], "variants": variants,
            "translations": [{"locale": "en", "title": "Selected owner product", "handle": "selected-owner-product"}]
        })).expect("complete selected Product fixture");
        Self {
            product,
            reply,
            reads: Mutex::new(Vec::new()),
            lists: Mutex::new(Vec::new()),
            catalog_reads: Mutex::new(Vec::new()),
        }
    }

    fn identity(&self, variant_id: Uuid) -> ProductVariantIdentitySnapshot {
        ProductVariantIdentitySnapshot {
            tenant_id: self.product.tenant_id,
            product_id: self.product.id,
            variant_id,
        }
    }

    async fn respond(
        &self,
        ids: Vec<Uuid>,
    ) -> Result<Vec<ProductVariantIdentitySnapshot>, PortError> {
        match &self.reply {
            IdentityReply::Exact => Ok(ids.into_iter().map(|id| self.identity(id)).collect()),
            IdentityReply::Fixed(response) => Ok(response.clone()),
            IdentityReply::Missing => Err(PortError::not_found(
                "product.variant_identity_missing",
                SECRET,
            )),
            IdentityReply::Unavailable { retryable } => Err(PortError::new(
                PortErrorKind::Unavailable,
                "adapter.internal",
                SECRET,
                *retryable,
            )),
            IdentityReply::Pending => std::future::pending().await,
        }
    }

    fn runtime(self: &Arc<Self>, identity: bool) -> ProductCatalogReadRuntime {
        let runtime = ProductCatalogReadRuntime::external(self.clone());
        if identity {
            runtime.with_variant_identity_read_port(self.clone())
        } else {
            runtime
        }
    }
}

#[async_trait]
impl ProductVariantIdentityReadPort for SelectedProduct {
    async fn read_variant_identities(
        &self,
        context: PortContext,
        request: ProductVariantIdentitiesRequest,
    ) -> Result<Vec<ProductVariantIdentitySnapshot>, PortError> {
        self.reads.lock().unwrap().push((context, request.clone()));
        self.respond(request.variant_ids).await
    }

    async fn list_product_variant_identities(
        &self,
        context: PortContext,
        request: ProductVariantIdentityListRequest,
    ) -> Result<Vec<ProductVariantIdentitySnapshot>, PortError> {
        self.lists.lock().unwrap().push((context, request.clone()));
        let ids = if request.product_ids.contains(&self.product.id) {
            self.product
                .variants
                .iter()
                .map(|variant| variant.id)
                .collect()
        } else {
            Vec::new()
        };
        self.respond(ids).await
    }
}

#[async_trait]
impl ProductCatalogReadPort for SelectedProduct {
    async fn read_product_projection(
        &self,
        context: PortContext,
        _request: ProductProjectionRequest,
    ) -> Result<rustok_product::dto::ProductResponse, PortError> {
        self.catalog_reads
            .lock()
            .unwrap()
            .push(("product", context));
        Ok(self.product.clone())
    }

    async fn read_variant_product_projection(
        &self,
        context: PortContext,
        _request: VariantProductProjectionRequest,
    ) -> Result<rustok_product::dto::ProductResponse, PortError> {
        self.catalog_reads
            .lock()
            .unwrap()
            .push(("variant", context));
        Ok(self.product.clone())
    }

    async fn list_published_products(
        &self,
        context: PortContext,
        _request: PublishedProductsRequest,
    ) -> Result<rustok_product::StorefrontProductList, PortError> {
        self.catalog_reads
            .lock()
            .unwrap()
            .push(("unexpected_published", context));
        Err(PortError::invariant_violation(
            "fixture.unexpected_catalog_discovery",
            "SKU pricing must not discover another catalog",
        ))
    }

    async fn list_legacy_storefront_products(
        &self,
        context: PortContext,
        request: rustok_product::LegacyStorefrontProductsRequest,
    ) -> Result<rustok_product::LegacyStorefrontProductList, PortError> {
        self.catalog_reads
            .lock()
            .unwrap()
            .push(("storefront_page", context));
        Ok(rustok_product::LegacyStorefrontProductList {
            items: vec![rustok_product::LegacyStorefrontProductListItem {
                id: self.product.id,
                status: self.product.status.clone(),
                title: "Selected owner product".to_owned(),
                handle: "selected-owner-product".to_owned(),
                seller_id: None,
                vendor: None,
                product_type: Some("digital".to_owned()),
                shipping_profile_slug: "default".to_owned(),
                tags: Vec::new(),
                created_at: self.product.created_at,
                published_at: self.product.published_at,
            }],
            total: 1,
            page: request.page,
            per_page: request.per_page,
            has_next: false,
        })
    }

    async fn read_storefront_product_projection(
        &self,
        context: PortContext,
        _request: rustok_product::StorefrontProductProjectionRequest,
    ) -> Result<Option<rustok_product::dto::ProductResponse>, PortError> {
        self.catalog_reads
            .lock()
            .unwrap()
            .push(("storefront_detail", context));
        Ok(Some(self.product.clone()))
    }
}

fn events() -> TransactionalEventBus {
    TransactionalEventBus::new(Arc::new(MemoryTransport::new()))
}

fn context(tenant_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(Uuid::from_u128(90).to_string()),
        "en",
        "selected-product-identity-test",
    )
    .with_deadline(Duration::from_secs(2))
    .with_claim("products:read")
    .with_role("test-reader")
    .with_causation_id("test-causation")
    .with_traceparent("00-00000000000000000000000000000001-0000000000000002-01")
    .with_idempotency_key("identity-observation-only")
}

fn request(product_id: Uuid, variant_ids: Vec<Uuid>) -> ResolveProductPricesRequest {
    ResolveProductPricesRequest {
        product_id: Some(product_id),
        variant_ids,
        pricing: PriceResolutionContext {
            currency_code: "usd".to_owned(),
            quantity: Some(3),
            region_id: None,
            price_list_id: None,
            channel_id: None,
            channel_slug: None,
        },
    }
}

async fn database(with_pricing_tables: bool) -> DatabaseConnection {
    let mut options = ConnectOptions::new("sqlite::memory:");
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await.unwrap();
    if with_pricing_tables {
        let backend = DatabaseBackend::Sqlite;
        let schema = Schema::new(backend);
        for statement in [
            schema.create_table_from_entity(crate::entities::price_list::Entity),
            schema.create_table_from_entity(crate::entities::price_list_translation::Entity),
            schema.create_table_from_entity(crate::entities::price::Entity),
        ] {
            db.execute_raw(backend.build(&statement)).await.unwrap();
        }
    }
    let row = db.query_one_raw(Statement::from_string(DatabaseBackend::Sqlite,
        "SELECT COUNT(*) AS total FROM sqlite_master WHERE name IN ('products', 'product_variants')"))
        .await.unwrap().unwrap();
    assert_eq!(
        row.try_get::<i64>("", "total").unwrap(),
        0,
        "the fixture must not make local Product identity available"
    );
    db
}

async fn price(db: &DatabaseConnection, variant_id: Uuid) {
    crate::entities::price::ActiveModel {
        id: Set(Uuid::new_v4()),
        variant_id: Set(variant_id),
        price_list_id: Set(None),
        channel_id: Set(None),
        channel_slug: Set(None),
        currency_code: Set("USD".to_owned()),
        region_id: Set(None),
        amount: Set(Decimal::new(1250, 2)),
        compare_at_amount: Set(Some(Decimal::from(20))),
        legacy_amount: Set(None),
        legacy_compare_at_amount: Set(None),
        min_quantity: Set(None),
        max_quantity: Set(None),
    }
    .insert(db)
    .await
    .unwrap();
}

fn assert_context(actual: &PortContext, expected: &PortContext) {
    assert_eq!(
        serde_json::to_value(actual).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
}

#[tokio::test]
async fn selected_external_owner_prices_without_any_local_product_storage() {
    let db = database(true).await;
    let tenant = Uuid::from_u128(1);
    let product = Uuid::from_u128(2);
    let priced = Uuid::from_u128(3);
    let unpriced = Uuid::from_u128(4);
    price(&db, priced).await;
    let owner = Arc::new(SelectedProduct::new(
        tenant,
        product,
        &[priced, unpriced],
        IdentityReply::Exact,
    ));
    let selected = owner.runtime(true);
    assert_eq!(selected.profile(), ProductCatalogReadProfile::External);
    let identity = selected.variant_identity_read_port().unwrap();
    assert!(Arc::ptr_eq(
        &identity,
        &selected.clone().variant_identity_read_port().unwrap()
    ));
    let ctx = context(tenant);
    let input = request(product, vec![unpriced, priced]);
    let snapshot = in_process_pricing_read_port(db, events(), selected)
        .resolve_product_prices(ctx.clone(), input.clone())
        .await
        .unwrap();
    assert_eq!(snapshot.items.len(), 2);
    assert_eq!(snapshot.items[0].variant_id, unpriced);
    assert!(snapshot.items[0].price.is_none());
    assert_eq!(snapshot.items[1].variant_id, priced);
    assert_eq!(snapshot.items[1].product_id, product);
    assert_eq!(
        snapshot.items[1].price.as_ref().unwrap().amount,
        Decimal::new(1250, 2)
    );
    assert_eq!(snapshot.context.currency_code, "USD");
    assert_eq!(snapshot.context.quantity, Some(3));
    let calls = owner.reads.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_context(&calls[0].0, &ctx);
    assert_eq!(calls[0].1.variant_ids, input.variant_ids);
    assert_eq!(calls[0].1.product_id, Some(product));
    assert!(owner.catalog_reads.lock().unwrap().is_empty());
}

#[tokio::test]
async fn missing_selected_identity_capability_is_not_an_embedded_fallback() {
    let tenant = Uuid::from_u128(1);
    let product = Uuid::from_u128(2);
    let sku = Uuid::from_u128(3);
    let owner = Arc::new(SelectedProduct::new(
        tenant,
        product,
        &[sku],
        IdentityReply::Exact,
    ));
    let error = in_process_pricing_read_port(database(false).await, events(), owner.runtime(false))
        .resolve_product_prices(context(tenant), request(product, vec![sku]))
        .await
        .unwrap_err();
    assert_eq!(error.kind, PortErrorKind::Unavailable);
    assert_eq!(
        error.code,
        "pricing.product_identity_capability_unavailable"
    );
    assert!(!error.retryable);
    assert!(owner.reads.lock().unwrap().is_empty());
    assert!(owner.catalog_reads.lock().unwrap().is_empty());
}

#[tokio::test]
async fn malformed_selected_identity_never_reaches_price_storage() {
    let tenant = Uuid::from_u128(1);
    let product = Uuid::from_u128(2);
    let ids = vec![Uuid::from_u128(3), Uuid::from_u128(4)];
    let valid = ids
        .iter()
        .map(|id| ProductVariantIdentitySnapshot {
            tenant_id: tenant,
            product_id: product,
            variant_id: *id,
        })
        .collect::<Vec<_>>();
    let mut cases = vec![
        Vec::new(),
        vec![valid[0].clone()],
        vec![valid[1].clone(), valid[0].clone()],
        vec![valid[0].clone(), valid[0].clone()],
    ];
    let mut extra = valid.clone();
    extra.push(valid[0].clone());
    cases.push(extra);
    for field in 0..4 {
        let mut value = valid.clone();
        match field {
            0 => value[0].tenant_id = Uuid::from_u128(99),
            1 => value[0].product_id = Uuid::from_u128(99),
            2 => value[0].variant_id = Uuid::from_u128(99),
            _ => value[0].product_id = Uuid::nil(),
        }
        cases.push(value);
    }
    for malformed in cases {
        let owner = Arc::new(SelectedProduct::new(
            tenant,
            product,
            &ids,
            IdentityReply::Fixed(malformed),
        ));
        let error =
            in_process_pricing_read_port(database(false).await, events(), owner.runtime(true))
                .resolve_product_prices(context(tenant), request(product, ids.clone()))
                .await
                .unwrap_err();
        assert_eq!(error.kind, PortErrorKind::InvariantViolation);
        assert_eq!(error.code, "pricing.variant_identity_invalid");
        assert_eq!(owner.reads.lock().unwrap().len(), 1);
        assert!(owner.catalog_reads.lock().unwrap().is_empty());
    }
}

#[tokio::test]
async fn selected_owner_failures_preserve_kind_retryability_and_do_not_reselect() {
    let tenant = Uuid::from_u128(1);
    let product = Uuid::from_u128(2);
    let sku = Uuid::from_u128(3);
    for (reply, kind, retryable) in [
        (IdentityReply::Missing, PortErrorKind::NotFound, false),
        (
            IdentityReply::Unavailable { retryable: false },
            PortErrorKind::Unavailable,
            false,
        ),
        (
            IdentityReply::Unavailable { retryable: true },
            PortErrorKind::Unavailable,
            true,
        ),
    ] {
        let owner = Arc::new(SelectedProduct::new(tenant, product, &[sku], reply));
        let ctx = context(tenant).with_channel("selected-channel");
        let error =
            in_process_pricing_read_port(database(false).await, events(), owner.runtime(true))
                .resolve_product_prices(ctx.clone(), request(product, vec![sku]))
                .await
                .unwrap_err();
        assert_eq!(error.kind, kind);
        assert_eq!(error.retryable, retryable);
        assert!(!error.message.contains(SECRET));
        assert_eq!(owner.reads.lock().unwrap().len(), 1);
        assert_context(&owner.reads.lock().unwrap()[0].0, &ctx);
    }
}

#[tokio::test]
async fn selected_identity_wait_is_bounded_by_the_original_read_budget() {
    let tenant = Uuid::from_u128(1);
    let product = Uuid::from_u128(2);
    let sku = Uuid::from_u128(3);
    let owner = Arc::new(SelectedProduct::new(
        tenant,
        product,
        &[sku],
        IdentityReply::Pending,
    ));
    let ctx = context(tenant).with_deadline(Duration::from_millis(10));
    let error = in_process_pricing_read_port(database(false).await, events(), owner.runtime(true))
        .resolve_product_prices(ctx.clone(), request(product, vec![sku]))
        .await
        .unwrap_err();
    assert_eq!(error.kind, PortErrorKind::Timeout);
    assert_context(&owner.reads.lock().unwrap()[0].0, &ctx);
    assert!(owner.catalog_reads.lock().unwrap().is_empty());
}

#[tokio::test]
async fn inspection_page_uses_the_same_selected_catalog_and_identity_owner() {
    let tenant = Uuid::from_u128(1);
    let product = Uuid::from_u128(2);
    let ids = vec![Uuid::from_u128(3), Uuid::from_u128(4)];
    let owner = Arc::new(SelectedProduct::new(
        tenant,
        product,
        &ids,
        IdentityReply::Exact,
    ));
    let db = database(true).await;
    price(&db, ids[0]).await;
    let ctx = context(tenant);
    let page = in_process_pricing_read_port(db, events(), owner.runtime(true))
        .list_storefront_product_pricing(
            ctx.clone(),
            StorefrontProductPricingListRequest {
                fallback_locale: Some("en".to_owned()),
                channel_id: None,
                channel_slug: None,
                page: 1,
                per_page: 24,
            },
        )
        .await
        .unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].title, "Selected owner product");
    assert_eq!(page.items[0].variant_count, 2);
    assert_eq!(page.items[0].sale_variant_count, 1);
    assert_eq!(page.items[0].currencies, vec!["USD".to_owned()]);
    let calls = owner.catalog_reads.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "storefront_page");
    assert_context(&calls[0].1, &ctx);
    let lists = owner.lists.lock().unwrap();
    assert_eq!(lists.len(), 1);
    assert_context(&lists[0].0, &ctx);
    assert_eq!(lists[0].1.product_ids, vec![product]);
}

#[tokio::test]
async fn inspection_page_rejects_foreign_or_duplicate_owner_skus_before_price_io() {
    let tenant = Uuid::from_u128(1);
    let product = Uuid::from_u128(2);
    let sku = Uuid::from_u128(3);
    let valid = ProductVariantIdentitySnapshot {
        tenant_id: tenant,
        product_id: product,
        variant_id: sku,
    };
    for response in [
        vec![ProductVariantIdentitySnapshot {
            tenant_id: Uuid::from_u128(99),
            ..valid.clone()
        }],
        vec![ProductVariantIdentitySnapshot {
            product_id: Uuid::from_u128(99),
            ..valid.clone()
        }],
        vec![valid.clone(), valid],
    ] {
        let owner = Arc::new(SelectedProduct::new(
            tenant,
            product,
            &[sku],
            IdentityReply::Fixed(response),
        ));
        let error =
            in_process_pricing_read_port(database(false).await, events(), owner.runtime(true))
                .list_storefront_product_pricing(
                    context(tenant),
                    StorefrontProductPricingListRequest {
                        fallback_locale: None,
                        channel_id: None,
                        channel_slug: None,
                        page: 1,
                        per_page: 24,
                    },
                )
                .await
                .unwrap_err();
        assert_eq!(error.kind, PortErrorKind::InvariantViolation);
        assert_eq!(error.code, "pricing.variant_identity_invalid");
    }
}

#[tokio::test]
async fn admin_projection_uses_selected_catalog_and_retains_full_caller_context() {
    let tenant = Uuid::from_u128(1);
    let product = Uuid::from_u128(2);
    let sku = Uuid::from_u128(3);
    let owner = Arc::new(SelectedProduct::new(
        tenant,
        product,
        &[sku],
        IdentityReply::Exact,
    ));
    let db = database(true).await;
    price(&db, sku).await;
    let ctx = context(tenant);
    let detail = in_process_pricing_read_port(db, events(), owner.runtime(true))
        .read_admin_product_pricing_projection(
            ctx.clone(),
            AdminProductPricingProjectionRequest {
                product_id: product,
                fallback_locale: Some("en".to_owned()),
                selected_price_list_id: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(detail.id, product);
    assert_eq!(detail.variants.len(), 1);
    assert_eq!(detail.variants[0].prices[0].amount, Decimal::new(1250, 2));
    assert_context(&owner.catalog_reads.lock().unwrap()[0].1, &ctx);
}

#[tokio::test]
async fn admin_projection_refuses_another_tenant_or_product_before_price_io() {
    let tenant = Uuid::from_u128(1);
    let product = Uuid::from_u128(2);
    let sku = Uuid::from_u128(3);
    for foreign_tenant in [false, true] {
        let owner = Arc::new(SelectedProduct::new(
            if foreign_tenant {
                Uuid::from_u128(99)
            } else {
                tenant
            },
            if foreign_tenant {
                product
            } else {
                Uuid::from_u128(99)
            },
            &[sku],
            IdentityReply::Exact,
        ));
        let error =
            in_process_pricing_read_port(database(false).await, events(), owner.runtime(true))
                .read_admin_product_pricing_projection(
                    context(tenant),
                    AdminProductPricingProjectionRequest {
                        product_id: product,
                        fallback_locale: None,
                        selected_price_list_id: None,
                    },
                )
                .await
                .unwrap_err();
        assert_eq!(error.kind, PortErrorKind::InvariantViolation);
        assert_eq!(error.code, "pricing.catalog_identity_invalid");
    }
}

#[tokio::test]
async fn storefront_projection_refuses_a_different_stable_link_without_local_substitution() {
    let tenant = Uuid::from_u128(1);
    let product = Uuid::from_u128(2);
    let sku = Uuid::from_u128(3);
    let owner = Arc::new(SelectedProduct::new(
        tenant,
        product,
        &[sku],
        IdentityReply::Exact,
    ));
    let error = in_process_pricing_read_port(database(false).await, events(), owner.runtime(true))
        .read_storefront_product_pricing_projection(
            context(tenant),
            StorefrontProductPricingProjectionRequest {
                handle: "another-product".to_owned(),
                fallback_locale: Some("en".to_owned()),
                public_channel_id: None,
                public_channel_slug: None,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(error.kind, PortErrorKind::InvariantViolation);
    assert_eq!(
        owner.catalog_reads.lock().unwrap()[0].0,
        "storefront_detail"
    );
}

#[tokio::test]
async fn external_variant_authoring_refuses_before_local_locks_even_when_identity_reads_exist() {
    let tenant = Uuid::from_u128(1);
    let product = Uuid::from_u128(2);
    let sku = Uuid::from_u128(3);
    let owner = Arc::new(SelectedProduct::new(
        tenant,
        product,
        &[sku],
        IdentityReply::Exact,
    ));
    let error = in_process_pricing_write_port(database(false).await, events(), owner.runtime(true))
        .upsert_variant_price(
            context(tenant),
            UpsertVariantPriceRequest {
                variant_id: sku,
                price_list_id: None,
                currency_code: "USD".to_owned(),
                amount: Decimal::from(10),
                compare_at_amount: None,
                channel_id: None,
                channel_slug: None,
                min_quantity: None,
                max_quantity: None,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(error.kind, PortErrorKind::Unavailable);
    assert_eq!(error.code, "pricing.product_write_capability_unavailable");
    assert!(!error.retryable);
    assert!(owner.reads.lock().unwrap().is_empty());
    assert!(owner.catalog_reads.lock().unwrap().is_empty());
}

#[tokio::test]
async fn embedded_discount_preview_must_prove_tenant_ownership_before_reading_price_rows() {
    let tenant = Uuid::from_u128(1);
    let product = Uuid::from_u128(2);
    let sku = Uuid::from_u128(3);
    let owner = Arc::new(SelectedProduct::new(
        Uuid::from_u128(99),
        product,
        &[sku],
        IdentityReply::Exact,
    ));
    let selected =
        ProductCatalogReadRuntime::new(owner.clone(), ProductCatalogReadProfile::EmbeddedNative)
            .with_variant_identity_read_port(owner.clone());
    let error = in_process_pricing_read_port(database(false).await, events(), selected)
        .preview_variant_discount(
            context(tenant),
            PreviewVariantDiscountRequest {
                variant_id: sku,
                price_list_id: None,
                currency_code: "USD".to_owned(),
                discount_percent: Decimal::from(10),
                channel_id: None,
                channel_slug: None,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(error.kind, PortErrorKind::InvariantViolation);
    assert_eq!(error.code, "pricing.variant_identity_invalid");
}

#[tokio::test]
async fn module_path_pricing_factories_keep_the_admitted_facade_and_selected_runtime() {
    let tenant = Uuid::from_u128(1);
    let product = Uuid::from_u128(2);
    let sku = Uuid::from_u128(3);
    let owner = Arc::new(SelectedProduct::new(
        tenant,
        product,
        &[sku],
        IdentityReply::Missing,
    ));
    let error = crate::ports::in_process_pricing_read_port(
        database(false).await,
        events(),
        owner.runtime(true),
    )
    .resolve_product_prices(context(tenant), request(product, vec![sku]))
    .await
    .unwrap_err();
    assert_eq!(error.kind, PortErrorKind::NotFound);
    assert_eq!(owner.reads.lock().unwrap().len(), 1);
    let service = PricingService::new(database(false).await, events())
        .with_product_catalog_read_runtime(owner.runtime(true));
    let error = crate::InProcessPricingReadPort::from_service(service)
        .resolve_product_prices(context(tenant), request(product, vec![sku]))
        .await
        .unwrap_err();
    assert_eq!(error.kind, PortErrorKind::NotFound);
}

#[tokio::test]
async fn foreign_price_list_refusal_is_not_a_missing_product_projection() {
    let tenant = Uuid::from_u128(1);
    let product = Uuid::from_u128(2);
    let sku = Uuid::from_u128(3);
    let owner = Arc::new(SelectedProduct::new(
        tenant,
        product,
        &[sku],
        IdentityReply::Exact,
    ));
    let db = database(true).await;
    let list_id = Uuid::from_u128(50);
    let now = chrono::Utc::now().fixed_offset();
    crate::entities::price_list::ActiveModel {
        id: Set(list_id),
        tenant_id: Set(Uuid::from_u128(99)),
        r#type: Set("sale".to_owned()),
        status: Set("active".to_owned()),
        channel_id: Set(None),
        channel_slug: Set(None),
        rule_kind: Set(None),
        adjustment_percent: Set(None),
        starts_at: Set(None),
        ends_at: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(&db)
    .await
    .unwrap();
    let error = in_process_pricing_read_port(db, events(), owner.runtime(true))
        .read_admin_product_pricing_projection(
            context(tenant),
            AdminProductPricingProjectionRequest {
                product_id: product,
                fallback_locale: None,
                selected_price_list_id: Some(list_id),
            },
        )
        .await
        .unwrap_err();
    assert_eq!(error.kind, PortErrorKind::NotFound);
    assert_eq!(error.code, "pricing.price_list_not_found");
    assert_ne!(error.code, "pricing.product_not_found");
}

#[tokio::test]
async fn storefront_stable_link_keeps_product_owner_any_locale_lookup_behavior() {
    let tenant = Uuid::from_u128(1);
    let product = Uuid::from_u128(2);
    let sku = Uuid::from_u128(3);
    let mut selected = SelectedProduct::new(tenant, product, &[sku], IdentityReply::Exact);
    selected
        .product
        .translations
        .push(rustok_product::dto::ProductTranslationResponse {
            locale: "fr".to_owned(),
            title: "Produit choisi".to_owned(),
            handle: "produit-choisi".to_owned(),
            description: None,
            meta_title: None,
            meta_description: None,
        });
    let owner = Arc::new(selected);
    let detail = in_process_pricing_read_port(database(false).await, events(), owner.runtime(true))
        .read_storefront_product_pricing_projection(
            context(tenant),
            StorefrontProductPricingProjectionRequest {
                handle: "produit-choisi".to_owned(),
                fallback_locale: Some("en".to_owned()),
                public_channel_id: None,
                public_channel_slug: None,
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(detail.id, product);
    assert_eq!(
        owner.catalog_reads.lock().unwrap()[0].0,
        "storefront_detail"
    );
}
