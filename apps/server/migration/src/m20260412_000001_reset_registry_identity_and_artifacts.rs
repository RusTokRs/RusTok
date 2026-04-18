use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;
use serde_json::json;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        add_registry_identity_columns(manager).await?;
        backfill_registry_identity_columns(manager.get_connection()).await?;
        drop_legacy_registry_identity_columns(manager.get_connection()).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        add_legacy_registry_identity_columns(manager).await?;
        backfill_legacy_registry_identity_columns(manager.get_connection()).await?;
        drop_new_registry_identity_columns(manager.get_connection()).await
    }
}

async fn add_registry_identity_columns(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .alter_table(
            Table::alter()
                .table(RegistryModuleOwners::Table)
                .add_column(ColumnDef::new(RegistryModuleOwners::OwnerPrincipal).json_binary())
                .add_column(ColumnDef::new(RegistryModuleOwners::BoundByPrincipal).json_binary())
                .to_owned(),
        )
        .await?;

    manager
        .alter_table(
            Table::alter()
                .table(RegistryPublishRequests::Table)
                .add_column(
                    ColumnDef::new(RegistryPublishRequests::RequestedByPrincipal).json_binary(),
                )
                .add_column(
                    ColumnDef::new(RegistryPublishRequests::PublisherPrincipal).json_binary(),
                )
                .add_column(
                    ColumnDef::new(RegistryPublishRequests::ApprovedByPrincipal).json_binary(),
                )
                .add_column(
                    ColumnDef::new(RegistryPublishRequests::RejectedByPrincipal).json_binary(),
                )
                .add_column(
                    ColumnDef::new(RegistryPublishRequests::ChangesRequestedByPrincipal)
                        .json_binary(),
                )
                .add_column(
                    ColumnDef::new(RegistryPublishRequests::HeldByPrincipal).json_binary(),
                )
                .add_column(
                    ColumnDef::new(RegistryPublishRequests::ArtifactStorageKey)
                        .text()
                        .null(),
                )
                .to_owned(),
        )
        .await?;

    manager
        .alter_table(
            Table::alter()
                .table(RegistryModuleReleases::Table)
                .add_column(
                    ColumnDef::new(RegistryModuleReleases::PublisherPrincipal).json_binary(),
                )
                .add_column(
                    ColumnDef::new(RegistryModuleReleases::YankedByPrincipal).json_binary(),
                )
                .add_column(
                    ColumnDef::new(RegistryModuleReleases::ArtifactStorageKey)
                        .text()
                        .null(),
                )
                .to_owned(),
        )
        .await?;

    manager
        .alter_table(
            Table::alter()
                .table(RegistryGovernanceEvents::Table)
                .add_column(
                    ColumnDef::new(RegistryGovernanceEvents::ActorPrincipal).json_binary(),
                )
                .add_column(
                    ColumnDef::new(RegistryGovernanceEvents::PublisherPrincipal).json_binary(),
                )
                .to_owned(),
        )
        .await
}

