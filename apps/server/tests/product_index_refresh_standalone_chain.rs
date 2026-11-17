#![cfg(all(feature = "mod-product", feature = "mod-social_graph"))]

use std::{env, error::Error, future::Future, net::SocketAddr, path::Path, time::Duration};

use rustok_api::{PortActor, PortContext};
use rustok_events::{
    ContractEventEnvelope, ContractEventPayload, DomainEvent, EventEnvelope,
    PRODUCT_INDEX_LOCALE_REFRESH_REQUESTED_EVENT_TYPE, ProductIndexRefreshEvent,
};
use rustok_iggy::{IggyConfig, IggyConsumerPositionObserver};
use rustok_index::{
    IndexValue, LocaleKey, PostgresSchemaRegistrationStore, SchemaRef, SharedIndexSchemaRegistry,
    derive_index_schema_source_event_id,
};
use rustok_product::services::flex_schema_projection::ProductFlexSchemaProjection;
use rustok_product_ports::{
    ProductCatalogCommandPort, ProductLocaleReplayKey, ProductLocaleReplayPort,
    ProductTranslationInput, UpdateProductInput,
    grpc::{GrpcProductCatalogReadConnectionConfig, ProductCatalogGrpcBearerToken},
};
use rustok_server::modules::{ManifestManager, build_registry};
use rustok_test_utils::{
    connect_postgres, create_postgres_database, drop_postgres_database_if_exists,
    postgres_database_url, unique_postgres_database_name,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, Value};
use sea_orm_migration::{MigrationTrait, MigratorTrait, SchemaManager};
use serde_json::{Value as Json, json};
use tokio::{
    net::{TcpListener, TcpStream},
    process::{Child, Command},
    time::{sleep, timeout},
};
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

const TENANT: Uuid = Uuid::from_u128(1); // The development HTTP host's default tenant.
const PRODUCT: Uuid = Uuid::from_u128(101);
const ACTOR: Uuid = Uuid::from_u128(201);
const CATALOG_TOKEN: &str = "fixture-catalog-read";
const COMMAND_TOKEN: &str = "fixture-catalog-command";
const LOCALE_TOKEN: &str = "fixture-locale-replay";
const VARIANT_TOKEN: &str = "fixture-variant-replay";
const CHANNEL_TOKEN: &str = "fixture-channel-identity";
const TITLE: &str = "Title written by the standalone Product owner";

#[tokio::test]
#[ignore = "requires disposable PostgreSQL, external Iggy, and the standalone owner and relay binaries"]
async fn standalone_product_command_reaches_the_http_hosts_index() -> TestResult<()> {
    let admin_url = env::var("RUSTOK_MIGRATION_SMOKE_ADMIN_URL")?;
    let admin = connect_postgres(&admin_url).await?;
    let mut names = Vec::new();
    let result = async {
        for scope in ["owner", "channel", "host"] {
            let name = unique_postgres_database_name(&format!("product_chain_{scope}"));
            create_postgres_database(&admin, &name).await?;
            names.push(name);
        }
        let urls: Vec<_> = names
            .iter()
            .map(|name| postgres_database_url(&admin_url, name))
            .collect();
        verify(&urls[0], &urls[1], &urls[2]).await
    }
    .await;
    let mut cleanup_errors = Vec::new();
    for name in names {
        if let Err(error) = drop_postgres_database_if_exists(&admin, &name).await {
            cleanup_errors.push(format!("{name}: {error}"));
        }
    }
    if !cleanup_errors.is_empty() {
        return Err(format!(
            "verification: {result:?}; cleanup: {}",
            cleanup_errors.join("; ")
        )
        .into());
    }
    result
}

