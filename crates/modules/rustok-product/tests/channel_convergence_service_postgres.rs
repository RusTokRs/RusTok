/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use std::{env, net::SocketAddr, path::Path, time::Duration};

use rustok_api::{PortActor, PortContext, PortErrorKind};
use rustok_product::{
    PRODUCT_LOCALE_REPLAY_FRESHNESS_PENDING, ProductLocaleReplayKey, ProductLocaleReplayPort,
    ProductLocaleReplayScan, ProductLocaleReplayState,
    ProductSalesChannelIndexRelationConvergenceClaimOutcome as ClaimOutcome,
    ProductSalesChannelIndexRelationConvergenceStore,
};
use rustok_product_ports::grpc::{
    GrpcProductCatalogReadConnectionConfig, GrpcProductCatalogReadProvider,
    ProductCatalogGrpcBearerToken,
};
use rustok_test_utils::{
    connect_postgres, create_postgres_database, drop_postgres_database_if_exists,
    postgres_database_url, unique_postgres_database_name,
};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, DbErr, RuntimeErr, Statement, TransactionTrait,
};
use sea_orm_migration::SchemaManager;
use serde_json::json;
use tokio::{
    net::{TcpListener, TcpStream},
    process::{Child, Command},
};
use uuid::Uuid;

#[path = "support/postgres.rs"]
mod postgres;

const TOKEN: &str = "test-channel-identity-token";
const LOCALE_TOKEN: &str = "test-product-locale-replay-token";
const INITIAL_PRODUCTS: u128 = 10;
const REPLAY_DESCRIPTION_BYTES: i32 = 512 * 1024;
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[tokio::test]
#[ignore = "requires disposable PostgreSQL and the compiled Channel service binary"]
async fn standalone_product_resumes_channel_convergence_across_separate_databases() -> TestResult {
    let channel_binary = env::var_os("RUSTOK_PRODUCT_CHANNEL_TEST_CHANNEL_BINARY")
        .ok_or("RUSTOK_PRODUCT_CHANNEL_TEST_CHANNEL_BINARY is required")?;
    postgres::with_product_postgres_database("remote_channel_product", |db| {
        run_with_channel_database(db, channel_binary.into())
    })
    .await
}

async fn run_with_channel_database(
    product: DatabaseConnection,
    binary: std::path::PathBuf,
) -> TestResult {
    let admin_url = env::var("RUSTOK_MIGRATION_SMOKE_ADMIN_URL")?;
    let product_name: String = product
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT current_database() AS name",
        ))
        .await?
        .ok_or("missing Product database name")?
        .try_get("", "name")?;
    let product_url = postgres_database_url(&admin_url, &product_name);
    let admin = connect_postgres(&admin_url).await?;
    let name = unique_postgres_database_name("remote_channel_owner");
    create_postgres_database(&admin, &name).await?;
    let result = async {
        let url = postgres_database_url(&admin_url, &name);
        let channel = connect_postgres(&url).await?;
        verify(&product, &product_url, &channel, &url, &binary).await
    }
    .await;
    let cleanup = drop_postgres_database_if_exists(&admin, &name).await;
    result?;
    cleanup?;
    Ok(())
}