async fn add_legacy_registry_identity_columns(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .alter_table(
            Table::alter()
                .table(RegistryModuleOwners::Table)
                .add_column(
                    ColumnDef::new(RegistryModuleOwners::OwnerActor)
                        .string_len(128)
                        .null(),
                )
                .add_column(
                    ColumnDef::new(RegistryModuleOwners::BoundBy)
                        .string_len(128)
                        .null(),
                )
                .to_owned(),
        )
        .await?;

    manager
        .alter_table(
            Table::alter()
                .table(RegistryPublishRequests::Table)
                .add_column(
                    ColumnDef::new(RegistryPublishRequests::RequestedBy)
                        .string_len(128)
                        .null(),
                )
                .add_column(
                    ColumnDef::new(RegistryPublishRequests::PublisherIdentity)
                        .string_len(128)
                        .null(),
                )
                .add_column(
                    ColumnDef::new(RegistryPublishRequests::ApprovedBy)
                        .string_len(128)
                        .null(),
                )
                .add_column(
                    ColumnDef::new(RegistryPublishRequests::RejectedBy)
                        .string_len(128)
                        .null(),
                )
                .add_column(
                    ColumnDef::new(RegistryPublishRequests::ChangesRequestedBy)
                        .string_len(128)
                        .null(),
                )
                .add_column(
                    ColumnDef::new(RegistryPublishRequests::HeldBy)
                        .string_len(128)
                        .null(),
                )
                .add_column(ColumnDef::new(RegistryPublishRequests::ArtifactPath).text().null())
                .add_column(ColumnDef::new(RegistryPublishRequests::ArtifactUrl).text().null())
                .to_owned(),
        )
        .await?;

    manager
        .alter_table(
            Table::alter()
                .table(RegistryModuleReleases::Table)
                .add_column(
                    ColumnDef::new(RegistryModuleReleases::Publisher)
                        .string_len(128)
                        .null(),
                )
                .add_column(
                    ColumnDef::new(RegistryModuleReleases::YankedBy)
                        .string_len(128)
                        .null(),
                )
                .add_column(ColumnDef::new(RegistryModuleReleases::ArtifactPath).text().null())
                .add_column(ColumnDef::new(RegistryModuleReleases::ArtifactUrl).text().null())
                .to_owned(),
        )
        .await?;

    manager
        .alter_table(
            Table::alter()
                .table(RegistryGovernanceEvents::Table)
                .add_column(
                    ColumnDef::new(RegistryGovernanceEvents::Actor)
                        .string_len(128)
                        .null(),
                )
                .add_column(
                    ColumnDef::new(RegistryGovernanceEvents::Publisher)
                        .string_len(128)
                        .null(),
                )
                .to_owned(),
        )
        .await
}

async fn backfill_registry_identity_columns(db: &SchemaManagerConnection<'_>) -> Result<(), DbErr> {
    backfill_registry_module_owners(db).await?;
    backfill_registry_publish_requests(db).await?;
    backfill_registry_module_releases(db).await?;
    backfill_registry_governance_events(db).await
}

async fn backfill_legacy_registry_identity_columns(
    db: &SchemaManagerConnection<'_>,
) -> Result<(), DbErr> {
    restore_registry_module_owners(db).await?;
    restore_registry_publish_requests(db).await?;
    restore_registry_module_releases(db).await?;
    restore_registry_governance_events(db).await
}

async fn backfill_registry_module_owners(db: &SchemaManagerConnection<'_>) -> Result<(), DbErr> {
    let backend = db.get_database_backend();
    let rows = db
        .query_all(Statement::from_string(
            backend,
            "SELECT slug, owner_actor, bound_by FROM registry_module_owners".to_string(),
        ))
        .await?;
    for row in rows {
        let slug = row.try_get::<String>("", "slug")?;
        let owner_actor = row.try_get::<String>("", "owner_actor")?;
        let bound_by = row.try_get::<String>("", "bound_by")?;
        execute_update(
            db,
            "UPDATE registry_module_owners SET owner_principal = {json1}, bound_by_principal = {json2} WHERE slug = {pk}",
            vec![
                principal_json_string(&owner_actor).into(),
                principal_json_string(&bound_by).into(),
                slug.into(),
            ],
        )
        .await?;
    }
    Ok(())
}

