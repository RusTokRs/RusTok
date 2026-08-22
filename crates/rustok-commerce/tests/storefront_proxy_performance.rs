use axum::{
    Extension, Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use rust_decimal::Decimal;
use rustok_api::{TenantContext, context::TenantContextExtension};
use rustok_commerce::controllers::{CommerceHttpRuntime, store};
use rustok_product::{
    CatalogService,
    dto::{CreateProductInput, CreateVariantInput, PriceInput, ProductTranslationInput},
};
use rustok_taxonomy::entities::{taxonomy_term_route_key, translation_change};
use rustok_test_utils::{db::setup_test_db, mock_transactional_event_bus};
use sea_orm::{ConnectionTrait, DatabaseBackend, DbBackend, Schema, Statement};
use serde_json::{Value, json};
use std::{
    fs,
    str::FromStr,
    time::{Duration, Instant},
};
use tokio::task::JoinSet;
use tower::ServiceExt;
use uuid::Uuid;

mod support;

const PRODUCT_COUNT: usize = 100;
const REQUEST_COUNT: usize = 256;
const WARMUP_REQUESTS: usize = 16;
const CONCURRENCY: usize = 16;
const SEARCH_GROUPS: usize = 10;

async fn ensure_proxy_taxonomy_extension_schema(db: &sea_orm::DatabaseConnection) {
    assert_eq!(db.get_database_backend(), DbBackend::Sqlite);
    let builder = db.get_database_backend();
    let schema = Schema::new(builder);

    for mut statement in [
        schema.create_table_from_entity(taxonomy_term_route_key::Entity),
        schema.create_table_from_entity(translation_change::Entity),
    ] {
        statement.if_not_exists();
        db.execute(builder.build(&statement))
            .await
            .expect("taxonomy extension table should exist for proxy catalog lifecycle");
    }
}

async fn seed_tenant(db: &sea_orm::DatabaseConnection, tenant_id: Uuid) {
    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "INSERT INTO tenants (id, name, slug, domain, settings, default_locale, is_active, created_at, updated_at)\n         VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        vec![
            tenant_id.into(),
            "Proxy Benchmark Tenant".into(),
            format!("proxy-benchmark-{tenant_id}").into(),
            sea_orm::Value::String(None),
            json!({}).to_string().into(),
            "en".into(),
            true.into(),
        ],
    ))
    .await
    .expect("benchmark tenant should be inserted");

    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "INSERT INTO tenant_locales (id, tenant_id, locale, name, native_name, is_default, is_enabled, fallback_locale, created_at)\n         VALUES (?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)",
        vec![
            Uuid::new_v4().into(),
            tenant_id.into(),
            "en".into(),
            "English".into(),
            "English".into(),
            true.into(),
            true.into(),
            sea_orm::Value::String(None),
        ],
    ))
    .await
    .expect("benchmark tenant locale should be inserted");

    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "INSERT INTO tenant_modules (id, tenant_id, module_slug, enabled, settings, created_at, updated_at)\n         VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        vec![
            Uuid::new_v4().into(),
            tenant_id.into(),
            "commerce".into(),
            true.into(),
            json!({}).to_string().into(),
        ],
    ))
    .await
    .expect("commerce module should be enabled for benchmark tenant");
}

fn fixture(index: usize) -> CreateProductInput {
    let group = index % SEARCH_GROUPS;
    CreateProductInput {
        translations: vec![ProductTranslationInput {
            locale: "en".to_string(),
            title: format!("Proxy Product {index:04} bench-group-{group:02}"),
            description: Some(format!("Proxy storefront benchmark product {index:04}")),
            handle: Some(format!("proxy-product-{index:04}")),
            meta_title: None,
            meta_description: None,
        }],
        options: vec![],
        variants: vec![CreateVariantInput {
            sku: Some(format!("PROXY-SKU-{index:04}")),
            barcode: None,
            shipping_profile_slug: None,
            option1: Some("Default".to_string()),
            option2: None,
            option3: None,
            prices: vec![PriceInput {
                currency_code: "USD".to_string(),
                channel_id: None,
                channel_slug: None,
                amount: Decimal::from_str("19.99").expect("valid decimal"),
                compare_at_amount: None,
            }],
            inventory_quantity: 1000,
            inventory_policy: "deny".to_string(),
            weight: None,
            weight_unit: None,
        }],
        seller_id: None,
        vendor: Some("Proxy Vendor".to_string()),
        product_type: Some("benchmark".to_string()),
        shipping_profile_slug: None,
        primary_category_id: None,
        tags: vec!["benchmark".to_string()],
        metadata: json!({ "benchmark": true }),
        publish: false,
    }
}

async fn request_once(app: Router, tenant_id: Uuid, uri: String) -> (Duration, usize, StatusCode) {
    let request = Request::builder()
        .method("GET")
        .uri(uri)
        .header("X-Tenant-ID", tenant_id.to_string())
        .header("Accept-Language", "en")
        .body(Body::empty())
        .expect("benchmark request");
    let started = Instant::now();
    let response = app
        .oneshot(request)
        .await
        .expect("benchmark request should complete");
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("benchmark response body should read");
    (started.elapsed(), body.len(), status)
}

async fn validate_json(app: Router, tenant_id: Uuid, uri: &str) -> Value {
    let request = Request::builder()
        .method("GET")
        .uri(uri)
        .header("X-Tenant-ID", tenant_id.to_string())
        .header("Accept-Language", "en")
        .body(Body::empty())
        .expect("validation request");
    let response = app
        .oneshot(request)
        .await
        .expect("validation request should complete");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("validation body should read");
    serde_json::from_slice(&body).expect("validation response should be JSON")
}