async fn verify(owner_url: &str, channel_url: &str, host_url: &str) -> TestResult<()> {
    let fixture = tempfile::tempdir()?;
    let broker = json!({
        "mode": "external",
        "external": {
            "addresses": [env::var("RUSTOK_INDEX_PRODUCT_REFRESH_TEST_IGGY_ADDRESS")?],
            "protocol": "tcp",
            "username": env::var("RUSTOK_INDEX_PRODUCT_REFRESH_TEST_IGGY_USERNAME")?,
            "password": env::var("RUSTOK_INDEX_PRODUCT_REFRESH_TEST_IGGY_PASSWORD")?,
        },
        "topology": {"stream_name": format!("product-chain-{}", Uuid::new_v4().simple()), "domain_partitions": 1},
    });
    let relay_config = fixture.path().join("iggy.json");
    std::fs::write(&relay_config, serde_json::to_vec(&broker)?)?;
    let broker_config: IggyConfig = serde_json::from_value(broker.clone())?;
    let mut host_config: Json = serde_yaml::from_str(include_str!("../config/development.yaml"))?;
    host_config["database"]["enable_logging"] = json!(false);
    host_config["database"]["auto_migrate"] = json!(false);
    host_config["settings"]["rustok"]["events"]["delivery_profile"] = json!("outbox_iggy");
    host_config["settings"]["rustok"]["events"]["iggy"] = broker;
    std::fs::write(
        fixture.path().join("development.yaml"),
        serde_yaml::to_string(&host_config)?,
    )?;

    let manifest = host_manifest()?;
    let owner_db = connect_postgres(owner_url).await?;
    let channel_db = connect_postgres(channel_url).await?;
    setup_owners(&owner_db, &channel_db).await?;
    let host_db = connect_postgres(host_url).await?;
    let schema = setup_index(&host_db).await?;
    let channel_address = unused_address().await?;
    let mut channel = command(
        env::var_os("RUSTOK_PRODUCT_INDEX_CHAIN_CHANNEL_BINARY").ok_or("missing Channel binary")?,
    )
    .env("RUSTOK_CHANNEL_DATABASE_URL", channel_url)
    .env("RUSTOK_CHANNEL_SERVICE_BIND", channel_address.to_string())
    .env("RUSTOK_CHANNEL_SERVICE_ALLOW_INSECURE_LOOPBACK", "true")
    .env("RUSTOK_CHANNEL_INDEX_IDENTITY_TENANTS", TENANT.to_string())
    .env(
        "RUSTOK_CHANNEL_INDEX_IDENTITY_GRPC_BEARER_TOKEN",
        CHANNEL_TOKEN,
    )
    .spawn()?;
    listening("Channel", &mut channel, channel_address).await?;

    let product_address = unused_address().await?;
    let endpoint = format!("http://{product_address}");
    let mut owner = command(
        env::var_os("RUSTOK_PRODUCT_INDEX_CHAIN_PRODUCT_BINARY").ok_or("missing Product binary")?,
    )
    .env("RUSTOK_PRODUCT_CATALOG_DATABASE_URL", owner_url)
    .env(
        "RUSTOK_PRODUCT_CATALOG_SERVICE_BIND",
        product_address.to_string(),
    )
    .env(
        "RUSTOK_PRODUCT_CATALOG_SERVICE_ALLOW_INSECURE_LOOPBACK",
        "true",
    )
    .env(
        "RUSTOK_PRODUCT_CATALOG_TRUSTED_SERVICE_ACTOR",
        "fixture.catalog",
    )
    .env("RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN", CATALOG_TOKEN)
    .env(
        "RUSTOK_PRODUCT_CATALOG_COMMAND_GRPC_BEARER_TOKEN",
        COMMAND_TOKEN,
    )
    .env("RUSTOK_PRODUCT_CATALOG_COMMAND_ACTOR_ID", ACTOR.to_string())
    .env("RUSTOK_PRODUCT_USE_FLEX_SCHEMA_PROJECTION", "true")
    .env(
        "RUSTOK_PRODUCT_LOCALE_REPLAY_GRPC_BEARER_TOKEN",
        LOCALE_TOKEN,
    )
    .env(
        "RUSTOK_PRODUCT_VARIANT_REPLAY_GRPC_BEARER_TOKEN",
        VARIANT_TOKEN,
    )
    .env("RUSTOK_PRODUCT_INDEX_REFRESH_TENANTS", TENANT.to_string())
    .env("RUSTOK_PRODUCT_INDEX_REFRESH_POLL_MS", "100")
    .env(
        "RUSTOK_PRODUCT_CHANNEL_INDEX_GRPC_ENDPOINT",
        format!("http://{channel_address}"),
    )
    .env("RUSTOK_PRODUCT_CHANNEL_INDEX_TENANTS", TENANT.to_string())
    .env("RUSTOK_PRODUCT_CHANNEL_INDEX_POLL_MS", "100")
    .env(
        "RUSTOK_PRODUCT_CHANNEL_INDEX_ALLOW_INSECURE_LOOPBACK",
        "true",
    )
    .env(
        "RUSTOK_CHANNEL_INDEX_IDENTITY_GRPC_BEARER_TOKEN",
        CHANNEL_TOKEN,
    )
    .spawn()?;
    listening("Product", &mut owner, product_address).await?;

    let host_address = unused_address().await?;
    let mut host = command(env!("CARGO_BIN_EXE_rustok-server"))
        .current_dir(fixture.path())
        .env("DATABASE_URL", host_url)
        .env("RUSTOK_CONFIG_DIR", fixture.path())
        .env("RUSTOK_MODULES_MANIFEST", manifest.path())
        .env("RUSTOK_ENV", "development")
        .env("RUSTOK_BINDING", "127.0.0.1")
        .env("RUSTOK_PORT", host_address.port().to_string())
        .env("RUSTOK_PRODUCT_DEPLOYMENT", "grpc")
        .env("RUSTOK_PRODUCT_CATALOG_GRPC_ENDPOINT", &endpoint)
        .env(
            "RUSTOK_PRODUCT_CATALOG_GRPC_ALLOW_INSECURE_LOOPBACK",
            "true",
        )
        .env("RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN", CATALOG_TOKEN)
        .env(
            "RUSTOK_PRODUCT_CATALOG_COMMAND_GRPC_BEARER_TOKEN",
            COMMAND_TOKEN,
        )
        .env(
            "RUSTOK_PRODUCT_LOCALE_REPLAY_GRPC_BEARER_TOKEN",
            LOCALE_TOKEN,
        )
        .env(
            "RUSTOK_PRODUCT_VARIANT_REPLAY_GRPC_BEARER_TOKEN",
            VARIANT_TOKEN,
        )
        .env("RUSTOK_PRODUCT_INDEX_REFRESH_CONSUMER_ENABLED", "true")
        .env("RUSTOK_PRODUCT_INDEX_REFRESH_CONSUMER_IDLE_POLL_MS", "100")
        .spawn()?;
    listening("HTTP host", &mut host, host_address).await?;

    let connection =
        GrpcProductCatalogReadConnectionConfig::new(endpoint).allow_insecure_loopback(true);
    let port = connection
        .connect_command()
        .await?
        .with_authentication(ProductCatalogGrpcBearerToken::new(COMMAND_TOKEN)?);
    let response = port
        .update_product(
            PortContext::new(
                TENANT.to_string(),
                PortActor::service("fixture.command"),
                "en",
                "product-index-chain",
            )
            .with_idempotency_key("chain-update-product")
            .with_deadline(Duration::from_secs(10)),
            PRODUCT,
            UpdateProductInput {
                expected_revision: Some(1),
                translations: Some(vec![ProductTranslationInput {
                    locale: "en".to_owned(),
                    title: TITLE.to_owned(),
                    handle: Some("chain-product".to_owned()),
                    description: None,
                    meta_title: None,
                    meta_description: None,
                }]),
                ..Default::default()
            },
        )
        .await?;
    assert_eq!(response.revision, 2);

    let root = owner_db.query_one_raw(sql(
        "SELECT payload FROM sys_events WHERE event_type = 'product.updated' AND status = 'pending'", [],
    )).await?.ok_or("command did not commit a pending root event")?;
    let root: EventEnvelope = serde_json::from_value(root.try_get("", "payload")?)?;
    assert_eq!(
        root.event,
        DomainEvent::ProductUpdated {
            product_id: PRODUCT
        }
    );
    let refresh = wait_for(
        "owner locale refresh with exact command causation",
        || async {
            Ok(owner_db
                .query_all_raw(sql(
                    "SELECT payload FROM sys_events WHERE event_type = $1 AND status = 'pending'",
                    [PRODUCT_INDEX_LOCALE_REFRESH_REQUESTED_EVENT_TYPE.into()],
                ))
                .await?
                .into_iter()
                .map(|row| -> TestResult<_> {
                    Ok(serde_json::from_value::<ContractEventEnvelope>(
                        row.try_get("", "payload")?,
                    )?)
                })
                .collect::<TestResult<Vec<_>>>()?
                .into_iter()
                .find(|envelope| envelope.causation_id() == Some(root.id)))
        },
    )
    .await?;
    let ContractEventPayload::ProductIndexRefresh(
        ProductIndexRefreshEvent::LocaleRefreshRequested {
            product_id,
            locale,
            source_version,
            ..
        },
    ) = refresh.payload()?
    else {
        return Err("expected a typed locale refresh".into());
    };
    assert_eq!((*product_id, locale.as_str()), (PRODUCT, "en"));
    let replay = connection
        .connect()
        .await?
        .with_locale_replay_authentication(ProductCatalogGrpcBearerToken::new(LOCALE_TOKEN)?);
    let source = wait_for("Product's Channel witness and localized replay", || async {
        let result = replay
            .load_locale_replay(
                PortContext::new(
                    TENANT.to_string(),
                    PortActor::service("fixture.index"),
                    "en",
                    "chain-replay",
                )
                .with_deadline(Duration::from_secs(5)),
                vec![ProductLocaleReplayKey {
                    product_id: PRODUCT,
                    locale: "en".into(),
                }],
            )
            .await;
        match result {
            Ok(rows) => Ok(rows.into_iter().next()),
            Err(error) if error.retryable => Ok(None),
            Err(error) => Err(error.into()),
        }
    })
    .await?;
    assert_eq!(source.product_source_version.get(), *source_version);
    // The broker acknowledges the refresh envelope. Index deduplicates the owner mutation.
    let mutation_id = derive_index_schema_source_event_id(
        "rustok-product.product-replay",
        TENANT,
        &schema,
        PRODUCT,
        Some(&LocaleKey::new("en")?),
        source.source_version,
    )?;
    let index_row =
        || {
            host_db.query_one_raw(sql(
            "SELECT entry.payload, delivery.source_version = entry.source_version AS same_version
         FROM index_inbox delivery JOIN index_entities entry USING
             (tenant_id, module_name, entity_name, schema_version, entity_id, locale_key)
         WHERE delivery.tenant_id = $1 AND delivery.entity_id = $2 AND delivery.delivery_id = $3
           AND delivery.state = 'applied' AND delivery.locale_key = 'en' AND NOT entry.is_deleted",
            [TENANT.into(), PRODUCT.into(), mutation_id.to_string().into()],
        ))
        };
    assert!(
        index_row().await?.is_none(),
        "Index applied an event before the owner relay started"
    );
    let mut relay = command(
        env::var_os("RUSTOK_PRODUCT_INDEX_CHAIN_OUTBOX_BINARY").ok_or("missing Outbox binary")?,
    )
    .env("RUSTOK_OUTBOX_DATABASE_URL", owner_url)
    .env("RUSTOK_OUTBOX_IGGY_CONFIG_PATH", &relay_config)
    .spawn()?;
    let row = wait_for(
        "the HTTP host to apply the exact Product refresh to Index",
        || async { Ok(index_row().await?) },
    )
    .await?;
    still_running("Outbox", &mut relay)?;
    let payload: Json = row.try_get("", "payload")?;
    assert_eq!(
        serde_json::from_value::<IndexValue>(payload["title"].clone())?,
        IndexValue::String(TITLE.to_owned())
    );
    assert!(row.try_get::<bool>("", "same_version")?);
    wait_for("the owner relay to mark the refresh dispatched", || async {
        Ok(owner_db
            .query_one_raw(sql(
                "SELECT id FROM sys_events WHERE id = $1 AND status = 'dispatched'",
                [refresh.id().into()],
            ))
            .await?)
    })
    .await?;
    assert!(
        host_db
            .query_one_raw(sql(
                "SELECT id FROM products WHERE id = $1",
                [PRODUCT.into()]
            ))
            .await?
            .is_none(),
        "host must not have a local Product row"
    );
    let observer = IggyConsumerPositionObserver::connect(
        &broker_config,
        "rustok-product-index-refresh",
        "domain",
    )
    .await?;
    wait_for(
        "the HTTP host to acknowledge the broker backlog",
        || async {
            let snapshot = observer.snapshot().await?;
            Ok((snapshot.partition_count() == 1
                && snapshot.partitions[0].messages_count >= 2
                && snapshot.total_lag() == Some(0))
            .then_some(()))
        },
    )
    .await?;
    Ok(())
}