async fn verify(
    product: &DatabaseConnection,
    product_url: &str,
    channel: &DatabaseConnection,
    channel_url: &str,
    channel_binary: &Path,
) -> TestResult {
    let tenant = Uuid::new_v4();
    let channel_id = Uuid::new_v4();
    seed(product, channel, tenant, channel_id).await?;
    assert!(!SchemaManager::new(product).has_table("channels").await?);
    assert!(!SchemaManager::new(channel).has_table("products").await?);

    let store = ProductSalesChannelIndexRelationConvergenceStore::new(product.clone());
    let ClaimOutcome::Claimed(claim) = store.claim(tenant, 1, Duration::from_secs(30)).await?
    else {
        return Err("expected a queued Product visibility request".into());
    };
    store
        .retry(&claim, Duration::from_secs(1), "fixture_backoff")
        .await?;
    assert_eq!(
        store.claim(tenant, 1, Duration::from_secs(30)).await?,
        ClaimOutcome::Busy
    );

    let channel_bind = unused_address().await?;
    let endpoint = format!("http://{channel_bind}");
    let (mut owner, product_bind) = start_product(product_url, &endpoint, tenant).await?;
    // The Product listener starts while Channel is unavailable.
    assert_eq!(checkpoint(product, tenant).await?.cursor, 0);
    assert_eq!(
        witness_count(product, tenant, 1, json!([channel_id])).await?,
        0
    );

    let mut channel_process = clean_command(channel_binary)
        .env("RUSTOK_CHANNEL_DATABASE_URL", channel_url)
        .env("RUSTOK_CHANNEL_SERVICE_BIND", channel_bind.to_string())
        .env("RUSTOK_CHANNEL_SERVICE_ALLOW_INSECURE_LOOPBACK", "true")
        .env("RUSTOK_CHANNEL_INDEX_IDENTITY_TENANTS", tenant.to_string())
        .env("RUSTOK_CHANNEL_INDEX_IDENTITY_GRPC_BEARER_TOKEN", TOKEN)
        .spawn()?;
    wait_for_listener(&mut channel_process, channel_bind).await?;

    let first_page = wait_for_checkpoint(product, tenant, &mut owner, |state| {
        state.sweep == Some(1) && state.after == Some(Uuid::from_u128(8))
    })
    .await?;
    assert_eq!(first_page.cursor, INITIAL_PRODUCTS as i64);
    let replay = locale_provider(product_bind, LOCALE_TOKEN).await?;
    let page = replay
        .scan_locale_replay(
            replay_context(tenant),
            ProductLocaleReplayScan {
                locale: Some("en".into()),
                after: None,
                limit: 1,
            },
        )
        .await?;
    assert_eq!(page.entries[0].product_id, Uuid::from_u128(1));
    assert_eq!(page.entries[0].source_version, 1);
    assert!(page.entries[0].product_source_version.get() > page.entries[0].source_version);
    assert_eq!(page.next, Some(first_locale_key()));
    assert!(
        matches!(&page.entries[0].state, ProductLocaleReplayState::Live(fields)
        if fields.title == "Product 1" && fields.sales_channel_ids == [channel_id])
    );
    verify_large_locale_replay(&replay, tenant).await?;
    let denied = locale_provider(product_bind, "test-product-read-token").await?;
    let error = denied
        .load_locale_replay(replay_context(tenant), vec![first_locale_key()])
        .await
        .unwrap_err();
    assert_eq!(error.kind, PortErrorKind::Forbidden);
    assert_eq!(
        witness_count(product, tenant, 1, json!([channel_id])).await?,
        INITIAL_PRODUCTS as i64
    );
    owner.kill().await?;
    assert_eq!(checkpoint(product, tenant).await?, first_page);

    let change = channel.begin().await?;
    change
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE channels SET slug = 'new' WHERE tenant_id = $1 AND id = $2",
            [tenant.into(), channel_id.into()],
        ))
        .await?;
    change
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE channel_index_identity_generations SET generation = 2 WHERE tenant_id = $1",
            [tenant.into()],
        ))
        .await?;
    change.commit().await?;

    let ClaimOutcome::Claimed(held) = store.claim(tenant, 2, Duration::from_secs(30)).await? else {
        return Err("expected work to hold during the freshness check".into());
    };
    let (mut owner, product_bind) = start_product(product_url, &endpoint, tenant).await?;
    let replay = locale_provider(product_bind, LOCALE_TOKEN).await?;
    let error = replay
        .load_locale_replay(replay_context(tenant), vec![first_locale_key()])
        .await
        .unwrap_err();
    assert_eq!(error.code, PRODUCT_LOCALE_REPLAY_FRESHNESS_PENDING);
    assert!(error.retryable);
    store
        .retry(&held, Duration::from_secs(1), "fixture_replay_freshness")
        .await?;
    wait_for_checkpoint(product, tenant, &mut owner, |state| {
        state.generation == Some(2) && state.sweep.is_none()
    })
    .await?;
    assert_eq!(
        witness_count(product, tenant, 2, json!([])).await?,
        INITIAL_PRODUCTS as i64
    );
    let current = replay
        .load_locale_replay(replay_context(tenant), vec![first_locale_key()])
        .await?;
    assert_eq!(
        current[0].product_source_version,
        page.entries[0].product_source_version
    );
    assert!(current[0].source_version > page.entries[0].source_version);
    assert!(
        matches!(&current[0].state, ProductLocaleReplayState::Live(fields) if fields.sales_channel_ids.is_empty())
    );

    product
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE products SET metadata = $3 WHERE tenant_id = $1 AND id = $2",
            [
                tenant.into(),
                Uuid::from_u128(1).into(),
                visibility("new").into(),
            ],
        ))
        .await?;
    wait_for_checkpoint(product, tenant, &mut owner, |state| {
        state.cursor > first_page.cursor
    })
    .await?;
    assert_eq!(
        witness_count(product, tenant, 2, json!([channel_id])).await?,
        1
    );
    owner.kill().await?;

    // Reproduce a historical cursor past an unapplied change, then upgrade.
    product
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE products SET metadata = $3 WHERE tenant_id = $1 AND id = $2",
            [
                tenant.into(),
                Uuid::from_u128(2).into(),
                visibility("new").into(),
            ],
        ))
        .await?;
    let ClaimOutcome::Claimed(missed) = store.claim(tenant, 2, Duration::from_secs(30)).await?
    else {
        return Err("expected a visibility request to simulate the historical gap".into());
    };
    store.complete_visibility(&missed).await?;
    assert_eq!(
        witness_count(product, tenant, 2, json!([channel_id])).await?,
        1
    );
    let migration = rustok_product::migrations::migrations()
        .into_iter()
        .find(|migration| {
            migration.name() == "m20261117_000044_order_product_channel_convergence_commits"
        })
        .ok_or("missing Product convergence migration")?;
    let manager = SchemaManager::new(product);
    migration.down(&manager).await?;
    migration.up(&manager).await?;
    assert!(checkpoint(product, tenant).await?.sweep_required);
    let (mut owner, product_bind) = start_product(product_url, &endpoint, tenant).await?;
    let repaired = wait_for_checkpoint(product, tenant, &mut owner, |state| {
        state.generation == Some(2) && !state.sweep_required && state.sweep.is_none()
    })
    .await?;
    assert!(repaired.cursor > first_page.cursor);
    assert_eq!(
        witness_count(product, tenant, 2, json!([channel_id])).await?,
        2
    );
    channel_process.kill().await?;
    product
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "DELETE FROM products WHERE tenant_id = $1 AND id = $2",
            [tenant.into(), Uuid::from_u128(1).into()],
        ))
        .await?;
    let replay = locale_provider(product_bind, LOCALE_TOKEN).await?;
    let deleted = replay
        .load_locale_replay(replay_context(tenant), vec![first_locale_key()])
        .await?;
    assert!(matches!(
        deleted[0].state,
        ProductLocaleReplayState::Deleted
    ));
    assert!(deleted[0].product_source_version > current[0].product_source_version);
    owner.kill().await?;
    verify_visibility_queue_commit_order(product, tenant).await?;
    Ok(())
}