async fn backfill_registry_publish_requests(
    db: &SchemaManagerConnection<'_>,
) -> Result<(), DbErr> {
    let backend = db.get_database_backend();
    let rows = db
        .query_all(Statement::from_string(
            backend,
            "SELECT id, slug, version, requested_by, publisher_identity, approved_by, rejected_by, changes_requested_by, held_by, artifact_path, artifact_url FROM registry_publish_requests".to_string(),
        ))
        .await?;
    for row in rows {
        let id = row.try_get::<String>("", "id")?;
        let slug = row.try_get::<String>("", "slug")?;
        let version = row.try_get::<String>("", "version")?;
        let requested_by = row.try_get::<String>("", "requested_by")?;
        let publisher_identity = row.try_get::<Option<String>>("", "publisher_identity")?;
        let approved_by = row.try_get::<Option<String>>("", "approved_by")?;
        let rejected_by = row.try_get::<Option<String>>("", "rejected_by")?;
        let changes_requested_by = row.try_get::<Option<String>>("", "changes_requested_by")?;
        let held_by = row.try_get::<Option<String>>("", "held_by")?;
        let artifact_path = row.try_get::<Option<String>>("", "artifact_path")?;
        let artifact_url = row.try_get::<Option<String>>("", "artifact_url")?;
        let artifact_storage_key =
            derive_artifact_storage_key(Some(&id), &slug, &version, artifact_path.as_deref(), artifact_url.as_deref());
        if let (Some(path), Some(key)) = (artifact_path.as_deref(), artifact_storage_key.as_deref()) {
            let _ = copy_legacy_artifact_to_default_storage(path, key);
        }
        execute_update(
            db,
            "UPDATE registry_publish_requests SET requested_by_principal = {json1}, publisher_principal = {json2}, approved_by_principal = {json3}, rejected_by_principal = {json4}, changes_requested_by_principal = {json5}, held_by_principal = {json6}, artifact_storage_key = {json7} WHERE id = {pk}",
            vec![
                principal_json_string(&requested_by).into(),
                publisher_identity.map(|value| principal_json_string(&value)).into(),
                approved_by.map(|value| principal_json_string(&value)).into(),
                rejected_by.map(|value| principal_json_string(&value)).into(),
                changes_requested_by
                    .map(|value| principal_json_string(&value))
                    .into(),
                held_by.map(|value| principal_json_string(&value)).into(),
                artifact_storage_key.into(),
                id.into(),
            ],
        )
        .await?;
    }
    Ok(())
}

async fn backfill_registry_module_releases(
    db: &SchemaManagerConnection<'_>,
) -> Result<(), DbErr> {
    let backend = db.get_database_backend();
    let rows = db
        .query_all(Statement::from_string(
            backend,
            "SELECT id, request_id, slug, version, publisher, yanked_by, artifact_path, artifact_url FROM registry_module_releases".to_string(),
        ))
        .await?;
    for row in rows {
        let id = row.try_get::<String>("", "id")?;
        let request_id = row.try_get::<Option<String>>("", "request_id")?;
        let slug = row.try_get::<String>("", "slug")?;
        let version = row.try_get::<String>("", "version")?;
        let publisher = row.try_get::<String>("", "publisher")?;
        let yanked_by = row.try_get::<Option<String>>("", "yanked_by")?;
        let artifact_path = row.try_get::<Option<String>>("", "artifact_path")?;
        let artifact_url = row.try_get::<Option<String>>("", "artifact_url")?;
        let artifact_storage_key = derive_artifact_storage_key(
            request_id.as_deref().or(Some(&id)),
            &slug,
            &version,
            artifact_path.as_deref(),
            artifact_url.as_deref(),
        );
        if let (Some(path), Some(key)) = (artifact_path.as_deref(), artifact_storage_key.as_deref()) {
            let _ = copy_legacy_artifact_to_default_storage(path, key);
        }
        execute_update(
            db,
            "UPDATE registry_module_releases SET publisher_principal = {json1}, yanked_by_principal = {json2}, artifact_storage_key = {json3} WHERE id = {pk}",
            vec![
                principal_json_string(&publisher).into(),
                yanked_by.map(|value| principal_json_string(&value)).into(),
                artifact_storage_key.into(),
                id.into(),
            ],
        )
        .await?;
    }
    Ok(())
}

async fn backfill_registry_governance_events(
    db: &SchemaManagerConnection<'_>,
) -> Result<(), DbErr> {
    let backend = db.get_database_backend();
    let rows = db
        .query_all(Statement::from_string(
            backend,
            "SELECT id, actor, publisher FROM registry_governance_events".to_string(),
        ))
        .await?;
    for row in rows {
        let id = row.try_get::<String>("", "id")?;
        let actor = row.try_get::<String>("", "actor")?;
        let publisher = row.try_get::<Option<String>>("", "publisher")?;
        execute_update(
            db,
            "UPDATE registry_governance_events SET actor_principal = {json1}, publisher_principal = {json2} WHERE id = {pk}",
            vec![
                principal_json_string(&actor).into(),
                publisher.map(|value| principal_json_string(&value)).into(),
                id.into(),
            ],
        )
        .await?;
    }
    Ok(())
}

