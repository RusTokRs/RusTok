use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(TaxonomyCategoryPresentations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TaxonomyCategoryPresentations::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TaxonomyCategoryPresentations::TermId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TaxonomyCategoryPresentations::IconKey)
                            .string_len(64)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(TaxonomyCategoryPresentations::Color)
                            .string_len(9)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(TaxonomyCategoryPresentations::ImageMediaId)
                            .uuid()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(TaxonomyCategoryPresentations::CoverMediaId)
                            .uuid()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(TaxonomyCategoryPresentations::Revision)
                            .big_integer()
                            .not_null()
                            .default(1),
                    )
                    .col(
                        ColumnDef::new(TaxonomyCategoryPresentations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(TaxonomyCategoryPresentations::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .name("pk_taxonomy_category_presentations")
                            .col(TaxonomyCategoryPresentations::TenantId)
                            .col(TaxonomyCategoryPresentations::TermId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_taxonomy_category_presentations_term")
                            .from_tbl(TaxonomyCategoryPresentations::Table)
                            .from_col(TaxonomyCategoryPresentations::TenantId)
                            .from_col(TaxonomyCategoryPresentations::TermId)
                            .to_tbl(TaxonomyTerms::Table)
                            .to_col(TaxonomyTerms::TenantId)
                            .to_col(TaxonomyTerms::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_taxonomy_category_presentations_image_media")
                    .table(TaxonomyCategoryPresentations::Table)
                    .col(TaxonomyCategoryPresentations::TenantId)
                    .col(TaxonomyCategoryPresentations::ImageMediaId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_taxonomy_category_presentations_cover_media")
                    .table(TaxonomyCategoryPresentations::Table)
                    .col(TaxonomyCategoryPresentations::TenantId)
                    .col(TaxonomyCategoryPresentations::CoverMediaId)
                    .to_owned(),
            )
            .await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres => install_postgres_guard(manager).await?,
            DatabaseBackend::Sqlite => install_sqlite_guards(manager).await?,
            backend => {
                return Err(DbErr::Custom(format!(
                    "taxonomy Category presentation migration does not support {backend:?}",
                )));
            }
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
DROP TRIGGER IF EXISTS taxonomy_category_presentation_guard ON taxonomy_category_presentations;
DROP FUNCTION IF EXISTS taxonomy_validate_category_presentation();
"#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                let connection = manager.get_connection();
                connection
                    .execute_unprepared(
                        "DROP TRIGGER IF EXISTS taxonomy_category_presentation_insert_guard",
                    )
                    .await?;
                connection
                    .execute_unprepared(
                        "DROP TRIGGER IF EXISTS taxonomy_category_presentation_update_guard",
                    )
                    .await?;
            }
            backend => {
                return Err(DbErr::Custom(format!(
                    "taxonomy Category presentation migration does not support {backend:?}",
                )));
            }
        }

        manager
            .drop_table(
                Table::drop()
                    .table(TaxonomyCategoryPresentations::Table)
                    .to_owned(),
            )
            .await
    }
}

async fn install_postgres_guard(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
CREATE OR REPLACE FUNCTION taxonomy_validate_category_presentation()
RETURNS trigger AS $$
BEGIN
    IF NEW.revision < 1 THEN
        RAISE EXCEPTION 'taxonomy Category presentation revision must be positive';
    END IF;

    IF NOT EXISTS (
        SELECT 1
          FROM taxonomy_terms term
         WHERE term.tenant_id = NEW.tenant_id
           AND term.id = NEW.term_id
           AND term.kind = 'category'
    ) THEN
        RAISE EXCEPTION 'taxonomy Category presentation term is missing or is not a Category';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS taxonomy_category_presentation_guard ON taxonomy_category_presentations;
CREATE TRIGGER taxonomy_category_presentation_guard
BEFORE INSERT OR UPDATE OF tenant_id, term_id, revision
ON taxonomy_category_presentations
FOR EACH ROW
EXECUTE FUNCTION taxonomy_validate_category_presentation();
"#,
        )
        .await?;
    Ok(())
}

async fn install_sqlite_guards(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        "DROP TRIGGER IF EXISTS taxonomy_category_presentation_insert_guard".to_string(),
        "DROP TRIGGER IF EXISTS taxonomy_category_presentation_update_guard".to_string(),
        sqlite_guard_sql("INSERT", "taxonomy_category_presentation_insert_guard"),
        sqlite_guard_sql("UPDATE", "taxonomy_category_presentation_update_guard"),
    ] {
        connection.execute_unprepared(&statement).await?;
    }
    Ok(())
}

fn sqlite_guard_sql(operation: &str, trigger_name: &str) -> String {
    format!(
        r#"
CREATE TRIGGER {trigger_name}
BEFORE {operation} ON taxonomy_category_presentations
FOR EACH ROW
BEGIN
    SELECT CASE
        WHEN NEW.revision < 1
        THEN RAISE(ABORT, 'taxonomy Category presentation revision must be positive')
    END;

    SELECT CASE
        WHEN NOT EXISTS (
            SELECT 1
              FROM taxonomy_terms term
             WHERE term.tenant_id = NEW.tenant_id
               AND term.id = NEW.term_id
               AND term.kind = 'category'
        )
        THEN RAISE(ABORT, 'taxonomy Category presentation term is missing or is not a Category')
    END;
END
"#,
    )
}

#[derive(DeriveIden)]
enum TaxonomyCategoryPresentations {
    Table,
    TenantId,
    TermId,
    IconKey,
    Color,
    ImageMediaId,
    CoverMediaId,
    Revision,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum TaxonomyTerms {
    Table,
    TenantId,
    Id,
}
