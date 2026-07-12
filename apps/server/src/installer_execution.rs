use eyre::{bail, eyre, Result};
use rustok_auth::{AuthUserBootstrapDbWriter, AuthUserBootstrapRequest};
use rustok_installer::{
    execute_seed_profile, DatabaseEngine, InstallAdminOutcome, InstallAdminPort,
    InstallApplyOptions, InstallApplyOutput, InstallDatabasePort, InstallDatabaseReady,
    InstallExecutionError, InstallExecutor, InstallPersistencePort, InstallPlan, InstallReceipt,
    InstallReceiptRecord, InstallSchemaPort, InstallSeedOutcome, InstallSeedPort,
    InstallSessionRecord, InstallState, InstallVerificationOutcome, InstallVerificationPort,
    SeedExecutionError, SeedExecutionRequest, SeedIdentityPort, SeedModulePort, SeedProfile,
    SeedRolePort, SeedTenant, SeedTenantPort, SeedTenantRequest, SeedUser, SeedUserRequest,
    TenantBootstrap,
};
use rustok_installer_persistence::InstallerPersistenceService;
use rustok_migrations::Migrator;
use rustok_rbac::RbacRoleAssignmentDbWriter;
use rustok_tenant::{
    PortActor, PortContext, PortErrorKind, TenantReadPort, TenantReadProjection, TenantReadRequest,
    TenantReadSelector, TenantService,
};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::MigratorTrait;
use std::time::Duration;
use url::Url;
use uuid::Uuid;

use crate::models::users;
use crate::modules::build_registry;
use crate::services::effective_module_policy::EffectiveModulePolicyService;
use crate::services::module_lifecycle::ModuleLifecycleService;
use crate::services::rbac_service::RbacService;

const DEFAULT_PG_ADMIN_URL: &str = "postgres://postgres:postgres@localhost:5432/postgres";
const INSTALLER_TENANT_READ_DEADLINE: Duration = Duration::from_secs(15);

pub async fn apply_plan(
    plan: InstallPlan,
    options: InstallApplyOptions,
) -> Result<InstallerApplyOutput> {
    rustok_installer::execute_install_apply(&ServerInstallerPorts, plan, options)
        .await
        .map_err(|error| eyre!(error.to_string()))
}

/// HTTP-host adapter for the portable installer executor contract.
pub struct ServerInstallExecutor;

#[async_trait::async_trait]
impl InstallExecutor for ServerInstallExecutor {
    async fn apply(
        &self,
        plan: InstallPlan,
        options: InstallApplyOptions,
    ) -> std::result::Result<InstallApplyOutput, InstallExecutionError> {
        apply_plan(plan, options)
            .await
            .map_err(|error| InstallExecutionError::new(error.to_string()))
    }
}

/// SeaORM and domain-writer adapter for the portable installer state machine.
struct ServerInstallerPorts;

#[async_trait::async_trait]
impl InstallDatabasePort for ServerInstallerPorts {
    type Runtime = DatabaseConnection;

    async fn prepare_database(
        &self,
        plan: &InstallPlan,
        database_url: &str,
        options: &InstallApplyOptions,
    ) -> std::result::Result<InstallDatabaseReady<Self::Runtime>, InstallExecutionError> {
        let ready = prepare_database(plan, database_url, options.pg_admin_url.as_deref())
            .await
            .map_err(execution_error)?;
        Ok(InstallDatabaseReady {
            runtime: ready.connection,
            database_name: ready.database_name,
            created_database: ready.created_database,
        })
    }
}

#[async_trait::async_trait]
impl InstallSchemaPort<DatabaseConnection> for ServerInstallerPorts {
    async fn apply_schema(
        &self,
        runtime: &DatabaseConnection,
    ) -> std::result::Result<(), InstallExecutionError> {
        apply_schema_migrations(runtime)
            .await
            .map_err(execution_error)
    }
}

