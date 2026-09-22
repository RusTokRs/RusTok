use async_trait::async_trait;
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use uuid::Uuid;

use rustok_api::{
    PortError, SharedStaticModuleSettingsReader, StaticModuleSettingsReader,
    StaticModuleSettingsSnapshot,
};

/// Database-backed owner implementation for static/native tenant-module settings.
///
/// All reads stay inside the module control-plane crate. Consumer modules receive
/// only the typed runtime port and therefore cannot bypass lifecycle ownership.
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
        let backend = self.db.get_database_backend();
        let query = match backend {
            sea_orm::DbBackend::Sqlite => {
                "SELECT enabled, CAST(settings AS TEXT) AS settings_json                  FROM tenant_modules WHERE tenant_id = ?1 AND module_slug = ?2 LIMIT 1"
            }
            sea_orm::DbBackend::Postgres => {
                "SELECT enabled, settings::text AS settings_json                  FROM tenant_modules WHERE tenant_id = $1 AND module_slug = $2 LIMIT 1"
            }
            sea_orm::DbBackend::MySql => {
                "SELECT enabled, CAST(settings AS CHAR) AS settings_json                  FROM tenant_modules WHERE tenant_id = ? AND module_slug = ? LIMIT 1"
            }
            _ => {
                return Err(PortError::unavailable(
                    "modules.static_settings_unavailable",
                    "Static module settings are unavailable for this database backend",
                ))
            }
        };

        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                query,
                vec![tenant_id.into(), module_slug.into()],
            ))
            .await
            .map_err(|_| {
                PortError::unavailable(
                    "modules.static_settings_unavailable",
                    "Static module settings are temporarily unavailable",
                )
            })?;

        let Some(row) = row else {
            return Ok(None);
        };

        let enabled: bool = row.try_get("", "enabled").map_err(|_| {
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
        let settings = serde_json::from_str(&encoded).map_err(|_| {
            PortError::invariant_violation(
                "modules.static_settings_corrupt",
                "Static module settings JSON is invalid",
            )
        })?;

        Ok(Some(StaticModuleSettingsSnapshot { enabled, settings }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectionTrait, Database};

    async fn db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.expect("database");
        db.execute_unprepared(
            "CREATE TABLE tenant_modules (
                tenant_id TEXT NOT NULL,
                module_slug TEXT NOT NULL,
                enabled BOOLEAN NOT NULL,
                settings TEXT NOT NULL,
                PRIMARY KEY (tenant_id, module_slug)
            )",
        )
        .await
        .expect("schema");
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

        assert!(!snapshot.enabled);
        assert_eq!(snapshot.settings["sitemap_enabled"], false);

        let foreign = reader
            .settings(foreign_tenant_id, "seo")
            .await
            .expect("foreign settings read")
            .expect("foreign exact row");
        assert!(foreign.enabled);
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
