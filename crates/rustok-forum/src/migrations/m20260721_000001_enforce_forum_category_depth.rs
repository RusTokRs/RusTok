use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => up_postgres(manager).await,
            DatabaseBackend::Sqlite => up_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum category depth migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => down_postgres(manager).await,
            DatabaseBackend::Sqlite => down_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum category depth migration does not support {backend:?}"
            ))),
        }
    }
}

async fn up_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
DO $$
BEGIN
    IF EXISTS (
        WITH RECURSIVE category_paths AS (
            SELECT
                tenant_id,
                id AS origin_id,
                parent_id,
                0 AS depth,
                ARRAY[id] AS path,
                FALSE AS cycle
            FROM forum_categories

            UNION ALL

            SELECT
                category_paths.tenant_id,
                category_paths.origin_id,
                parent.parent_id,
                category_paths.depth + 1,
                category_paths.path || parent.id,
                parent.id = ANY(category_paths.path)
            FROM category_paths
            JOIN forum_categories parent
              ON parent.tenant_id = category_paths.tenant_id
             AND parent.id = category_paths.parent_id
            WHERE category_paths.parent_id IS NOT NULL
              AND NOT category_paths.cycle
        )
        SELECT 1
        FROM category_paths
        WHERE depth > 16 OR cycle
    ) THEN
        RAISE EXCEPTION
            'forum category depth migration blocked: existing hierarchy exceeds depth 16 or contains a cycle';
    END IF;
END $$;

CREATE OR REPLACE FUNCTION forum_validate_category_depth()
RETURNS trigger AS $$
DECLARE
    invalid_hierarchy BOOLEAN;
BEGIN
    PERFORM pg_advisory_xact_lock(hashtextextended(NEW.tenant_id::text, 0));

    IF NEW.parent_id IS NULL THEN
        RETURN NEW;
    END IF;

    IF NOT EXISTS (
        SELECT 1
        FROM forum_categories parent
        WHERE parent.tenant_id = NEW.tenant_id
          AND parent.id = NEW.parent_id
    ) THEN
        RAISE EXCEPTION 'forum category parent is missing or belongs to another tenant';
    END IF;

    WITH RECURSIVE ancestors AS (
        SELECT
            parent.id,
            parent.parent_id,
            1 AS depth,
            ARRAY[parent.id] AS path,
            FALSE AS cycle
        FROM forum_categories parent
        WHERE parent.tenant_id = NEW.tenant_id
          AND parent.id = NEW.parent_id

        UNION ALL

        SELECT
            parent.id,
            parent.parent_id,
            ancestors.depth + 1,
            ancestors.path || parent.id,
            parent.id = ANY(ancestors.path)
        FROM ancestors
        JOIN forum_categories parent
          ON parent.tenant_id = NEW.tenant_id
         AND parent.id = ancestors.parent_id
        WHERE ancestors.parent_id IS NOT NULL
          AND NOT ancestors.cycle
    )
    SELECT EXISTS (
        SELECT 1
        FROM ancestors
        WHERE depth > 16 OR cycle
    )
    INTO invalid_hierarchy;

    IF invalid_hierarchy THEN
        RAISE EXCEPTION 'forum category hierarchy exceeds maximum depth 16';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_categories_depth_guard ON forum_categories;
CREATE TRIGGER forum_categories_depth_guard
BEFORE INSERT OR UPDATE OF tenant_id, parent_id
ON forum_categories
FOR EACH ROW
EXECUTE FUNCTION forum_validate_category_depth();
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
DROP TRIGGER IF EXISTS forum_categories_depth_guard ON forum_categories;
DROP FUNCTION IF EXISTS forum_validate_category_depth();
"#,
        )
        .await?;
    Ok(())
}

