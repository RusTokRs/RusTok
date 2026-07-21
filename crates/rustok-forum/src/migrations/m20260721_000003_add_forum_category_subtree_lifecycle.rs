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
                    .table(Alias::new("forum_category_lifecycle"))
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
                        ColumnDef::new(Alias::new("archived_at"))
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("updated_at"))
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_forum_category_lifecycle_category")
                            .from(
                                Alias::new("forum_category_lifecycle"),
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
                    .name("idx_forum_category_lifecycle_tenant")
                    .table(Alias::new("forum_category_lifecycle"))
                    .col(Alias::new("tenant_id"))
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
            .drop_table(
                Table::drop()
                    .table(Alias::new("forum_category_lifecycle"))
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
CREATE OR REPLACE FUNCTION forum_validate_category_lifecycle_write()
RETURNS trigger AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM forum_categories category
        WHERE category.id = NEW.category_id
          AND category.tenant_id = NEW.tenant_id
    ) THEN
        RAISE EXCEPTION 'forum category lifecycle tenant mismatch';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM forum_categories child
        WHERE child.parent_id = NEW.category_id
          AND child.tenant_id = NEW.tenant_id
          AND NOT EXISTS (
              SELECT 1
              FROM forum_category_lifecycle child_lifecycle
              WHERE child_lifecycle.category_id = child.id
                AND child_lifecycle.tenant_id = NEW.tenant_id
          )
    ) THEN
        RAISE EXCEPTION 'archived forum category cannot have active child';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_validate_category_lifecycle_delete()
RETURNS trigger AS $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM forum_categories category
        JOIN forum_category_lifecycle parent_lifecycle
          ON parent_lifecycle.category_id = category.parent_id
         AND parent_lifecycle.tenant_id = OLD.tenant_id
        WHERE category.id = OLD.category_id
          AND category.tenant_id = OLD.tenant_id
    ) THEN
        RAISE EXCEPTION 'active forum category cannot have archived parent';
    END IF;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_category_lifecycle_write_guard ON forum_category_lifecycle;
CREATE TRIGGER forum_category_lifecycle_write_guard
BEFORE INSERT OR UPDATE OF category_id, tenant_id
ON forum_category_lifecycle
FOR EACH ROW
EXECUTE FUNCTION forum_validate_category_lifecycle_write();

DROP TRIGGER IF EXISTS forum_category_lifecycle_delete_guard ON forum_category_lifecycle;
CREATE TRIGGER forum_category_lifecycle_delete_guard
BEFORE DELETE ON forum_category_lifecycle
FOR EACH ROW
EXECUTE FUNCTION forum_validate_category_lifecycle_delete();

CREATE OR REPLACE FUNCTION forum_validate_category_parent_lifecycle()
RETURNS trigger AS $$
BEGIN
    IF NEW.parent_id IS NOT NULL
       AND NOT EXISTS (
           SELECT 1
           FROM forum_category_lifecycle own_lifecycle
           WHERE own_lifecycle.category_id = NEW.id
             AND own_lifecycle.tenant_id = NEW.tenant_id
       )
       AND EXISTS (
           SELECT 1
           FROM forum_category_lifecycle parent_lifecycle
           WHERE parent_lifecycle.category_id = NEW.parent_id
             AND parent_lifecycle.tenant_id = NEW.tenant_id
       ) THEN
        RAISE EXCEPTION 'active forum category cannot have archived parent';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_categories_parent_lifecycle_guard ON forum_categories;
CREATE TRIGGER forum_categories_parent_lifecycle_guard
BEFORE INSERT OR UPDATE OF tenant_id, parent_id
ON forum_categories
FOR EACH ROW
EXECUTE FUNCTION forum_validate_category_parent_lifecycle();

