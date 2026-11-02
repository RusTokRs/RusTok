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
                .execute_unprepared("CREATE VIEW tenant_identity_keys AS SELECT id FROM tenants")
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() == DatabaseBackend::Sqlite {
            manager
                .get_connection()
                .execute_unprepared("DROP VIEW tenant_identity_keys")
                .await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm_migration::sea_orm::{ConnectionTrait, Database};

    #[tokio::test]
    async fn tenant_identity_keys_preserve_storage_and_are_read_only() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared("CREATE TABLE tenants (id BLOB PRIMARY KEY NOT NULL)")
            .await
            .unwrap();
        db.execute_unprepared("INSERT INTO tenants (id) VALUES (x'0011'), ('0011')")
            .await
            .unwrap();
        let manager = SchemaManager::new(&db);
        Migration.up(&manager).await.unwrap();
        let rows = db
            .query_all_raw(sea_orm_migration::sea_orm::Statement::from_string(
                DatabaseBackend::Sqlite,
                "SELECT typeof(id) AS storage FROM tenant_identity_keys ORDER BY storage",
            ))
            .await
            .unwrap();
        let types: Vec<String> = rows
            .iter()
            .map(|row| row.try_get("", "storage").unwrap())
            .collect();
        assert_eq!(types, ["blob", "text"]);
        assert!(
            db.execute_unprepared("INSERT INTO tenant_identity_keys (id) VALUES ('other')")
                .await
                .is_err()
        );
        Migration.down(&manager).await.unwrap();
        assert!(
            db.query_all_raw(sea_orm_migration::sea_orm::Statement::from_string(
                DatabaseBackend::Sqlite,
                "SELECT id FROM tenant_identity_keys",
            ))
            .await
            .is_err()
        );
    }
}
