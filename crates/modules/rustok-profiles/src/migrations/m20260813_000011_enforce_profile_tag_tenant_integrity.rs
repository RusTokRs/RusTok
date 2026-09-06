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
                "profile tag tenant integrity migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => down_postgres(manager).await,
            DatabaseBackend::Sqlite => down_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "profile tag tenant integrity migration does not support {backend:?}"
            ))),
        }
    }
}

async fn up_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
UPDATE profile_tags relation
SET tenant_id = profile.tenant_id
FROM profiles profile
WHERE profile.user_id = relation.profile_user_id
  AND relation.tenant_id IS DISTINCT FROM profile.tenant_id;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM profile_tags relation
        LEFT JOIN profiles profile
          ON profile.user_id = relation.profile_user_id
        LEFT JOIN taxonomy_terms term
          ON term.id = relation.term_id
        WHERE profile.user_id IS NULL
           OR relation.tenant_id <> profile.tenant_id
           OR term.id IS NULL
           OR term.tenant_id <> relation.tenant_id
    ) THEN
        RAISE EXCEPTION
            'profile tag tenant integrity migration blocked: invalid legacy relation';
    END IF;
END $$;

CREATE UNIQUE INDEX IF NOT EXISTS uq_profiles_tenant_user_id
    ON profiles (tenant_id, user_id);

ALTER TABLE profile_tags
    DROP CONSTRAINT IF EXISTS fk_profile_tags_profile;
ALTER TABLE profile_tags
    DROP CONSTRAINT IF EXISTS fk_profile_tags_term;
ALTER TABLE profile_tags
    DROP CONSTRAINT IF EXISTS fk_profile_tags_profile_tenant;
ALTER TABLE profile_tags
    DROP CONSTRAINT IF EXISTS fk_profile_tags_term_tenant;

ALTER TABLE profile_tags
    ADD CONSTRAINT fk_profile_tags_profile_tenant
    FOREIGN KEY (tenant_id, profile_user_id)
    REFERENCES profiles (tenant_id, user_id)
    ON UPDATE CASCADE
    ON DELETE CASCADE;

ALTER TABLE profile_tags
    ADD CONSTRAINT fk_profile_tags_term_tenant
    FOREIGN KEY (tenant_id, term_id)
    REFERENCES taxonomy_terms (tenant_id, id)
    ON UPDATE CASCADE
    ON DELETE CASCADE;
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
ALTER TABLE profile_tags
    DROP CONSTRAINT IF EXISTS fk_profile_tags_profile_tenant;
ALTER TABLE profile_tags
    DROP CONSTRAINT IF EXISTS fk_profile_tags_term_tenant;

ALTER TABLE profile_tags
    ADD CONSTRAINT fk_profile_tags_profile
    FOREIGN KEY (profile_user_id)
    REFERENCES profiles (user_id)
    ON UPDATE CASCADE
    ON DELETE CASCADE;
ALTER TABLE profile_tags
    ADD CONSTRAINT fk_profile_tags_term
    FOREIGN KEY (term_id)
    REFERENCES taxonomy_terms (id)
    ON UPDATE CASCADE
    ON DELETE CASCADE;

DROP INDEX IF EXISTS uq_profiles_tenant_user_id;
"#,
        )
        .await?;
    Ok(())
}

async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();

    connection
        .execute_unprepared(
            "UPDATE profile_tags
             SET tenant_id = (
                 SELECT profile.tenant_id
                 FROM profiles profile
                 WHERE profile.user_id = profile_tags.profile_user_id
             )
             WHERE EXISTS (
                 SELECT 1
                 FROM profiles profile
                 WHERE profile.user_id = profile_tags.profile_user_id
                   AND profile.tenant_id <> profile_tags.tenant_id
             )",
        )
        .await?;

    ensure_sqlite_legacy_relations_valid(manager).await?;

    for statement in [
        "CREATE UNIQUE INDEX IF NOT EXISTS uq_profiles_tenant_user_id
         ON profiles (tenant_id, user_id)",
        r#"CREATE TRIGGER profile_tags_tenant_insert
           BEFORE INSERT ON profile_tags
           FOR EACH ROW
           WHEN NOT EXISTS (
                 SELECT 1 FROM profiles profile
                 WHERE profile.user_id = NEW.profile_user_id
                   AND profile.tenant_id = NEW.tenant_id
             )
             OR NOT EXISTS (
                 SELECT 1 FROM taxonomy_terms term
                 WHERE term.id = NEW.term_id
                   AND term.tenant_id = NEW.tenant_id
             )
           BEGIN
               SELECT RAISE(ABORT, 'profile tag tenant mismatch');
           END"#,
        r#"CREATE TRIGGER profile_tags_tenant_update
           BEFORE UPDATE OF tenant_id, profile_user_id, term_id ON profile_tags
           FOR EACH ROW
           WHEN NOT EXISTS (
                 SELECT 1 FROM profiles profile
                 WHERE profile.user_id = NEW.profile_user_id
                   AND profile.tenant_id = NEW.tenant_id
             )
             OR NOT EXISTS (
                 SELECT 1 FROM taxonomy_terms term
                 WHERE term.id = NEW.term_id
                   AND term.tenant_id = NEW.tenant_id
             )
           BEGIN
               SELECT RAISE(ABORT, 'profile tag tenant mismatch');
           END"#,
    ] {
        connection.execute_unprepared(statement).await?;
    }

    Ok(())
}

async fn down_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        "DROP TRIGGER IF EXISTS profile_tags_tenant_insert",
        "DROP TRIGGER IF EXISTS profile_tags_tenant_update",
        "DROP INDEX IF EXISTS uq_profiles_tenant_user_id",
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}

async fn ensure_sqlite_legacy_relations_valid(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let row = manager
        .get_connection()
        .query_one_raw(Statement::from_string(
            DatabaseBackend::Sqlite,
            r#"
SELECT COUNT(*) AS invalid_count
FROM profile_tags relation
LEFT JOIN profiles profile
  ON profile.user_id = relation.profile_user_id
LEFT JOIN taxonomy_terms term
  ON term.id = relation.term_id
WHERE profile.user_id IS NULL
   OR relation.tenant_id <> profile.tenant_id
   OR term.id IS NULL
   OR term.tenant_id <> relation.tenant_id
"#
            .to_string(),
        ))
        .await?
        .ok_or_else(|| {
            DbErr::Custom("failed to validate profile tag tenant integrity".to_string())
        })?;
    let invalid_count: i64 = row.try_get("", "invalid_count")?;
    if invalid_count != 0 {
        return Err(DbErr::Custom(
            "profile tag tenant integrity migration blocked: invalid legacy relation".to_string(),
        ));
    }
    Ok(())
}
