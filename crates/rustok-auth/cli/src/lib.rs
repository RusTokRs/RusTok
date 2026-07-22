//! External operational command adapters for `rustok-auth`.

use std::time::Duration;

use rustok_api::{PortActor, PortContext};
use rustok_auth::{generate_refresh_token, hash_password};
use rustok_cli_core::{
    CliCoreError, CliCoreResult, CommandDescriptor, CommandOutcome, CommandProvider, CommandRequest,
};
use rustok_runtime::{RuntimeComposition, db_clone};
use rustok_tenant::{TenantReadPort, TenantService};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use uuid::Uuid;

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
                "Create an OAuth application for local development or bootstrap operations",
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
        let db = db_clone(self.runtime.require_host().map_err(command_failed)?);
        let tenant_id = resolve_tenant_id(&db, options).await?;
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
            .execute(Statement::from_string(
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
    let client_id = Uuid::new_v4();
    let client_secret = format!(
        "sk_live_{}{}",
        generate_refresh_token(),
        generate_refresh_token()
    );
    let client_secret_hash = hash_password(&client_secret).map_err(command_failed)?;
    let backend = db.get_database_backend();
    let placeholders = match backend {
        DbBackend::Sqlite => (
            "?1", "?2", "?3", "?4", "?5", "?6", "?7", "?8", "?9", "?10", "?11", "?12", "?13",
            "?14", "?15",
        ),
        _ => (
            "$1", "$2", "$3", "$4", "$5", "$6", "$7", "$8", "$9", "$10", "$11", "$12", "$13",
            "$14", "$15",
        ),
    };
    let sql = format!(
        "INSERT INTO oauth_apps (id, tenant_id, name, slug, description, app_type, client_id, client_secret_hash, redirect_uris, scopes, grant_types, granted_permissions, auto_created, is_active, metadata) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
        placeholders.0,
        placeholders.1,
        placeholders.2,
        placeholders.3,
        placeholders.4,
        placeholders.5,
        placeholders.6,
        placeholders.7,
        placeholders.8,
        placeholders.9,
        placeholders.10,
        placeholders.11,
        placeholders.12,
        placeholders.13,
        placeholders.14,
    );
    db.execute(Statement::from_sql_and_values(
        backend,
        sql,
        vec![
            Uuid::new_v4().into(),
            tenant_id.into(),
            name.clone().into(),
            slug.into(),
            Some("Created via rustok-cli oauth create-app".to_string()).into(),
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
    Ok(CreatedOAuthApp {
        name,
        client_id,
        client_secret,
    })
}

async fn resolve_tenant_id(
    db: &DatabaseConnection,
    options: &serde_json::Map<String, serde_json::Value>,
) -> CliCoreResult<Uuid> {
    if let Some(raw) = option(options, "tenant_id") {
        return Uuid::parse_str(raw).map_err(|_| CliCoreError::InvalidInput {
            message: "--tenant-id must be a UUID".to_string(),
        });
    }
    TenantService::new(db.clone())
        .read_default_active_tenant(
            PortContext::new("platform", PortActor::system(), "en", "oauth-create-app")
                .with_deadline(Duration::from_secs(5)),
        )
        .await
        .map(|tenant| tenant.id)
        .map_err(|error| command_failed(error.message))
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
