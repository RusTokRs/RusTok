#![cfg(feature = "mod-commerce")]

use std::{net::SocketAddr, sync::Arc, time::Duration};

use axum::{Router, http::StatusCode};
use rustok_build::DeploymentProfile;
use rustok_channel::{ChannelService, CreateChannelInput};
use rustok_core::events::EventTransport;
use rustok_migrations::SqliteTestMigrator;
use rustok_server::{
    auth::AuthConfig,
    common::settings::{RuntimeHostMode, RustokSettings, TenantRuntimeProfile},
    middleware::{rate_limit::PathRateLimitMiddlewareState, tenant},
    models::tenants,
    modules::{DeploymentSurfaceContract, build_registry},
    routes::build_application_router,
    services::{
        app_router::compose_application_router,
        app_runtime::AppRuntimeBootstrap,
        cache_runtime::ensure_cache_service,
        event_bus::transactional_event_bus_from_context,
        graphql_schema::init_graphql_schema,
        marketplace_catalog_adapter::ServerMarketplaceCatalog,
        module_event_dispatcher::build_shared_runtime_extensions_with_host_providers,
        product_catalog_deployment::configure_product_catalog_deployment,
        server_runtime_context::{ServerAuthRuntime, ServerRuntimeContext},
    },
};
use sea_orm::{ActiveModelTrait, ConnectionTrait, Database, DatabaseBackend, Set, Statement};
use sea_orm_migration::MigratorTrait;
use serde_json::{Value, json};
use tokio::{net::TcpListener, sync::Notify};
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

#[tokio::test]
#[ignore = "requires the standalone Product and Taxonomy process fixture"]
async fn mounted_storefront_lists_use_the_selected_product_process() -> TestResult<()> {
    let tenant_id: Uuid = std::env::var("RUSTOK_PRODUCT_TEST_TENANT_ID")?.parse()?;
    let product_id: Uuid = std::env::var("RUSTOK_PRODUCT_TEST_PRODUCT_ID")?.parse()?;
    let taxonomy_available: bool =
        std::env::var("RUSTOK_PRODUCT_TEST_TAXONOMY_AVAILABLE")?.parse()?;
    let app = compose_storefront(tenant_id).await?;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("http://{}", listener.local_addr()?);
    let stop = Arc::new(Notify::new());
    let shutdown = stop.clone();
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(async move { shutdown.notified().await })
        .await
    });
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(10))
        .build()?;
    let rest = client
        .get(format!(
            "{endpoint}/store/products?locale=de&page=2&per_page=1"
        ))
        .header("X-Tenant-ID", tenant_id.to_string())
        .send()
        .await?;
    let graphql = client
        .post(format!("{endpoint}/api/graphql"))
        .header("X-Tenant-ID", tenant_id.to_string())
        .json(&json!({
            "query": "{ storefrontProducts(locale: \"de\", filter: { page: 2, perPage: 1 }) { items { id title tags } total } }"
        }))
        .send().await?;
    let (rest_status, rest_body) = response_json(rest).await?;
    let (graphql_status, graphql_body) = response_json(graphql).await?;
    stop.notify_one();
    server.await??;

    assert_eq!(graphql_status, StatusCode::OK, "{graphql_body}");
    if taxonomy_available {
        assert_eq!(rest_status, StatusCode::OK, "{rest_body}");
        assert!(graphql_body.get("errors").is_none(), "{graphql_body}");
        let expected = json!({
            "id": product_id,
            "title": "Published fixture",
            "tags": ["Remote tag"],
        });
        for item in [
            &rest_body["data"][0],
            &graphql_body["data"]["storefrontProducts"]["items"][0],
        ] {
            assert_eq!(item["id"], expected["id"], "{item}");
            assert_eq!(item["title"], expected["title"], "{item}");
            assert_eq!(item["tags"], expected["tags"], "{item}");
        }
        assert_eq!(rest_body["meta"]["total"], 2);
        assert_eq!(graphql_body["data"]["storefrontProducts"]["total"], 2);
    } else {
        assert_eq!(rest_status, StatusCode::SERVICE_UNAVAILABLE, "{rest_body}");
        assert_eq!(
            rest_body["code"], "commerce_store_product_unavailable",
            "{rest_body}"
        );
        assert_eq!(rest_body["retryable"], true, "{rest_body}");
        assert_eq!(
            graphql_body["errors"][0]["extensions"]["code"], "PRODUCT_TEMPORARILY_UNAVAILABLE",
            "{graphql_body}"
        );
        assert_eq!(
            graphql_body["errors"][0]["extensions"]["retryable"], true,
            "{graphql_body}"
        );
        assert!(graphql_body["data"]["storefrontProducts"].is_null());
    }
    println!("PRODUCT_REMOTE_STOREFRONT_HTTP_OK");
    Ok(())
}

