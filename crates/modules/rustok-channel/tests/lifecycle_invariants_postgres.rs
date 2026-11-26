/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! PostgreSQL half of the `m20261126_000013` proof.
//!
//! The SQLite suite (`lifecycle_invariants_sqlite.rs`) covers the parts that exist
//! on both backends. Three invariants are PostgreSQL-only by construction — the
//! `SET NOT NULL` on the denormalized `channel_targets.tenant_id`, the composite
//! `(tenant_id, channel_id)` foreign key and the `down()` symmetry — so they can
//! only be proven against a real PostgreSQL database.
//!
//! Run with a disposable server, as with `index_identity_postgres.rs`:
//!
//! ```text
//! RUSTOK_MIGRATION_SMOKE_ADMIN_URL=postgres://… cargo test -p rustok-channel \
//!   --test lifecycle_invariants_postgres -- --ignored
//! ```

use std::{env, error::Error};

use rustok_channel::ChannelService;
use rustok_test_utils::{
    connect_postgres, create_postgres_database, drop_postgres_database_if_exists,
    postgres_database_url, unique_postgres_database_name,
};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, Statement, sea_query::Value,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const LIFECYCLE_MIGRATION: &str = "m20261126_000013_channel_lifecycle_invariants";

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL database (RUSTOK_MIGRATION_SMOKE_ADMIN_URL)"]
async fn channel_lifecycle_invariants_hold_on_postgres() -> TestResult {
    let admin_url = env::var("RUSTOK_MIGRATION_SMOKE_ADMIN_URL")?;
    let admin = connect_postgres(&admin_url).await?;
    let name = unique_postgres_database_name("channel_lifecycle_invariants");
    create_postgres_database(&admin, &name).await?;

    let result = async {
        let url = postgres_database_url(&admin_url, &name);
        let db = connect_postgres(&url).await?;
        install_owner_schema(&db).await?;
        verify_up_invariants(&db).await?;
        verify_down_restores_the_retired_column(&db).await
    }
    .await;

    let cleanup = drop_postgres_database_if_exists(&admin, &name).await;
    result?;
    cleanup?;
    Ok(())
}

/// Creates the two tables the channel schema references from outside the module
/// and applies every channel migration, so the test exercises the real DDL.
async fn install_owner_schema(db: &DatabaseConnection) -> TestResult {
    db.execute_unprepared(
        "CREATE TABLE tenants (
            id uuid PRIMARY KEY,
            name text NOT NULL,
            slug text NOT NULL UNIQUE,
            domain text UNIQUE,
            settings jsonb NOT NULL DEFAULT '{}'::jsonb,
            default_locale text NOT NULL DEFAULT 'en',
            is_active boolean NOT NULL DEFAULT true,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now()
        )",
    )
    .await?;
    db.execute_unprepared(
        "CREATE TABLE o_auth_apps (
            id uuid PRIMARY KEY,
            tenant_id uuid NOT NULL,
            name text NOT NULL,
            slug text NOT NULL,
            app_type text NOT NULL DEFAULT 'machine',
            is_active boolean NOT NULL DEFAULT true,
            created_at timestamptz NOT NULL DEFAULT now(),
            updated_at timestamptz NOT NULL DEFAULT now()
        )",
    )
    .await?;

    let manager = SchemaManager::new(db);
    for migration in rustok_channel::migrations::migrations() {
        migration.up(&manager).await?;
    }
    Ok(())
}

async fn seed_tenant(db: &DatabaseConnection, tenant_id: Uuid, slug: &str) -> TestResult {
    execute(
        db,
        "INSERT INTO tenants (id, name, slug) VALUES ($1, $2, $3)",
        vec![
            tenant_id.into(),
            format!("{slug} tenant").into(),
            slug.to_string().into(),
        ],
    )
    .await
}

async fn execute(db: &DatabaseConnection, sql: &str, values: Vec<Value>) -> TestResult {
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        sql,
        values,
    ))
    .await?;
    Ok(())
}

/// Runs a statement the schema is expected to reject and returns the driver
/// message, so a test can assert which constraint did the rejecting.
async fn expect_rejected(db: &DatabaseConnection, sql: &str, values: Vec<Value>) -> String {
    match db
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            values,
        ))
        .await
    {
        Ok(_) => panic!("statement should have been rejected by the schema: {sql}"),
        Err(error) => error.to_string(),
    }
}

async fn count_rows(db: &DatabaseConnection, sql: &str, values: Vec<Value>) -> TestResult<i64> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            values,
        ))
        .await?
        .ok_or("count query returned no row")?;
    Ok(row.try_get::<i64>("", "count")?)
}

