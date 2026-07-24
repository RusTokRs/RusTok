use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("forum_category_policies"))
                    .add_column(
                        ColumnDef::new(Alias::new("visibility_override")).string_len(32),
                    )
                    .to_owned(),
            )
            .await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres => up_postgres(manager).await,
            DatabaseBackend::Sqlite => up_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum category visibility migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => down_postgres(manager).await?,
            DatabaseBackend::Sqlite => down_sqlite(manager).await?,
            backend => {
                return Err(DbErr::Custom(format!(
                    "rustok-forum category visibility migration does not support {backend:?}"
                )));
            }
        }

        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("forum_category_policies"))
                    .drop_column(Alias::new("visibility_override"))
                    .to_owned(),
            )
            .await
    }
}

async fn up_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
ALTER TABLE forum_category_policies
    DROP CONSTRAINT IF EXISTS ck_forum_category_visibility_override;
ALTER TABLE forum_category_policies
    ADD CONSTRAINT ck_forum_category_visibility_override
    CHECK (
        visibility_override IS NULL
        OR visibility_override = 'authenticated'
    );
"#,
        )
        .await?;
    Ok(())
}

async fn down_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            "ALTER TABLE forum_category_policies DROP CONSTRAINT IF EXISTS ck_forum_category_visibility_override",
        )
        .await?;
    Ok(())
}

async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        "DROP TRIGGER IF EXISTS forum_category_visibility_override_insert",
        "DROP TRIGGER IF EXISTS forum_category_visibility_override_update",
        r#"CREATE TRIGGER forum_category_visibility_override_insert
           BEFORE INSERT ON forum_category_policies
           FOR EACH ROW
           WHEN NEW.visibility_override IS NOT NULL
             AND NEW.visibility_override <> 'authenticated'
           BEGIN
               SELECT RAISE(ABORT, 'forum category visibility override must narrow to authenticated');
           END"#,
        r#"CREATE TRIGGER forum_category_visibility_override_update
           BEFORE UPDATE OF visibility_override ON forum_category_policies
           FOR EACH ROW
           WHEN NEW.visibility_override IS NOT NULL
             AND NEW.visibility_override <> 'authenticated'
           BEGIN
               SELECT RAISE(ABORT, 'forum category visibility override must narrow to authenticated');
           END"#,
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}

async fn down_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        "DROP TRIGGER IF EXISTS forum_category_visibility_override_update",
        "DROP TRIGGER IF EXISTS forum_category_visibility_override_insert",
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}
