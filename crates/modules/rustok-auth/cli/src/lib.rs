//! # RusToK Auth CLI
//!
//! Operational command adapters for the RusToK authentication module.
//!
//! Exposes administrative CLI commands for OAuth application registration,
//! credential management, and expired session cleanup.

#[cfg(test)]
mod contract_tests;

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
            CommandDescriptor::new(
                "auth",
                "sessions-cleanup",
                "Remove auth sessions past the retention window",
            ),
            CommandDescriptor::new(
                "auth",
                "jwt-authority-init",
                "Import the offline JWT ledger into a new independent authority",
            ),
            CommandDescriptor::new(
                "auth",
                "jwt-witness-init",
                "Initialize an independent JWT rollback witness offline",
            ),
            CommandDescriptor::new(
                "auth",
                "jwt-witness-recover",
                "Restore the exact authenticated JWT witness snapshot offline",
            ),
            CommandDescriptor::new(
                "auth",
                "jwt-revoke",
                "Irreversibly revoke an enrolled JWT key across the deployment",
            ),
        ]
    }

    async fn execute(&self, request: CommandRequest) -> CliCoreResult<CommandOutcome> {
        match (request.namespace.as_str(), request.name.as_str()) {
            ("oauth", "create-app") => self.create_app(request.args).await,
            ("auth", "sessions-cleanup") if request.dry_run => {
                Err(CliCoreError::DryRunNotSupported {
                    namespace: request.namespace,
                    name: request.name,
                })
            }
            ("auth", "sessions-cleanup") => self.cleanup_sessions().await,
            (
                "auth",
                "jwt-revoke" | "jwt-authority-init" | "jwt-witness-init" | "jwt-witness-recover",
            ) if request.dry_run => Err(CliCoreError::DryRunNotSupported {
                namespace: request.namespace,
                name: request.name,
            }),
            ("auth", "jwt-revoke") => self.revoke_jwt(request.args).await,
            ("auth", "jwt-authority-init") => self.init_jwt_authority(request.args).await,
            ("auth", "jwt-witness-init") => self.manage_jwt_witness(request.args, false).await,
            ("auth", "jwt-witness-recover") => self.manage_jwt_witness(request.args, true).await,
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

    async fn revoke_jwt(&self, args: serde_json::Value) -> CliCoreResult<CommandOutcome> {
        let (purpose, kid) = jwt_revocation_options(&args)?;
        // The stable root is never accepted as a command-line argument or
        // returned in output. The Auth owner authenticates it against the ledger.
        let root = std::env::var("RUSTOK_JWT_REVOCATION_ROOT").map_err(|_| {
            CliCoreError::InvalidInput {
                message: "RUSTOK_JWT_REVOCATION_ROOT is required".into(),
            }
        })?;
        let authority = required_jwt_authority()?;
        let receipt = authority
            .revoke_key(&root, purpose, kid)
            .await
            .map_err(command_failed)?;
        let data = serde_json::to_value(receipt).map_err(command_failed)?;
        Ok(CommandOutcome::success("JWT key revocation recorded").with_data(data))
    }

    async fn init_jwt_authority(&self, args: serde_json::Value) -> CliCoreResult<CommandOutcome> {
        validate_authority_init_options(&args)?;
        let root = std::env::var("RUSTOK_JWT_REVOCATION_ROOT").map_err(|_| {
            CliCoreError::InvalidInput {
                message: "RUSTOK_JWT_REVOCATION_ROOT is required".into(),
            }
        })?;
        let authority = required_jwt_authority()?;
        let source = db_clone(self.runtime.require_host().map_err(command_failed)?);
        authority
            .import_offline(&source, &root)
            .await
            .map_err(command_failed)?;
        Ok(CommandOutcome::success(
            "Independent JWT authority initialized",
        ))
    }

    async fn manage_jwt_witness(
        &self,
        args: serde_json::Value,
        recover: bool,
    ) -> CliCoreResult<CommandOutcome> {
        validate_authority_init_options(&args)?;
        let root = std::env::var("RUSTOK_JWT_REVOCATION_ROOT").map_err(|_| {
            CliCoreError::InvalidInput {
                message: "RUSTOK_JWT_REVOCATION_ROOT is required".into(),
            }
        })?;
        let authority = required_jwt_authority()?;
        if recover {
            authority
                .recover_witness(&root)
                .await
                .map_err(command_failed)?;
            Ok(CommandOutcome::success(
                "JWT authority restored from the authenticated witness",
            ))
        } else {
            authority
                .init_witness(&root)
                .await
                .map_err(command_failed)?;
            Ok(CommandOutcome::success(
                "Independent JWT rollback witness initialized",
            ))
        }
    }

    async fn cleanup_sessions(&self) -> CliCoreResult<CommandOutcome> {
        let db = db_clone(self.runtime.require_host().map_err(command_failed)?);
        let database_uri = std::env::var("RUSTOK_DATABASE_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                std::env::var("DATABASE_URL")
                    .ok()
                    .filter(|value| !value.trim().is_empty())
            })
            .unwrap_or_default();
        rustok_auth::application_clock::configure_late_observation_store(&db, &database_uri)
            .await
            .map_err(command_failed)?;
        let deleted = rustok_auth::retention::purge_expired_sessions(&db)
            .await
            .map_err(command_failed)?;
        Ok(
            CommandOutcome::success("Auth sessions past retention removed").with_data(
                serde_json::json!({
                    "deleted_sessions": deleted,
                }),
            ),
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
        generate_refresh_token().map_err(command_failed)?,
        generate_refresh_token().map_err(command_failed)?
    );
    let client_secret_hash = hash_password(&client_secret).map_err(command_failed)?;
    let backend = db.get_database_backend();
    let transaction = db.begin().await.map_err(command_failed)?;

    let app_sql = match backend {
        DbBackend::MySql => {
            "INSERT INTO oauth_apps (id, tenant_id, slug, app_type, client_id, client_secret_hash, redirect_uris, scopes, grant_types, granted_permissions, auto_created, is_active, metadata) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        }
        DbBackend::Sqlite => {
            "INSERT INTO oauth_apps (id, tenant_id, slug, app_type, client_id, client_secret_hash, redirect_uris, scopes, grant_types, granted_permissions, auto_created, is_active, metadata) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"
        }
        _ => {
            "INSERT INTO oauth_apps (id, tenant_id, slug, app_type, client_id, client_secret_hash, redirect_uris, scopes, grant_types, granted_permissions, auto_created, is_active, metadata) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)"
        }
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
        DbBackend::MySql => {
            "INSERT INTO oauth_app_translations (id, tenant_id, app_id, locale, name, description) VALUES (?, ?, ?, ?, ?, ?)"
        }
        DbBackend::Sqlite => {
            "INSERT INTO oauth_app_translations (id, tenant_id, app_id, locale, name, description) VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
        }
        _ => {
            "INSERT INTO oauth_app_translations (id, tenant_id, app_id, locale, name, description) VALUES ($1, $2, $3, $4, $5, $6)"
        }
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
    let tenant_id = Uuid::parse_str(raw).map_err(|_| CliCoreError::InvalidInput {
        message: "--tenant-id must be a UUID".to_string(),
    })?;
    if tenant_id.is_nil() {
        return Err(CliCoreError::InvalidInput {
            message: "--tenant-id must not be nil".to_string(),
        });
    }
    Ok(tenant_id)
}

fn required_jwt_authority() -> CliCoreResult<rustok_auth::JwtAuthorityConfig> {
    rustok_auth::JwtAuthorityConfig::from_env().map_err(command_failed)?
        .ok_or_else(|| CliCoreError::InvalidInput {
            message: "RUSTOK_JWT_AUTHORITY_DATABASE_URL, RUSTOK_JWT_AUTHORITY_ID and RUSTOK_JWT_WITNESS_DATABASE_URL are required".into(),
        })
}

fn validate_authority_init_options(args: &serde_json::Value) -> CliCoreResult<()> {
    let valid = args
        .get("options")
        .and_then(serde_json::Value::as_object)
        .is_some_and(|options| {
            options.len() == 1
                && options.get("confirm").and_then(serde_json::Value::as_bool) == Some(true)
        })
        && args
            .get("positionals")
            .is_none_or(|value| value.as_array().is_some_and(|p| p.is_empty()));
    if !valid {
        return Err(CliCoreError::InvalidInput {
            message: "JWT authority/witness commands require --confirm and no other arguments"
                .into(),
        });
    }
    Ok(())
}

fn jwt_revocation_options(args: &serde_json::Value) -> CliCoreResult<(&str, &str)> {
    let invalid = || {
        CliCoreError::InvalidInput {
        message: "auth jwt-revoke requires --purpose access|password-reset|email-verification|invite --kid <id> --confirm and no other arguments".into(),
    }
    };
    let opts = args
        .get("options")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(invalid)?;
    if opts.len() != 3
        || opts.get("confirm").and_then(serde_json::Value::as_bool) != Some(true)
        || args
            .get("positionals")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|p| !p.is_empty())
    {
        return Err(invalid());
    }
    let purpose = option(opts, "purpose").ok_or_else(invalid)?;
    let kid = option(opts, "kid").ok_or_else(invalid)?;
    if !matches!(
        purpose,
        "access" | "password-reset" | "email-verification" | "invite"
    ) || kid.is_empty()
        || kid.len() > 64
        || !kid
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(invalid());
    }
    Ok((purpose, kid))
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
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(CliCoreError::InvalidInput {
            message: format!("{flag} must not be empty"),
        })
    } else if trimmed.chars().count() > 255 {
        Err(CliCoreError::InvalidInput {
            message: format!("{flag} must be at most 255 characters"),
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
    use super::{
        DEVELOPMENT_APP_LOCALE, create_development_app, required_tenant_id, validate_identity,
    };
    use rustok_cli_core::CliCoreError;
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
    use uuid::Uuid;

    #[tokio::test]
    async fn direct_provider_cannot_execute_dry_run_revocation() {
        use rustok_cli_core::{CommandProvider, CommandRequest};
        let provider = super::AuthCommandProvider {
            runtime: rustok_runtime::RuntimeComposition::without_database(serde_json::Value::Null),
        };
        let result = provider
            .execute(CommandRequest::new(
                "auth",
                "jwt-revoke",
                serde_json::json!({"options":{"purpose":"access","kid":"key-1","confirm":true}}),
                true,
            ))
            .await;
        assert!(matches!(
            result,
            Err(CliCoreError::DryRunNotSupported { .. })
        ));
    }

    #[test]
    fn authority_import_requires_explicit_confirmation() {
        assert!(
            super::validate_authority_init_options(
                &serde_json::json!({"options":{"confirm":true}})
            )
            .is_ok()
        );
        for args in [
            serde_json::json!({"options":{}}),
            serde_json::json!({"options":{"confirm":"true"}}),
            serde_json::json!({"options":{"confirm":true,"reset":true}}),
            serde_json::json!({"options":{"confirm":true},"positionals":["other"]}),
        ] {
            assert!(super::validate_authority_init_options(&args).is_err());
        }
    }

    #[test]
    fn jwt_revocation_requires_exact_target_and_confirmation() {
        let args = serde_json::json!({"options":{"purpose":"access","kid":"key-1","confirm":true}});
        assert_eq!(
            super::jwt_revocation_options(&args).unwrap(),
            ("access", "key-1")
        );
        for args in [
            serde_json::json!({"options":{"purpose":"access","kid":"key-1"}}),
            serde_json::json!({"options":{"purpose":"access","kid":"key-1","confirm":"true"}}),
            serde_json::json!({"options":{"purpose":"all","kid":"key-1","confirm":true}}),
            serde_json::json!({"options":{"purpose":"access","kid":"*","confirm":true}}),
            serde_json::json!({"options":{"purpose":"access","kid":"key-1","confirm":true,"root":"secret"}}),
        ] {
            assert!(super::jwt_revocation_options(&args).is_err());
        }
    }

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
    fn oauth_create_app_rejects_nil_tenant_id() {
        let mut options = serde_json::Map::new();
        options.insert(
            "tenant_id".to_string(),
            serde_json::json!(Uuid::nil().to_string()),
        );
        assert!(matches!(
            required_tenant_id(&options),
            Err(CliCoreError::InvalidInput { message })
                if message == "--tenant-id must not be nil"
        ));
    }

    #[test]
    fn validate_identity_bounds() {
        assert!(validate_identity("", "--name").is_err());
        assert!(validate_identity("   ", "--name").is_err());
        assert!(validate_identity("valid-name", "--name").is_ok());
        let oversized = "a".repeat(256);
        assert!(validate_identity(&oversized, "--name").is_err());
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
