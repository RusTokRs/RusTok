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
                    .table(Alias::new("forum_categories"))
                    .add_column(
                        ColumnDef::new(Alias::new("archived_at")).timestamp_with_time_zone(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_forum_categories_tenant_archived")
                    .table(Alias::new("forum_categories"))
                    .col(Alias::new("tenant_id"))
                    .col(Alias::new("archived_at"))
                    .to_owned(),
            )
            .await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres => up_postgres(manager).await,
            DatabaseBackend::Sqlite => up_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum category subtree lifecycle migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => down_postgres(manager).await?,
            DatabaseBackend::Sqlite => down_sqlite(manager).await?,
            backend => {
                return Err(DbErr::Custom(format!(
                    "rustok-forum category subtree lifecycle migration does not support {backend:?}"
                )))
            }
        }

        manager
            .drop_index(
                Index::drop()
                    .name("idx_forum_categories_tenant_archived")
                    .table(Alias::new("forum_categories"))
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("forum_categories"))
                    .drop_column(Alias::new("archived_at"))
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
CREATE OR REPLACE FUNCTION forum_validate_category_archive_hierarchy()
RETURNS trigger AS $$
BEGIN
    IF NEW.archived_at IS NULL
       AND NEW.parent_id IS NOT NULL
       AND EXISTS (
           SELECT 1
           FROM forum_categories parent
           WHERE parent.id = NEW.parent_id
             AND parent.tenant_id = NEW.tenant_id
             AND parent.archived_at IS NOT NULL
       ) THEN
        RAISE EXCEPTION 'active forum category cannot have archived parent';
    END IF;

    IF NEW.archived_at IS NOT NULL
       AND EXISTS (
           SELECT 1
           FROM forum_categories child
           WHERE child.parent_id = NEW.id
             AND child.tenant_id = NEW.tenant_id
             AND child.archived_at IS NULL
       ) THEN
        RAISE EXCEPTION 'archived forum category cannot have active child';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_categories_archive_hierarchy_guard ON forum_categories;
CREATE TRIGGER forum_categories_archive_hierarchy_guard
BEFORE INSERT OR UPDATE OF tenant_id, parent_id, archived_at
ON forum_categories
FOR EACH ROW
EXECUTE FUNCTION forum_validate_category_archive_hierarchy();

CREATE OR REPLACE FUNCTION forum_validate_topic_category_policy()
RETURNS trigger AS $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM forum_categories category
        WHERE category.id = NEW.category_id
          AND category.tenant_id = NEW.tenant_id
          AND category.archived_at IS NOT NULL
    ) OR EXISTS (
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
DROP TRIGGER IF EXISTS forum_categories_archive_hierarchy_guard ON forum_categories;
DROP FUNCTION IF EXISTS forum_validate_category_archive_hierarchy();

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
"#,
        )
        .await?;
    Ok(())
}

async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        "DROP TRIGGER IF EXISTS forum_categories_archive_hierarchy_insert",
        "DROP TRIGGER IF EXISTS forum_categories_archive_hierarchy_update",
        "DROP TRIGGER IF EXISTS forum_topics_category_policy_insert",
        "DROP TRIGGER IF EXISTS forum_topics_category_policy_update",
        r#"CREATE TRIGGER forum_categories_archive_hierarchy_insert
           BEFORE INSERT ON forum_categories
           FOR EACH ROW
           WHEN (
               NEW.archived_at IS NULL
               AND NEW.parent_id IS NOT NULL
               AND EXISTS (
                   SELECT 1
                   FROM forum_categories parent
                   WHERE parent.id = NEW.parent_id
                     AND parent.tenant_id = NEW.tenant_id
                     AND parent.archived_at IS NOT NULL
               )
           )
           BEGIN
               SELECT RAISE(ABORT, 'active forum category cannot have archived parent');
           END"#,
        r#"CREATE TRIGGER forum_categories_archive_hierarchy_update
           BEFORE UPDATE OF tenant_id, parent_id, archived_at ON forum_categories
           FOR EACH ROW
           WHEN (
               NEW.archived_at IS NULL
               AND NEW.parent_id IS NOT NULL
               AND EXISTS (
                   SELECT 1
                   FROM forum_categories parent
                   WHERE parent.id = NEW.parent_id
                     AND parent.tenant_id = NEW.tenant_id
                     AND parent.archived_at IS NOT NULL
               )
           ) OR (
               NEW.archived_at IS NOT NULL
               AND EXISTS (
                   SELECT 1
                   FROM forum_categories child
                   WHERE child.parent_id = NEW.id
                     AND child.tenant_id = NEW.tenant_id
                     AND child.archived_at IS NULL
               )
           )
           BEGIN
               SELECT RAISE(ABORT, 'forum category archive hierarchy violation');
           END"#,
        r#"CREATE TRIGGER forum_topics_category_policy_insert
           BEFORE INSERT ON forum_topics
           FOR EACH ROW
           WHEN EXISTS (
               SELECT 1
               FROM forum_categories category
               WHERE category.id = NEW.category_id
                 AND category.tenant_id = NEW.tenant_id
                 AND category.archived_at IS NOT NULL
           ) OR EXISTS (
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
               FROM forum_categories category
               WHERE category.id = NEW.category_id
                 AND category.tenant_id = NEW.tenant_id
                 AND category.archived_at IS NOT NULL
           ) OR EXISTS (
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
        "DROP TRIGGER IF EXISTS forum_categories_archive_hierarchy_insert",
        "DROP TRIGGER IF EXISTS forum_categories_archive_hierarchy_update",
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
