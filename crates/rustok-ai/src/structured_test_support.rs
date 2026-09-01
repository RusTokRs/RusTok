use std::path::Path;

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend,
    Set, Statement,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use serde_json::json;
use uuid::Uuid;

use crate::{
    AiProviderConfig, ProviderCapability, ProviderTargetId,
    entities::{ai_provider_profiles, ai_task_profiles},
    migrations::m20260729_000001_structured_execution::Migration,
};

pub(crate) async fn database() -> DatabaseConnection {
    let database = Database::connect("sqlite::memory:")
        .await
        .expect("structured runtime test database");
    initialize(database).await
}

pub(crate) async fn database_at(database_path: &Path) -> DatabaseConnection {
    initialize(connect_file(database_path, true).await).await
}

pub(crate) async fn connect_file(
    database_path: &Path,
    create_if_missing: bool,
) -> DatabaseConnection {
    let database_path = database_path.to_path_buf();
    let mut options =
        ConnectOptions::new("sqlite://structured-process-placeholder.sqlite?mode=rwc");
    options
        .max_connections(4)
        .min_connections(1)
        .sqlx_logging(false)
        .map_sqlx_sqlite_opts(move |options| {
            options
                .filename(database_path.clone())
                .create_if_missing(create_if_missing)
        });
    Database::connect(options)
        .await
        .expect("structured runtime file database")
}

async fn initialize(database: DatabaseConnection) -> DatabaseConnection {
    database
        .execute_unprepared(
            "PRAGMA foreign_keys = ON; \
             CREATE TABLE tenants (id UUID PRIMARY KEY); \
             CREATE TABLE ai_provider_profiles (\
                 id UUID PRIMARY KEY, tenant_id UUID NOT NULL, slug TEXT NOT NULL, \
                 display_name TEXT NOT NULL, provider_slug TEXT NOT NULL, \
                 provider_target_id TEXT NOT NULL, model TEXT NOT NULL, credential_refs JSON NOT NULL, \
                 temperature REAL NULL, max_tokens INTEGER NULL, is_active BOOLEAN NOT NULL, \
                 capabilities JSON NOT NULL, allowed_task_profiles JSON NOT NULL, \
                 denied_task_profiles JSON NOT NULL, restricted_role_slugs JSON NOT NULL, \
                 metadata JSON NOT NULL, created_by UUID NULL, updated_by UUID NULL, \
                 created_at TIMESTAMPTZ NOT NULL, updated_at TIMESTAMPTZ NOT NULL); \
             CREATE TABLE ai_task_profiles (\
                 id UUID PRIMARY KEY, tenant_id UUID NOT NULL, slug TEXT NOT NULL, \
                 display_name TEXT NOT NULL, description TEXT NULL, target_capability TEXT NOT NULL, \
                 system_prompt TEXT NULL, allowed_provider_profile_ids JSON NOT NULL, \
                 preferred_provider_profile_ids JSON NOT NULL, fallback_strategy TEXT NOT NULL, \
                 tool_profile_id UUID NULL, approval_policy JSON NOT NULL, \
                 default_execution_mode TEXT NOT NULL, is_active BOOLEAN NOT NULL, metadata JSON NOT NULL, \
                 created_by UUID NULL, updated_by UUID NULL, \
                 created_at TIMESTAMPTZ NOT NULL, updated_at TIMESTAMPTZ NOT NULL)",
        )
        .await
        .expect("structured runtime owner tables");
    Migration
        .up(&SchemaManager::new(&database))
        .await
        .expect("structured runtime migration");
    database
}

pub(crate) async fn insert_tenant(database: &DatabaseConnection, tenant_id: Uuid) {
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO tenants (id) VALUES (?)".to_string(),
            vec![tenant_id.into()],
        ))
        .await
        .expect("structured runtime tenant");
}

pub(crate) async fn insert_provider_profile(
    database: &DatabaseConnection,
    tenant_id: Uuid,
    provider_id: Uuid,
    slug: &str,
) {
    let now = Utc::now();
    ai_provider_profiles::ActiveModel {
        id: Set(provider_id),
        tenant_id: Set(tenant_id),
        slug: Set(slug.to_string()),
        display_name: Set(format!("{slug} provider")),
        provider_slug: Set("openai_compatible".to_string()),
        provider_target_id: Set("openai_compatible".to_string()),
        model: Set(format!("{slug}-model")),
        credential_refs: Set(json!({})),
        temperature: Set(None),
        max_tokens: Set(None),
        is_active: Set(true),
        capabilities: Set(json!([ProviderCapability::StructuredGeneration.slug()])),
        allowed_task_profiles: Set(json!([])),
        denied_task_profiles: Set(json!([])),
        restricted_role_slugs: Set(json!([])),
        metadata: Set(json!({})),
        created_by: Set(None),
        updated_by: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(database)
    .await
    .expect("structured runtime provider profile");
}

pub(crate) async fn insert_live_provider_profile(
    database: &DatabaseConnection,
    tenant_id: Uuid,
    provider_id: Uuid,
    target_id: &ProviderTargetId,
    config: &AiProviderConfig,
) {
    let now = Utc::now();
    ai_provider_profiles::ActiveModel {
        id: Set(provider_id),
        tenant_id: Set(tenant_id),
        slug: Set("live-structured-provider".to_string()),
        display_name: Set("Live structured provider".to_string()),
        provider_slug: Set(config.provider_slug.to_string()),
        provider_target_id: Set(target_id.to_string()),
        model: Set(config.model.clone()),
        credential_refs: Set(serde_json::to_value(&config.credential_refs)
            .expect("live structured credential references")),
        temperature: Set(config.temperature),
        max_tokens: Set(config
            .max_tokens
            .map(|value| i32::try_from(value).expect("live structured max tokens"))),
        is_active: Set(true),
        capabilities: Set(json!([ProviderCapability::StructuredGeneration.slug()])),
        allowed_task_profiles: Set(json!([])),
        denied_task_profiles: Set(json!([])),
        restricted_role_slugs: Set(json!([])),
        metadata: Set(json!({"evidence": "deployment_live_structured_probe"})),
        created_by: Set(None),
        updated_by: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(database)
    .await
    .expect("live structured provider profile");
}

pub(crate) async fn insert_task_profile(
    database: &DatabaseConnection,
    tenant_id: Uuid,
    provider_ids: &[Uuid],
) {
    insert_task_profile_for(database, tenant_id, "machine_translation", provider_ids).await;
}

pub(crate) async fn insert_task_profile_for(
    database: &DatabaseConnection,
    tenant_id: Uuid,
    task_slug: &str,
    provider_ids: &[Uuid],
) {
    let now = Utc::now();
    ai_task_profiles::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        slug: Set(task_slug.to_string()),
        display_name: Set(format!("{task_slug} structured task")),
        description: Set(None),
        target_capability: Set(ProviderCapability::StructuredGeneration.slug().to_string()),
        system_prompt: Set(None),
        allowed_provider_profile_ids: Set(json!(provider_ids)),
        preferred_provider_profile_ids: Set(json!(provider_ids)),
        fallback_strategy: Set("ordered".to_string()),
        tool_profile_id: Set(None),
        approval_policy: Set(json!({})),
        default_execution_mode: Set("direct".to_string()),
        is_active: Set(true),
        metadata: Set(json!({})),
        created_by: Set(None),
        updated_by: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(database)
    .await
    .expect("structured runtime task profile");
}