async fn verify_visibility_queue_commit_order(db: &DatabaseConnection, tenant: Uuid) -> TestResult {
    let append = |tenant: Uuid| {
        Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO product_sales_channel_index_relation_convergence_requests
             (tenant_id, product_id, product_source_version) VALUES ($1, $2, 1)
             RETURNING sequence_no",
            [tenant.into(), Uuid::new_v4().into()],
        )
    };
    let first = db.begin().await?;
    let earlier: i64 = first
        .query_one_raw(append(tenant))
        .await?
        .ok_or("missing first visibility request")?
        .try_get("", "sequence_no")?;
    let later = db.begin().await?;
    later
        .execute_unprepared("SET LOCAL lock_timeout = '150ms'")
        .await?;
    let error = later
        .query_one_raw(append(tenant))
        .await
        .expect_err("same-tenant request must wait for the previous writer");
    let (DbErr::Query(RuntimeErr::SqlxError(error)) | DbErr::Exec(RuntimeErr::SqlxError(error))) =
        error
    else {
        return Err(error.into());
    };
    assert_eq!(
        error
            .as_database_error()
            .and_then(|error| error.code())
            .as_deref(),
        Some("55P03")
    );
    later.rollback().await?;
    let independent = db.begin().await?;
    independent
        .execute_unprepared("SET LOCAL lock_timeout = '150ms'")
        .await?;
    assert!(
        independent
            .query_one_raw(append(Uuid::new_v4()))
            .await?
            .is_some()
    );
    independent.commit().await?;
    first.commit().await?;
    let later: i64 = db
        .query_one_raw(append(tenant))
        .await?
        .ok_or("missing subsequent visibility request")?
        .try_get("", "sequence_no")?;
    assert!(later > earlier);
    Ok(())
}