async fn verify_up_invariants(db: &DatabaseConnection) -> TestResult {
    let tenant = Uuid::new_v4();
    let other_tenant = Uuid::new_v4();
    seed_tenant(db, tenant, "tenant").await?;
    seed_tenant(db, other_tenant, "other").await?;

    let service = ChannelService::new(db.clone());
    let first = service
        .create_channel(rustok_channel::CreateChannelInput {
            tenant_id: tenant,
            slug: "first".to_string(),
            name: "First".to_string(),
            settings: None,
        })
        .await?;
    let second = service
        .create_channel(rustok_channel::CreateChannelInput {
            tenant_id: tenant,
            slug: "second".to_string(),
            name: "Second".to_string(),
            settings: None,
        })
        .await?;
    let foreign = service
        .create_channel(rustok_channel::CreateChannelInput {
            tenant_id: other_tenant,
            slug: "foreign".to_string(),
            name: "Foreign".to_string(),
            settings: None,
        })
        .await?;

    // 1. One default channel per tenant, enforced by the partial unique index.
    let message = expect_rejected(
        db,
        "UPDATE channels SET is_default = true WHERE id = $1",
        vec![second.id.into()],
    )
    .await;
    assert!(
        message.contains("uq_channels_tenant_default"),
        "a second default channel must be rejected by uq_channels_tenant_default, got: {message}"
    );

    // 2. A target must carry a tenant (the denormalized column is NOT NULL).
    let message = expect_rejected(
        db,
        "INSERT INTO channel_targets (id, channel_id, target_type, value)
         VALUES ($1, $2, 'web_domain', 'untenanted.example.test')",
        vec![Uuid::new_v4().into(), first.id.into()],
    )
    .await;
    assert!(
        message.contains("tenant_id"),
        "channel_targets.tenant_id must be NOT NULL, got: {message}"
    );

    // 3. A target can only point at a channel of its own tenant (composite FK).
    let message = expect_rejected(
        db,
        "INSERT INTO channel_targets (id, channel_id, tenant_id, target_type, value)
         VALUES ($1, $2, $3, 'web_domain', 'cross-tenant.example.test')",
        vec![
            Uuid::new_v4().into(),
            first.id.into(),
            other_tenant.into(),
        ],
    )
    .await;
    assert!(
        message.contains("fk_channel_targets_tenant_channel"),
        "the composite (tenant_id, channel_id) FK must reject a foreign channel, got: {message}"
    );

    // 4. One web host per tenant, and the same host stays available to other tenants.
    execute(
        db,
        "INSERT INTO channel_targets (id, channel_id, tenant_id, target_type, value, is_primary)
         VALUES ($1, $2, $3, 'web_domain', 'shop.example.test', true)",
        vec![
            Uuid::new_v4().into(),
            first.id.into(),
            tenant.into(),
        ],
    )
    .await?;
    let message = expect_rejected(
        db,
        "INSERT INTO channel_targets (id, channel_id, tenant_id, target_type, value)
         VALUES ($1, $2, $3, 'web_domain', 'shop.example.test', false)",
        vec![
            Uuid::new_v4().into(),
            second.id.into(),
            tenant.into(),
        ],
    )
    .await;
    assert!(
        message.contains("uq_channel_targets_tenant_host"),
        "a host claimed twice in one tenant must be rejected by uq_channel_targets_tenant_host, got: {message}"
    );
    execute(
        db,
        "INSERT INTO channel_targets (id, channel_id, tenant_id, target_type, value)
         VALUES ($1, $2, $3, 'web_domain', 'shop.example.test')",
        vec![
            Uuid::new_v4().into(),
            foreign.id.into(),
            other_tenant.into(),
        ],
    )
    .await?;

    // 5. One primary target per channel, independent of the write that sets it.
    let message = expect_rejected(
        db,
        "INSERT INTO channel_targets (id, channel_id, tenant_id, target_type, value, is_primary)
         VALUES ($1, $2, $3, 'mobile_app', 'com.example.app', true)",
        vec![
            Uuid::new_v4().into(),
            first.id.into(),
            tenant.into(),
        ],
    )
    .await;
    assert!(
        message.contains("uq_channel_targets_channel_primary"),
        "a second primary target must be rejected by uq_channel_targets_channel_primary, got: {message}"
    );

    // 6. Deleting a channel cascades to its targets (same statement the owner uses).
    execute(
        db,
        "DELETE FROM channels WHERE id = $1",
        vec![first.id.into()],
    )
    .await?;
    let remaining = count_rows(
        db,
        "SELECT count(*) AS count FROM channel_targets WHERE channel_id = $1",
        vec![first.id.into()],
    )
    .await?;
    assert_eq!(
        remaining, 0,
        "the composite FK and the channel FK must cascade target rows away with the channel"
    );

    Ok(())
}

/// `down()` must put the schema back the way `up()` found it: the retired column
/// returns and the composite FK/indexes disappear.
async fn verify_down_restores_the_retired_column(db: &DatabaseConnection) -> TestResult {
    let tenant = Uuid::new_v4();
    seed_tenant(db, tenant, "down").await?;
    let service = ChannelService::new(db.clone());
    service
        .create_channel(rustok_channel::CreateChannelInput {
            tenant_id: tenant,
            slug: "rolling-back".to_string(),
            name: "Rolling back".to_string(),
            settings: None,
        })
        .await?;

    let manager = SchemaManager::new(db);
    let migration = rustok_channel::migrations::migrations()
        .into_iter()
        .find(|migration| migration.name() == LIFECYCLE_MIGRATION)
        .ok_or("missing channel lifecycle migration")?;
    migration.down(&manager).await?;

    db.query_one_raw(Statement::from_string(
        DbBackend::Postgres,
        "SELECT status FROM channels LIMIT 1",
    ))
    .await?;

    // The partial unique index is gone, so a second default row is accepted again.
    let first = service
        .create_channel(rustok_channel::CreateChannelInput {
            tenant_id: tenant,
            slug: "after-rollback-a".to_string(),
            name: "After rollback A".to_string(),
            settings: None,
        })
        .await?;
    let second = service
        .create_channel(rustok_channel::CreateChannelInput {
            tenant_id: tenant,
            slug: "after-rollback-b".to_string(),
            name: "After rollback B".to_string(),
            settings: None,
        })
        .await?;
    execute(
        db,
        "UPDATE channels SET is_default = true WHERE id = $1",
        vec![second.id.into()],
    )
    .await?;

    // The denormalized column and its composite FK are gone: a target insert no
    // longer needs (and can no longer carry) a tenant.
    execute(
        db,
        "INSERT INTO channel_targets (id, channel_id, target_type, value)
         VALUES ($1, $2, 'web_domain', 'after-rollback.example.test')",
        vec![Uuid::new_v4().into(), first.id.into()],
    )
    .await?;

    Ok(())
}
