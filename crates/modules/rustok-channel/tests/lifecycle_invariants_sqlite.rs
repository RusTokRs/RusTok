/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! SQLite proof for the lifecycle and invariant work landed with migration
//! `m20261126_000013`: one default channel per tenant, default promotion on
//! deactivation and deletion, one web-host claim per tenant, one primary target
//! per channel, one active policy set per tenant, closed `settings` shape and the
//! `404` (not `500`) answer for an already-removed resolution rule.
//!
//! The partial unique indexes are the subject of several tests: the owner repairs
//! legacy rows and orders its writes so those indexes are never violated by the
//! happy path, and a direct `UPDATE` here proves the index would reject the write
//! that the owner avoids.

use rustok_channel::{
    ChannelError, ChannelResolutionRuleDefinition, ChannelResponse, ChannelService,
    CreateChannelInput, CreateChannelResolutionPolicySetInput, CreateChannelResolutionRuleInput,
    CreateChannelTargetInput, ResolutionAction, ResolutionPredicate, UpdateChannelInput,
    migrations,
};
use rustok_test_utils::setup_test_db;
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use sea_orm_migration::SchemaManager;
use serde_json::json;
use uuid::Uuid;

async fn setup_channel_db() -> DatabaseConnection {
    let db = setup_test_db().await;
    db.execute_raw(Statement::from_string(
        db.get_database_backend(),
        r#"
        CREATE TABLE tenants (
            id TEXT PRIMARY KEY NOT NULL,
            name TEXT NOT NULL,
            slug TEXT NOT NULL UNIQUE,
            domain TEXT NULL UNIQUE,
            settings TEXT NOT NULL DEFAULT '{}',
            default_locale TEXT NOT NULL DEFAULT 'en',
            is_active BOOLEAN NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    ))
    .await
    .expect("tenants table should exist for channel foreign keys");
    db.execute_raw(Statement::from_string(
        db.get_database_backend(),
        r#"
        CREATE TABLE o_auth_apps (
            id TEXT PRIMARY KEY NOT NULL,
            tenant_id TEXT NOT NULL,
            name TEXT NOT NULL,
            slug TEXT NOT NULL,
            app_type TEXT NOT NULL DEFAULT 'machine',
            is_active BOOLEAN NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    ))
    .await
    .expect("o_auth_apps table should exist for channel foreign keys");
    let manager = SchemaManager::new(&db);
    for migration in migrations::migrations() {
        migration
            .up(&manager)
            .await
            .expect("channel migration should apply");
    }
    db
}

async fn seed_tenant(db: &DatabaseConnection, tenant_id: Uuid, slug: &str) {
    db.execute_raw(Statement::from_sql_and_values(
        db.get_database_backend(),
        "INSERT INTO tenants (id, name, slug, settings, default_locale, is_active, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        [
            tenant_id.into(),
            format!("{slug} tenant").into(),
            slug.to_string().into(),
            "{}".to_string().into(),
            "en".to_string().into(),
            true.into(),
        ],
    ))
    .await
    .expect("tenant should be inserted");
}

async fn create_channel(db: &DatabaseConnection, tenant_id: Uuid, slug: &str) -> ChannelResponse {
    ChannelService::new(db.clone())
        .create_channel(CreateChannelInput {
            tenant_id,
            slug: slug.to_string(),
            name: slug.to_string(),
            settings: None,
        })
        .await
        .unwrap_or_else(|error| panic!("channel '{slug}' should be created: {error:?}"))
}

/// Pins `created_at` so the "oldest active channel is promoted" assertions do not
/// depend on how far apart two inserts happened to land. All values written by a
/// test share one literal format, so their text ordering is the intended one.
async fn pin_created_at(db: &DatabaseConnection, channel_id: Uuid, timestamp: &str) {
    db.execute_raw(Statement::from_sql_and_values(
        db.get_database_backend(),
        "UPDATE channels SET created_at = ? WHERE id = ?",
        [timestamp.to_string().into(), channel_id.into()],
    ))
    .await
    .expect("created_at should be pinned");
}

async fn add_web_target(db: &DatabaseConnection, channel_id: Uuid, host: &str, is_primary: bool) {
    ChannelService::new(db.clone())
        .add_target(
            channel_id,
            CreateChannelTargetInput {
                target_type: "web_domain".to_string(),
                value: host.to_string(),
                is_primary,
                settings: None,
            },
        )
        .await
        .unwrap_or_else(|error| panic!("target '{host}' should be created: {error:?}"));
}

/// Runs a statement that the schema is expected to reject and returns the error
/// message, so a test can assert *why* it was rejected.
async fn expect_rejected(
    db: &DatabaseConnection,
    sql: &str,
    values: Vec<sea_orm::Value>,
) -> String {
    let result = db
        .execute_raw(Statement::from_sql_and_values(
            db.get_database_backend(),
            sql,
            values,
        ))
        .await;
    match result {
        Ok(_) => panic!("statement should have been rejected by a unique index: {sql}"),
        Err(error) => error.to_string(),
    }
}

#[tokio::test]
async fn only_one_default_channel_per_tenant() {
    let db = setup_channel_db().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id, "tenant").await;

    let first = create_channel(&db, tenant_id, "first").await;
    let second = create_channel(&db, tenant_id, "second").await;
    assert!(
        first.is_default,
        "the first channel of a tenant is the default"
    );
    assert!(
        !second.is_default,
        "a second channel must not become the default"
    );

    let message = expect_rejected(
        &db,
        "UPDATE channels SET is_default = 1 WHERE id = ?",
        vec![second.id.into()],
    )
    .await;
    assert!(
        message.to_lowercase().contains("unique"),
        "uq_channels_tenant_default must reject a second default row, got: {message}"
    );
}