async fn verify_large_locale_replay(
    replay: &GrpcProductCatalogReadProvider,
    tenant: Uuid,
) -> TestResult {
    let mut expected: Vec<_> = (1..=INITIAL_PRODUCTS)
        .map(|id| (Uuid::from_u128(id), "en".to_owned()))
        .collect();
    expected.insert(5, (Uuid::from_u128(5), "fr".to_owned()));

    let mut after = None;
    let mut actual = Vec::new();
    let mut pages = 0;
    loop {
        let page = replay
            .scan_locale_replay(
                replay_context(tenant),
                ProductLocaleReplayScan {
                    locale: None,
                    after: after.clone(),
                    limit: 1_000,
                },
            )
            .await?;
        pages += 1;
        assert!(!page.entries.is_empty());
        assert!(
            pages <= expected.len(),
            "the cursor must advance on every page"
        );
        for entry in &page.entries {
            assert_eq!(entry.tenant_id, tenant);
            assert!(
                matches!(&entry.state, ProductLocaleReplayState::Live(fields)
                if fields.description.as_ref().is_some_and(|text| text.len() == REPLAY_DESCRIPTION_BYTES as usize))
            );
            actual.push((entry.product_id, entry.locale.clone()));
        }
        let Some(next) = page.next else {
            break;
        };
        assert_ne!(after.as_ref(), Some(&next));
        assert_eq!(
            page.entries.last().map(|entry| entry.key()),
            Some(next.clone())
        );
        after = Some(next);
    }
    assert!(pages > 1, "the requested page exceeds the gRPC byte limit");
    assert_eq!(actual, expected);
    Ok(())
}

async fn seed(
    product: &DatabaseConnection,
    channel: &DatabaseConnection,
    tenant: Uuid,
    id: Uuid,
) -> TestResult {
    channel.execute_unprepared(
        "CREATE TABLE channels (id uuid PRIMARY KEY, tenant_id uuid NOT NULL, slug text NOT NULL);
         CREATE TABLE channel_index_identity_generations (tenant_id uuid PRIMARY KEY, generation bigint NOT NULL)",
    ).await?;
    channel
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO channels VALUES ($1, $2, 'web')",
            [id.into(), tenant.into()],
        ))
        .await?;
    channel
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO channel_index_identity_generations VALUES ($1, 1)",
            [tenant.into()],
        ))
        .await?;
    product
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO tenants (id) VALUES ($1)",
            [tenant.into()],
        ))
        .await?;
    for id in 1..=INITIAL_PRODUCTS {
        product
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "INSERT INTO products (tenant_id, id, metadata) VALUES ($1, $2, $3)",
                [
                    tenant.into(),
                    Uuid::from_u128(id).into(),
                    visibility("web").into(),
                ],
            ))
            .await?;
        product.execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO product_translations (id, tenant_id, product_id, locale, title, handle, description)
             VALUES ($1, $2, $3, 'en', $4, $5, repeat('d', $6))",
            [Uuid::new_v4().into(), tenant.into(), Uuid::from_u128(id).into(),
                format!("Product {id}").into(), format!("product-{id}").into(), REPLAY_DESCRIPTION_BYTES.into()],
        )).await?;
    }
    product.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO product_translations (id, tenant_id, product_id, locale, title, handle, description)
         VALUES ($1, $2, $3, 'fr', 'Produit 5', 'produit-5', repeat('d', $4))",
        [Uuid::new_v4().into(), tenant.into(), Uuid::from_u128(5).into(), REPLAY_DESCRIPTION_BYTES.into()],
    )).await?;
    Ok(())
}

fn visibility(slug: &str) -> serde_json::Value {
    json!({"channel_visibility":{"allowed_channel_slugs":[slug]}})
}

fn clean_command(binary: impl AsRef<Path>) -> Command {
    let mut command = Command::new(binary.as_ref());
    for (key, _) in env::vars_os().filter(|(key, _)| key.to_string_lossy().starts_with("RUSTOK_")) {
        command.env_remove(key);
    }
    command.kill_on_drop(true);
    command
}

async fn unused_address() -> TestResult<SocketAddr> {
    Ok(TcpListener::bind("127.0.0.1:0").await?.local_addr()?)
}