async fn setup_index(db: &DatabaseConnection) -> TestResult<SchemaRef> {
    rustok_migrations::Migrator::up(db, None).await?;
    db.execute_raw(sql(
        "INSERT INTO tenants (id, name, slug) VALUES ($1, 'Product chain fixture', 'product-chain')",
        [TENANT.into()],
    )).await?;
    let extensions = rustok_distribution::build_runtime_extensions(&build_registry())?;
    let schemas = extensions
        .get::<SharedIndexSchemaRegistry>()
        .ok_or("missing runtime Index schemas")?;
    let store = PostgresSchemaRegistrationStore::new(db.clone());
    let mut product = None;
    for registered in schemas.registry().iter() {
        store.register(TENANT, &registered.schema).await?;
        let reference = &registered.schema.reference;
        if reference.module.as_str() == "rustok-product" && reference.entity.as_str() == "product" {
            product = Some(reference.clone());
        }
    }
    product.ok_or_else(|| "missing runtime Product Index schema".into())
}

fn host_manifest() -> TestResult<tempfile::NamedTempFile> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut manifest = ManifestManager::load_from_path(workspace.join("modules.toml"))?;
    let registry = build_registry();
    manifest.modules.retain(|slug, _| registry.contains(slug));
    manifest
        .settings
        .default_enabled
        .retain(|slug| registry.contains(slug));
    manifest.build.server.embed_admin = false;
    manifest.build.server.embed_storefront = false;
    // Module asset paths are required to stay inside the workspace of this manifest.
    let fixture = tempfile::Builder::new()
        .prefix(".product-index-chain-")
        .suffix(".toml")
        .tempfile_in(workspace.canonicalize()?)?;
    ManifestManager::save_to_path(fixture.path(), &manifest)?;
    Ok(fixture)
}

