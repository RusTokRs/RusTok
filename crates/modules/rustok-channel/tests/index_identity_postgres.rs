/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use std::{env, error::Error, time::Duration};

use rustok_api::{PortActor, PortContext, PortErrorKind};
use rustok_channel::{
    CHANNEL_INDEX_IDENTITY_READ_CLAIM, ChannelIndexIdentityReadPort, ChannelIndexIdentitySelector,
    PostgresChannelIndexIdentity,
};
use rustok_channel_ports::grpc::GrpcChannelIndexIdentity;
use rustok_core::MigrationSource;
use rustok_test_utils::{
    connect_postgres, create_postgres_database, drop_postgres_database_if_exists,
    postgres_database_url, unique_postgres_database_name,
};
use sea_orm::{
    AccessMode, ConnectionTrait, DatabaseConnection, DbBackend, IsolationLevel, Statement,
    TransactionTrait,
};
use sea_orm_migration::SchemaManager;
use tokio::{net::TcpListener, process::Command};
use uuid::Uuid;

const TOKEN: &str = "channel-index-fixture-token";
const TENANT: Uuid = Uuid::from_u128(1);
const OTHER_TENANT: Uuid = Uuid::from_u128(2);
const FIRST: Uuid = Uuid::from_u128(10);
const SECOND: Uuid = Uuid::from_u128(20);
const THIRD: Uuid = Uuid::from_u128(30);
type TestResult<T = ()> = Result<T, Box<dyn Error>>;

fn context(tenant_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::service("test-index"),
        "und",
        "channel-fixture",
    )
    .with_claim(CHANNEL_INDEX_IDENTITY_READ_CLAIM)
    .with_deadline(Duration::from_secs(5))
}

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL database and starts the standalone Channel binary"]
async fn channel_identity_generation_and_membership_match_in_process_and_grpc() -> TestResult {
    let admin_url = env::var("RUSTOK_MIGRATION_SMOKE_ADMIN_URL")?;
    let admin = connect_postgres(&admin_url).await?;
    let name = unique_postgres_database_name("channel_index_identity");
    create_postgres_database(&admin, &name).await?;
    let result = async {
        let url = postgres_database_url(&admin_url, &name);
        let db = connect_postgres(&url).await?;
        install_owner_schema(&db).await?;
        verify(&db, &url).await
    }
    .await;
    let cleanup = drop_postgres_database_if_exists(&admin, &name).await;
    result?;
    cleanup?;
    Ok(())
}

async fn install_owner_schema(db: &DatabaseConnection) -> TestResult {
    db.execute_unprepared(
        "CREATE TABLE channels (id uuid PRIMARY KEY, tenant_id uuid NOT NULL, slug text NOT NULL,
           is_active boolean NOT NULL DEFAULT true)",
    )
    .await?;
    rustok_channel::ChannelModule
        .migrations()
        .into_iter()
        .find(|migration| {
            migration.name() == "m20260807_000012_add_channel_index_identity_generation"
        })
        .ok_or("missing Channel identity migration")?
        .up(&SchemaManager::new(db))
        .await?;
    Ok(())
}

