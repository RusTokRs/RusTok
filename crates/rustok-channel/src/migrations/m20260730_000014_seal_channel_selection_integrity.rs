use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{
    ConnectionTrait, DatabaseBackend, Statement, TransactionTrait,
};
use uuid::Uuid;

const HOST_TARGET_TYPE: &str = "web_domain";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => seal_integrity(manager).await,
            DatabaseBackend::Sqlite => {
                // SeaORM 1.1 wraps PostgreSQL migration runs in a transaction, but
                // not SQLite runs. The previous migration has already installed
                // write guards, so perform the final validation/rebuild atomically.
                let transaction = manager.get_connection().begin().await?;
                let transaction_manager = SchemaManager::new(&transaction);
                seal_integrity(&transaction_manager).await?;
                transaction.commit().await
            }
            DatabaseBackend::MySql => Err(DbErr::Custom(
                "channel selection integrity does not support MySQL; durable channel generation requires PostgreSQL or SQLite"
                    .to_string(),
            )),
        }
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Forward-only verification/backfill seal. Removing it would allow an
        // interrupted SQLite migration to leave incomplete derived claim state.
        Ok(())
    }
}

async fn seal_integrity(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    reject_cross_tenant_oauth_binding(manager).await?;
    reject_cross_tenant_policy_action(manager).await?;
    reject_duplicate_host_claims(manager).await?;
    reject_duplicate_primary_targets(manager).await?;
    rebuild_host_claims(manager).await?;
    verify_host_claim_projection(manager).await
}

async fn reject_cross_tenant_oauth_binding(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    if !manager.has_table("channel_oauth_apps").await?
        || !manager.has_table("oauth_apps").await?
        || !manager.has_table("channels").await?
    {
        return Ok(());
    }

    let mismatch = manager
        .get_connection()
        .query_one(Statement::from_string(
            manager.get_database_backend(),
            r#"
            SELECT binding.id, channel.tenant_id AS channel_tenant_id,
                   app.tenant_id AS app_tenant_id
            FROM channel_oauth_apps binding
            JOIN channels channel ON channel.id = binding.channel_id
            JOIN oauth_apps app ON app.id = binding.oauth_app_id
            WHERE channel.tenant_id <> app.tenant_id
            LIMIT 1
            "#
            .to_string(),
        ))
        .await?;

    if let Some(row) = mismatch {
        let binding_id: Uuid = row.try_get("", "id")?;
        let channel_tenant_id: Uuid = row.try_get("", "channel_tenant_id")?;
        let app_tenant_id: Uuid = row.try_get("", "app_tenant_id")?;
        return Err(DbErr::Custom(format!(
            "channel OAuth binding {binding_id} crosses tenants {channel_tenant_id} and {app_tenant_id}"
        )));
    }
    Ok(())
}

async fn reject_cross_tenant_policy_action(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    if !manager
        .has_table("channel_resolution_policy_rules")
        .await?
        || !manager
            .has_table("channel_resolution_policy_sets")
            .await?
        || !manager.has_table("channels").await?
    {
        return Ok(());
    }

    let mismatch = manager
        .get_connection()
        .query_one(Statement::from_string(
            manager.get_database_backend(),
            r#"
            SELECT rule.id, policy_set.tenant_id AS policy_tenant_id,
                   channel.tenant_id AS channel_tenant_id
            FROM channel_resolution_policy_rules rule
            JOIN channel_resolution_policy_sets policy_set
              ON policy_set.id = rule.policy_set_id
            JOIN channels channel ON channel.id = rule.action_channel_id
            WHERE policy_set.tenant_id <> channel.tenant_id
            LIMIT 1
            "#
            .to_string(),
        ))
        .await?;

    if let Some(row) = mismatch {
        let rule_id: Uuid = row.try_get("", "id")?;
        let policy_tenant_id: Uuid = row.try_get("", "policy_tenant_id")?;
        let channel_tenant_id: Uuid = row.try_get("", "channel_tenant_id")?;
        return Err(DbErr::Custom(format!(
            "channel policy rule {rule_id} crosses tenants {policy_tenant_id} and {channel_tenant_id}"
        )));
    }
    Ok(())
}

