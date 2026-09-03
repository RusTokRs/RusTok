//! External operational command adapters for `rustok-auth`.

use rustok_auth::{generate_refresh_token, hash_password};
use rustok_cli_core::{
    CliCoreError, CliCoreResult, CommandDescriptor, CommandOutcome, CommandProvider, CommandRequest,
};
use rustok_runtime::{RuntimeComposition, db_clone};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait};
use uuid::Uuid;

const DEVELOPMENT_APP_LOCALE: &str = "en";
const DEVELOPMENT_APP_DESCRIPTION: &str = "Created via rustok-cli oauth create-app";

pub struct AuthCommandProvider {
    runtime: RuntimeComposition,
}

#[async_trait::async_trait]
impl CommandProvider for AuthCommandProvider {
    fn commands(&self) -> Vec<CommandDescriptor> {
        vec![
            CommandDescriptor::new(
                "oauth",
                "create-app",
                "Create an OAuth application for an explicitly selected tenant",
            ),
            CommandDescriptor::new("auth", "sessions-cleanup", "Remove expired auth sessions"),
        ]
    }

    async fn execute(&self, request: CommandRequest) -> CliCoreResult<CommandOutcome> {
        match (request.namespace.as_str(), request.name.as_str()) {
            ("oauth", "create-app") => self.create_app(request.args).await,
            ("auth", "sessions-cleanup") => self.cleanup_sessions().await,
            _ => Err(CliCoreError::UnknownCommand {
                namespace: request.namespace,
                name: request.name,
            }),
        }
    }
}

impl AuthCommandProvider {
    async fn create_app(&self, args: serde_json::Value) -> CliCoreResult<CommandOutcome> {
        let options = options(&args)?;
        let tenant_id = required_tenant_id(options)?;
        let db = db_clone(self.runtime.require_host().map_err(command_failed)?);
        let name = option(options, "name")
            .unwrap_or("Development App")
            .to_string();
        let slug = option(options, "slug").unwrap_or("dev-app").to_string();
        validate_identity(&name, "--name")?;
        validate_identity(&slug, "--slug")?;
        let created = create_development_app(&db, tenant_id, name, slug).await?;
        Ok(
            CommandOutcome::success("OAuth application created").with_data(serde_json::json!({
                "tenant_id": tenant_id,
                "name": created.name,
                "app_type": "third_party",
                "client_id": created.client_id,
                "client_secret": created.client_secret,
            })),
        )
    }

    async fn cleanup_sessions(&self) -> CliCoreResult<CommandOutcome> {
        let db = db_clone(self.runtime.require_host().map_err(command_failed)?);
        let result = db
            .execute_raw(Statement::from_string(
                db.get_database_backend(),
                "DELETE FROM sessions WHERE expires_at < CURRENT_TIMESTAMP".to_string(),
            ))
            .await
            .map_err(command_failed)?;
        Ok(
            CommandOutcome::success("Expired auth sessions removed").with_data(serde_json::json!({
                "deleted_sessions": result.rows_affected(),
            })),
        )
    }
}

pub fn command_provider(runtime: &RuntimeComposition) -> Box<dyn CommandProvider> {
    Box::new(AuthCommandProvider {
        runtime: runtime.clone(),
    })
}

struct CreatedOAuthApp {
    name: String,
    client_id: Uuid,
    client_secret: String,
}

async fn create_development_app(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    name: String,
    slug: String,
) -> CliCoreResult<CreatedOAuthApp> {
    let app_id = Uuid::new_v4();
    let client_id = Uuid::new_v4();
    let client_secret = format!(
        "sk_live_{}{}",
        generate_refresh_token(),
        generate_refresh_token()
    );
    let client_secret_hash = hash_password(&client_secret).map_err(command_failed)?;
    let backend = db.get_database_backend();
    let transaction = db.begin().await.map_err(command_failed)?;

    let app_sql = match backend {
        DbBackend::Postgres => {
            "INSERT INTO oauth_apps (id, tenant_id, slug, app_type, client_id, client_secret_hash, redirect_uris, scopes, grant_types, granted_permissions, auto_created, is_active, metadata) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)"
        }
        DbBackend::MySql => {
            "INSERT INTO oauth_apps (id, tenant_id, slug, app_type, client_id, client_secret_hash, redirect_uris, scopes, grant_types, granted_permissions, auto_created, is_active, metadata) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        }
        DbBackend::Sqlite => {
            "INSERT INTO oauth_apps (id, tenant_id, slug, app_type, client_id, client_secret_hash, redirect_uris, scopes, grant_types, granted_permissions, auto_created, is_active, metadata) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"
        }
        _ => unreachable!("unsupported SeaORM database backend"),
    };
    transaction
        .execute_raw(Statement::from_sql_and_values(
            backend,
            app_sql,
            vec![
                app_id.into(),
                tenant_id.into(),
                slug.into(),
                "third_party".into(),
                client_id.into(),
                client_secret_hash.into(),
                serde_json::json!([
                    "http://localhost:3000/api/auth/callback",
                    "http://localhost:1420"
                ])
                .into(),
                serde_json::json!(["openid", "profile", "email", "offline_access"]).into(),
                serde_json::json!(["authorization_code", "refresh_token"]).into(),
                serde_json::json!([]).into(),
                false.into(),
                true.into(),
                serde_json::json!({}).into(),
            ],
        ))
        .await
        .map_err(command_failed)?;

    let translation_sql = match backend {
        DbBackend::Postgres => {
            "INSERT INTO oauth_app_translations (id, tenant_id, app_id, locale, name, description) VALUES ($1, $2, $3, $4, $5, $6)"
        }
        DbBackend::MySql => {
            "INSERT INTO oauth_app_translations (id, tenant_id, app_id, locale, name, description) VALUES (?, ?, ?, ?, ?, ?)"
        }
        DbBackend::Sqlite => {
            "INSERT INTO oauth_app_translations (id, tenant_id, app_id, locale, name, description) VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
        }
        _ => unreachable!("unsupported SeaORM database backend"),
    };
    transaction
        .execute_raw(Statement::from_sql_and_values(
            backend,
            translation_sql,
            vec![
                Uuid::new_v4().into(),
                tenant_id.into(),
                app_id.into(),
                DEVELOPMENT_APP_LOCALE.into(),
                name.clone().into(),
                Some(DEVELOPMENT_APP_DESCRIPTION.to_string()).into(),
            ],
        ))
        .await
        .map_err(command_failed)?;

    transaction.commit().await.map_err(command_failed)?;
    Ok(CreatedOAuthApp {
        name,
        client_id,
        client_secret,
    })
}

fn required_tenant_id(options: &serde_json::Map<String, serde_json::Value>) -> CliCoreResult<Uuid> {
    let raw = option(options, "tenant_id").ok_or_else(|| CliCoreError::InvalidInput {
        message: "--tenant-id is required for oauth create-app".to_string(),
    })?;
    Uuid::parse_str(raw).map_err(|_| CliCoreError::InvalidInput {
        message: "--tenant-id must be a UUID".to_string(),
    })
}

fn options(args: &serde_json::Value) -> CliCoreResult<&serde_json::Map<String, serde_json::Value>> {
    args.get("options")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| CliCoreError::InvalidInput {
            message: "oauth create-app expects normalized command options".to_string(),
        })
}

