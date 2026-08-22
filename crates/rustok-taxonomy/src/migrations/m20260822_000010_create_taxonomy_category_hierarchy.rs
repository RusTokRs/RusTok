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
                    .table(TaxonomyCategoryHierarchy::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TaxonomyCategoryHierarchy::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TaxonomyCategoryHierarchy::TermId)
                            .uuid()
                            .not_null(),
                    )
                    .col(ColumnDef::new(TaxonomyCategoryHierarchy::ParentTermId).uuid())
                    .col(
                        ColumnDef::new(TaxonomyCategoryHierarchy::Position)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .primary_key(
                        Index::create()
                            .name("pk_taxonomy_category_hierarchy")
                            .col(TaxonomyCategoryHierarchy::TenantId)
                            .col(TaxonomyCategoryHierarchy::TermId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_taxonomy_category_hierarchy_term")
                            .from_tbl(TaxonomyCategoryHierarchy::Table)
                            .from_col(TaxonomyCategoryHierarchy::TenantId)
                            .from_col(TaxonomyCategoryHierarchy::TermId)
                            .to_tbl(TaxonomyTerms::Table)
                            .to_col(TaxonomyTerms::TenantId)
                            .to_col(TaxonomyTerms::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_taxonomy_category_hierarchy_parent")
                            .from_tbl(TaxonomyCategoryHierarchy::Table)
                            .from_col(TaxonomyCategoryHierarchy::TenantId)
                            .from_col(TaxonomyCategoryHierarchy::ParentTermId)
                            .to_tbl(TaxonomyTerms::Table)
                            .to_col(TaxonomyTerms::TenantId)
                            .to_col(TaxonomyTerms::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_taxonomy_category_hierarchy_parent_position")
                    .table(TaxonomyCategoryHierarchy::Table)
                    .col(TaxonomyCategoryHierarchy::TenantId)
                    .col(TaxonomyCategoryHierarchy::ParentTermId)
                    .col(TaxonomyCategoryHierarchy::Position)
                    .col(TaxonomyCategoryHierarchy::TermId)
                    .to_owned(),
            )
            .await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres => install_postgres_guard(manager).await?,
            DatabaseBackend::Sqlite => install_sqlite_guards(manager).await?,
            backend => {
                return Err(DbErr::Custom(format!(
                    "taxonomy Category hierarchy migration does not support {backend:?}",
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
DROP TRIGGER IF EXISTS taxonomy_category_hierarchy_guard ON taxonomy_category_hierarchy;
DROP FUNCTION IF EXISTS taxonomy_validate_category_hierarchy();
"#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                let connection = manager.get_connection();
                connection
                    .execute_unprepared(
                        "DROP TRIGGER IF EXISTS taxonomy_category_hierarchy_insert_guard",
                    )
                    .await?;
                connection
                    .execute_unprepared(
                        "DROP TRIGGER IF EXISTS taxonomy_category_hierarchy_update_guard",
                    )
                    .await?;
            }
            backend => {
                return Err(DbErr::Custom(format!(
                    "taxonomy Category hierarchy migration does not support {backend:?}",
                )));
            }
        }

        manager
            .drop_table(
                Table::drop()
                    .table(TaxonomyCategoryHierarchy::Table)
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
CREATE OR REPLACE FUNCTION taxonomy_validate_category_hierarchy()
RETURNS trigger AS $$
DECLARE
    child_scope_type TEXT;
    child_scope_value TEXT;
    invalid_hierarchy BOOLEAN;
BEGIN
    -- Serialize hierarchy mutations per tenant. Service-level preflight remains useful for friendly
    -- errors, while this lock makes the storage invariant safe under concurrent writers.
    PERFORM pg_advisory_xact_lock(hashtextextended(NEW.tenant_id::text, 0));

    IF NEW.position < 0 THEN
        RAISE EXCEPTION 'taxonomy Category position must be zero or greater';
    END IF;

    SELECT scope_type, scope_value
      INTO child_scope_type, child_scope_value
      FROM taxonomy_terms
     WHERE tenant_id = NEW.tenant_id
       AND id = NEW.term_id
       AND kind = 'category';

    IF NOT FOUND THEN
        RAISE EXCEPTION 'taxonomy Category hierarchy child is missing or is not a Category';
    END IF;

    IF NEW.parent_term_id IS NULL THEN
        RETURN NEW;
    END IF;

    IF NEW.parent_term_id = NEW.term_id THEN
        RAISE EXCEPTION 'taxonomy Category cannot be its own parent';
    END IF;

    IF NOT EXISTS (
        SELECT 1
          FROM taxonomy_terms parent
         WHERE parent.tenant_id = NEW.tenant_id
           AND parent.id = NEW.parent_term_id
           AND parent.kind = 'category'
           AND parent.scope_type = child_scope_type
           AND parent.scope_value = child_scope_value
    ) THEN
        RAISE EXCEPTION 'taxonomy Category parent is missing, not a Category, or uses another scope';
    END IF;

    WITH RECURSIVE ancestors AS (
        SELECT
            NEW.parent_term_id AS id,
            hierarchy.parent_term_id AS parent_id,
            1 AS depth,
            NEW.parent_term_id = NEW.term_id AS cycle
        LEFT JOIN taxonomy_category_hierarchy hierarchy
          ON hierarchy.tenant_id = NEW.tenant_id
         AND hierarchy.term_id = NEW.parent_term_id

        UNION ALL

        SELECT
            hierarchy.term_id AS id,
            hierarchy.parent_term_id AS parent_id,
            ancestors.depth + 1 AS depth,
            hierarchy.term_id = NEW.term_id AS cycle
        FROM ancestors
        JOIN taxonomy_category_hierarchy hierarchy
          ON hierarchy.tenant_id = NEW.tenant_id
         AND hierarchy.term_id = ancestors.parent_id
        WHERE ancestors.parent_id IS NOT NULL
          AND NOT ancestors.cycle
    )
    SELECT EXISTS (
        SELECT 1 FROM ancestors
        WHERE cycle OR depth > 16
    ) INTO invalid_hierarchy;

    IF invalid_hierarchy THEN
        RAISE EXCEPTION 'taxonomy Category hierarchy contains a cycle or exceeds maximum depth 16';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS taxonomy_category_hierarchy_guard ON taxonomy_category_hierarchy;
CREATE TRIGGER taxonomy_category_hierarchy_guard
BEFORE INSERT OR UPDATE OF tenant_id, term_id, parent_term_id, position
ON taxonomy_category_hierarchy
FOR EACH ROW
EXECUTE FUNCTION taxonomy_validate_category_hierarchy();
"#,
        )
        .await?;
    Ok(())
}

async fn install_sqlite_guards(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        "DROP TRIGGER IF EXISTS taxonomy_category_hierarchy_insert_guard",
        "DROP TRIGGER IF EXISTS taxonomy_category_hierarchy_update_guard",
        sqlite_guard_sql("INSERT", "taxonomy_category_hierarchy_insert_guard"),
        sqlite_guard_sql("UPDATE", "taxonomy_category_hierarchy_update_guard"),
    ] {
        connection.execute_unprepared(&statement).await?;
    }
    Ok(())
}

fn sqlite_guard_sql(operation: &str, trigger_name: &str) -> String {
    format!(
        r#"
CREATE TRIGGER {trigger_name}
BEFORE {operation} ON taxonomy_category_hierarchy
FOR EACH ROW
BEGIN
    SELECT CASE
        WHEN NEW.position < 0
        THEN RAISE(ABORT, 'taxonomy Category position must be zero or greater')
    END;

    SELECT CASE
        WHEN NOT EXISTS (
            SELECT 1 FROM taxonomy_terms child
            WHERE child.tenant_id = NEW.tenant_id
              AND child.id = NEW.term_id
              AND child.kind = 'category'
        )
        THEN RAISE(ABORT, 'taxonomy Category hierarchy child is missing or is not a Category')
    END;

    SELECT CASE
        WHEN NEW.parent_term_id = NEW.term_id
        THEN RAISE(ABORT, 'taxonomy Category cannot be its own parent')
    END;

    SELECT CASE
        WHEN NEW.parent_term_id IS NOT NULL AND NOT EXISTS (
            SELECT 1
            FROM taxonomy_terms child
            JOIN taxonomy_terms parent
              ON parent.tenant_id = child.tenant_id
             AND parent.id = NEW.parent_term_id
             AND parent.kind = 'category'
             AND parent.scope_type = child.scope_type
             AND parent.scope_value = child.scope_value
            WHERE child.tenant_id = NEW.tenant_id
              AND child.id = NEW.term_id
              AND child.kind = 'category'
        )
        THEN RAISE(ABORT, 'taxonomy Category parent is missing, not a Category, or uses another scope')
    END;

    SELECT CASE
        WHEN NEW.parent_term_id IS NOT NULL AND EXISTS (
            WITH RECURSIVE ancestors(id, parent_id, depth, cycle) AS (
                SELECT
                    NEW.parent_term_id,
                    hierarchy.parent_term_id,
                    1,
                    NEW.parent_term_id = NEW.term_id
                FROM (SELECT 1)
                LEFT JOIN taxonomy_category_hierarchy hierarchy
                  ON hierarchy.tenant_id = NEW.tenant_id
                 AND hierarchy.term_id = NEW.parent_term_id

                UNION ALL

                SELECT
                    hierarchy.term_id,
                    hierarchy.parent_term_id,
                    ancestors.depth + 1,
                    hierarchy.term_id = NEW.term_id
                FROM ancestors
                JOIN taxonomy_category_hierarchy hierarchy
                  ON hierarchy.tenant_id = NEW.tenant_id
                 AND hierarchy.term_id = ancestors.parent_id
                WHERE ancestors.parent_id IS NOT NULL
                  AND ancestors.cycle = 0
            )
            SELECT 1 FROM ancestors
            WHERE cycle = 1 OR depth > 16
        )
        THEN RAISE(ABORT, 'taxonomy Category hierarchy contains a cycle or exceeds maximum depth 16')
    END;
END
"#,
    )
}

#[derive(DeriveIden)]
enum TaxonomyCategoryHierarchy {
    Table,
    TenantId,
    TermId,
    ParentTermId,
    Position,
}

#[derive(DeriveIden)]
enum TaxonomyTerms {
    Table,
    TenantId,
    Id,
}