#[async_trait::async_trait]
impl InstallPersistencePort<DatabaseConnection> for ServerInstallerPorts {
    async fn create_session(
        &self,
        runtime: &DatabaseConnection,
        plan: &InstallPlan,
    ) -> std::result::Result<InstallSessionRecord, InstallExecutionError> {
        InstallerPersistenceService::new(runtime.clone())
            .create_session(plan, None, None)
            .await
            .map(session_record)
            .map_err(execution_error)
    }

    async fn acquire_lock(
        &self,
        runtime: &DatabaseConnection,
        session: InstallSessionRecord,
        owner: &str,
        ttl_secs: i64,
    ) -> std::result::Result<InstallSessionRecord, InstallExecutionError> {
        let persistence = InstallerPersistenceService::new(runtime.clone());
        let model = persistence
            .get_session(session.id)
            .await
            .map_err(execution_error)?
            .ok_or_else(|| {
                InstallExecutionError::new(format!("install session {} not found", session.id))
            })?;
        persistence
            .acquire_lock(model, owner, chrono::Duration::seconds(ttl_secs.max(1)))
            .await
            .map(session_record)
            .map_err(execution_error)
    }

    async fn record_receipt(
        &self,
        runtime: &DatabaseConnection,
        receipt: &InstallReceipt,
    ) -> std::result::Result<InstallReceiptRecord, InstallExecutionError> {
        InstallerPersistenceService::new(runtime.clone())
            .record_receipt(receipt)
            .await
            .map(|model| InstallReceiptRecord {
                id: model.id,
                input_checksum: model.input_checksum,
            })
            .map_err(execution_error)
    }

    async fn set_state(
        &self,
        runtime: &DatabaseConnection,
        session_id: Uuid,
        state: InstallState,
    ) -> std::result::Result<InstallSessionRecord, InstallExecutionError> {
        InstallerPersistenceService::new(runtime.clone())
            .set_state(session_id, state)
            .await
            .map(session_record)
            .map_err(execution_error)
    }

    async fn set_tenant_id(
        &self,
        runtime: &DatabaseConnection,
        session_id: Uuid,
        tenant_id: Uuid,
    ) -> std::result::Result<InstallSessionRecord, InstallExecutionError> {
        InstallerPersistenceService::new(runtime.clone())
            .set_tenant_id(session_id, tenant_id)
            .await
            .map(session_record)
            .map_err(execution_error)
    }
}

#[async_trait::async_trait]
impl InstallSeedPort<DatabaseConnection> for ServerInstallerPorts {
    async fn apply_seed(
        &self,
        runtime: &DatabaseConnection,
        plan: &InstallPlan,
    ) -> std::result::Result<InstallSeedOutcome, InstallExecutionError> {
        apply_seed_profile(runtime, plan)
            .await
            .map(|outcome| InstallSeedOutcome {
                tenant_id: outcome.tenant_id,
                tenant_slug: outcome.tenant_slug,
                tenant_created: outcome.tenant_created,
                enabled_modules: outcome.enabled_modules,
                disabled_modules: outcome.disabled_modules,
                demo_customer_created: outcome.demo_customer_created,
            })
            .map_err(execution_error)
    }
}

#[async_trait::async_trait]
impl InstallAdminPort<DatabaseConnection> for ServerInstallerPorts {
    async fn provision_admin(
        &self,
        runtime: &DatabaseConnection,
        plan: &InstallPlan,
        tenant_id: Uuid,
        password: &str,
    ) -> std::result::Result<InstallAdminOutcome, InstallExecutionError> {
        provision_admin(runtime, plan, tenant_id, password)
            .await
            .map(|outcome| InstallAdminOutcome {
                user_id: outcome.user_id,
                email: outcome.email,
                created: outcome.created,
            })
            .map_err(execution_error)
    }
}