async fn reject_duplicate_host_claims(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let duplicate = manager
        .get_connection()
        .query_one(Statement::from_string(
            manager.get_database_backend(),
            format!(
                r#"
                SELECT channel.tenant_id, target.value
                FROM channel_targets target
                JOIN channels channel ON channel.id = target.channel_id
                WHERE target.target_type = '{HOST_TARGET_TYPE}'
                GROUP BY channel.tenant_id, target.value
                HAVING COUNT(*) > 1
                LIMIT 1
                "#
            ),
        ))
        .await?;

    if let Some(row) = duplicate {
        let tenant_id: Uuid = row.try_get("", "tenant_id")?;
        let value: String = row.try_get("", "value")?;
        return Err(DbErr::Custom(format!(
            "tenant {tenant_id} has duplicate channel host claim `{value}`"
        )));
    }
    Ok(())
}

async fn reject_duplicate_primary_targets(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let duplicate = manager
        .get_connection()
        .query_one(Statement::from_string(
            manager.get_database_backend(),
            r#"
            SELECT channel_id
            FROM channel_targets
            WHERE is_primary = TRUE
            GROUP BY channel_id
            HAVING COUNT(*) > 1
            LIMIT 1
            "#
            .to_string(),
        ))
        .await?;

    if let Some(row) = duplicate {
        let channel_id: Uuid = row.try_get("", "channel_id")?;
        return Err(DbErr::Custom(format!(
            "channel {channel_id} has multiple primary targets"
        )));
    }
    Ok(())
}

async fn rebuild_host_claims(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    connection
        .execute_unprepared("DELETE FROM channel_host_target_claims")
        .await?;
    connection
        .execute_unprepared(&format!(
            r#"
            INSERT INTO channel_host_target_claims (target_id, tenant_id, target_type, value)
            SELECT target.id, channel.tenant_id, target.target_type, target.value
            FROM channel_targets target
            JOIN channels channel ON channel.id = target.channel_id
            WHERE target.target_type = '{HOST_TARGET_TYPE}'
            "#
        ))
        .await?;
    Ok(())
}

