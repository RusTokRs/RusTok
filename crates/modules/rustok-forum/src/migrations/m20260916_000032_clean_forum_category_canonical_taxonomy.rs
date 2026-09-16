use sea_orm::{DatabaseBackend, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let backend = manager.get_database_backend();

        // 1. Drop table forum_category_taxonomy_bindings
        manager
            .drop_table(
                Table::drop()
                    .table(ForumCategoryTaxonomyBindings::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;

        // 2. Drop parent FK and columns (parent_id, position, icon, color) from forum_categories
        match backend {
            DatabaseBackend::Postgres => {
                db.execute(Statement::from_string(
                    backend,
                    "ALTER TABLE forum_categories DROP CONSTRAINT IF EXISTS fk_forum_categories_parent_tenant;".to_string(),
                ))
                .await?;
                db.execute(Statement::from_string(
                    backend,
                    "ALTER TABLE forum_categories DROP CONSTRAINT IF EXISTS fk_forum_categories_parent;".to_string(),
                ))
                .await?;
                db.execute(Statement::from_string(
                    backend,
                    "ALTER TABLE forum_categories DROP COLUMN IF EXISTS parent_id, DROP COLUMN IF EXISTS position, DROP COLUMN IF EXISTS icon, DROP COLUMN IF EXISTS color;".to_string(),
                ))
                .await?;
            }
            DatabaseBackend::Sqlite => {
                // SQLite drops columns using alter table drop column
                let _ = db
                    .execute(Statement::from_string(
                        backend,
                        "ALTER TABLE forum_categories DROP COLUMN parent_id;".to_string(),
                    ))
                    .await;
                let _ = db
                    .execute(Statement::from_string(
                        backend,
                        "ALTER TABLE forum_categories DROP COLUMN position;".to_string(),
                    ))
                    .await;
                let _ = db
                    .execute(Statement::from_string(
                        backend,
                        "ALTER TABLE forum_categories DROP COLUMN icon;".to_string(),
                    ))
                    .await;
                let _ = db
                    .execute(Statement::from_string(
                        backend,
                        "ALTER TABLE forum_categories DROP COLUMN color;".to_string(),
                    ))
                    .await;
            }
            other => {
                return Err(DbErr::Custom(format!(
                    "Clean forum category canonical taxonomy migration does not support {other:?}"
                )));
            }
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Intentionally irreversible as part of target taxonomy architecture cutover.
        Ok(())
    }
}

#[derive(DeriveIden)]
enum ForumCategoryTaxonomyBindings {
    #[iden = "forum_category_taxonomy_bindings"]
    Table,
}
