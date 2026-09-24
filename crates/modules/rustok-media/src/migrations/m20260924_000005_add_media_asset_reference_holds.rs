use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ReferenceHolds::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ReferenceHolds::ReferenceId)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ReferenceHolds::TenantId).uuid().not_null())
                    .col(ColumnDef::new(ReferenceHolds::MediaId).uuid().not_null())
                    .col(
                        ColumnDef::new(ReferenceHolds::OwnerModule)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ReferenceHolds::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_media_asset_reference_holds_tenant")
                            .from(ReferenceHolds::Table, ReferenceHolds::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .to_owned(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_media_asset_reference_holds_asset")
                            .from_tbl(ReferenceHolds::Table)
                            .from_col(ReferenceHolds::TenantId)
                            .from_col(ReferenceHolds::MediaId)
                            .to_tbl(Assets::Table)
                            .to_col(Assets::TenantId)
                            .to_col(Assets::Id)
                            .on_delete(ForeignKeyAction::Restrict)
                            .to_owned(),
                    )
                    .to_owned(),
            )
            .await?;

        for index in [
            Index::create()
                .name("idx_media_asset_reference_holds_tenant_media")
                .table(ReferenceHolds::Table)
                .col(ReferenceHolds::TenantId)
                .col(ReferenceHolds::MediaId)
                .to_owned(),
            Index::create()
                .name("idx_media_asset_reference_holds_owner")
                .table(ReferenceHolds::Table)
                .col(ReferenceHolds::TenantId)
                .col(ReferenceHolds::OwnerModule)
                .col(ReferenceHolds::ReferenceId)
                .to_owned(),
        ] {
            manager.create_index(index).await?;
        }

        let connection = manager.get_connection();
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                connection
                    .execute_unprepared("DROP TRIGGER IF EXISTS media_asset_reference_admission_guard ON media_asset_reference_holds")
                    .await?;
                connection
                    .execute_unprepared(
                        r#"
CREATE OR REPLACE FUNCTION media_asset_reference_admission_guard_fn()
RETURNS trigger AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM media_assets asset
        JOIN media_blobs blob
          ON blob.id = asset.active_blob_id
         AND blob.tenant_id = asset.tenant_id
         AND blob.asset_id = asset.id
        WHERE asset.tenant_id = NEW.tenant_id
          AND asset.id = NEW.media_id
          AND asset.lifecycle_state = 'active'
          AND blob.state = 'ready'
    ) THEN
        RAISE EXCEPTION 'media asset reference not admissible';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
CREATE TRIGGER media_asset_reference_admission_guard
BEFORE INSERT ON media_asset_reference_holds
FOR EACH ROW
EXECUTE FUNCTION media_asset_reference_admission_guard_fn();
"#,
                    )
                    .await?;

                connection
                    .execute_unprepared("DROP TRIGGER IF EXISTS media_asset_reference_delete_guard ON media_assets")
                    .await?;
                connection
                    .execute_unprepared(
                        r#"
CREATE OR REPLACE FUNCTION media_asset_reference_delete_guard_fn()
RETURNS trigger AS $$
BEGIN
    IF NEW.lifecycle_state IN ('delete_pending', 'deleted')
       AND NEW.lifecycle_state <> OLD.lifecycle_state
       AND EXISTS (
           SELECT 1
           FROM media_asset_reference_holds hold
           WHERE hold.tenant_id = OLD.tenant_id
             AND hold.media_id = OLD.id
       ) THEN
        RAISE EXCEPTION 'media asset has retained references';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
CREATE TRIGGER media_asset_reference_delete_guard
BEFORE UPDATE OF lifecycle_state ON media_assets
FOR EACH ROW
EXECUTE FUNCTION media_asset_reference_delete_guard_fn();
"#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                connection
                    .execute_unprepared("DROP TRIGGER IF EXISTS media_asset_reference_admission_guard")
                    .await?;
                connection
                    .execute_unprepared(
                        r#"
CREATE TRIGGER media_asset_reference_admission_guard
BEFORE INSERT ON media_asset_reference_holds
FOR EACH ROW
WHEN NOT EXISTS (
    SELECT 1
    FROM media_assets asset
    JOIN media_blobs blob
      ON blob.id = asset.active_blob_id
     AND blob.tenant_id = asset.tenant_id
     AND blob.asset_id = asset.id
    WHERE asset.tenant_id = NEW.tenant_id
      AND asset.id = NEW.media_id
      AND asset.lifecycle_state = 'active'
      AND blob.state = 'ready'
)
BEGIN
    SELECT RAISE(ABORT, 'media asset reference not admissible');
END
"#,
                    )
                    .await?;

                connection
                    .execute_unprepared("DROP TRIGGER IF EXISTS media_asset_reference_delete_guard")
                    .await?;
                connection
                    .execute_unprepared(
                        r#"
CREATE TRIGGER media_asset_reference_delete_guard
BEFORE UPDATE OF lifecycle_state ON media_assets
FOR EACH ROW
WHEN NEW.lifecycle_state IN ('delete_pending', 'deleted')
 AND NEW.lifecycle_state <> OLD.lifecycle_state
 AND EXISTS (
    SELECT 1
    FROM media_asset_reference_holds hold
    WHERE hold.tenant_id = OLD.tenant_id
      AND hold.media_id = OLD.id
)
BEGIN
    SELECT RAISE(ABORT, 'media asset has retained references');
END
"#,
                    )
                    .await?;
            }
            backend => {
                return Err(DbErr::Custom(format!(
                    "rustok-media asset reference migration does not support {backend:?}"
                )));
            }
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                connection
                    .execute_unprepared("DROP TRIGGER IF EXISTS media_asset_reference_delete_guard ON media_assets")
                    .await?;
                connection
                    .execute_unprepared("DROP FUNCTION IF EXISTS media_asset_reference_delete_guard_fn()")
                    .await?;
                connection
                    .execute_unprepared("DROP TRIGGER IF EXISTS media_asset_reference_admission_guard ON media_asset_reference_holds")
                    .await?;
                connection
                    .execute_unprepared("DROP FUNCTION IF EXISTS media_asset_reference_admission_guard_fn()")
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                connection
                    .execute_unprepared("DROP TRIGGER IF EXISTS media_asset_reference_delete_guard")
                    .await?;
                connection
                    .execute_unprepared("DROP TRIGGER IF EXISTS media_asset_reference_admission_guard")
                    .await?;
            }
            backend => {
                return Err(DbErr::Custom(format!(
                    "rustok-media asset reference rollback does not support {backend:?}"
                )));
            }
        }
        manager
            .drop_table(Table::drop().table(ReferenceHolds::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum ReferenceHolds {
    #[iden = "media_asset_reference_holds"]
    Table,
    ReferenceId,
    TenantId,
    MediaId,
    OwnerModule,
    CreatedAt,
}

#[derive(Iden)]
enum Assets {
    #[iden = "media_assets"]
    Table,
    Id,
    TenantId,
}

#[derive(Iden)]
enum Tenants {
    #[iden = "tenants"]
    Table,
    Id,
}