async fn restore_registry_module_owners(db: &SchemaManagerConnection<'_>) -> Result<(), DbErr> {
    let rows = select_rows(
        db,
        "SELECT slug, owner_principal, bound_by_principal FROM registry_module_owners",
    )
    .await?;
    for row in rows {
        let slug = row.try_get::<String>("", "slug")?;
        let owner_principal = row.try_get::<String>("", "owner_principal")?;
        let bound_by_principal = row.try_get::<String>("", "bound_by_principal")?;
        execute_update(
            db,
            "UPDATE registry_module_owners SET owner_actor = {json1}, bound_by = {json2} WHERE slug = {pk}",
            vec![
                principal_string_from_json(&owner_principal).into(),
                principal_string_from_json(&bound_by_principal).into(),
                slug.into(),
            ],
        )
        .await?;
    }
    Ok(())
}

async fn restore_registry_publish_requests(
    db: &SchemaManagerConnection<'_>,
) -> Result<(), DbErr> {
    let rows = select_rows(
        db,
        "SELECT id, requested_by_principal, publisher_principal, approved_by_principal, rejected_by_principal, changes_requested_by_principal, held_by_principal, artifact_storage_key FROM registry_publish_requests",
    )
    .await?;
    for row in rows {
        let id = row.try_get::<String>("", "id")?;
        let requested_by = row.try_get::<String>("", "requested_by_principal")?;
        let publisher = row.try_get::<Option<String>>("", "publisher_principal")?;
        let approved_by = row.try_get::<Option<String>>("", "approved_by_principal")?;
        let rejected_by = row.try_get::<Option<String>>("", "rejected_by_principal")?;
        let changes_requested_by =
            row.try_get::<Option<String>>("", "changes_requested_by_principal")?;
        let held_by = row.try_get::<Option<String>>("", "held_by_principal")?;
        let artifact_storage_key = row.try_get::<Option<String>>("", "artifact_storage_key")?;
        execute_update(
            db,
            "UPDATE registry_publish_requests SET requested_by = {json1}, publisher_identity = {json2}, approved_by = {json3}, rejected_by = {json4}, changes_requested_by = {json5}, held_by = {json6}, artifact_path = {json7}, artifact_url = {json7} WHERE id = {pk}",
            vec![
                principal_string_from_json(&requested_by).into(),
                publisher.map(|value| principal_string_from_json(&value)).into(),
                approved_by.map(|value| principal_string_from_json(&value)).into(),
                rejected_by.map(|value| principal_string_from_json(&value)).into(),
                changes_requested_by
                    .map(|value| principal_string_from_json(&value))
                    .into(),
                held_by.map(|value| principal_string_from_json(&value)).into(),
                artifact_storage_key.clone().into(),
                id.into(),
            ],
        )
        .await?;
    }
    Ok(())
}

async fn restore_registry_module_releases(
    db: &SchemaManagerConnection<'_>,
) -> Result<(), DbErr> {
    let rows = select_rows(
        db,
        "SELECT id, publisher_principal, yanked_by_principal, artifact_storage_key FROM registry_module_releases",
    )
    .await?;
    for row in rows {
        let id = row.try_get::<String>("", "id")?;
        let publisher = row.try_get::<String>("", "publisher_principal")?;
        let yanked_by = row.try_get::<Option<String>>("", "yanked_by_principal")?;
        let artifact_storage_key = row.try_get::<Option<String>>("", "artifact_storage_key")?;
        execute_update(
            db,
            "UPDATE registry_module_releases SET publisher = {json1}, yanked_by = {json2}, artifact_path = {json3}, artifact_url = {json3} WHERE id = {pk}",
            vec![
                principal_string_from_json(&publisher).into(),
                yanked_by.map(|value| principal_string_from_json(&value)).into(),
                artifact_storage_key.into(),
                id.into(),
            ],
        )
        .await?;
    }
    Ok(())
}