async fn compose_storefront(tenant_id: Uuid) -> TestResult<Router> {
    let db = Database::connect("sqlite::memory:").await?;
    SqliteTestMigrator::up(&db, None).await?;
    let now = chrono::Utc::now().fixed_offset();
    tenants::ActiveModel {
        id: Set(tenant_id),
        name: Set("Remote catalog consumer".into()),
        slug: Set("catalog-consumer".into()),
        domain: Set(None),
        settings: Set(json!({})),
        default_locale: Set("en".into()),
        is_active: Set(true),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(&db)
    .await?;
    for slug in ["product", "commerce"] {
        db.execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO tenant_modules (id, tenant_id, module_slug, enabled, settings, created_at, updated_at)
             VALUES (?, ?, ?, TRUE, '{}', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            [Uuid::new_v4().into(), tenant_id.into(), slug.into()],
        )).await?;
    }
    let channels = ChannelService::new(db.clone());
    let web = channels
        .create_channel(CreateChannelInput {
            tenant_id,
            slug: "web".into(),
            name: "Web".into(),
            settings: None,
        })
        .await?;
    channels.set_default_channel(web.id).await?;

    let mut settings = RustokSettings::default();
    settings.runtime.host_mode = RuntimeHostMode::Api;
    settings.tenant.profile = TenantRuntimeProfile::Development;
    let ctx = ServerRuntimeContext::new(db.clone(), settings.clone());
    let cache = ensure_cache_service(&ctx);
    tenant::init_tenant_cache_infrastructure(&ctx, &cache).await?;
    ctx.shared_insert(Arc::new(rustok_outbox::OutboxTransport::new(db)) as Arc<dyn EventTransport>);
    ctx.shared_insert(transactional_event_bus_from_context(&ctx));
    ctx.shared_insert(rustok_storage::StorageRuntime::in_memory());
    configure_product_catalog_deployment(&ctx).await?;

    let registry = build_registry();
    let auth = AuthConfig::new("remote-storefront-fixture-secret-with-32-bytes".into());
    let extensions = build_shared_runtime_extensions_with_host_providers(
        &registry,
        &settings,
        ctx.clone(),
        auth.clone(),
    )?;
    ctx.shared_insert(extensions);
    ctx.shared_insert(registry.clone());
    ctx.shared_insert(rustok_modules::SharedModuleMarketplaceCatalog(Arc::new(
        ServerMarketplaceCatalog::new(ctx.clone(), registry.clone()),
    )));
    let graphql_schema = init_graphql_schema(&ctx)?;
    let auth = ServerAuthRuntime::new(ctx.clone(), auth);
    let runtime = AppRuntimeBootstrap {
        deployment_surfaces: DeploymentSurfaceContract {
            profile: DeploymentProfile::HeadlessApi,
            embed_admin: false,
            embed_storefront: false,
        },
        registry,
        graphql_schema,
        rate_limit_state: PathRateLimitMiddlewareState {
            policies: Arc::new(Vec::new()),
            auth_config: None,
            trusted_auth_dimensions: false,
            request_trust: settings.runtime.request_trust.clone(),
        },
    };
    Ok(compose_application_router(
        build_application_router(RuntimeHostMode::Api, &settings).with_state(auth.clone()),
        ctx,
        auth,
        json!({}),
        runtime,
        &settings,
    )?)
}

async fn response_json(response: reqwest::Response) -> TestResult<(StatusCode, Value)> {
    let status = response.status();
    let body = response.bytes().await?;
    let json = serde_json::from_slice(&body).map_err(|error| {
        std::io::Error::other(format!(
            "{status}: {error}: {}",
            String::from_utf8_lossy(&body)
        ))
    })?;
    Ok((status, json))
}
