use sea_orm_migration::prelude::*;

use crate::backend::SupportedBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if SupportedBackend::new(manager.get_database_backend())? == SupportedBackend::Sqlite {
            for sql in [
                "CREATE INDEX idx_tenant_modules_tenant_nocase ON tenant_modules (tenant_id COLLATE NOCASE)",
                "CREATE INDEX idx_static_lifecycle_tenant_nocase ON module_static_tenant_lifecycle (tenant_id COLLATE NOCASE)",
                "CREATE INDEX idx_module_operations_tenant_key_nocase ON module_operations (tenant_id COLLATE NOCASE, idempotency_key COLLATE NOCASE)",
                "CREATE INDEX idx_module_operations_id_nocase ON module_operations (id COLLATE NOCASE)",
                "CREATE INDEX idx_module_policy_cursor_tenant_nocase ON module_policy_revision_cursors (tenant_id COLLATE NOCASE, consumer_key)",
                "CREATE UNIQUE INDEX uq_module_policy_cursor_uuid ON module_policy_revision_cursors (\
                 replace(replace(replace(replace(lower(CASE WHEN typeof(tenant_id) = 'blob' THEN hex(tenant_id) ELSE CAST(tenant_id AS TEXT) END), 'urn:uuid:', ''), '{', ''), '}', ''), '-', ''), consumer_key)",
                "CREATE UNIQUE INDEX uq_module_operations_uuid_representations ON module_operations (\
                 replace(replace(replace(replace(lower(CASE WHEN typeof(tenant_id) = 'blob' THEN hex(tenant_id) ELSE CAST(tenant_id AS TEXT) END), 'urn:uuid:', ''), '{', ''), '}', ''), '-', ''), \
                 replace(replace(replace(replace(lower(CASE WHEN typeof(idempotency_key) = 'blob' THEN hex(idempotency_key) ELSE CAST(idempotency_key AS TEXT) END), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '')\
                 ) WHERE idempotency_key IS NOT NULL",
            ] {
                manager.get_connection().execute_unprepared(sql).await?;
            }
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if SupportedBackend::new(manager.get_database_backend())? == SupportedBackend::Sqlite {
            for name in [
                "uq_module_operations_uuid_representations",
                "uq_module_policy_cursor_uuid",
                "idx_module_policy_cursor_tenant_nocase",
                "idx_module_operations_id_nocase",
                "idx_module_operations_tenant_key_nocase",
                "idx_static_lifecycle_tenant_nocase",
                "idx_tenant_modules_tenant_nocase",
            ] {
                manager
                    .get_connection()
                    .execute_unprepared(&format!("DROP INDEX {name}"))
                    .await?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement, TransactionTrait};

    #[tokio::test]
    async fn sqlite_case_insensitive_lookups_use_indexes() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        for sql in [
            "CREATE TABLE tenant_modules (tenant_id BLOB)",
            "CREATE TABLE module_static_tenant_lifecycle (tenant_id BLOB)",
            "CREATE TABLE module_operations (id BLOB, tenant_id BLOB, idempotency_key BLOB)",
            "CREATE TABLE module_policy_revision_cursors (tenant_id BLOB, consumer_key TEXT)",
        ] {
            db.execute_unprepared(sql).await.unwrap();
        }
        let manager = SchemaManager::new(&db);
        Migration.up(&manager).await.unwrap();
        db.execute_unprepared("INSERT INTO module_operations VALUES (x'01', x'abcdefabcdef4abc8defabcdefabcdef', x'00112233445566778899aabbccddeeff')").await.unwrap();
        assert!(db.execute_unprepared("INSERT INTO module_operations VALUES (x'02', 'URN:UUID:abcdefab-cdef-4abc-8def-abcdefabcdef', '{00112233-4455-6677-8899-aabbccddeeff}')").await.is_err());
        db.execute_unprepared("INSERT INTO module_policy_revision_cursors VALUES (x'abcdefabcdef4abc8defabcdefabcdef', 'worker')").await.unwrap();
        assert!(db.execute_unprepared("INSERT INTO module_policy_revision_cursors VALUES ('{abcdefab-cdef-4abc-8def-abcdefabcdef}', 'worker')").await.is_err());
        db.execute_unprepared("INSERT INTO module_policy_revision_cursors VALUES ('{abcdefab-cdef-4abc-8def-abcdefabcdef}', 'other')").await.unwrap();
        for (query, index) in [
            (
                "SELECT * FROM tenant_modules WHERE tenant_id COLLATE NOCASE = 'a'",
                "idx_tenant_modules_tenant_nocase",
            ),
            (
                "SELECT * FROM module_static_tenant_lifecycle WHERE tenant_id COLLATE NOCASE = 'a'",
                "idx_static_lifecycle_tenant_nocase",
            ),
            (
                "SELECT * FROM module_operations WHERE tenant_id COLLATE NOCASE = 'a'",
                "idx_module_operations_tenant_key_nocase",
            ),
            (
                "SELECT * FROM module_operations WHERE id COLLATE NOCASE = 'a'",
                "idx_module_operations_id_nocase",
            ),
            (
                "SELECT * FROM module_policy_revision_cursors WHERE tenant_id COLLATE NOCASE = 'a'",
                "idx_module_policy_cursor_tenant_nocase",
            ),
        ] {
            let row = db
                .query_one_raw(Statement::from_string(
                    DbBackend::Sqlite,
                    format!("EXPLAIN QUERY PLAN {query}"),
                ))
                .await
                .unwrap()
                .unwrap();
            let detail: String = row.try_get("", "detail").unwrap();
            assert!(detail.contains(index), "{detail}");
        }
        Migration.down(&manager).await.unwrap();
    }

    #[tokio::test]
    async fn sqlite_existing_uuid_collisions_abort_index_migration() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        for sql in [
            "CREATE TABLE tenant_modules (tenant_id BLOB)",
            "CREATE TABLE module_static_tenant_lifecycle (tenant_id BLOB)",
            "CREATE TABLE module_operations (id BLOB, tenant_id BLOB, idempotency_key BLOB)",
            "CREATE TABLE module_policy_revision_cursors (tenant_id BLOB, consumer_key TEXT)",
            "INSERT INTO module_operations VALUES (x'01', x'abcdefabcdef4abc8defabcdefabcdef', x'00112233445566778899aabbccddeeff')",
            "INSERT INTO module_operations VALUES (x'02', 'URN:UUID:abcdefab-cdef-4abc-8def-abcdefabcdef', '{00112233-4455-6677-8899-aabbccddeeff}')",
        ] {
            db.execute_unprepared(sql).await.unwrap();
        }
        let txn = db.begin().await.unwrap();
        assert!(Migration.up(&SchemaManager::new(&txn)).await.is_err());
        txn.rollback().await.unwrap();
        let row = db.query_one_raw(Statement::from_string(DbBackend::Sqlite,
            "SELECT (SELECT COUNT(*) FROM module_operations) AS rows, (SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name LIKE 'idx_%_nocase') AS indexes")).await.unwrap().unwrap();
        assert_eq!(row.try_get::<i64>("", "rows").unwrap(), 2);
        assert_eq!(row.try_get::<i64>("", "indexes").unwrap(), 0);
    }
}