async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    ensure_existing_depth_is_valid(manager).await?;

    let connection = manager.get_connection();
    for statement in [
        "DROP TRIGGER IF EXISTS forum_categories_depth_insert",
        "DROP TRIGGER IF EXISTS forum_categories_depth_update",
        r#"CREATE TRIGGER forum_categories_depth_insert
           BEFORE INSERT ON forum_categories
           FOR EACH ROW
           WHEN NEW.parent_id IS NOT NULL AND (
               NOT EXISTS (
                   SELECT 1 FROM forum_categories parent
                   WHERE parent.tenant_id = NEW.tenant_id
                     AND parent.id = NEW.parent_id
               ) OR EXISTS (
                   WITH RECURSIVE ancestors(id, parent_id, depth, path, cycle) AS (
                       SELECT
                           parent.id,
                           parent.parent_id,
                           1,
                           ',' || parent.id || ',',
                           0
                       FROM forum_categories parent
                       WHERE parent.tenant_id = NEW.tenant_id
                         AND parent.id = NEW.parent_id

                       UNION ALL

                       SELECT
                           parent.id,
                           parent.parent_id,
                           ancestors.depth + 1,
                           ancestors.path || parent.id || ',',
                           instr(ancestors.path, ',' || parent.id || ',') > 0
                       FROM ancestors
                       JOIN forum_categories parent
                         ON parent.tenant_id = NEW.tenant_id
                        AND parent.id = ancestors.parent_id
                       WHERE ancestors.parent_id IS NOT NULL
                         AND ancestors.cycle = 0
                   )
                   SELECT 1 FROM ancestors
                   WHERE depth > 16 OR cycle = 1
               )
           )
           BEGIN
               SELECT RAISE(ABORT, 'forum category hierarchy exceeds maximum depth 16 or parent is missing');
           END"#,
        r#"CREATE TRIGGER forum_categories_depth_update
           BEFORE UPDATE OF tenant_id, parent_id ON forum_categories
           FOR EACH ROW
           WHEN NEW.parent_id IS NOT NULL AND (
               NOT EXISTS (
                   SELECT 1 FROM forum_categories parent
                   WHERE parent.tenant_id = NEW.tenant_id
                     AND parent.id = NEW.parent_id
               ) OR EXISTS (
                   WITH RECURSIVE ancestors(id, parent_id, depth, path, cycle) AS (
                       SELECT
                           parent.id,
                           parent.parent_id,
                           1,
                           ',' || parent.id || ',',
                           0
                       FROM forum_categories parent
                       WHERE parent.tenant_id = NEW.tenant_id
                         AND parent.id = NEW.parent_id

                       UNION ALL

                       SELECT
                           parent.id,
                           parent.parent_id,
                           ancestors.depth + 1,
                           ancestors.path || parent.id || ',',
                           instr(ancestors.path, ',' || parent.id || ',') > 0
                       FROM ancestors
                       JOIN forum_categories parent
                         ON parent.tenant_id = NEW.tenant_id
                        AND parent.id = ancestors.parent_id
                       WHERE ancestors.parent_id IS NOT NULL
                         AND ancestors.cycle = 0
                   )
                   SELECT 1 FROM ancestors
                   WHERE depth > 16 OR cycle = 1
               )
           )
           BEGIN
               SELECT RAISE(ABORT, 'forum category hierarchy exceeds maximum depth 16 or parent is missing');
           END"#,
    ] {
        connection.execute_unprepared(statement).await?;
    }

    Ok(())
}

async fn down_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        "DROP TRIGGER IF EXISTS forum_categories_depth_insert",
        "DROP TRIGGER IF EXISTS forum_categories_depth_update",
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}

async fn ensure_existing_depth_is_valid(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let row = manager
        .get_connection()
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            r#"
WITH RECURSIVE category_paths(tenant_id, origin_id, parent_id, depth, path, cycle) AS (
    SELECT tenant_id, id, parent_id, 0, ',' || id || ',', 0
    FROM forum_categories

    UNION ALL

    SELECT
        category_paths.tenant_id,
        category_paths.origin_id,
        parent.parent_id,
        category_paths.depth + 1,
        category_paths.path || parent.id || ',',
        instr(category_paths.path, ',' || parent.id || ',') > 0
    FROM category_paths
    JOIN forum_categories parent
      ON parent.tenant_id = category_paths.tenant_id
     AND parent.id = category_paths.parent_id
    WHERE category_paths.parent_id IS NOT NULL
      AND category_paths.cycle = 0
)
SELECT COUNT(*) AS invalid_count
FROM category_paths
WHERE depth > 16 OR cycle = 1
"#
            .to_string(),
        ))
        .await?
        .ok_or_else(|| DbErr::Custom("failed to validate forum category depth".to_string()))?;
    let invalid_count: i64 = row.try_get("", "invalid_count")?;
    if invalid_count != 0 {
        return Err(DbErr::Custom(
            "forum category depth migration blocked: existing hierarchy exceeds depth 16"
                .to_string(),
        ));
    }
    Ok(())
}