#[async_trait::async_trait]
impl InstallVerificationPort<DatabaseConnection> for ServerInstallerPorts {
    async fn verify_installation(
        &self,
        runtime: &DatabaseConnection,
        plan: &InstallPlan,
        tenant_id: Uuid,
    ) -> std::result::Result<InstallVerificationOutcome, InstallExecutionError> {
        verify_installation(runtime, plan, tenant_id)
            .await
            .map(|outcome| InstallVerificationOutcome {
                tenant_id: outcome.tenant_id,
                tenant_slug: outcome.tenant_slug,
                admin_user_id: outcome.admin_user_id,
                enabled_modules: outcome.enabled_modules,
            })
            .map_err(execution_error)
    }
}

fn session_record(
    model: rustok_installer_persistence::entities::install_session::Model,
) -> InstallSessionRecord {
    InstallSessionRecord {
        id: model.id,
        tenant_id: model.tenant_id,
        lock_owner: model.lock_owner,
        lock_expires_at: model.lock_expires_at,
    }
}

fn execution_error(error: impl std::fmt::Display) -> InstallExecutionError {
    InstallExecutionError::new(error.to_string())
}

struct DatabaseReady {
    connection: DatabaseConnection,
    database_name: Option<String>,
    created_database: bool,
}

async fn prepare_database(
    plan: &InstallPlan,
    database_url: &str,
    pg_admin_url: Option<&str>,
) -> Result<DatabaseReady> {
    let target = parse_database_target(&plan.database.engine, database_url)?;
    let mut created_database = false;

    if plan.database.create_if_missing {
        if plan.database.engine != DatabaseEngine::Postgres {
            bail!("--create-database is only supported for postgres install plans");
        }
        let admin_url = pg_admin_url.unwrap_or(DEFAULT_PG_ADMIN_URL);
        created_database = ensure_postgres_database(admin_url, &target).await?;
    }

    let connection = Database::connect(database_url)
        .await
        .map_err(|error| eyre!("failed to connect installer database: {error}"))?;
    connection
        .query_one(Statement::from_string(
            connection.get_database_backend(),
            "SELECT 1".to_string(),
        ))
        .await
        .map_err(|error| eyre!("failed installer database readiness query: {error}"))?;

    Ok(DatabaseReady {
        connection,
        database_name: target.database_name,
        created_database,
    })
}

async fn apply_schema_migrations(db: &DatabaseConnection) -> Result<()> {
    Migrator::up(db, None)
        .await
        .map_err(|error| eyre!("failed to apply installer schema migrations: {error}"))
}

struct ServerInstallerSeedTenantPort<'a> {
    db: &'a DatabaseConnection,
}

struct ServerInstallerSeedIdentityPort<'a> {
    db: &'a DatabaseConnection,
}

struct ServerInstallerSeedRolePort<'a> {
    db: &'a DatabaseConnection,
}

struct ServerInstallerSeedModulePort<'a> {
    db: &'a DatabaseConnection,
    registry: rustok_core::ModuleRegistry,
}

#[async_trait::async_trait]
impl SeedTenantPort for ServerInstallerSeedTenantPort<'_> {
    async fn ensure_seed_tenant(
        &self,
        request: SeedTenantRequest,
    ) -> Result<SeedTenant, SeedExecutionError> {
        let (tenant, created) = TenantService::new(self.db.clone())
            .ensure_tenant(rustok_tenant::CreateTenantInput {
                name: request.name,
                slug: request.slug,
                domain: request.domain,
            })
            .await
            .map_err(seed_dependency_error)?;
        Ok(SeedTenant {
            id: tenant.id,
            slug: tenant.slug,
            created,
        })
    }
}

#[async_trait::async_trait]
impl SeedIdentityPort for ServerInstallerSeedIdentityPort<'_> {
    async fn ensure_seed_user(
        &self,
        request: SeedUserRequest,
    ) -> Result<SeedUser, SeedExecutionError> {
        let user = ensure_installer_user(self.db, &request)
            .await
            .map_err(seed_dependency_error)?;
        Ok(user)
    }
}