fn percentile_ms(samples: &[Duration], percentile: f64) -> f64 {
    let mut nanos = samples.iter().map(Duration::as_nanos).collect::<Vec<_>>();
    nanos.sort_unstable();
    let index = (((nanos.len() - 1) as f64) * percentile).ceil() as usize;
    nanos[index] as f64 / 1_000_000.0
}

async fn benchmark_uri(app: Router, tenant_id: Uuid, uri: &str) -> Value {
    for _ in 0..WARMUP_REQUESTS {
        let (_, _, status) = request_once(app.clone(), tenant_id, uri.to_string()).await;
        assert_eq!(status, StatusCode::OK);
    }

    let started = Instant::now();
    let mut samples = Vec::with_capacity(REQUEST_COUNT);
    let mut bytes = 0usize;
    let mut completed = 0usize;

    while completed < REQUEST_COUNT {
        let batch = (REQUEST_COUNT - completed).min(CONCURRENCY);
        let mut tasks = JoinSet::new();
        for _ in 0..batch {
            let app = app.clone();
            let uri = uri.to_string();
            tasks.spawn(request_once(app, tenant_id, uri));
        }
        while let Some(result) = tasks.join_next().await {
            let (elapsed, body_bytes, status) = result.expect("benchmark task should join");
            assert_eq!(status, StatusCode::OK);
            samples.push(elapsed);
            bytes += body_bytes;
        }
        completed += batch;
    }

    let wall = started.elapsed();
    let rps = REQUEST_COUNT as f64 / wall.as_secs_f64();
    let cores = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1);

    json!({
        "requests": REQUEST_COUNT,
        "concurrency": CONCURRENCY,
        "wall_ms": wall.as_secs_f64() * 1000.0,
        "rps": rps,
        "rps_per_available_core": rps / cores as f64,
        "p50_ms": percentile_ms(&samples, 0.50),
        "p95_ms": percentile_ms(&samples, 0.95),
        "p99_ms": percentile_ms(&samples, 0.99),
        "average_response_bytes": bytes as f64 / REQUEST_COUNT as f64,
    })
}

fn proc_status_kib(field: &str) -> Option<u64> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    status.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        if key != field {
            return None;
        }
        value
            .split_whitespace()
            .next()
            .and_then(|number| number.parse::<u64>().ok())
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "proxy performance benchmark; run explicitly in the proxy-performance workflow"]
async fn storefront_sqlite_proxy_benchmark() {
    let db = setup_test_db().await;
    support::ensure_commerce_schema(&db).await;
    ensure_proxy_taxonomy_extension_schema(&db).await;
    let tenant_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id).await;
    let actor_id = Uuid::new_v4();
    let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
    let mut selected_product_id = None;

    for index in 0..PRODUCT_COUNT {
        let created = catalog
            .create_product(tenant_id, actor_id, fixture(index))
            .await
            .expect("proxy product should be created");
        let published = catalog
            .publish_product(tenant_id, actor_id, created.id)
            .await
            .expect("proxy product should be published");
        if index == PRODUCT_COUNT / 2 {
            selected_product_id = Some(published.id);
        }
    }

    let tenant = TenantContext {
        id: tenant_id,
        name: "Proxy Benchmark Tenant".to_string(),
        slug: format!("proxy-benchmark-{tenant_id}"),
        domain: None,
        settings: json!({}),
        default_locale: "en".to_string(),
        is_active: true,
    };
    let runtime = CommerceHttpRuntime::in_process(db, mock_transactional_event_bus());
    let app = Router::new()
        .nest("/store", store::axum_router())
        .with_state(runtime)
        .layer(Extension(TenantContextExtension(tenant)));

    let selected_product_id = selected_product_id.expect("selected product id");
    let search = validate_json(
        app.clone(),
        tenant_id,
        "/store/products?page=1&per_page=24&search=bench-group-00",
    )
    .await;
    assert_eq!(
        search["meta"]["total"],
        json!(PRODUCT_COUNT / SEARCH_GROUPS),
        "proxy search must preserve deterministic cardinality"
    );

    let catalog_result = benchmark_uri(
        app.clone(),
        tenant_id,
        "/store/products?page=1&per_page=24",
    )
    .await;
    let detail_result = benchmark_uri(
        app.clone(),
        tenant_id,
        &format!("/store/products/{selected_product_id}"),
    )
    .await;
    let search_result = benchmark_uri(
        app,
        tenant_id,
        "/store/products?page=1&per_page=24&search=bench-group-00",
    )
    .await;

    let cores = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1);
    let report = json!({
        "contract": "rustok_storefront_sqlite_proxy_v1",
        "scope": "in-process Axum transport + real commerce controllers/services + SQLite; no TCP, PostgreSQL, Redis, search service, reverse proxy, or Magento instance",
        "dataset": {
            "products": PRODUCT_COUNT,
            "search_groups": SEARCH_GROUPS,
            "expected_search_matches": PRODUCT_COUNT / SEARCH_GROUPS,
        },
        "runner": {
            "available_parallelism": cores,
            "tokio_worker_threads": 4,
        },
        "operations": {
            "catalog": catalog_result,
            "product": detail_result,
            "search": search_result,
        },
        "process": {
            "vm_rss_mib": proc_status_kib("VmRSS").map(|value| value as f64 / 1024.0),
            "vm_hwm_mib": proc_status_kib("VmHWM").map(|value| value as f64 / 1024.0),
        },
        "publication_rule": "proxy-only; never publish as RusTok-vs-Magento e2e evidence",
    });

    println!("RUSTOK_PROXY_PERF_JSON={report}");
}
