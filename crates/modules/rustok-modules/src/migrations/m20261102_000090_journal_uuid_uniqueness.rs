use sea_orm_migration::prelude::*;

use crate::backend::SupportedBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if SupportedBackend::new(manager.get_database_backend())? == SupportedBackend::Sqlite {
            // SQLite's ordinary unique index distinguishes UUID blobs from
            // equivalent UUID text. Index the normalized logical UUIDs so a
            // legacy writer cannot concurrently admit the same receipt.
            // Existing collisions fail index creation without changing rows.
            manager.get_connection().execute_unprepared(
                "CREATE UNIQUE INDEX uq_module_operations_logical_idempotency \
                 ON module_operations ( \
                   lower(replace(CASE WHEN typeof(tenant_id) = 'blob' THEN hex(tenant_id) ELSE CAST(tenant_id AS TEXT) END, '-', '')), \
                   lower(replace(CASE WHEN typeof(idempotency_key) = 'blob' THEN hex(idempotency_key) ELSE CAST(idempotency_key AS TEXT) END, '-', '')) \
                 ) WHERE idempotency_key IS NOT NULL",
            ).await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if SupportedBackend::new(manager.get_database_backend())? == SupportedBackend::Sqlite {
            manager
                .get_connection()
                .execute_unprepared("DROP INDEX uq_module_operations_logical_idempotency")
                .await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::Database;

    #[tokio::test]
    async fn sqlite_logical_journal_identity_rejects_mixed_uuid_encodings() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared("CREATE TABLE module_operations (id INTEGER PRIMARY KEY, tenant_id BLOB NOT NULL, idempotency_key BLOB)").await.unwrap();
        db.execute_unprepared("INSERT INTO module_operations VALUES (1, x'00112233445566778899aabbccddeeff', x'ffeeddccbbaa99887766554433221100'), (2, '00112233-4455-6677-8899-AABBCCDDEEFF', 'ffeeddcc-bbaa-9988-7766-554433221100')").await.unwrap();
        let manager = SchemaManager::new(&db);
        assert!(Migration.up(&manager).await.is_err());
        let count = db
            .query_one_raw(sea_orm::Statement::from_string(
                sea_orm::DbBackend::Sqlite,
                "SELECT COUNT(*) AS count FROM module_operations",
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get::<i64>("", "count")
            .unwrap();
        assert_eq!(count, 2, "a collision must preserve both receipts");
        db.execute_unprepared("DELETE FROM module_operations WHERE id = 2")
            .await
            .unwrap();
        Migration.up(&manager).await.unwrap();
        for sql in [
            "INSERT INTO module_operations VALUES (2, '00112233-4455-6677-8899-aabbccddeeff', x'ffeeddccbbaa99887766554433221100')",
            "INSERT INTO module_operations VALUES (2, x'00112233445566778899aabbccddeeff', 'FFEEDDCC-BBAA-9988-7766-554433221100')",
            "INSERT INTO module_operations VALUES (2, '00112233445566778899aabbccddeeff', 'ffeeddcc-bbaa-9988-7766-554433221100')",
        ] {
            assert!(
                db.execute_unprepared(sql).await.is_err(),
                "mixed encoding must collide"
            );
        }
        db.execute_unprepared("INSERT INTO module_operations VALUES (3, x'10112233445566778899aabbccddeeff', x'ffeeddccbbaa99887766554433221100'), (4, x'00112233445566778899aabbccddeeff', NULL), (5, x'00112233445566778899aabbccddeeff', NULL)").await.unwrap();
        Migration.down(&manager).await.unwrap();
        db.execute_unprepared("INSERT INTO module_operations VALUES (6, '00112233-4455-6677-8899-aabbccddeeff', 'ffeeddcc-bbaa-9988-7766-554433221100')").await.unwrap();
    }
}