#[tokio::test]
async fn only_one_active_policy_set_per_tenant() {
    let db = setup_channel_db().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id, "tenant").await;
    let service = ChannelService::new(db.clone());

    service
        .create_resolution_policy_set(CreateChannelResolutionPolicySetInput {
            tenant_id,
            slug: "active".to_string(),
            name: "Active".to_string(),
            is_active: true,
        })
        .await
        .expect("first policy set should be created");
    let second = service
        .create_resolution_policy_set(CreateChannelResolutionPolicySetInput {
            tenant_id,
            slug: "later".to_string(),
            name: "Later".to_string(),
            is_active: false,
        })
        .await
        .expect("second policy set should be created");

    let message = expect_rejected(
        &db,
        "UPDATE channel_resolution_policy_sets SET is_active = 1 WHERE id = ?",
        vec![second.id.into()],
    )
    .await;
    assert!(
        message.to_lowercase().contains("unique"),
        "uq_channel_resolution_policy_sets_tenant_active must reject a second active set, got: {message}"
    );
}

#[tokio::test]
async fn deactivating_the_default_promotes_the_oldest_active_channel() {
    let db = setup_channel_db().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id, "tenant").await;
    let service = ChannelService::new(db.clone());

    let default_channel = create_channel(&db, tenant_id, "default").await;
    let promoted = create_channel(&db, tenant_id, "promoted").await;
    let youngest = create_channel(&db, tenant_id, "youngest").await;
    pin_created_at(&db, default_channel.id, "2026-01-01 00:00:01").await;
    pin_created_at(&db, promoted.id, "2026-01-01 00:00:02").await;
    pin_created_at(&db, youngest.id, "2026-01-01 00:00:03").await;

    let deactivated = service
        .set_channel_active(tenant_id, default_channel.id, false)
        .await
        .expect("default channel should be deactivated");
    assert!(!deactivated.is_active);
    assert!(
        !deactivated.is_default,
        "an inactive channel must not stay the tenant default"
    );

    let current_default = service
        .get_default_channel(tenant_id)
        .await
        .expect("default lookup should succeed")
        .expect("the tenant must keep a default while an active channel exists");
    assert_eq!(
        current_default.channel.id, promoted.id,
        "the oldest remaining active channel is promoted"
    );
    assert_ne!(current_default.channel.id, youngest.id);
    assert!(current_default.channel.is_active);
}

#[tokio::test]
async fn deactivating_the_last_active_channel_clears_the_default() {
    let db = setup_channel_db().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id, "tenant").await;
    let service = ChannelService::new(db.clone());

    let only_channel = create_channel(&db, tenant_id, "only").await;
    assert!(only_channel.is_default);

    service
        .set_channel_active(tenant_id, only_channel.id, false)
        .await
        .expect("the only channel should be deactivated");

    assert!(
        service
            .get_default_channel(tenant_id)
            .await
            .expect("default lookup should succeed")
            .is_none(),
        "a tenant without active channels has no default"
    );
}

#[tokio::test]
async fn deleting_the_default_promotes_the_oldest_active_channel() {
    let db = setup_channel_db().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id, "tenant").await;
    let service = ChannelService::new(db.clone());

    let default_channel = create_channel(&db, tenant_id, "default").await;
    let promoted = create_channel(&db, tenant_id, "promoted").await;
    pin_created_at(&db, default_channel.id, "2026-01-01 00:00:01").await;
    pin_created_at(&db, promoted.id, "2026-01-01 00:00:02").await;

    service
        .delete_channel(tenant_id, default_channel.id)
        .await
        .expect("the default channel should be deleted");

    let current_default = service
        .get_default_channel(tenant_id)
        .await
        .expect("default lookup should succeed")
        .expect("the surviving channel must become the default");
    assert_eq!(current_default.channel.id, promoted.id);
}

