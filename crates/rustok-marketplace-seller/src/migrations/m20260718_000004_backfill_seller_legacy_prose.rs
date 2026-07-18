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
        if !matches!(backend, DbBackend::Postgres | DbBackend::Sqlite) {
            return Err(DbErr::Migration(format!(
                "marketplace seller legacy prose cutover does not support {backend:?}"
            )));
        }

        let rows = connection
            .query_all(Statement::from_string(
                backend,
                "SELECT id, tenant_id, status, onboarding_status, onboarding_note, suspension_reason FROM marketplace_sellers WHERE onboarding_note IS NOT NULL OR suspension_reason IS NOT NULL ORDER BY tenant_id, id".to_string(),
            ))
            .await?;
        let imported_at = Utc::now().fixed_offset();

        for row in rows {
            let seller_id = Uuid::try_get(&row, "", "id")?;
            let tenant_id = Uuid::try_get(&row, "", "tenant_id")?;
            let status = String::try_get(&row, "", "status")?;
            let onboarding_status = String::try_get(&row, "", "onboarding_status")?;
            let onboarding_note = Option::<String>::try_get(&row, "", "onboarding_note")?;
            let suspension_reason = Option::<String>::try_get(&row, "", "suspension_reason")?;

            if let Some(note) = onboarding_note.filter(|value| !value.trim().is_empty()) {
                insert_legacy_snapshot(
                    connection,
                    backend,
                    tenant_id,
                    seller_id,
                    "legacy_onboarding_snapshot",
                    note,
                    json!({
                        "source_column": "onboarding_note",
                        "observed_onboarding_status": onboarding_status,
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
                    seller_id,
                    "legacy_suspension_snapshot",
                    note,
                    json!({
                        "source_column": "suspension_reason",
                        "observed_seller_status": status,
                        "original_actor_known": false,
                        "original_locale_known": false
                    }),
                    imported_at,
                )
                .await?;
            }
        }

        manager
            .alter_table(
                Table::alter()
                    .table(MarketplaceSellers::Table)
                    .drop_column(MarketplaceSellers::OnboardingNote)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(MarketplaceSellers::Table)
                    .drop_column(MarketplaceSellers::SuspensionReason)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration(
            "marketplace seller legacy prose cutover is intentionally irreversible: mutable onboarding and suspension snapshots were normalized into immutable legacy_snapshot events"
                .to_string(),
        ))
    }
}

#[allow(clippy::too_many_arguments)]
async fn insert_legacy_snapshot<C: ConnectionTrait>(
    connection: &C,
    backend: DbBackend,
    tenant_id: Uuid,
    seller_id: Uuid,
    event_kind: &str,
    note: String,
    metadata: serde_json::Value,
    imported_at: chrono::DateTime<chrono::FixedOffset>,
) -> Result<(), DbErr> {
    let values = [
        generate_id().into(),
        tenant_id.into(),
        seller_id.into(),
        event_kind.into(),
        note.into(),
        metadata.to_string().into(),
        imported_at.into(),
    ];
    let statement = match backend {
        DbBackend::Postgres => Statement::from_sql_and_values(
            backend,
            "INSERT INTO marketplace_seller_events (id, tenant_id, seller_id, actor_id, event_kind, locale, provenance, note, metadata, created_at) VALUES ($1, $2, $3, NULL, $4, NULL, 'legacy_snapshot', $5, $6::jsonb, $7)",
            values,
        ),
        DbBackend::Sqlite => Statement::from_sql_and_values(
            backend,
            "INSERT INTO marketplace_seller_events (id, tenant_id, seller_id, actor_id, event_kind, locale, provenance, note, metadata, created_at) VALUES (?, ?, ?, NULL, ?, NULL, 'legacy_snapshot', ?, json(?), ?)",
            values,
        ),
        _ => unreachable!(),
    };
    connection.execute(statement).await?;
    Ok(())
}

#[derive(Iden)]
enum MarketplaceSellers {
    Table,
    OnboardingNote,
    SuspensionReason,
}
