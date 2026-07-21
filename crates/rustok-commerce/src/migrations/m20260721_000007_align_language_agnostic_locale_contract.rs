use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
ALTER TABLE collection_translations
    ALTER COLUMN locale TYPE VARCHAR(32);
ALTER TABLE product_category_translations
    ALTER COLUMN locale TYPE VARCHAR(32);
"#,
                    )
                    .await?;
            }
            DatabaseBackend::MySql => {
                for statement in [
                    "ALTER TABLE collection_translations MODIFY COLUMN locale VARCHAR(32) NOT NULL",
                    "ALTER TABLE product_category_translations MODIFY COLUMN locale VARCHAR(32) NOT NULL",
                ] {
                    manager
                        .get_connection()
                        .execute_unprepared(statement)
                        .await?;
                }
            }
            DatabaseBackend::Sqlite => rebuild_sqlite_translation_tables(manager).await?,
        }
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Forward-only by design: narrowing locale columns could truncate normalized BCP47-like
        // tags such as `pt-BR`, `zh-Hant`, or future tags up to the platform 32-byte limit.
        Ok(())
    }
}

async fn rebuild_sqlite_translation_tables(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
ALTER TABLE collection_translations RENAME TO collection_translations_locale_v5;
CREATE TABLE collection_translations (
    id TEXT NOT NULL PRIMARY KEY,
    collection_id TEXT NOT NULL,
    locale VARCHAR(32) NOT NULL,
    title VARCHAR(255) NOT NULL,
    handle VARCHAR(255) NOT NULL,
    description TEXT NULL,
    CONSTRAINT fk_collection_translations_collection
        FOREIGN KEY (collection_id) REFERENCES collections(id) ON DELETE CASCADE
);
INSERT INTO collection_translations (id, collection_id, locale, title, handle, description)
SELECT id, collection_id, locale, title, handle, description
FROM collection_translations_locale_v5;
DROP TABLE collection_translations_locale_v5;
CREATE UNIQUE INDEX idx_collection_trans_unique
    ON collection_translations (collection_id, locale);
CREATE INDEX idx_collection_trans_handle
    ON collection_translations (locale, handle);

ALTER TABLE product_category_translations
    RENAME TO product_category_translations_locale_v5;
CREATE TABLE product_category_translations (
    id TEXT NOT NULL PRIMARY KEY,
    category_id TEXT NOT NULL,
    locale VARCHAR(32) NOT NULL,
    name VARCHAR(255) NOT NULL,
    handle VARCHAR(255) NOT NULL,
    description TEXT NULL,
    CONSTRAINT fk_product_category_translations_category
        FOREIGN KEY (category_id) REFERENCES product_categories(id) ON DELETE CASCADE
);
INSERT INTO product_category_translations (id, category_id, locale, name, handle, description)
SELECT id, category_id, locale, name, handle, description
FROM product_category_translations_locale_v5;
DROP TABLE product_category_translations_locale_v5;
CREATE UNIQUE INDEX idx_product_cat_trans_unique
    ON product_category_translations (category_id, locale);
CREATE INDEX idx_product_cat_trans_handle
    ON product_category_translations (locale, handle);
"#,
        )
        .await?;
    Ok(())
}