async fn verify(db: &DatabaseConnection, url: &str) -> TestResult {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let bind = listener.local_addr()?;
    drop(listener);
    let endpoint = format!("http://{bind}");
    let mut command = Command::new(env!("CARGO_BIN_EXE_rustok-channel-service"));
    for (key, _) in
        env::vars_os().filter(|(key, _)| key.to_string_lossy().starts_with("RUSTOK_CHANNEL_"))
    {
        command.env_remove(key);
    }
    let mut child = command
        .env("RUSTOK_CHANNEL_DATABASE_URL", url)
        .env("RUSTOK_CHANNEL_SERVICE_BIND", bind.to_string())
        .env("RUSTOK_CHANNEL_SERVICE_ALLOW_INSECURE_LOOPBACK", "true")
        .env("RUSTOK_CHANNEL_INDEX_IDENTITY_TENANTS", TENANT.to_string())
        .env("RUSTOK_CHANNEL_INDEX_IDENTITY_GRPC_BEARER_TOKEN", TOKEN)
        .kill_on_drop(true)
        .spawn()?;
    let remote = tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            if let Some(status) = child.try_wait()? {
                return Err(std::io::Error::other(format!(
                    "Channel service exited: {status}"
                )));
            }
            if let Ok(port) = GrpcChannelIndexIdentity::connect(&endpoint, TOKEN, None, true).await
            {
                return Ok(port);
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await??;

    let local = PostgresChannelIndexIdentity::new(db.clone());
    let outcome = verify_observations(db, &local, &remote, &endpoint).await;
    let stop = child.kill().await;
    outcome?;
    stop?;
    Ok(())
}

async fn verify_observations(
    db: &DatabaseConnection,
    local: &PostgresChannelIndexIdentity,
    remote: &GrpcChannelIndexIdentity,
    endpoint: &str,
) -> TestResult {
    use ChannelIndexIdentitySelector::{All, GenerationOnly, Slugs};
    let wrong_token =
        GrpcChannelIndexIdentity::connect(endpoint, "wrong-token", None, true).await?;
    assert_eq!(
        wrong_token
            .read_index_identity(context(TENANT), All)
            .await
            .unwrap_err()
            .kind,
        PortErrorKind::Forbidden,
    );
    assert_eq!(
        remote
            .read_index_identity(context(OTHER_TENANT), All)
            .await
            .unwrap_err()
            .kind,
        PortErrorKind::Forbidden,
    );
    let mut denied = context(TENANT);
    denied.claims.clear();
    assert_eq!(
        local
            .read_index_identity(denied, All)
            .await
            .unwrap_err()
            .kind,
        PortErrorKind::Forbidden,
    );
    let initial = remote.read_index_identity(context(TENANT), All).await?;
    assert_eq!(initial.generation, 0);
    assert!(initial.channel_ids.is_empty());

    for (id, tenant, slug, active) in [
        (SECOND, TENANT, "mobile", true),
        (FIRST, TENANT, " Web ", false),
        (THIRD, OTHER_TENANT, "web", true),
    ] {
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO channels VALUES ($1, $2, $3, $4)",
            [id.into(), tenant.into(), slug.into(), active.into()],
        ))
        .await?;
    }
    let both = remote.read_index_identity(context(TENANT), All).await?;
    assert_eq!(both, local.read_index_identity(context(TENANT), All).await?);
    assert_eq!(both.generation, 2);
    assert_eq!(both.channel_ids, [FIRST, SECOND]);

    let selector = Slugs(vec!["web".to_owned()]);
    let shared_snapshot = db
        .begin_with_config(
            Some(IsolationLevel::RepeatableRead),
            Some(AccessMode::ReadOnly),
        )
        .await?;
    let before =
        PostgresChannelIndexIdentity::read_in(&shared_snapshot, TENANT, selector.clone()).await?;
    assert_eq!(
        before,
        remote
            .read_index_identity(context(TENANT), selector.clone())
            .await?
    );
    assert_eq!(before.channel_ids, [FIRST]);
    assert_eq!(before.generation, both.generation);

    let writer = db.begin().await?;
    writer
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE channels SET slug = 'renamed' WHERE id = $1",
            [FIRST.into()],
        ))
        .await?;
    assert_eq!(
        before,
        remote
            .read_index_identity(context(TENANT), selector.clone())
            .await?
    );
    writer.commit().await?;
    let after = remote
        .read_index_identity(context(TENANT), selector.clone())
        .await?;
    assert_eq!(after.generation, 3);
    assert!(after.channel_ids.is_empty());
    assert_eq!(
        before,
        PostgresChannelIndexIdentity::read_in(&shared_snapshot, TENANT, selector).await?
    );
    shared_snapshot.commit().await?;

    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE channels SET is_active = true WHERE id = $1",
        [FIRST.into()],
    ))
    .await?;
    assert_eq!(
        remote
            .read_index_identity(context(TENANT), GenerationOnly)
            .await?
            .generation,
        3
    );
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "DELETE FROM channels WHERE tenant_id = $1",
        [TENANT.into()],
    ))
    .await?;
    let deleted = remote.read_index_identity(context(TENANT), All).await?;
    assert_eq!(deleted.generation, 5);
    assert!(deleted.channel_ids.is_empty());

    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO channels (id, tenant_id, slug)
         SELECT gen_random_uuid(), $1, n::text FROM generate_series(1, 1025) n",
        [TENANT.into()],
    ))
    .await?;
    let overflow = remote
        .read_index_identity(context(TENANT), All)
        .await
        .unwrap_err();
    assert_eq!(overflow.code, "channel.index_identity_limit");
    let generation_only = remote
        .read_index_identity(context(TENANT), GenerationOnly)
        .await?;
    assert_eq!(generation_only.generation, 1030);
    assert!(generation_only.channel_ids.is_empty());

    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "DELETE FROM channel_index_identity_generations WHERE tenant_id = $1",
        [OTHER_TENANT.into()],
    ))
    .await?;
    let broken = local
        .read_index_identity(context(OTHER_TENANT), GenerationOnly)
        .await
        .unwrap_err();
    assert_eq!(broken.kind, PortErrorKind::InvariantViolation);
    Ok(())
}
