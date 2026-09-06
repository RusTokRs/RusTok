use sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseTransaction, FromQueryResult, Statement,
    TransactionTrait,
};
use sea_orm_migration::prelude::*;

#[derive(Debug, FromQueryResult)]
struct CountRow {
    count: i64,
}

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Ok(());
        }

        if !base_alt_column_exists(manager).await? {
            // This cutover is intentionally irreversible. The migration smoke
            // harness may mark it down and then reapply it, but the retired
            // compatibility column must not be recreated for that cycle.
            return Ok(());
        }

        let txn = manager.get_connection().begin().await?;
        ensure_unowned_alt_fits_canonical_storage(&txn).await?;
        ensure_deterministic_translation_ids_are_available(&txn).await?;
        preserve_unowned_alt_copy(&txn).await?;
        txn.execute_raw(Statement::from_string(
            DatabaseBackend::Postgres,
            "ALTER TABLE product_images DROP COLUMN alt_text".to_owned(),
        ))
        .await?;
        txn.commit().await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Intentionally irreversible. Canonical localized image copy is owned by
        // product_image_translations. Recreating product_images.alt_text would
        // reintroduce a second localized source of truth with unknown provenance.
        Ok(())
    }
}

async fn base_alt_column_exists(manager: &SchemaManager<'_>) -> Result<bool, DbErr> {
    let count = CountRow::find_by_statement(Statement::from_string(
        DatabaseBackend::Postgres,
        r#"
            SELECT COUNT(*)::BIGINT AS count
            FROM information_schema.columns
            WHERE table_schema = current_schema()
              AND table_name = 'product_images'
              AND column_name = 'alt_text'
        "#,
    ))
    .one(manager.get_connection())
    .await?
    .map(|row| row.count)
    .unwrap_or_default();

    Ok(count != 0)
}

async fn ensure_unowned_alt_fits_canonical_storage(
    txn: &DatabaseTransaction,
) -> Result<(), DbErr> {
    let oversized = CountRow::find_by_statement(Statement::from_string(
        DatabaseBackend::Postgres,
        r#"
            SELECT COUNT(*)::BIGINT AS count
            FROM product_images image
            WHERE image.alt_text IS NOT NULL
              AND char_length(image.alt_text) > 255
              AND NOT EXISTS (
                  SELECT 1
                  FROM product_image_translations translation
                  WHERE translation.image_id = image.id
                    AND translation.locale = 'und'
              )
        "#,
    ))
    .one(txn)
    .await?
    .map(|row| row.count)
    .unwrap_or_default();

    if oversized != 0 {
        return Err(DbErr::Migration(format!(
            "Product image alt storage retirement blocked: {oversized} unowned compatibility value(s) exceed the canonical 255-character limit",
        )));
    }

    Ok(())
}

async fn ensure_deterministic_translation_ids_are_available(
    txn: &DatabaseTransaction,
) -> Result<(), DbErr> {
    let collisions = CountRow::find_by_statement(Statement::from_string(
        DatabaseBackend::Postgres,
        r#"
            SELECT COUNT(*)::BIGINT AS count
            FROM product_images image
            JOIN product_image_translations collision
              ON collision.id = md5('product-image-alt-und:' || image.id::text)::uuid
            WHERE image.alt_text IS NOT NULL
              AND NOT EXISTS (
                  SELECT 1
                  FROM product_image_translations existing
                  WHERE existing.image_id = image.id
                    AND existing.locale = 'und'
              )
              AND (
                  collision.image_id IS DISTINCT FROM image.id
                  OR collision.locale IS DISTINCT FROM 'und'
              )
        "#,
    ))
    .one(txn)
    .await?
    .map(|row| row.count)
    .unwrap_or_default();

    if collisions != 0 {
        return Err(DbErr::Migration(format!(
            "Product image alt storage retirement blocked: {collisions} deterministic translation id collision(s) detected",
        )));
    }

    Ok(())
}

async fn preserve_unowned_alt_copy(txn: &DatabaseTransaction) -> Result<(), DbErr> {
    txn.execute_raw(Statement::from_string(
        DatabaseBackend::Postgres,
        r#"
            INSERT INTO product_image_translations (id, image_id, locale, alt_text)
            SELECT
                md5('product-image-alt-und:' || image.id::text)::uuid,
                image.id,
                'und',
                image.alt_text
            FROM product_images image
            WHERE image.alt_text IS NOT NULL
              AND NOT EXISTS (
                  SELECT 1
                  FROM product_image_translations existing
                  WHERE existing.image_id = image.id
                    AND existing.locale = 'und'
              )
            ON CONFLICT (image_id, locale) DO NOTHING
        "#
        .to_owned(),
    ))
    .await?;

    Ok(())
}
