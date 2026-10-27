use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;
use serde_json::Value;
use uuid::Uuid;

use crate::backend::SupportedBackend;

/// Admission-time projection of the collision-checked UI resources
/// (navigation routes and storefront slots) declared by each immutable
/// installation descriptor. The activation-time collision check becomes
/// an indexed lookup over this table instead of deserializing every
/// active descriptor on the scope surface inside the activation
/// transaction. Rows are written once, in the same transaction that
/// creates the installation, and are immutable like the descriptor they
/// project.
///
/// The table deliberately carries no PostgreSQL RLS policy: activation
/// must compare a tenant candidate against platform contributions (and a
/// platform candidate against every tenant overlay), exactly like the
/// transition checkpoint fence. Scope discrimination is explicit in the
/// `scope_kind`/`tenant_id` columns and in every reader's predicate.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        let statements: &[&str] = match backend {
            SupportedBackend::Postgres => &[
                "CREATE TABLE module_artifact_ui_resources (\
                    installation_id UUID NOT NULL,\
                    resource_kind TEXT NOT NULL,\
                    resource_key TEXT NOT NULL,\
                    scope_kind TEXT NOT NULL,\
                    tenant_id UUID NULL,\
                    PRIMARY KEY (installation_id, resource_kind, resource_key)\
                )",
                "CREATE INDEX module_artifact_ui_resources_lookup_idx \
                 ON module_artifact_ui_resources (resource_kind, resource_key)",
            ],
            SupportedBackend::Sqlite => &[
                "CREATE TABLE module_artifact_ui_resources (\
                    installation_id TEXT NOT NULL,\
                    resource_kind TEXT NOT NULL,\
                    resource_key TEXT NOT NULL,\
                    scope_kind TEXT NOT NULL,\
                    tenant_id TEXT NULL,\
                    PRIMARY KEY (installation_id, resource_kind, resource_key)\
                )",
                "CREATE INDEX module_artifact_ui_resources_lookup_idx \
                 ON module_artifact_ui_resources (resource_kind, resource_key)",
            ],
        };
        let connection = manager.get_connection();
        for statement in statements {
            connection
                .execute_raw(Statement::from_string(
                    backend.db_backend(),
                    (*statement).to_string(),
                ))
                .await?;
        }
        backfill_ui_resources(connection, backend).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        manager
            .get_connection()
            .execute_raw(Statement::from_string(
                backend.db_backend(),
                "DROP TABLE module_artifact_ui_resources".to_string(),
            ))
            .await?;
        Ok(())
    }
}

/// Projects UI resources for installations admitted before this
/// migration. A stored descriptor that no longer parses fails the
/// upgrade loudly: silently skipping it would exempt that installation
/// from collision checks forever.
async fn backfill_ui_resources(
    db: &SchemaManagerConnection<'_>,
    backend: SupportedBackend,
) -> Result<(), DbErr> {
    let rows = db
        .query_all_raw(Statement::from_string(
            backend.db_backend(),
            "SELECT CAST(installation_id AS TEXT) AS installation_id, scope_kind, \
             CAST(tenant_id AS TEXT) AS tenant_id, CAST(descriptor AS TEXT) AS descriptor \
             FROM module_artifact_installations"
                .to_string(),
        ))
        .await?;
    for row in rows {
        let installation_id: String = row.try_get("", "installation_id")?;
        let installation_id = Uuid::parse_str(&installation_id)
            .map_err(|error| DbErr::Migration(format!("stored installation id: {error}")))?;
        let scope_kind: String = row.try_get("", "scope_kind")?;
        let tenant_id: Option<String> = row.try_get("", "tenant_id")?;
        let tenant_id = match tenant_id {
            Some(raw) => Some(
                Uuid::parse_str(&raw)
                    .map_err(|error| DbErr::Migration(format!("stored tenant id: {error}")))?,
            ),
            None => None,
        };
        let descriptor_json: String = row.try_get("", "descriptor")?;
        let descriptor: Value = serde_json::from_str(&descriptor_json).map_err(|error| {
            DbErr::Migration(format!(
                "stored descriptor for installation {installation_id} does not parse: {error}"
            ))
        })?;
        for (resource_kind, resource_key) in projected_ui_resources(&descriptor) {
            db.execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "INSERT INTO module_artifact_ui_resources \
                     (installation_id, resource_kind, resource_key, scope_kind, tenant_id) \
                     VALUES ({}, {}, {}, {}, {})",
                    backend.placeholder(1),
                    backend.placeholder(2),
                    backend.placeholder(3),
                    backend.placeholder(4),
                    backend.placeholder(5),
                ),
                vec![
                    backend.uuid_value(installation_id),
                    resource_kind.into(),
                    resource_key.into(),
                    scope_kind.clone().into(),
                    backend.optional_uuid_value(tenant_id),
                ],
            ))
            .await?;
        }
    }
    Ok(())
}

/// Mirrors `artifact_ui_resources` over the raw descriptor JSON: only
/// navigation routes and storefront slots participate in collision
/// checks. Unknown or malformed contribution shapes are ignored here
/// exactly as the typed extractor ignores non-colliding content kinds;
/// descriptor *validity* is enforced at admission, not by this
/// projection.
fn projected_ui_resources(descriptor: &Value) -> Vec<(&'static str, String)> {
    let mut resources = Vec::new();
    let Some(contributions) = descriptor.get("ui_contributions").and_then(Value::as_array) else {
        return resources;
    };
    for contribution in contributions {
        let Some(content) = contribution.get("content") else {
            continue;
        };
        match content.get("kind").and_then(Value::as_str) {
            Some("navigation") => {
                if let Some(route) = content.get("route").and_then(Value::as_str) {
                    resources.push(("navigation_route", route.to_string()));
                }
            }
            Some("storefront_slot") => {
                if let Some(slot) = content.get("slot").and_then(Value::as_str) {
                    resources.push(("storefront_slot", slot.to_string()));
                }
            }
            _ => {}
        }
    }
    resources.sort();
    resources.dedup();
    resources
}