async fn restore_registry_governance_events(
    db: &SchemaManagerConnection<'_>,
) -> Result<(), DbErr> {
    let rows = select_rows(
        db,
        "SELECT id, actor_principal, publisher_principal FROM registry_governance_events",
    )
    .await?;
    for row in rows {
        let id = row.try_get::<String>("", "id")?;
        let actor = row.try_get::<String>("", "actor_principal")?;
        let publisher = row.try_get::<Option<String>>("", "publisher_principal")?;
        execute_update(
            db,
            "UPDATE registry_governance_events SET actor = {json1}, publisher = {json2} WHERE id = {pk}",
            vec![
                principal_string_from_json(&actor).into(),
                publisher.map(|value| principal_string_from_json(&value)).into(),
                id.into(),
            ],
        )
        .await?;
    }
    Ok(())
}

async fn drop_legacy_registry_identity_columns(db: &SchemaManagerConnection<'_>) -> Result<(), DbErr> {
    drop_columns(
        db,
        "registry_module_owners",
        &["owner_actor", "bound_by"],
    )
    .await?;
    drop_columns(
        db,
        "registry_publish_requests",
        &[
            "requested_by",
            "publisher_identity",
            "approved_by",
            "rejected_by",
            "changes_requested_by",
            "held_by",
            "artifact_path",
            "artifact_url",
        ],
    )
    .await?;
    drop_columns(
        db,
        "registry_module_releases",
        &["publisher", "yanked_by", "artifact_path", "artifact_url"],
    )
    .await?;
    drop_columns(
        db,
        "registry_governance_events",
        &["actor", "publisher"],
    )
    .await
}

async fn drop_new_registry_identity_columns(db: &SchemaManagerConnection<'_>) -> Result<(), DbErr> {
    drop_columns(
        db,
        "registry_module_owners",
        &["owner_principal", "bound_by_principal"],
    )
    .await?;
    drop_columns(
        db,
        "registry_publish_requests",
        &[
            "requested_by_principal",
            "publisher_principal",
            "approved_by_principal",
            "rejected_by_principal",
            "changes_requested_by_principal",
            "held_by_principal",
            "artifact_storage_key",
        ],
    )
    .await?;
    drop_columns(
        db,
        "registry_module_releases",
        &["publisher_principal", "yanked_by_principal", "artifact_storage_key"],
    )
    .await?;
    drop_columns(
        db,
        "registry_governance_events",
        &["actor_principal", "publisher_principal"],
    )
    .await
}

async fn select_rows(db: &SchemaManagerConnection<'_>, sql: &str) -> Result<Vec<sea_orm::QueryResult>, DbErr> {
    db.query_all(Statement::from_string(
        db.get_database_backend(),
        sql.to_string(),
    ))
    .await
}

async fn execute_update(
    db: &SchemaManagerConnection<'_>,
    template: &str,
    values: Vec<sea_orm::Value>,
) -> Result<(), DbErr> {
    let backend = db.get_database_backend();
    let sql = placeholder_sql(backend, template);
    db.execute(Statement::from_sql_and_values(backend, sql, values))
        .await?;
    Ok(())
}

fn placeholder_sql(backend: DbBackend, template: &str) -> String {
    let placeholders = match backend {
        DbBackend::Sqlite => ["?1", "?2", "?3", "?4", "?5", "?6", "?7", "?8"],
        _ => ["$1", "$2", "$3", "$4", "$5", "$6", "$7", "$8"],
    };
    let mut sql = template.to_string();
    for (index, placeholder) in placeholders.into_iter().enumerate() {
        sql = sql.replace(&format!("{{json{}}}", index + 1), placeholder);
    }
    let pk_placeholder = placeholders[values_index_from_template(template) - 1];
    sql.replace("{pk}", pk_placeholder)
}

fn values_index_from_template(template: &str) -> usize {
    template.matches("{json").count() + 1
}

async fn drop_columns(
    db: &SchemaManagerConnection<'_>,
    table: &str,
    columns: &[&str],
) -> Result<(), DbErr> {
    let backend = db.get_database_backend();
    for column in columns {
        db.execute(Statement::from_string(
            backend,
            format!("ALTER TABLE {table} DROP COLUMN {column}"),
        ))
        .await?;
    }
    Ok(())
}