async fn start_product(url: &str, endpoint: &str, tenant: Uuid) -> TestResult<(Child, SocketAddr)> {
    let bind = unused_address().await?;
    let mut child = clean_command(env!("CARGO_BIN_EXE_rustok-product-catalog-service"))
        .env("RUSTOK_PRODUCT_CATALOG_DATABASE_URL", url)
        .env("RUSTOK_PRODUCT_CATALOG_SERVICE_BIND", bind.to_string())
        .env(
            "RUSTOK_PRODUCT_CATALOG_SERVICE_ALLOW_INSECURE_LOOPBACK",
            "true",
        )
        .env(
            "RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN",
            "test-product-read-token",
        )
        .env(
            "RUSTOK_PRODUCT_CATALOG_TRUSTED_SERVICE_ACTOR",
            "test.catalog",
        )
        .env("RUSTOK_PRODUCT_CHANNEL_INDEX_GRPC_ENDPOINT", endpoint)
        .env(
            "RUSTOK_PRODUCT_LOCALE_REPLAY_GRPC_BEARER_TOKEN",
            LOCALE_TOKEN,
        )
        .env("RUSTOK_PRODUCT_CHANNEL_INDEX_TENANTS", tenant.to_string())
        .env("RUSTOK_PRODUCT_CHANNEL_INDEX_POLL_MS", "1500")
        .env("RUSTOK_CHANNEL_INDEX_IDENTITY_GRPC_BEARER_TOKEN", TOKEN)
        .env(
            "RUSTOK_PRODUCT_CHANNEL_INDEX_ALLOW_INSECURE_LOOPBACK",
            "true",
        )
        .spawn()?;
    wait_for_listener(&mut child, bind).await?;
    Ok((child, bind))
}

async fn locale_provider(
    bind: SocketAddr,
    token: &str,
) -> TestResult<GrpcProductCatalogReadProvider> {
    Ok(
        GrpcProductCatalogReadConnectionConfig::new(format!("http://{bind}"))
            .allow_insecure_loopback(true)
            .connect()
            .await?
            .with_locale_replay_authentication(ProductCatalogGrpcBearerToken::new(token)?),
    )
}

fn replay_context(tenant: Uuid) -> PortContext {
    PortContext::new(
        tenant.to_string(),
        PortActor::service("index-fixture"),
        "en",
        "replay-test",
    )
    .with_deadline(Duration::from_secs(5))
}

fn first_locale_key() -> ProductLocaleReplayKey {
    ProductLocaleReplayKey {
        product_id: Uuid::from_u128(1),
        locale: "en".into(),
    }
}

async fn wait_for_listener(child: &mut Child, address: SocketAddr) -> TestResult {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Some(exit) = child.try_wait()? {
                return Err(format!("service exited before listening: {exit}").into());
            }
            if TcpStream::connect(address).await.is_ok() {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
    })
    .await?
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Checkpoint {
    cursor: i64,
    generation: Option<i64>,
    sweep: Option<i64>,
    after: Option<Uuid>,
    sweep_required: bool,
}

async fn checkpoint(db: &DatabaseConnection, tenant: Uuid) -> TestResult<Checkpoint> {
    let row = db.query_one_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT visibility_cursor, channel_identity_generation, sweep_generation, sweep_after_product_id, sweep_required
           FROM product_sales_channel_index_relation_convergence_state WHERE tenant_id = $1",
        [tenant.into()],
    )).await?.ok_or("missing Product checkpoint")?;
    Ok(Checkpoint {
        cursor: row.try_get("", "visibility_cursor")?,
        generation: row.try_get("", "channel_identity_generation")?,
        sweep: row.try_get("", "sweep_generation")?,
        after: row.try_get("", "sweep_after_product_id")?,
        sweep_required: row.try_get("", "sweep_required")?,
    })
}

async fn wait_for_checkpoint(
    db: &DatabaseConnection,
    tenant: Uuid,
    owner: &mut Child,
    ready: impl Fn(Checkpoint) -> bool,
) -> TestResult<Checkpoint> {
    tokio::time::timeout(Duration::from_secs(35), async {
        loop {
            if let Some(exit) = owner.try_wait()? {
                return Err(format!("Product exited while waiting for convergence: {exit}").into());
            }
            let state = checkpoint(db, tenant).await?;
            if ready(state) {
                return Ok(state);
            }
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
    })
    .await?
}

async fn witness_count(
    db: &DatabaseConnection,
    tenant: Uuid,
    generation: i64,
    membership: serde_json::Value,
) -> TestResult<i64> {
    let row = db.query_one_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT count(*) AS count FROM (
            SELECT DISTINCT ON (w.product_id) w.product_id, w.channel_identity_generation, r.channel_ids
              FROM product_sales_channel_index_relation_freshness_snapshots w
              JOIN product_sales_channel_index_relation_snapshots r
                ON r.tenant_id = w.tenant_id AND r.product_id = w.product_id AND r.relation_epoch = w.relation_epoch
             WHERE w.tenant_id = $1 ORDER BY w.product_id, w.sequence_no DESC
        ) latest WHERE channel_identity_generation = $2 AND channel_ids = $3",
        [tenant.into(), generation.into(), membership.into()],
    )).await?.ok_or("missing witness count")?;
    Ok(row.try_get("", "count")?)
}
