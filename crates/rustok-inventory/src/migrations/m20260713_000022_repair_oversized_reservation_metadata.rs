use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

const MAX_RESERVATION_METADATA_BYTES: usize = 32 * 1024;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => repair_postgres(manager).await?,
            DatabaseBackend::Sqlite => repair_sqlite(manager).await?,
            _ => {}
        }
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Oversized arbitrary metadata cannot be reconstructed after compaction.
        Ok(())
    }
}

async fn repair_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(&format!(
            r#"
            UPDATE reservation_items
            SET metadata = jsonb_strip_nulls(jsonb_build_object(
                    'metadata_truncated', true,
                    'source', metadata -> 'source',
                    'variant_id', metadata -> 'variant_id',
                    'order_id', metadata -> 'order_id',
                    'order_line_item_id', metadata -> 'order_line_item_id',
                    'cart_line_item_id', metadata -> 'cart_line_item_id',
                    'inventory_disposition', metadata -> 'inventory_disposition',
                    'superseded_by', metadata -> 'superseded_by',
                    'reservation_id', id,
                    'external_id', external_id
                )),
                updated_at = CURRENT_TIMESTAMP
            WHERE octet_length(metadata::text) > {MAX_RESERVATION_METADATA_BYTES};

            ALTER TABLE reservation_items
                VALIDATE CONSTRAINT ck_reservation_items_metadata_size;
            "#,
        ))
        .await?;
    Ok(())
}

async fn repair_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(&format!(
            r#"
            UPDATE reservation_items
            SET metadata = json_object(
                    'metadata_truncated', json('true'),
                    'legacy_metadata_valid', json(CASE WHEN json_valid(metadata) THEN 'true' ELSE 'false' END),
                    'source', CASE WHEN json_valid(metadata) THEN json_extract(metadata, '$.source') END,
                    'variant_id', CASE WHEN json_valid(metadata) THEN json_extract(metadata, '$.variant_id') END,
                    'order_id', CASE WHEN json_valid(metadata) THEN json_extract(metadata, '$.order_id') END,
                    'order_line_item_id', CASE WHEN json_valid(metadata) THEN json_extract(metadata, '$.order_line_item_id') END,
                    'cart_line_item_id', CASE WHEN json_valid(metadata) THEN json_extract(metadata, '$.cart_line_item_id') END,
                    'inventory_disposition', CASE WHEN json_valid(metadata) THEN json_extract(metadata, '$.inventory_disposition') END,
                    'superseded_by', CASE WHEN json_valid(metadata) THEN json_extract(metadata, '$.superseded_by') END,
                    'reservation_id', id,
                    'external_id', external_id
                ),
                updated_at = CURRENT_TIMESTAMP
            WHERE length(CAST(metadata AS BLOB)) > {MAX_RESERVATION_METADATA_BYTES};
            "#,
        ))
        .await?;
    Ok(())
}
