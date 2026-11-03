use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() == DatabaseBackend::Sqlite {
            manager
                .get_connection()
                .execute_unprepared(
                    "CREATE INDEX idx_tenants_id_nocase ON tenants (id COLLATE NOCASE)",
                )
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() == DatabaseBackend::Sqlite {
            manager
                .get_connection()
                .execute_unprepared("DROP INDEX idx_tenants_id_nocase")
                .await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm_migration::sea_orm::{ConnectionTrait, Database, Statement};

    #[tokio::test]
    async fn sqlite_identity_lookup_uses_case_insensitive_index() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared("CREATE TABLE tenants (id BLOB PRIMARY KEY NOT NULL); CREATE VIEW tenant_identity_keys AS SELECT id FROM tenants").await.unwrap();
        let manager = SchemaManager::new(&db);
        Migration.up(&manager).await.unwrap();
        let row = db.query_one_raw(Statement::from_string(DatabaseBackend::Sqlite,
            "EXPLAIN QUERY PLAN SELECT id FROM tenant_identity_keys WHERE id COLLATE NOCASE IN ('abcdef', x'abcdef')")).await.unwrap().unwrap();
        let detail: String = row.try_get("", "detail").unwrap();
        assert!(detail.contains("idx_tenants_id_nocase"), "{detail}");
        Migration.down(&manager).await.unwrap();
    }
}