#[async_trait::async_trait]
impl SeedRolePort for ServerInstallerSeedRolePort<'_> {
    async fn assign_seed_role(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        role: rustok_core::UserRole,
    ) -> Result<(), SeedExecutionError> {
        RbacRoleAssignmentDbWriter::new(self.db.clone())
            .assign_role_permissions(tenant_id, user_id, role)
            .await
            .map_err(seed_dependency_error)?;
        RbacService::invalidate_user_rbac_caches(&tenant_id, &user_id).await;
        Ok(())
    }
}

#[async_trait::async_trait]
impl SeedModulePort for ServerInstallerSeedModulePort<'_> {
    async fn set_seed_module_enabled(
        &self,
        tenant_id: Uuid,
        module_slug: &str,
        enabled: bool,
        actor: &str,
    ) -> Result<(), SeedExecutionError> {
        ModuleLifecycleService::toggle_module_with_actor(
            self.db,
            &self.registry,
            tenant_id,
            module_slug,
            enabled,
            Some(actor.to_string()),
        )
        .await
        .map_err(seed_dependency_error)
    }
}

fn seed_dependency_error(error: impl std::fmt::Display) -> SeedExecutionError {
    SeedExecutionError::Dependency(error.to_string())
}

#[derive(Debug)]
struct SeedOutcome {
    tenant_id: Uuid,
    tenant_slug: String,
    tenant_created: bool,
    enabled_modules: Vec<String>,
    disabled_modules: Vec<String>,
    demo_customer_created: bool,
}

async fn apply_seed_profile(db: &DatabaseConnection, plan: &InstallPlan) -> Result<SeedOutcome> {
    let mut enabled_modules = plan.seed_profile.default_enabled_modules();
    enabled_modules.extend(plan.modules.enable.iter().cloned());
    let tenant_port = ServerInstallerSeedTenantPort { db };
    let identity_port = ServerInstallerSeedIdentityPort { db };
    let role_port = ServerInstallerSeedRolePort { db };
    let module_port = ServerInstallerSeedModulePort {
        db,
        registry: build_registry(),
    };
    let outcome = execute_seed_profile(
        SeedExecutionRequest {
            profile: plan.seed_profile,
            tenant: SeedTenantRequest {
                name: plan.tenant.name.clone(),
                slug: plan.tenant.slug.clone(),
                domain: None,
            },
            enabled_modules,
            disabled_modules: plan.modules.disable.clone(),
            admin: None,
            demo_customer_password: (plan.seed_profile == SeedProfile::Dev)
                .then(|| "dev-password-123".to_string()),
            actor: "installer".to_string(),
        },
        &tenant_port,
        &identity_port,
        &role_port,
        &module_port,
    )
    .await
    .map_err(|error| eyre!("failed to apply installer seed profile: {error}"))?;

    Ok(SeedOutcome {
        tenant_id: outcome.tenant.id,
        tenant_slug: outcome.tenant.slug,
        tenant_created: outcome.tenant.created,
        enabled_modules: outcome.enabled_modules,
        disabled_modules: outcome.disabled_modules,
        demo_customer_created: outcome
            .demo_customer
            .map(|user| user.created)
            .unwrap_or(false),
    })
}

#[derive(Debug)]
struct AdminOutcome {
    user_id: Uuid,
    email: String,
    created: bool,
}

async fn provision_admin(
    db: &DatabaseConnection,
    plan: &InstallPlan,
    tenant_id: Uuid,
    password: &str,
) -> Result<AdminOutcome> {
    ensure_user_with_role(
        db,
        tenant_id,
        &plan.admin.email,
        "Super Admin",
        password,
        rustok_core::UserRole::SuperAdmin,
    )
    .await
}

