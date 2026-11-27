use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

use crate::backend::SupportedBackend;

/// Legacy aggregate claims used a caller-supplied key without a receipt fence.
/// They cannot safely be classified as settings versus journal claims. Stop
/// old writers and reconcile every active aggregate claim before applying.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        let db = manager.get_connection();
        if manager.has_table("module_static_tenant_lifecycle").await? {
            if backend == SupportedBackend::Postgres {
                // Fail rather than silently inspect an RLS-filtered subset.
                db.execute_unprepared("SET LOCAL row_security = off")
                    .await?;
                db.execute_unprepared(
                    "LOCK TABLE module_static_tenant_lifecycle IN ACCESS EXCLUSIVE MODE",
                )
                .await?;
            }
            let row = db.query_one_raw(Statement::from_string(backend.db_backend(),
                "SELECT COUNT(*) AS n FROM module_static_tenant_lifecycle WHERE active_idempotency_key IS NOT NULL".to_string()
            )).await?.ok_or_else(|| DbErr::Migration("cannot inspect legacy lifecycle claims".into()))?;
            let count: i64 = row.try_get("", "n")?;
            if count != 0 {
                return Err(DbErr::Migration("reconcile all active static lifecycle claims and stop old writers before upgrading settings claim fencing".into()));
            }
            // Ownership must be attached to the same row as the claim. A
            // separate receipt-fence lookup is insufficient here: journal
            // workers may not have a tenant RLS scope, and a check followed
            // by a CAS would race with a claim transfer. The preflight above
            // guarantees there are no legacy active owners to misclassify.
            db.execute_unprepared("ALTER TABLE module_static_tenant_lifecycle ADD COLUMN claim_owner TEXT NOT NULL DEFAULT 'journal' CHECK (claim_owner IN ('journal', 'settings'))").await?;
            if backend == SupportedBackend::Postgres {
                db.execute_unprepared("SET LOCAL row_security = on").await?;
            }
        }
        let sql_type = match backend {
            SupportedBackend::Postgres => "UUID",
            SupportedBackend::Sqlite => "TEXT",
        };
        db.execute_unprepared(&format!(
            "CREATE TABLE module_static_settings_claim_fences (receipt_id {sql_type} PRIMARY KEY NOT NULL, tenant_id {sql_type} NOT NULL, module_slug TEXT NOT NULL, claim_token {sql_type} NOT NULL)"
        )).await?;
        if backend == SupportedBackend::Postgres {
            db.execute_unprepared(
                "ALTER TABLE module_static_settings_claim_fences ENABLE ROW LEVEL SECURITY",
            )
            .await?;
            db.execute_unprepared("CREATE POLICY module_static_settings_claim_fences_scope ON module_static_settings_claim_fences USING (tenant_id::text = current_setting('rustok.tenant_id', true)) WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))").await?;
        }
        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration(
            "settings claim fencing cannot be safely downgraded while operations may exist".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{Database, DbBackend, TransactionTrait};
    use uuid::Uuid;

    #[tokio::test]
    #[ignore = "requires RUSTOK_MODULES_TEST_DATABASE_URL and PostgreSQL SET ROLE privileges"]
    async fn postgres_preflight_and_fence_rls_are_fail_closed() {
        let url = std::env::var("RUSTOK_MODULES_TEST_DATABASE_URL")
            .expect("RUSTOK_MODULES_TEST_DATABASE_URL is required");
        let db = Database::connect(url).await.unwrap();
        let tx = db.begin().await.unwrap();
        let schema = format!("settings_fence_test_{}", Uuid::new_v4().simple());
        tx.execute_unprepared(&format!(
            "CREATE SCHEMA {schema}; GRANT USAGE ON SCHEMA {schema} TO pg_read_all_data; \
             SET LOCAL search_path TO {schema}, public; \
             CREATE TABLE module_static_tenant_lifecycle (tenant_id UUID, active_idempotency_key UUID)"
        )).await.unwrap();
        let tenant_a = Uuid::new_v4();
        let tenant_b = Uuid::new_v4();
        let receipt = Uuid::new_v4();
        let old_token = Uuid::new_v4();
        let new_token = Uuid::new_v4();
        tx.execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO module_static_tenant_lifecycle VALUES ($1, $2)",
            vec![tenant_a.into(), old_token.into()],
        ))
        .await
        .unwrap();
        let manager = SchemaManager::new(&tx);
        assert!(
            matches!(Migration.up(&manager).await, Err(DbErr::Migration(message)) if message.contains("reconcile all active"))
        );
        tx.execute_unprepared("DELETE FROM module_static_tenant_lifecycle")
            .await
            .unwrap();
        Migration.up(&manager).await.unwrap();
        tx.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,
            "INSERT INTO module_static_tenant_lifecycle (tenant_id, active_idempotency_key, claim_owner) VALUES ($1, $2, 'settings')",
            vec![tenant_a.into(), old_token.into()])).await.unwrap();
        let row = tx
            .query_one_raw(Statement::from_string(
                DbBackend::Postgres,
                "SELECT claim_owner FROM module_static_tenant_lifecycle".to_string(),
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            row.try_get::<String>("", "claim_owner").unwrap(),
            "settings"
        );
        tx.execute_unprepared(&format!(
            "GRANT SELECT, INSERT, UPDATE, DELETE ON {schema}.module_static_settings_claim_fences TO pg_read_all_data"
        )).await.unwrap();
        tx.execute_unprepared("SET LOCAL ROLE pg_read_all_data")
            .await
            .unwrap();
        let role = tx.query_one_raw(Statement::from_string(DbBackend::Postgres,
            "SELECT NOT rolsuper AND NOT rolbypassrls AS enforced FROM pg_roles WHERE rolname = current_user".to_string()
        )).await.unwrap().unwrap();
        assert!(role.try_get::<bool>("", "enforced").unwrap());
        tx.execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT set_config('rustok.tenant_id', $1, true)",
            vec![tenant_a.to_string().into()],
        ))
        .await
        .unwrap();
        let upsert = "INSERT INTO module_static_settings_claim_fences (receipt_id, tenant_id, module_slug, claim_token) \
            VALUES ($1, $2, $3, $4) ON CONFLICT (receipt_id) DO UPDATE SET \
            tenant_id = EXCLUDED.tenant_id, module_slug = EXCLUDED.module_slug, claim_token = EXCLUDED.claim_token";
        for token in [old_token, new_token] {
            tx.execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                upsert,
                vec![
                    receipt.into(),
                    tenant_a.into(),
                    "forum".into(),
                    token.into(),
                ],
            ))
            .await
            .unwrap();
        }
        let delete = "DELETE FROM module_static_settings_claim_fences WHERE receipt_id=$1 AND tenant_id=$2 AND module_slug=$3 AND claim_token=$4";
        let old = tx
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                delete,
                vec![
                    receipt.into(),
                    tenant_a.into(),
                    "forum".into(),
                    old_token.into(),
                ],
            ))
            .await
            .unwrap();
        assert_eq!(old.rows_affected(), 0);
        tx.execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT set_config('rustok.tenant_id', $1, true)",
            vec![tenant_b.to_string().into()],
        ))
        .await
        .unwrap();
        let rows = tx
            .query_all_raw(Statement::from_string(
                DbBackend::Postgres,
                "SELECT receipt_id FROM module_static_settings_claim_fences".to_string(),
            ))
            .await
            .unwrap();
        assert!(rows.is_empty());
        let foreign = tx
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                delete,
                vec![
                    receipt.into(),
                    tenant_a.into(),
                    "forum".into(),
                    new_token.into(),
                ],
            ))
            .await
            .unwrap();
        assert_eq!(foreign.rows_affected(), 0);
        tx.rollback().await.unwrap();
    }
}
