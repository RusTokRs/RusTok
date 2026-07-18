use chrono::Utc;
use rustok_core::generate_id;
use sea_orm::{ConnectionTrait, DbBackend, Statement, TryGetable};
use sea_orm_migration::prelude::*;
use serde_json::json;
use uuid::Uuid;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        let backend = manager.get_database_backend();

        match backend {
            DbBackend::Postgres => {
                for statement in [
                    "ALTER TABLE marketplace_listing_events ALTER COLUMN actor_id DROP NOT NULL",
                    "ALTER TABLE marketplace_listing_events ALTER COLUMN locale DROP NOT NULL",
                    "ALTER TABLE marketplace_listing_events ADD COLUMN provenance VARCHAR(32) NOT NULL DEFAULT 'command'",
                    "ALTER TABLE marketplace_listing_events ADD CONSTRAINT ck_marketplace_listing_events_attribution CHECK ((provenance = 'command' AND actor_id IS NOT NULL AND locale IS NOT NULL) OR (provenance = 'legacy_snapshot' AND actor_id IS NULL AND locale IS NULL))",
                ] {
                    connection.execute_unprepared(statement).await?;
                }
            }
            DbBackend::Sqlite => {
                for statement in [
                    "CREATE TABLE marketplace_listing_events_v2 (id TEXT NOT NULL PRIMARY KEY, tenant_id TEXT NOT NULL, listing_id TEXT NOT NULL, actor_id TEXT NULL, event_kind VARCHAR(48) NOT NULL, locale VARCHAR(32) NULL, provenance VARCHAR(32) NOT NULL DEFAULT 'command', note TEXT NULL, metadata JSON NOT NULL DEFAULT '{}', created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, CONSTRAINT fk_marketplace_listing_events_tenant_listing FOREIGN KEY (tenant_id, listing_id) REFERENCES marketplace_listings (tenant_id, id) ON UPDATE CASCADE ON DELETE CASCADE, CONSTRAINT ck_marketplace_listing_events_attribution CHECK ((provenance = 'command' AND actor_id IS NOT NULL AND locale IS NOT NULL) OR (provenance = 'legacy_snapshot' AND actor_id IS NULL AND locale IS NULL)))",
                    "INSERT INTO marketplace_listing_events_v2 (id, tenant_id, listing_id, actor_id, event_kind, locale, provenance, note, metadata, created_at) SELECT id, tenant_id, listing_id, actor_id, event_kind, locale, 'command', note, metadata, created_at FROM marketplace_listing_events",
                    "DROP TABLE marketplace_listing_events",
                    "ALTER TABLE marketplace_listing_events_v2 RENAME TO marketplace_listing_events",
                    "CREATE INDEX idx_marketplace_listing_events_timeline ON marketplace_listing_events (tenant_id, listing_id, created_at)",
                    "CREATE INDEX idx_marketplace_listing_events_kind ON marketplace_listing_events (tenant_id, event_kind, created_at)",
                    "CREATE INDEX idx_marketplace_listing_events_actor ON marketplace_listing_events (tenant_id, actor_id, created_at)",
                ] {
                    connection.execute_unprepared(statement).await?;
                }
            }
            other => {
                return Err(DbErr::Migration(format!(
                    "marketplace listing event provenance migration does not support {other:?}"
                )))
            }
        }

        let rows = connection
            .query_all(Statement::from_string(
                backend,
                "SELECT id, tenant_id, approval_status, status, approval_note, suspension_reason FROM marketplace_listings WHERE approval_note IS NOT NULL OR suspension_reason IS NOT NULL ORDER BY tenant_id, id".to_string(),
            ))
            .await?;
        let imported_at = Utc::now().fixed_offset();

        for row in rows {
            let listing_id = Uuid::try_get(&row, "", "id")?;
            let tenant_id = Uuid::try_get(&row, "", "tenant_id")?;
            let approval_status = String::try_get(&row, "", "approval_status")?;
            let status = String::try_get(&row, "", "status")?;
            let approval_note = Option::<String>::try_get(&row, "", "approval_note")?;
            let suspension_reason = Option::<String>::try_get(&row, "", "suspension_reason")?;

            if let Some(note) = approval_note.filter(|value| !value.trim().is_empty()) {
                insert_legacy_snapshot(
                    connection,
                    backend,
                    tenant_id,
                    listing_id,
                    "legacy_approval_snapshot",
                    note,
                    json!({
                        "source_column": "approval_note",
                        "observed_approval_status": approval_status,
                        "original_actor_known": false,
                        "original_locale_known": false
                    }),
                    imported_at,
                )
                .await?;
            }
            if let Some(note) = suspension_reason.filter(|value| !value.trim().is_empty()) {
                insert_legacy_snapshot(
                    connection,
                    backend,
                    tenant_id,
                    listing_id,
                    "legacy_suspension_snapshot",
                    note,
                    json!({
                        "source_column": "suspension_reason",
                        "observed_listing_status": status,
                        "original_actor_known": false,
                        "original_locale_known": false
                    }),
                    imported_at,
                )
                .await?;
            }
        }

        connection
            .execute_unprepared("ALTER TABLE marketplace_listings DROP COLUMN approval_note")
            .await?;
        connection
            .execute_unprepared("ALTER TABLE marketplace_listings DROP COLUMN suspension_reason")
            .await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration(
            "marketplace listing provenance cutover is intentionally irreversible: mutable moderation snapshots were normalized into immutable legacy_snapshot events"
                .to_string(),
        ))
    }
}

#[allow(clippy::too_many_arguments)]
async fn insert_legacy_snapshot<C: ConnectionTrait>(
    connection: &C,
    backend: DbBackend,
    tenant_id: Uuid,
    listing_id: Uuid,
    event_kind: &str,
    note: String,
    metadata: serde_json::Value,
    imported_at: chrono::DateTime<chrono::FixedOffset>,
) -> Result<(), DbErr> {
    let values = [
        generate_id().into(),
        tenant_id.into(),
        listing_id.into(),
        event_kind.into(),
        note.into(),
        metadata.to_string().into(),
        imported_at.into(),
    ];
    let statement = match backend {
        DbBackend::Postgres => Statement::from_sql_and_values(
            backend,
            "INSERT INTO marketplace_listing_events (id, tenant_id, listing_id, actor_id, event_kind, locale, provenance, note, metadata, created_at) VALUES ($1, $2, $3, NULL, $4, NULL, 'legacy_snapshot', $5, $6::jsonb, $7)",
            values,
        ),
        DbBackend::Sqlite => Statement::from_sql_and_values(
            backend,
            "INSERT INTO marketplace_listing_events (id, tenant_id, listing_id, actor_id, event_kind, locale, provenance, note, metadata, created_at) VALUES (?, ?, ?, NULL, ?, NULL, 'legacy_snapshot', ?, json(?), ?)",
            values,
        ),
        _ => unreachable!(),
    };
    connection.execute(statement).await?;
    Ok(())
}
