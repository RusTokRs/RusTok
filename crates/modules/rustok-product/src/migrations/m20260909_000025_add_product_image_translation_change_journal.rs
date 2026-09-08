use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Err(DbErr::Custom(
                "rustok-product migrations require PostgreSQL".to_owned(),
            ));
        }

        manager
            .get_connection()
            .execute_unprepared(
                r#"
CREATE TABLE product_image_translation_change_journal (
    change_seq BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    root_event_id UUID NOT NULL,
    tenant_id UUID NOT NULL,
    product_id UUID NOT NULL,
    image_id UUID NOT NULL,
    resource_revision VARCHAR(96) NOT NULL,
    lifecycle VARCHAR(16) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uq_product_image_translation_change_root_target
        UNIQUE (root_event_id, image_id),
    CONSTRAINT chk_product_image_translation_change_seq_positive
        CHECK (change_seq > 0),
    CONSTRAINT chk_product_image_translation_change_root_event_non_nil
        CHECK (root_event_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_product_image_translation_change_tenant_non_nil
        CHECK (tenant_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_product_image_translation_change_product_non_nil
        CHECK (product_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_product_image_translation_change_image_non_nil
        CHECK (image_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_product_image_translation_change_revision_nonblank
        CHECK (length(trim(resource_revision)) > 0),
    CONSTRAINT chk_product_image_translation_change_lifecycle
        CHECK (lifecycle IN ('active', 'archived', 'deleted'))
);

CREATE INDEX idx_product_image_translation_change_tenant_seq
    ON product_image_translation_change_journal (tenant_id, change_seq);

CREATE INDEX idx_product_image_translation_change_product_target_seq
    ON product_image_translation_change_journal (
        tenant_id,
        product_id,
        image_id,
        change_seq DESC
    );

CREATE INDEX idx_product_image_translation_change_target_seq
    ON product_image_translation_change_journal (
        tenant_id,
        image_id,
        change_seq DESC
    );
"#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Err(DbErr::Custom(
                "rustok-product migrations require PostgreSQL".to_owned(),
            ));
        }

        manager
            .get_connection()
            .execute_unprepared(
                "DROP TABLE IF EXISTS product_image_translation_change_journal;",
            )
            .await?;
        Ok(())
    }
}