async fn verify_host_claim_projection(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let missing_or_mismatched = manager
        .get_connection()
        .query_one(Statement::from_string(
            manager.get_database_backend(),
            format!(
                r#"
                SELECT target.id
                FROM channel_targets target
                JOIN channels channel ON channel.id = target.channel_id
                LEFT JOIN channel_host_target_claims claim
                  ON claim.target_id = target.id
                WHERE target.target_type = '{HOST_TARGET_TYPE}'
                  AND (
                    claim.target_id IS NULL
                    OR claim.tenant_id <> channel.tenant_id
                    OR claim.target_type <> target.target_type
                    OR claim.value <> target.value
                  )
                LIMIT 1
                "#
            ),
        ))
        .await?;
    if let Some(row) = missing_or_mismatched {
        let target_id: Uuid = row.try_get("", "id")?;
        return Err(DbErr::Custom(format!(
            "channel host claim projection is missing or stale for target {target_id}"
        )));
    }

    let extra = manager
        .get_connection()
        .query_one(Statement::from_string(
            manager.get_database_backend(),
            format!(
                r#"
                SELECT claim.target_id
                FROM channel_host_target_claims claim
                LEFT JOIN channel_targets target ON target.id = claim.target_id
                LEFT JOIN channels channel ON channel.id = target.channel_id
                WHERE target.id IS NULL
                   OR target.target_type <> '{HOST_TARGET_TYPE}'
                   OR claim.tenant_id <> channel.tenant_id
                   OR claim.target_type <> target.target_type
                   OR claim.value <> target.value
                LIMIT 1
                "#
            ),
        ))
        .await?;
    if let Some(row) = extra {
        let target_id: Uuid = row.try_get("", "target_id")?;
        return Err(DbErr::Custom(format!(
            "channel host claim projection contains stale target {target_id}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};

    async fn sqlite_selection_schema() -> sea_orm::DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared(
            r#"
            PRAGMA foreign_keys = ON;
            CREATE TABLE channels (id TEXT PRIMARY KEY NOT NULL, tenant_id TEXT NOT NULL);
            CREATE TABLE channel_targets (
                id TEXT PRIMARY KEY NOT NULL,
                channel_id TEXT NOT NULL,
                target_type TEXT NOT NULL,
                value TEXT NOT NULL,
                is_primary INTEGER NOT NULL,
                FOREIGN KEY (channel_id) REFERENCES channels(id) ON DELETE CASCADE
            );
            CREATE TABLE channel_host_target_claims (
                target_id TEXT PRIMARY KEY NOT NULL,
                tenant_id TEXT NOT NULL,
                target_type TEXT NOT NULL,
                value TEXT NOT NULL,
                FOREIGN KEY (target_id) REFERENCES channel_targets(id) ON DELETE CASCADE
            );
            CREATE UNIQUE INDEX uq_channel_host_target_claims_tenant_value
                ON channel_host_target_claims (tenant_id, target_type, value);
            "#,
        )
        .await
        .unwrap();
        db
    }

    #[tokio::test]
    async fn sqlite_seal_rebuilds_missing_host_claims_atomically() {
        let db = sqlite_selection_schema().await;
        let tenant_id = Uuid::new_v4();
        let channel_id = Uuid::new_v4();
        let target_id = Uuid::new_v4();
        db.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO channels (id, tenant_id) VALUES (?1, ?2)",
            vec![channel_id.into(), tenant_id.into()],
        ))
        .await
        .unwrap();
        db.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO channel_targets (id, channel_id, target_type, value, is_primary) VALUES (?1, ?2, ?3, ?4, 1)",
            vec![
                target_id.into(),
                channel_id.into(),
                HOST_TARGET_TYPE.into(),
                "seal.example".into(),
            ],
        ))
        .await
        .unwrap();

        Migration.up(&SchemaManager::new(&db)).await.unwrap();

        let count: i64 = db
            .query_one(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT COUNT(*) AS count FROM channel_host_target_claims WHERE target_id = ?1",
                vec![target_id.into()],
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get("", "count")
            .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn sqlite_seal_rejects_cross_tenant_oauth_binding() {
        let db = sqlite_selection_schema().await;
        db.execute_unprepared(
            r#"
            CREATE TABLE oauth_apps (id TEXT PRIMARY KEY NOT NULL, tenant_id TEXT NOT NULL);
            CREATE TABLE channel_oauth_apps (
                id TEXT PRIMARY KEY NOT NULL,
                channel_id TEXT NOT NULL,
                oauth_app_id TEXT NOT NULL
            );
            "#,
        )
        .await
        .unwrap();
        let channel_tenant = Uuid::new_v4();
        let app_tenant = Uuid::new_v4();
        let channel_id = Uuid::new_v4();
        let app_id = Uuid::new_v4();
        let binding_id = Uuid::new_v4();
        db.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO channels (id, tenant_id) VALUES (?1, ?2)",
            vec![channel_id.into(), channel_tenant.into()],
        ))
        .await
        .unwrap();
        db.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO oauth_apps (id, tenant_id) VALUES (?1, ?2)",
            vec![app_id.into(), app_tenant.into()],
        ))
        .await
        .unwrap();
        db.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO channel_oauth_apps (id, channel_id, oauth_app_id) VALUES (?1, ?2, ?3)",
            vec![binding_id.into(), channel_id.into(), app_id.into()],
        ))
        .await
        .unwrap();

        let error = Migration
            .up(&SchemaManager::new(&db))
            .await
            .expect_err("cross-tenant binding must block the seal");
        assert!(error.to_string().contains(&binding_id.to_string()));
    }
}