fn principal_json_string(value: &str) -> serde_json::Value {
    let normalized = value.trim();
    if let Some(raw) = normalized.strip_prefix("user:") {
        if let Ok(user_id) = Uuid::parse_str(raw) {
            return json!({
                "kind": "user",
                "user_id": user_id,
                "subject": format!("user:{user_id}"),
                "display_label": format!("user:{user_id}"),
                "legacy_label": serde_json::Value::Null,
            })
            ;
        }
    }
    if let Some(runner_id) = normalized.strip_prefix("remote-runner:") {
        return json!({
            "kind": "runner",
            "user_id": serde_json::Value::Null,
            "subject": format!("remote-runner:{runner_id}"),
            "display_label": format!("remote-runner:{runner_id}"),
            "legacy_label": serde_json::Value::Null,
        })
        ;
    }
    json!({
        "kind": "legacy",
        "user_id": serde_json::Value::Null,
        "subject": normalized,
        "display_label": normalized,
        "legacy_label": normalized,
    })
}

fn principal_string_from_json(value: &str) -> String {
    serde_json::from_str::<serde_json::Value>(value)
        .ok()
        .and_then(|json| {
            json.get("legacy_label")
                .and_then(|inner| inner.as_str())
                .map(ToString::to_string)
                .or_else(|| {
                    json.get("subject")
                        .and_then(|inner| inner.as_str())
                        .map(ToString::to_string)
                })
                .or_else(|| {
                    json.get("display_label")
                        .and_then(|inner| inner.as_str())
                        .map(ToString::to_string)
                })
        })
        .unwrap_or_else(|| value.to_string())
}

fn derive_artifact_storage_key(
    scope_id: Option<&str>,
    slug: &str,
    version: &str,
    artifact_path: Option<&str>,
    artifact_url: Option<&str>,
) -> Option<String> {
    if let Some(url) = artifact_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|value| value.strip_prefix("/registry-artifacts/"))
    {
        return Some(format!("registry/artifacts/{url}"));
    }
    let filename = artifact_path
        .and_then(|value| Path::new(value).file_name())
        .and_then(|value| value.to_str())
        .map(ToString::to_string)
        .or_else(|| artifact_url.map(Path::new).and_then(|value| value.file_name()).and_then(|value| value.to_str()).map(ToString::to_string))
        .unwrap_or_else(|| format!("{slug}-{version}.crate"));
    scope_id.map(|value| format!("registry/artifacts/{value}/{filename}"))
}

fn copy_legacy_artifact_to_default_storage(path: &str, key: &str) -> std::io::Result<()> {
    let source = Path::new(path);
    if !source.is_file() {
        return Ok(());
    }
    let destination = default_storage_root().join(key);
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if !destination.exists() {
        std::fs::copy(source, destination)?;
    }
    Ok(())
}

fn default_storage_root() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("storage")
        .join("media")
}

#[derive(DeriveIden)]
enum RegistryModuleOwners {
    Table,
    OwnerActor,
    BoundBy,
    OwnerPrincipal,
    BoundByPrincipal,
}

#[derive(DeriveIden)]
enum RegistryPublishRequests {
    Table,
    RequestedBy,
    PublisherIdentity,
    ApprovedBy,
    RejectedBy,
    ChangesRequestedBy,
    HeldBy,
    ArtifactPath,
    ArtifactUrl,
    RequestedByPrincipal,
    PublisherPrincipal,
    ApprovedByPrincipal,
    RejectedByPrincipal,
    ChangesRequestedByPrincipal,
    HeldByPrincipal,
    ArtifactStorageKey,
}

#[derive(DeriveIden)]
enum RegistryModuleReleases {
    Table,
    Publisher,
    YankedBy,
    ArtifactPath,
    ArtifactUrl,
    PublisherPrincipal,
    YankedByPrincipal,
    ArtifactStorageKey,
}

#[derive(DeriveIden)]
enum RegistryGovernanceEvents {
    Table,
    Actor,
    Publisher,
    ActorPrincipal,
    PublisherPrincipal,
}