async fn setup_owners(owner: &DatabaseConnection, channel: &DatabaseConnection) -> TestResult<()> {
    owner
        .execute_unprepared("CREATE TABLE tenants (id uuid PRIMARY KEY)")
        .await?;
    let manager = SchemaManager::new(owner);
    rustok_outbox::SysEventsMigration.up(&manager).await?;
    for migration in rustok_taxonomy::migrations::migrations()
        .into_iter()
        .chain(rustok_product::migrations::migrations())
    {
        migration.up(&manager).await?;
    }
    flex::cache_generation::create_field_definition_cache_generation_table(&manager).await?;
    owner
        .execute_raw(sql("INSERT INTO tenants (id) VALUES ($1)", [TENANT.into()]))
        .await?;
    ProductFlexSchemaProjection::new(owner.clone())
        .apply(
            TENANT,
            flex::FlexVersionedAttachedSchema {
                revision: 1,
                schema: flex::FlexAttachedSchema {
                    tenant_id: TENANT,
                    entity_type: "product".to_owned(),
                    definitions: Vec::new(),
                },
            },
        )
        .await?;
    owner
        .execute_raw(sql(
            "INSERT INTO products (tenant_id, id, metadata, revision) VALUES ($1, $2, '{}', 1)",
            [TENANT.into(), PRODUCT.into()],
        ))
        .await?;
    owner.execute_raw(sql("INSERT INTO product_translations (id, tenant_id, product_id, locale, title, handle) VALUES ($1, $2, $3, 'en', 'Before the command', 'chain-product')", [Uuid::new_v4().into(), TENANT.into(), PRODUCT.into()])).await?;
    channel.execute_unprepared(
        "CREATE TABLE channels (id uuid PRIMARY KEY, tenant_id uuid NOT NULL, slug text NOT NULL);
         CREATE TABLE channel_index_identity_generations (tenant_id uuid PRIMARY KEY, generation bigint NOT NULL)",
    ).await?;
    channel
        .execute_raw(sql(
            "INSERT INTO channel_index_identity_generations VALUES ($1, 1)",
            [TENANT.into()],
        ))
        .await?;
    Ok(())
}