CREATE OR REPLACE FUNCTION forum_validate_topic_category_policy()
RETURNS trigger AS $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM forum_category_lifecycle lifecycle
        WHERE lifecycle.category_id = NEW.category_id
          AND lifecycle.tenant_id = NEW.tenant_id
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
DROP TRIGGER IF EXISTS forum_categories_parent_lifecycle_guard ON forum_categories;
DROP FUNCTION IF EXISTS forum_validate_category_parent_lifecycle();
DROP TRIGGER IF EXISTS forum_category_lifecycle_delete_guard ON forum_category_lifecycle;
DROP FUNCTION IF EXISTS forum_validate_category_lifecycle_delete();
DROP TRIGGER IF EXISTS forum_category_lifecycle_write_guard ON forum_category_lifecycle;
DROP FUNCTION IF EXISTS forum_validate_category_lifecycle_write();

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
        "DROP TRIGGER IF EXISTS forum_category_lifecycle_write_insert",
        "DROP TRIGGER IF EXISTS forum_category_lifecycle_write_update",
        "DROP TRIGGER IF EXISTS forum_category_lifecycle_delete",
        "DROP TRIGGER IF EXISTS forum_categories_parent_lifecycle_insert",
        "DROP TRIGGER IF EXISTS forum_categories_parent_lifecycle_update",
        "DROP TRIGGER IF EXISTS forum_topics_category_policy_insert",
        "DROP TRIGGER IF EXISTS forum_topics_category_policy_update",
        r#"CREATE TRIGGER forum_category_lifecycle_write_insert
           BEFORE INSERT ON forum_category_lifecycle
           FOR EACH ROW
           WHEN NOT EXISTS (
               SELECT 1
               FROM forum_categories category
               WHERE category.id = NEW.category_id
                 AND category.tenant_id = NEW.tenant_id
           ) OR EXISTS (
               SELECT 1
               FROM forum_categories child
               WHERE child.parent_id = NEW.category_id
                 AND child.tenant_id = NEW.tenant_id
                 AND NOT EXISTS (
                     SELECT 1
                     FROM forum_category_lifecycle child_lifecycle
                     WHERE child_lifecycle.category_id = child.id
                       AND child_lifecycle.tenant_id = NEW.tenant_id
                 )
           )
           BEGIN
               SELECT RAISE(ABORT, 'forum category lifecycle write violation');
           END"#,
        r#"CREATE TRIGGER forum_category_lifecycle_write_update
           BEFORE UPDATE OF category_id, tenant_id ON forum_category_lifecycle
           FOR EACH ROW
           WHEN NOT EXISTS (
               SELECT 1
               FROM forum_categories category
               WHERE category.id = NEW.category_id
                 AND category.tenant_id = NEW.tenant_id
           ) OR EXISTS (
               SELECT 1
               FROM forum_categories child
               WHERE child.parent_id = NEW.category_id
                 AND child.tenant_id = NEW.tenant_id
                 AND NOT EXISTS (
                     SELECT 1
                     FROM forum_category_lifecycle child_lifecycle
                     WHERE child_lifecycle.category_id = child.id
                       AND child_lifecycle.tenant_id = NEW.tenant_id
                 )
           )
           BEGIN
               SELECT RAISE(ABORT, 'forum category lifecycle write violation');
           END"#,
        r#"CREATE TRIGGER forum_category_lifecycle_delete
           BEFORE DELETE ON forum_category_lifecycle
           FOR EACH ROW
           WHEN EXISTS (
               SELECT 1
               FROM forum_categories category
               JOIN forum_category_lifecycle parent_lifecycle
                 ON parent_lifecycle.category_id = category.parent_id
                AND parent_lifecycle.tenant_id = OLD.tenant_id
               WHERE category.id = OLD.category_id
                 AND category.tenant_id = OLD.tenant_id
           )
           BEGIN
               SELECT RAISE(ABORT, 'active forum category cannot have archived parent');
           END"#,
        r#"CREATE TRIGGER forum_categories_parent_lifecycle_insert
           BEFORE INSERT ON forum_categories
           FOR EACH ROW
           WHEN NEW.parent_id IS NOT NULL
            AND EXISTS (
                SELECT 1
                FROM forum_category_lifecycle parent_lifecycle
                WHERE parent_lifecycle.category_id = NEW.parent_id
                  AND parent_lifecycle.tenant_id = NEW.tenant_id
            )
           BEGIN
               SELECT RAISE(ABORT, 'active forum category cannot have archived parent');
           END"#,
        r#"CREATE TRIGGER forum_categories_parent_lifecycle_update
           BEFORE UPDATE OF tenant_id, parent_id ON forum_categories
           FOR EACH ROW
           WHEN NEW.parent_id IS NOT NULL
            AND NOT EXISTS (
                SELECT 1
                FROM forum_category_lifecycle own_lifecycle
                WHERE own_lifecycle.category_id = NEW.id
                  AND own_lifecycle.tenant_id = NEW.tenant_id
            )
            AND EXISTS (
                SELECT 1
                FROM forum_category_lifecycle parent_lifecycle
                WHERE parent_lifecycle.category_id = NEW.parent_id
                  AND parent_lifecycle.tenant_id = NEW.tenant_id
            )
           BEGIN
               SELECT RAISE(ABORT, 'active forum category cannot have archived parent');
           END"#,
        r#"CREATE TRIGGER forum_topics_category_policy_insert
           BEFORE INSERT ON forum_topics
           FOR EACH ROW
           WHEN EXISTS (
               SELECT 1
               FROM forum_category_lifecycle lifecycle
               WHERE lifecycle.category_id = NEW.category_id
                 AND lifecycle.tenant_id = NEW.tenant_id
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
               FROM forum_category_lifecycle lifecycle
               WHERE lifecycle.category_id = NEW.category_id
                 AND lifecycle.tenant_id = NEW.tenant_id
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
        "DROP TRIGGER IF EXISTS forum_category_lifecycle_write_insert",
        "DROP TRIGGER IF EXISTS forum_category_lifecycle_write_update",
        "DROP TRIGGER IF EXISTS forum_category_lifecycle_delete",
        "DROP TRIGGER IF EXISTS forum_categories_parent_lifecycle_insert",
        "DROP TRIGGER IF EXISTS forum_categories_parent_lifecycle_update",
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