#[tokio::test]
async fn a_web_host_can_only_be_claimed_once_per_tenant() {
    let db = setup_channel_db().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id, "tenant").await;

    let first = create_channel(&db, tenant_id, "first").await;
    let second = create_channel(&db, tenant_id, "second").await;
    add_web_target(&db, first.id, "shop.example.test", true).await;

    let error = ChannelService::new(db.clone())
        .add_target(
            second.id,
            CreateChannelTargetInput {
                target_type: "web_domain".to_string(),
                value: "shop.example.test".to_string(),
                is_primary: false,
                settings: None,
            },
        )
        .await
        .expect_err("a host claimed by another channel must be rejected");

    match error {
        ChannelError::TargetAlreadyExists(kind, value) => {
            assert_eq!(kind, "web_domain");
            assert_eq!(value, "shop.example.test");
        }
        other => panic!("expected TargetAlreadyExists, got {other:?}"),
    }
}

#[tokio::test]
async fn the_same_web_host_is_allowed_in_another_tenant() {
    let db = setup_channel_db().await;
    let first_tenant = Uuid::new_v4();
    let second_tenant = Uuid::new_v4();
    seed_tenant(&db, first_tenant, "first").await;
    seed_tenant(&db, second_tenant, "second").await;

    let first = create_channel(&db, first_tenant, "first").await;
    let second = create_channel(&db, second_tenant, "second").await;
    add_web_target(&db, first.id, "shop.example.test", true).await;
    add_web_target(&db, second.id, "shop.example.test", true).await;
}

#[tokio::test]
async fn adding_a_second_primary_target_demotes_the_previous_one() {
    let db = setup_channel_db().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id, "tenant").await;
    let service = ChannelService::new(db.clone());

    let channel = create_channel(&db, tenant_id, "channel").await;
    add_web_target(&db, channel.id, "first.example.test", true).await;
    add_web_target(&db, channel.id, "second.example.test", true).await;

    let detail = service
        .get_channel_detail(channel.id)
        .await
        .expect("channel detail should be readable");
    let primaries = detail
        .targets
        .iter()
        .filter(|target| target.is_primary)
        .collect::<Vec<_>>();
    assert_eq!(
        primaries.len(),
        1,
        "uq_channel_targets_channel_primary allows one primary"
    );
    assert_eq!(primaries[0].value, "second.example.test");
}

#[tokio::test]
async fn channel_settings_must_be_a_json_object() {
    let db = setup_channel_db().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id, "tenant").await;
    let service = ChannelService::new(db.clone());

    let created = service
        .create_channel(CreateChannelInput {
            tenant_id,
            slug: "invalid".to_string(),
            name: "Invalid".to_string(),
            settings: Some(json!([1, 2, 3])),
        })
        .await;
    assert!(
        matches!(created, Err(ChannelError::Validation(_))),
        "create_channel must reject settings that are not an object"
    );

    let channel = create_channel(&db, tenant_id, "valid").await;
    let updated = service
        .update_channel(
            tenant_id,
            channel.id,
            UpdateChannelInput {
                name: None,
                settings: Some(json!("scalar")),
            },
        )
        .await;
    assert!(
        matches!(updated, Err(ChannelError::Validation(_))),
        "update_channel must reject settings that are not an object"
    );

    let accepted = service
        .update_channel(
            tenant_id,
            channel.id,
            UpdateChannelInput {
                name: None,
                settings: Some(json!({ "theme": "dark" })),
            },
        )
        .await
        .expect("an object-shaped settings patch should be accepted");
    assert_eq!(accepted.settings, json!({ "theme": "dark" }));
}

#[tokio::test]
async fn removing_a_resolution_rule_twice_answers_not_found() {
    let db = setup_channel_db().await;
    let tenant_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id, "tenant").await;
    let service = ChannelService::new(db.clone());

    let channel = create_channel(&db, tenant_id, "channel").await;
    let policy_set = service
        .create_resolution_policy_set(CreateChannelResolutionPolicySetInput {
            tenant_id,
            slug: "set".to_string(),
            name: "Set".to_string(),
            is_active: true,
        })
        .await
        .expect("policy set should be created");
    let rule = service
        .create_resolution_rule(
            policy_set.id,
            CreateChannelResolutionRuleInput {
                priority: 10,
                is_active: true,
                definition: ChannelResolutionRuleDefinition {
                    predicates: vec![ResolutionPredicate::HostEquals(
                        "shop.example.test".to_string(),
                    )],
                    action: ResolutionAction::ResolveToChannel {
                        channel_id: channel.id,
                    },
                },
            },
        )
        .await
        .expect("policy rule should be created");

    service
        .remove_resolution_rule(policy_set.id, rule.id)
        .await
        .expect("the first removal should succeed");

    let second = service.remove_resolution_rule(policy_set.id, rule.id).await;
    assert!(
        matches!(second, Err(ChannelError::NotFound(id)) if id == rule.id),
        "a rule that is already gone must answer NotFound, not a storage error"
    );
}