fn sql<const N: usize>(query: &str, values: [Value; N]) -> Statement {
    Statement::from_sql_and_values(DbBackend::Postgres, query, values)
}

fn command(binary: impl AsRef<Path>) -> Command {
    let mut process = Command::new(binary.as_ref());
    for (key, _) in env::vars_os().filter(|(key, _)| key.to_string_lossy().starts_with("RUSTOK_")) {
        process.env_remove(key);
    }
    process.env("RUST_LOG", "warn,sqlx=error,rustok_server=info,rustok_product_catalog_service=info,rustok_outbox_service=info")
        .env("OTEL_ENABLED", "false").kill_on_drop(true);
    process
}

async fn unused_address() -> TestResult<SocketAddr> {
    Ok(TcpListener::bind("127.0.0.1:0").await?.local_addr()?)
}

async fn listening(name: &str, process: &mut Child, address: SocketAddr) -> TestResult<()> {
    timeout(Duration::from_secs(60), async {
        loop {
            still_running(name, process)?;
            if TcpStream::connect(address).await.is_ok() {
                return Ok(());
            }
            sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .map_err(|_| format!("{name} did not start listening"))?
}

fn still_running(name: &str, process: &mut Child) -> TestResult<()> {
    if let Some(status) = process.try_wait()? {
        return Err(format!("{name} exited early: {status}").into());
    }
    Ok(())
}

async fn wait_for<T, F: Future<Output = TestResult<Option<T>>>>(
    description: &str,
    mut poll: impl FnMut() -> F,
) -> TestResult<T> {
    timeout(Duration::from_secs(30), async {
        loop {
            if let Some(value) = poll().await? {
                return Ok(value);
            }
            sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .map_err(|_| format!("timed out waiting for {description}"))?
}
