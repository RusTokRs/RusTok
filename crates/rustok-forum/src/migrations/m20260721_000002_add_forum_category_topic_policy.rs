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
                    .table(Alias::new("forum_category_policies"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("category_id"))
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("tenant_id"))
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("allows_topics"))
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(Alias::new("updated_at"))
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_forum_category_policy_category")
                            .from(
                                Alias::new("forum_category_policies"),
                                Alias::new("category_id"),
                            )
                            .to(Alias::new("forum_categories"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_forum_category_policies_tenant")
                    .table(Alias::new("forum_category_policies"))
                    .col(Alias::new("tenant_id"))
                    .to_owned(),
            )
            .await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres => up_postgres(manager).await,
            DatabaseBackend::Sqlite => up_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum category topic policy migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => down_postgres(manager).await?,
            DatabaseBackend::Sqlite => down_sqlite(manager).await?,
            backend => {
                return Err(DbErr::Custom(format!(
                    "rustok-forum category topic policy migration does not support {backend:?}"
                )))
            }
        }

        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("forum_category_policies"))
                    .if_exists()
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
CREATE OR REPLACE FUNCTION forum_validate_topic_category_policy()
RETURNS trigger AS $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM forum_category_policies policy
        WHERE policy.tenant_id = NEW.tenant_id
          AND policy.category_id = NEW.category_id
          AND NOT policy.allows_topics
    ) THEN
        RAISE EXCEPTION 'forum category does not allow topic creation';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_topics_category_policy_guard ON forum_topics;
CREATE TRIGGER forum_topics_category_policy_guard
BEFORE INSERT OR UPDATE OF tenant_id, category_id
ON forum_topics
FOR EACH ROW
EXECUTE FUNCTION forum_validate_topic_category_policy();
"#,
        )
        .await?;
    Ok(())
}

async fn down_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
DROP TRIGGER IF EXISTS forum_topics_category_policy_guard ON forum_topics;
DROP FUNCTION IF EXISTS forum_validate_topic_category_policy();
"#,
        )
        .await?;
    Ok(())
}

async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        "DROP TRIGGER IF EXISTS forum_topics_category_policy_insert",
        "DROP TRIGGER IF EXISTS forum_topics_category_policy_update",
        r#"CREATE TRIGGER forum_topics_category_policy_insert
           BEFORE INSERT ON forum_topics
           FOR EACH ROW
           WHEN EXISTS (
               SELECT 1
               FROM forum_category_policies policy
               WHERE policy.tenant_id = NEW.tenant_id
                 AND policy.category_id = NEW.category_id
                 AND policy.allows_topics = 0
           )
           BEGIN
               SELECT RAISE(ABORT, 'forum category does not allow topic creation');
           END"#,
        r#"CREATE TRIGGER forum_topics_category_policy_update
           BEFORE UPDATE OF tenant_id, category_id ON forum_topics
           FOR EACH ROW
           WHEN EXISTS (
               SELECT 1
               FROM forum_category_policies policy
               WHERE policy.tenant_id = NEW.tenant_id
                 AND policy.category_id = NEW.category_id
                 AND policy.allows_topics = 0
           )
           BEGIN
               SELECT RAISE(ABORT, 'forum category does not allow topic creation');
           END"#,
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}

async fn down_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        "DROP TRIGGER IF EXISTS forum_topics_category_policy_insert",
        "DROP TRIGGER IF EXISTS forum_topics_category_policy_update",
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}
