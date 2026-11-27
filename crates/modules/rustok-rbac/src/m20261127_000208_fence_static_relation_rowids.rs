//! Prevent SQLite hidden-rowid replacement from bypassing relation history.
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

const RELATIONS: &[&str] = &["user_roles", "role_permissions"];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> { Some(true) }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => Ok(()),
            DatabaseBackend::Sqlite => {
                let db = manager.get_connection();
                // These UUID-primary-key tables have independent hidden rowids.
                // With recursive_triggers OFF a REPLACE victim need not run its
                // DELETE triggers. It can belong to a different authority owner.
                // Generic UPDATE covers all three aliases (rowid, _rowid_, oid).
                // NEW.rowid is undefined for an implicit rowid in BEFORE INSERT;
                // a collision denies conservatively rather than losing history.
                for table in RELATIONS {
                    db.execute_unprepared(&format!(
                        "CREATE TRIGGER rbac_{table}_rowid_update_guard
                         BEFORE UPDATE ON {table}
                         WHEN NEW.rowid IS NOT OLD.rowid
                         BEGIN SELECT RAISE(ABORT, 'RBAC relation rowid is immutable'); END;
                         CREATE TRIGGER rbac_{table}_rowid_insert_guard
                         BEFORE INSERT ON {table}
                         WHEN EXISTS (SELECT 1 FROM {table} WHERE rowid = NEW.rowid)
                         BEGIN SELECT RAISE(ABORT, 'RBAC relation rowid collision'); END;"
                    )).await?;
                }
                Ok(())
            }
            _ => Err(DbErr::Migration("unsupported static RBAC rowid backend".into())),
        }
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration("static RBAC relation history guards cannot be downgraded safely".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm_migration::sea_orm::{Database, Statement, TransactionTrait};

    #[tokio::test]
    async fn sqlite_hidden_rowid_conflicts_cannot_erase_other_relations() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared("PRAGMA recursive_triggers = OFF;
            CREATE TABLE user_roles (id TEXT PRIMARY KEY, user_id TEXT NOT NULL, role_id TEXT NOT NULL, UNIQUE(user_id,role_id));
            CREATE TABLE role_permissions (id TEXT PRIMARY KEY, role_id TEXT NOT NULL, permission_id TEXT NOT NULL, UNIQUE(role_id,permission_id));
            INSERT INTO user_roles(rowid,id,user_id,role_id) VALUES (7,'m1','u1','r1'),(8,'m2','u2','r2');
            INSERT INTO role_permissions(rowid,id,role_id,permission_id) VALUES (7,'g1','r1','p1'),(8,'g2','r2','p2');"
        ).await.unwrap();
        let tx = db.begin().await.unwrap();
        Migration.up(&SchemaManager::new(&tx)).await.unwrap();
        tx.commit().await.unwrap();
        for (table, columns) in [
            ("user_roles", "user_id,role_id"),
            ("role_permissions", "role_id,permission_id"),
        ] {
            for alias in ["rowid", "_rowid_", "oid"] {
                for sql in [
                    format!("UPDATE OR REPLACE {table} SET {alias} = 8 WHERE rowid = 7"),
                    format!("INSERT OR REPLACE INTO {table}({alias},id,{columns}) VALUES (8,'new','x','y')"),
                    format!("INSERT OR IGNORE INTO {table}({alias},id,{columns}) VALUES (8,'new','x','y')"),
                ] {
                    assert!(db.execute_unprepared(&sql).await.is_err(), "{sql}");
                }
            }
            let row = db.query_one_raw(Statement::from_string(DatabaseBackend::Sqlite,
                format!("SELECT COUNT(*) AS total FROM {table} WHERE rowid IN (7,8)")))
                .await.unwrap().unwrap();
            assert_eq!(row.try_get::<i64>("", "total").unwrap(), 2);
            // Normal owner inserts, exact no-ops and deletes are unaffected.
            db.execute_unprepared(&format!("INSERT INTO {table}(id,{columns}) VALUES ('new','x','y')")).await.unwrap();
            db.execute_unprepared(&format!("UPDATE {table} SET rowid = rowid")).await.unwrap();
            assert_eq!(db.execute_unprepared(&format!("DELETE FROM {table} WHERE id = 'new'")).await.unwrap().rows_affected(), 1);
        }
    }
}