async fn ensure_user_with_role(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    email: &str,
    name: &str,
    password: &str,
    role: rustok_core::UserRole,
) -> Result<AdminOutcome> {
    let user = ensure_installer_user(
        db,
        &SeedUserRequest {
            tenant_id,
            email: email.to_string(),
            name: name.to_string(),
            password: password.to_string(),
        },
    )
    .await?;
    RbacService::assign_role_permissions(db, &user.id, &tenant_id, role)
        .await
        .map_err(|error| eyre!("failed to assign installer user role: {error}"))?;

    Ok(AdminOutcome {
        user_id: user.id,
        email: user.email,
        created: user.created,
    })
}

async fn ensure_installer_user(
    db: &DatabaseConnection,
    request: &SeedUserRequest,
) -> Result<SeedUser> {
    let user = AuthUserBootstrapDbWriter::new(db.clone())
        .ensure_user(AuthUserBootstrapRequest {
            tenant_id: request.tenant_id,
            email: request.email.clone(),
            name: request.name.clone(),
            password: request.password.clone(),
        })
        .await
        .map_err(|error| {
            eyre!(
                "failed to provision installer user `{}`: {error}",
                request.email
            )
        })?;

    Ok(SeedUser {
        id: user.id,
        email: user.email,
        created: true,
    })
}

#[derive(Debug)]
struct VerifyOutcome {
    tenant_id: Uuid,
    tenant_slug: String,
    admin_user_id: Uuid,
    enabled_modules: Vec<String>,
}

async fn verify_installation(
    db: &DatabaseConnection,
    plan: &InstallPlan,
    tenant_id: Uuid,
) -> Result<VerifyOutcome> {
    let tenant = read_installer_tenant_by_slug(db, &plan.tenant.slug)
        .await
        .map_err(|error| eyre!("failed to verify installer tenant: {error}"))?
        .ok_or_else(|| eyre!("installer tenant `{}` was not created", plan.tenant.slug))?;
    if tenant.id != tenant_id {
        bail!(
            "installer tenant slug `{}` resolved to unexpected tenant {}",
            plan.tenant.slug,
            tenant.id
        );
    }

    let admin = users::Entity::find_by_email(db, tenant.id, &plan.admin.email)
        .await
        .map_err(|error| eyre!("failed to verify installer admin user: {error}"))?
        .ok_or_else(|| eyre!("installer admin `{}` was not created", plan.admin.email))?;
    let registry = build_registry();
    let enabled_modules = EffectiveModulePolicyService::list_enabled(db, &registry, tenant.id)
        .await
        .map_err(|error| eyre!("failed to verify installer module enablement: {error}"))?;

    Ok(VerifyOutcome {
        tenant_id: tenant.id,
        tenant_slug: tenant.slug,
        admin_user_id: admin.id,
        enabled_modules,
    })
}

async fn read_installer_tenant_by_slug(
    db: &DatabaseConnection,
    slug: &str,
) -> Result<Option<TenantReadProjection>> {
    let tenant_service = TenantService::new(db.clone());
    let context = PortContext::new(
        slug.to_string(),
        PortActor::service("rustok-installer.execution"),
        "und",
        format!("installer:tenant-read:{slug}"),
    )
    .with_deadline(INSTALLER_TENANT_READ_DEADLINE);
    let request = TenantReadRequest {
        selector: TenantReadSelector::Slug(slug.to_string()),
        include_inactive: true,
    };

    match tenant_service.read_tenant(context, request).await {
        // treat missing tenant as create candidate; all other port errors must surface.
        Ok(tenant) => Ok(Some(tenant)),
        Err(error) if error.kind == PortErrorKind::NotFound => Ok(None),
        Err(error) => Err(eyre!(
            "tenant read projection `{slug}` failed through TenantReadPort ({}): {}",
            error.code,
            error.message
        )),
    }
}

#[derive(Debug)]
struct DatabaseTarget {
    database_name: Option<String>,
    username: Option<String>,
    password: Option<String>,
}

