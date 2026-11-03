use async_trait::async_trait;
use sea_orm::{ConnectionTrait, DatabaseConnection, DatabaseTransaction, Statement};
use uuid::Uuid;

use crate::backend::SupportedBackend;
use rustok_api::{
    PortError, SharedStaticModuleSettingsReader, StaticModuleSettingsReader,
    StaticModuleSettingsSnapshot, StaticModuleSettingsTransactionReader,
};

/// Database-backed owner implementation for static/native tenant-module settings.
///
/// All reads stay inside the module control-plane crate. Consumer modules receive
/// only the typed runtime port and therefore cannot bypass lifecycle ownership.
/// SQLite recognizes both native UUID blobs and canonical UUID text. If both
/// representations contain rows for the same tenant, the read fails closed.
#[derive(Clone)]
pub struct DatabaseStaticModuleSettingsReader {
    db: DatabaseConnection,
}

impl DatabaseStaticModuleSettingsReader {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub fn shared(db: DatabaseConnection) -> SharedStaticModuleSettingsReader {
        SharedStaticModuleSettingsReader(std::sync::Arc::new(Self::new(db)))
    }
}

#[async_trait]
impl StaticModuleSettingsReader for DatabaseStaticModuleSettingsReader {
    async fn settings(
        &self,
        tenant_id: Uuid,
        module_slug: &str,
    ) -> Result<Option<StaticModuleSettingsSnapshot>, PortError> {
        let backend = SupportedBackend::new(self.db.get_database_backend()).map_err(|_| {
            PortError::unavailable(
                "modules.static_settings_unavailable",
                "Static module settings are temporarily unavailable",
            )
        })?;
        let tenant_keys = crate::backend::sqlite_uuid_keys("?1", "?3");
        let query = match backend {
            SupportedBackend::Sqlite => {
                format!("WITH identity AS (SELECT COUNT(*) AS parent_count FROM tenant_identity_keys WHERE id COLLATE NOCASE IN ({tenant_keys})) \
                 SELECT t.module_slug AS matched_module, t.enabled, CAST(t.settings AS TEXT) AS settings_json, \
                 identity.parent_count, \
                 (t.tenant_id = (SELECT id FROM tenant_identity_keys WHERE id COLLATE NOCASE IN ({tenant_keys}) LIMIT 1)) AS matches_parent, \
                 (EXISTS(SELECT 1 FROM tenant_modules WHERE tenant_id COLLATE NOCASE = ?1) AND \
                  EXISTS(SELECT 1 FROM tenant_modules WHERE tenant_id COLLATE NOCASE = ?3)) AS mixed_identity, \
                 EXISTS(SELECT 1 FROM tenant_modules orphan WHERE orphan.tenant_id COLLATE NOCASE IN ({tenant_keys}) AND \
                  NOT EXISTS(SELECT 1 FROM tenant_identity_keys parent WHERE parent.id = orphan.tenant_id)) AS orphaned_identity \
                 FROM identity LEFT JOIN tenant_modules t \
                 ON t.tenant_id COLLATE NOCASE IN ({tenant_keys}) AND t.module_slug = ?2 LIMIT 2")
            }
            SupportedBackend::Postgres => {
                "SELECT enabled, settings::text AS settings_json FROM tenant_modules WHERE tenant_id = $1 AND module_slug = $2 LIMIT 2".to_string()
            }
        };

        let mut values = vec![backend.uuid_value(tenant_id), module_slug.into()];
        if matches!(backend, SupportedBackend::Sqlite) {
            values.push(tenant_id.into());
        }
        let mut rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                query,
                values,
            ))
            .await
            .map_err(|_| {
                PortError::unavailable(
                    "modules.static_settings_unavailable",
                    "Static module settings are temporarily unavailable",
                )
            })?;

        if rows.len() > 1 {
            return Err(PortError::invariant_violation(
                "modules.static_settings_ambiguous",
                "Static module settings have conflicting tenant identities",
            ));
        }
        let Some(row) = rows.pop() else {
            return Ok(None);
        };
        if matches!(backend, SupportedBackend::Sqlite)
            && !crate::backend::sqlite_tenant_module_identity(&row).map_err(|_| {
                PortError::invariant_violation(
                    "modules.static_settings_ambiguous",
                    "Static module settings have conflicting tenant identities",
                )
            })?
        {
            return Ok(None);
        }

        let enabled: Option<bool> = row.try_get("", "enabled").map_err(|_| {
            PortError::invariant_violation(
                "modules.static_settings_corrupt",
                "Static module lifecycle state is invalid",
            )
        })?;
        let encoded: String = row.try_get("", "settings_json").map_err(|_| {
            PortError::invariant_violation(
                "modules.static_settings_corrupt",
                "Static module settings state is invalid",
            )
        })?;
        let settings: serde_json::Value = serde_json::from_str(&encoded).map_err(|_| {
            PortError::invariant_violation(
                "modules.static_settings_corrupt",
                "Static module settings JSON is invalid",
            )
        })?;
        if !settings.is_object() {
            return Err(PortError::invariant_violation(
                "modules.static_settings_corrupt",
                "Static module settings must be a JSON object",
            ));
        }

        Ok(Some(StaticModuleSettingsSnapshot { enabled, settings }))
    }
}