fn option<'a>(
    options: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Option<&'a str> {
    options.get(key).and_then(serde_json::Value::as_str)
}

fn validate_identity(value: &str, flag: &str) -> CliCoreResult<()> {
    if value.trim().is_empty() {
        Err(CliCoreError::InvalidInput {
            message: format!("{flag} must not be empty"),
        })
    } else {
        Ok(())
    }
}

fn command_failed(error: impl std::fmt::Display) -> CliCoreError {
    CliCoreError::CommandFailed {
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{DEVELOPMENT_APP_LOCALE, create_development_app, required_tenant_id};
    use rustok_cli_core::CliCoreError;
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
    use uuid::Uuid;

    #[test]
    fn oauth_create_app_requires_explicit_tenant_id() {
        let options = serde_json::Map::new();
        assert!(matches!(
            required_tenant_id(&options),
            Err(CliCoreError::InvalidInput { message })
                if message == "--tenant-id is required for oauth create-app"
        ));
    }

    #[test]
    fn oauth_create_app_rejects_invalid_tenant_id() {
        let mut options = serde_json::Map::new();
        options.insert("tenant_id".to_string(), serde_json::json!("not-a-uuid"));
        assert!(matches!(
            required_tenant_id(&options),
            Err(CliCoreError::InvalidInput { message })
                if message == "--tenant-id must be a UUID"
        ));
    }

    #[test]
    fn oauth_create_app_accepts_explicit_tenant_id() {
        let tenant_id = Uuid::new_v4();
        let mut options = serde_json::Map::new();
        options.insert(
            "tenant_id".to_string(),
            serde_json::json!(tenant_id.to_string()),
        );
        assert_eq!(required_tenant_id(&options).expect("tenant id"), tenant_id);
    }

    #[tokio::test]
    async fn development_app_uses_current_translation_schema() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("SQLite database");
        db.execute_unprepared(
            r#"CREATE TABLE oauth_apps (
                id TEXT PRIMARY KEY NOT NULL,
                tenant_id TEXT NOT NULL,
                slug TEXT NOT NULL,
                app_type TEXT NOT NULL,
                client_id TEXT NOT NULL,
                client_secret_hash TEXT NULL,
                redirect_uris TEXT NOT NULL,
                scopes TEXT NOT NULL,
                grant_types TEXT NOT NULL,
                granted_permissions TEXT NOT NULL,
                auto_created INTEGER NOT NULL,
                is_active INTEGER NOT NULL,
                metadata TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE oauth_app_translations (
                id TEXT PRIMARY KEY NOT NULL,
                tenant_id TEXT NOT NULL,
                app_id TEXT NOT NULL,
                locale TEXT NOT NULL,
                name TEXT NOT NULL,
                description TEXT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                UNIQUE (tenant_id, app_id, locale)
            );"#,
        )
        .await
        .expect("current OAuth schema");

        let tenant_id = Uuid::new_v4();
        let created = create_development_app(
            &db,
            tenant_id,
            "Development App".to_string(),
            "dev-app".to_string(),
        )
        .await
        .expect("development app");

        let row = db
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT a.slug, t.locale, t.name FROM oauth_apps a JOIN oauth_app_translations t ON t.tenant_id = a.tenant_id AND t.app_id = a.id WHERE a.tenant_id = ?1 AND a.client_id = ?2",
                vec![tenant_id.into(), created.client_id.into()],
            ))
            .await
            .expect("OAuth app query")
            .expect("OAuth app row");
        assert_eq!(row.try_get::<String>("", "slug").expect("slug"), "dev-app");
        assert_eq!(
            row.try_get::<String>("", "locale").expect("locale"),
            DEVELOPMENT_APP_LOCALE
        );
        assert_eq!(
            row.try_get::<String>("", "name").expect("name"),
            "Development App"
        );
    }
}