fn parse_database_target(engine: &DatabaseEngine, database_url: &str) -> Result<DatabaseTarget> {
    if *engine == DatabaseEngine::Sqlite {
        return Ok(DatabaseTarget {
            database_name: None,
            username: None,
            password: None,
        });
    }

    let parsed = Url::parse(database_url)
        .map_err(|error| eyre!("invalid postgres database URL: {error}"))?;
    match parsed.scheme() {
        "postgres" | "postgresql" => {}
        scheme => bail!("postgres install plan requires postgres URL, got `{scheme}`"),
    }

    let database_name = parsed
        .path_segments()
        .and_then(|mut segments| segments.next_back())
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(|| eyre!("postgres database URL must include a database name"))?
        .to_string();
    let username = parsed.username();
    if username.trim().is_empty() {
        bail!("postgres database URL must include a username");
    }

    Ok(DatabaseTarget {
        database_name: Some(database_name),
        username: Some(username.to_string()),
        password: parsed.password().map(ToString::to_string),
    })
}

async fn ensure_postgres_database(admin_url: &str, target: &DatabaseTarget) -> Result<bool> {
    let database_name = target
        .database_name
        .as_deref()
        .ok_or_else(|| eyre!("postgres database name is required"))?;
    let username = target
        .username
        .as_deref()
        .ok_or_else(|| eyre!("postgres username is required"))?;
    let password = target.password.as_deref().unwrap_or_default();

    let admin = Database::connect(admin_url)
        .await
        .map_err(|error| eyre!("failed to connect postgres admin database: {error}"))?;
    let role_exists = admin
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            format!(
                "SELECT 1 FROM pg_roles WHERE rolname = {}",
                quote_literal(username)
            ),
        ))
        .await
        .map_err(|error| eyre!("failed to inspect postgres role `{username}`: {error}"))?
        .is_some();
    if !role_exists {
        admin
            .execute(Statement::from_string(
                DbBackend::Postgres,
                format!(
                    "CREATE ROLE {} LOGIN PASSWORD {}",
                    quote_ident(username),
                    quote_literal(password)
                ),
            ))
            .await
            .map_err(|error| eyre!("failed to create postgres role `{username}`: {error}"))?;
    }

    let database_exists = admin
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            format!(
                "SELECT 1 FROM pg_database WHERE datname = {}",
                quote_literal(database_name)
            ),
        ))
        .await
        .map_err(|error| eyre!("failed to inspect postgres database `{database_name}`: {error}"))?
        .is_some();
    if database_exists {
        return Ok(false);
    }

    admin
        .execute(Statement::from_string(
            DbBackend::Postgres,
            format!(
                "CREATE DATABASE {} OWNER {}",
                quote_ident(database_name),
                quote_ident(username)
            ),
        ))
        .await
        .map_err(|error| eyre!("failed to create postgres database `{database_name}`: {error}"))?;

    Ok(true)
}

fn quote_ident(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn quote_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_postgres_database_target() {
        let target = parse_database_target(
            &DatabaseEngine::Postgres,
            "postgres://rustok:secret@localhost:5432/rustok_dev",
        )
        .expect("valid target");

        assert_eq!(target.database_name.as_deref(), Some("rustok_dev"));
        assert_eq!(target.username.as_deref(), Some("rustok"));
        assert_eq!(target.password.as_deref(), Some("secret"));
    }

    #[test]
    fn rejects_postgres_target_without_database_name() {
        let error = parse_database_target(
            &DatabaseEngine::Postgres,
            "postgres://rustok@localhost:5432/",
        )
        .expect_err("missing database name");

        assert!(error.to_string().contains("database name"));
    }

    #[test]
    fn quotes_postgres_identifiers_and_literals() {
        assert_eq!(quote_ident("tenant\"db"), "\"tenant\"\"db\"");
        assert_eq!(quote_literal("pa'ss"), "'pa''ss'");
    }
}