#[async_trait]
impl StaticModuleSettingsTransactionReader for DatabaseStaticModuleSettingsReader {
    async fn settings_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        module_slug: &str,
    ) -> Result<Option<StaticModuleSettingsSnapshot>, PortError> {
        let backend = SupportedBackend::new(txn.get_database_backend()).map_err(|_| {
            PortError::unavailable(
                "modules.static_settings_unavailable",
                "Static module settings are temporarily unavailable",
            )
        })?;
        let tenant_keys = crate::backend::sqlite_uuid_keys("?1", "?3");
        let query = match backend {
            SupportedBackend::Sqlite => {
                format!(
                    "WITH identity AS (SELECT COUNT(*) AS parent_count FROM tenant_identity_keys WHERE id COLLATE NOCASE IN ({tenant_keys})) \
                 SELECT t.module_slug AS matched_module, t.enabled, CAST(t.settings AS TEXT) AS settings_json, \
                 identity.parent_count, \
                 (t.tenant_id = (SELECT id FROM tenant_identity_keys WHERE id COLLATE NOCASE IN ({tenant_keys}) LIMIT 1)) AS matches_parent, \
                 (EXISTS(SELECT 1 FROM tenant_modules WHERE tenant_id COLLATE NOCASE = ?1) AND \
                  EXISTS(SELECT 1 FROM tenant_modules WHERE tenant_id COLLATE NOCASE = ?3)) AS mixed_identity, \
                 EXISTS(SELECT 1 FROM tenant_modules orphan WHERE orphan.tenant_id COLLATE NOCASE IN ({tenant_keys}) AND \
                  NOT EXISTS(SELECT 1 FROM tenant_identity_keys parent WHERE parent.id = orphan.tenant_id)) AS orphaned_identity \
                 FROM identity LEFT JOIN tenant_modules t \
                 ON t.tenant_id COLLATE NOCASE IN ({tenant_keys}) AND t.module_slug = ?2 LIMIT 2"
                )
            }
            SupportedBackend::Postgres => "SELECT enabled, settings::text AS settings_json \
                 FROM tenant_modules WHERE tenant_id = $1 AND module_slug = $2 LIMIT 2 FOR SHARE"
                .to_string(),
        };

        let mut values = vec![backend.uuid_value(tenant_id), module_slug.into()];
        if matches!(backend, SupportedBackend::Sqlite) {
            values.push(tenant_id.into());
        }
        let mut rows = txn
            .query_all_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                query,
                values,
            ))
            .await
            .map_err(|_| {
                PortError::unavailable(
                    "modules.static_settings_unavailable",
                    "Static module settings are temporarily unavailable",
                )
            })?;

        if rows.len() > 1 {
            return Err(PortError::invariant_violation(
                "modules.static_settings_ambiguous",
                "Static module settings have conflicting tenant identities",
            ));
        }
        let Some(row) = rows.pop() else {
            return Ok(None);
        };
        if matches!(backend, SupportedBackend::Sqlite)
            && !crate::backend::sqlite_tenant_module_identity(&row).map_err(|_| {
                PortError::invariant_violation(
                    "modules.static_settings_ambiguous",
                    "Static module settings have conflicting tenant identities",
                )
            })?
        {
            return Ok(None);
        }

        let enabled: Option<bool> = row.try_get("", "enabled").map_err(|_| {
            PortError::invariant_violation(
                "modules.static_settings_corrupt",
                "Static module lifecycle state is invalid",
            )
        })?;
        let encoded: String = row.try_get("", "settings_json").map_err(|_| {
            PortError::invariant_violation(
                "modules.static_settings_corrupt",
                "Static module settings state is invalid",
            )
        })?;
        let settings: serde_json::Value = serde_json::from_str(&encoded).map_err(|_| {
            PortError::invariant_violation(
                "modules.static_settings_corrupt",
                "Static module settings JSON is invalid",
            )
        })?;
        if !settings.is_object() {
            return Err(PortError::invariant_violation(
                "modules.static_settings_corrupt",
                "Static module settings must be a JSON object",
            ));
        }

        Ok(Some(StaticModuleSettingsSnapshot { enabled, settings }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectionTrait, Database};

    async fn db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        db.execute_unprepared("CREATE TABLE tenants (id BLOB PRIMARY KEY NOT NULL); CREATE VIEW tenant_identity_keys AS SELECT id FROM tenants")
            .await
            .expect("tenant schema");
        db.execute_unprepared(
            "CREATE TABLE tenant_modules (
                tenant_id TEXT NOT NULL,
                module_slug TEXT NOT NULL,
                enabled BOOLEAN NULL,
                settings TEXT NOT NULL,
                PRIMARY KEY (tenant_id, module_slug)
            )",
        )
        .await
        .expect("schema");
        db.execute_unprepared(
            "CREATE TRIGGER fixture_tenant AFTER INSERT ON tenant_modules BEGIN \
             INSERT OR IGNORE INTO tenants (id) VALUES (NEW.tenant_id); END",
        )
        .await
        .expect("tenant fixture trigger");
        db
    }

    #[tokio::test]
    async fn reader_returns_settings_and_enabled_state_for_exact_tenant_module() {
        let db = db().await;
        let tenant_id = Uuid::new_v4();
        let foreign_tenant_id = Uuid::new_v4();
        db.execute_unprepared(&format!(
            r#"INSERT INTO tenant_modules (tenant_id, module_slug, enabled, settings) VALUES
            ('{tenant_id}', 'seo', 0, '{{"sitemap_enabled":false}}'),
            ('{foreign_tenant_id}', 'seo', 1, '{{"sitemap_enabled":true}}')"#
        ))
        .await
        .expect("rows");

        let reader = DatabaseStaticModuleSettingsReader::new(db);
        let snapshot = reader
            .settings(tenant_id, "seo")
            .await
            .expect("settings read")
            .expect("exact row");

        assert_eq!(snapshot.enabled, Some(false));
        assert_eq!(snapshot.settings["sitemap_enabled"], false);

        let foreign = reader
            .settings(foreign_tenant_id, "seo")
            .await
            .expect("foreign settings read")
            .expect("foreign exact row");
        assert_eq!(foreign.enabled, Some(true));
        assert_eq!(foreign.settings["sitemap_enabled"], true);
    }

    #[tokio::test]
    async fn reader_returns_none_for_missing_module_row() {
        let reader = DatabaseStaticModuleSettingsReader::new(db().await);
        assert!(
            reader
                .settings(Uuid::new_v4(), "missing")
                .await
                .expect("missing read")
                .is_none()
        );
    }

    #[tokio::test]
    async fn readers_handle_uuid_case_and_reject_orphans_and_duplicate_parents() {
        use sea_orm::TransactionTrait;

        let db = db().await;
        let tenant_id = Uuid::parse_str("abcdefab-cdef-4abc-8def-abcdefabcdef").unwrap();
        let uppercase = tenant_id.to_string().to_uppercase();
        db.execute_raw(Statement::from_sql_and_values(
            sea_orm::DbBackend::Sqlite,
            "INSERT INTO tenant_modules (tenant_id, module_slug, enabled, settings) VALUES (?1, 'seo', 0, '{}')",
            vec![uppercase.clone().into()],
        )).await.unwrap();
        let reader = DatabaseStaticModuleSettingsReader::new(db.clone());
        assert_eq!(
            reader
                .settings(tenant_id, "seo")
                .await
                .unwrap()
                .unwrap()
                .enabled,
            Some(false)
        );
        let txn = db.begin().await.unwrap();
        assert_eq!(
            reader
                .settings_in_tx(&txn, tenant_id, "seo")
                .await
                .unwrap()
                .unwrap()
                .enabled,
            Some(false)
        );
        txn.rollback().await.unwrap();

        db.execute_raw(Statement::from_sql_and_values(
            sea_orm::DbBackend::Sqlite,
            "INSERT INTO tenants (id) VALUES (?1)",
            vec![tenant_id.into()],
        ))
        .await
        .unwrap();
        assert_eq!(
            reader.settings(tenant_id, "seo").await.unwrap_err().code,
            "modules.static_settings_ambiguous"
        );
        db.execute_raw(Statement::from_sql_and_values(
            sea_orm::DbBackend::Sqlite,
            "DELETE FROM tenants WHERE id = ?1",
            vec![tenant_id.into()],
        ))
        .await
        .unwrap();

        db.execute_unprepared("DROP TRIGGER fixture_tenant")
            .await
            .unwrap();
        db.execute_raw(Statement::from_sql_and_values(
            sea_orm::DbBackend::Sqlite,
            "INSERT INTO tenant_modules (tenant_id, module_slug, enabled, settings) VALUES (?1, 'other', 1, '{}')",
            vec![tenant_id.to_string().into()],
        )).await.unwrap();
        assert_eq!(
            reader.settings(tenant_id, "seo").await.unwrap_err().code,
            "modules.static_settings_ambiguous"
        );
        assert_eq!(
            reader
                .settings(tenant_id, "missing")
                .await
                .unwrap_err()
                .code,
            "modules.static_settings_ambiguous"
        );
    }

    #[tokio::test]
    async fn readers_find_all_supported_text_uuid_forms() {
        use sea_orm::TransactionTrait;

        let tenant_id = Uuid::parse_str("abcdefab-cdef-4abc-8def-abcdefabcdef").unwrap();
        for stored in [
            tenant_id.simple().to_string(),
            format!("URN:UUID:{tenant_id}"),
            format!("{{{tenant_id}}}"),
        ] {
            let db = db().await;
            db.execute_raw(Statement::from_sql_and_values(
                sea_orm::DbBackend::Sqlite,
                "INSERT INTO tenant_modules (tenant_id, module_slug, enabled, settings) VALUES (?1, 'seo', 0, '{}')",
                vec![stored.into()],
            )).await.unwrap();
            let reader = DatabaseStaticModuleSettingsReader::new(db.clone());
            assert_eq!(
                reader
                    .settings(tenant_id, "seo")
                    .await
                    .unwrap()
                    .unwrap()
                    .enabled,
                Some(false)
            );
            let txn = db.begin().await.unwrap();
            assert_eq!(
                reader
                    .settings_in_tx(&txn, tenant_id, "seo")
                    .await
                    .unwrap()
                    .unwrap()
                    .enabled,
                Some(false)
            );
            txn.rollback().await.unwrap();
            db.execute_raw(Statement::from_sql_and_values(
                sea_orm::DbBackend::Sqlite,
                "INSERT INTO tenants (id) VALUES (?1)",
                vec![tenant_id.into()],
            ))
            .await
            .unwrap();
            assert_eq!(
                reader.settings(tenant_id, "seo").await.unwrap_err().code,
                "modules.static_settings_ambiguous"
            );
        }
    }

    #[tokio::test]
    async fn readers_accept_native_uuid_and_reject_conflicting_representations() {
        use sea_orm::TransactionTrait;

        let db = db().await;
        let tenant_id = Uuid::new_v4();
        db.execute_raw(Statement::from_sql_and_values(
            sea_orm::DbBackend::Sqlite,
            "INSERT INTO tenant_modules (tenant_id, module_slug, enabled, settings) VALUES (?1, 'seo', 1, ?2)",
            vec![tenant_id.into(), r#"{"value":"native"}"#.into()],
        ))
        .await
        .expect("native UUID row");
        let reader = DatabaseStaticModuleSettingsReader::new(db.clone());
        assert_eq!(
            reader
                .settings(tenant_id, "seo")
                .await
                .unwrap()
                .unwrap()
                .settings["value"],
            "native"
        );
        let txn = db.begin().await.unwrap();
        assert_eq!(
            reader
                .settings_in_tx(&txn, tenant_id, "seo")
                .await
                .unwrap()
                .unwrap()
                .settings["value"],
            "native"
        );
        txn.rollback().await.unwrap();

        db.execute_raw(Statement::from_sql_and_values(
            sea_orm::DbBackend::Sqlite,
            "INSERT INTO tenants (id) VALUES (?1)",
            vec![tenant_id.to_string().into()],
        ))
        .await
        .unwrap();
        assert_eq!(
            reader.settings(tenant_id, "seo").await.unwrap_err().code,
            "modules.static_settings_ambiguous"
        );
        assert_eq!(
            reader
                .settings(tenant_id, "missing")
                .await
                .unwrap_err()
                .code,
            "modules.static_settings_ambiguous"
        );
        let txn = db.begin().await.unwrap();
        assert_eq!(
            reader
                .settings_in_tx(&txn, tenant_id, "seo")
                .await
                .unwrap_err()
                .code,
            "modules.static_settings_ambiguous"
        );
        txn.rollback().await.unwrap();
        db.execute_raw(Statement::from_sql_and_values(
            sea_orm::DbBackend::Sqlite,
            "DELETE FROM tenants WHERE id = ?1",
            vec![tenant_id.to_string().into()],
        ))
        .await
        .unwrap();

        db.execute_raw(Statement::from_sql_and_values(
            sea_orm::DbBackend::Sqlite,
            "INSERT INTO tenant_modules (tenant_id, module_slug, enabled, settings) VALUES (?1, 'seo', 0, ?2)",
            vec![tenant_id.to_string().into(), r#"{"value":"text"}"#.into()],
        ))
        .await
        .expect("conflicting text UUID row");
        let error = reader.settings(tenant_id, "seo").await.unwrap_err();
        assert_eq!(error.kind, rustok_api::PortErrorKind::InvariantViolation);
        assert_eq!(error.code, "modules.static_settings_ambiguous");
        let txn = db.begin().await.unwrap();
        let error = reader
            .settings_in_tx(&txn, tenant_id, "seo")
            .await
            .unwrap_err();
        assert_eq!(error.code, "modules.static_settings_ambiguous");
        txn.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn transactional_reader_returns_disabled_state_and_exact_tenant_scope() {
        use sea_orm::TransactionTrait;

        let db = db().await;
        let tenant_id = Uuid::new_v4();
        let foreign_tenant_id = Uuid::new_v4();

        db.execute_unprepared(&format!(
            r#"INSERT INTO tenant_modules (tenant_id, module_slug, enabled, settings) VALUES
            ('{tenant_id}', 'forum', 0, '{{"use_reactions":true}}'),
            ('{foreign_tenant_id}', 'forum', 1, '{{"use_reactions":false}}')"#
        ))
        .await
        .expect("rows");

        let reader = DatabaseStaticModuleSettingsReader::new(db.clone());
        let txn = db.begin().await.expect("transaction");

        let snapshot = reader
            .settings_in_tx(&txn, tenant_id, "forum")
            .await
            .expect("transaction settings read")
            .expect("exact disabled row");
        assert_eq!(snapshot.enabled, Some(false));
        assert_eq!(snapshot.settings["use_reactions"], true);

        let foreign = reader
            .settings_in_tx(&txn, foreign_tenant_id, "forum")
            .await
            .expect("foreign transaction settings read")
            .expect("foreign exact row");
        assert_eq!(foreign.enabled, Some(true));
        assert_eq!(foreign.settings["use_reactions"], false);

        txn.rollback().await.expect("rollback");
    }

    #[tokio::test]
    async fn reader_rejects_corrupt_settings_as_invariant_failure() {
        let db = db().await;
        let tenant_id = Uuid::new_v4();
        db.execute_unprepared(&format!(
            "INSERT INTO tenant_modules (tenant_id, module_slug, enabled, settings) VALUES ('{tenant_id}', 'seo', 1, '[]')"
        ))
        .await
        .expect("row");

        let reader = DatabaseStaticModuleSettingsReader::new(db);
        let error = reader
            .settings(tenant_id, "seo")
            .await
            .expect_err("corrupt settings must fail closed");
        assert_eq!(error.kind, rustok_api::PortErrorKind::InvariantViolation);
    }
}
