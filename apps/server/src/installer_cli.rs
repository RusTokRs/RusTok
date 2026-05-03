use eyre::{bail, eyre, Result};
use migration::Migrator;
use rustok_installer::{
    evaluate_preflight, redact_install_plan, AdminBootstrap, DatabaseConfig, DatabaseEngine,
    InstallEnvironment, InstallPlan, InstallProfile, InstallReceipt, InstallState, InstallStep,
    ModuleSelection, SecretMode, SecretRef, SecretValue, SeedProfile, TenantBootstrap,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ConnectionTrait, Database, DatabaseConnection, DbBackend,
    Statement,
};
use sea_orm_migration::MigratorTrait;
use url::Url;
use uuid::Uuid;

use crate::auth::hash_password;
use crate::models::{tenants, users};
use crate::modules::build_registry;
use crate::services::effective_module_policy::EffectiveModulePolicyService;
use crate::services::installer_persistence::InstallerPersistenceService;
use crate::services::module_lifecycle::ModuleLifecycleService;
use crate::services::rbac_service::RbacService;

#[derive(Debug, Clone)]
pub struct InstallerApplyOptions {
    pub lock_owner: String,
    pub lock_ttl_secs: i64,
    pub pg_admin_url: Option<String>,
}

impl Default for InstallerApplyOptions {
    fn default() -> Self {
        Self {
            lock_owner: "api".to_string(),
            lock_ttl_secs: 900,
            pg_admin_url: None,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct InstallerApplyOutput {
    pub status: String,
    pub session_id: Uuid,
    pub tenant_id: Option<Uuid>,
    pub lock_owner: Option<String>,
    pub lock_expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub preflight_receipt_id: Uuid,
    pub preflight_receipt_checksum: String,
    pub config_receipt_id: Uuid,
    pub config_receipt_checksum: String,
    pub database_receipt_id: Uuid,
    pub database_receipt_checksum: String,
    pub migrate_receipt_id: Uuid,
    pub migrate_receipt_checksum: String,
    pub seed_receipt_id: Uuid,
    pub seed_receipt_checksum: String,
    pub admin_receipt_id: Uuid,
    pub admin_receipt_checksum: String,
    pub verify_receipt_id: Uuid,
    pub verify_receipt_checksum: String,
    pub finalize_receipt_id: Uuid,
    pub finalize_receipt_checksum: String,
    pub next: Option<String>,
}

const DEFAULT_DATABASE_URL: &str = "postgres://rustok:rustok@localhost:5432/rustok_dev";
const DEFAULT_PG_ADMIN_URL: &str = "postgres://postgres:postgres@localhost:5432/postgres";
const DEFAULT_ADMIN_EMAIL: &str = "admin@local";
const DEFAULT_TENANT_SLUG: &str = "demo";
const DEFAULT_TENANT_NAME: &str = "Demo Workspace";

pub async fn try_handle(args: &[String]) -> Result<bool> {
    if args.get(1).map(String::as_str) != Some("install") {
        return Ok(false);
    }

    match args.get(2).map(String::as_str) {
        Some("preflight") => {
            let options = InstallCliOptions::parse(&args[3..])?;
            let plan = options.into_plan()?;
            let report = evaluate_preflight(&plan);
            println!("{}", serde_json::to_string_pretty(&report)?);
            if report.passed() {
                Ok(true)
            } else {
                bail!("installer preflight failed")
            }
        }
        Some("plan") => {
            let options = InstallCliOptions::parse(&args[3..])?;
            let plan = options.into_plan()?;
            println!(
                "{}",
                serde_json::to_string_pretty(&redact_install_plan(&plan))?
            );
            Ok(true)
        }
        Some("apply") => {
            let options = InstallCliOptions::parse(&args[3..])?;
            handle_apply(options).await?;
            Ok(true)
        }
        Some("--help") | Some("-h") | None => {
            print_usage();
            Ok(true)
        }
        Some(command) => bail!("unknown install command `{command}`"),
    }
}

#[derive(Debug)]
struct InstallCliOptions {
    environment: InstallEnvironment,
    profile: InstallProfile,
    database_engine: DatabaseEngine,
    database_url: String,
    database_secret_ref: Option<SecretRef>,
    create_database: bool,
    pg_admin_url: Option<String>,
    admin_email: String,
    admin_password: Option<String>,
    admin_password_ref: Option<SecretRef>,
    tenant_slug: String,
    tenant_name: String,
    seed_profile: SeedProfile,
    secrets_mode: SecretMode,
    enable_modules: Vec<String>,
    disable_modules: Vec<String>,
    lock_owner: String,
    lock_ttl_secs: i64,
}

impl InstallCliOptions {
    fn parse(args: &[String]) -> Result<Self> {
        let mut options = Self {
            environment: parse_env_var("RUSTOK_INSTALL_ENVIRONMENT", parse_environment)
                .unwrap_or(InstallEnvironment::Local),
            profile: parse_env_var("RUSTOK_INSTALL_PROFILE", parse_profile)
                .unwrap_or(InstallProfile::DevLocal),
            database_engine: parse_env_var("RUSTOK_INSTALL_DATABASE_ENGINE", parse_database_engine)
                .unwrap_or(DatabaseEngine::Postgres),
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_string()),
            database_secret_ref: std::env::var("RUSTOK_INSTALL_DATABASE_SECRET_REF")
                .ok()
                .as_deref()
                .map(parse_secret_ref)
                .transpose()?,
            create_database: false,
            pg_admin_url: std::env::var("RUSTOK_INSTALL_PG_ADMIN_URL").ok(),
            admin_email: std::env::var("SUPERADMIN_EMAIL")
                .or_else(|_| std::env::var("SEED_ADMIN_EMAIL"))
                .unwrap_or_else(|_| DEFAULT_ADMIN_EMAIL.to_string()),
            admin_password: std::env::var("SUPERADMIN_PASSWORD")
                .or_else(|_| std::env::var("SEED_ADMIN_PASSWORD"))
                .ok(),
            admin_password_ref: std::env::var("RUSTOK_INSTALL_ADMIN_PASSWORD_REF")
                .ok()
                .as_deref()
                .map(parse_secret_ref)
                .transpose()?,
            tenant_slug: std::env::var("SUPERADMIN_TENANT_SLUG")
                .or_else(|_| std::env::var("SEED_TENANT_SLUG"))
                .unwrap_or_else(|_| DEFAULT_TENANT_SLUG.to_string()),
            tenant_name: std::env::var("SUPERADMIN_TENANT_NAME")
                .or_else(|_| std::env::var("SEED_TENANT_NAME"))
                .unwrap_or_else(|_| DEFAULT_TENANT_NAME.to_string()),
            seed_profile: parse_env_var("RUSTOK_INSTALL_SEED_PROFILE", parse_seed_profile)
                .unwrap_or(SeedProfile::Minimal),
            secrets_mode: parse_env_var("RUSTOK_INSTALL_SECRETS_MODE", parse_secret_mode)
                .unwrap_or(SecretMode::Env),
            enable_modules: Vec::new(),
            disable_modules: Vec::new(),
            lock_owner: std::env::var("RUSTOK_INSTALL_LOCK_OWNER")
                .unwrap_or_else(|_| "cli".to_string()),
            lock_ttl_secs: std::env::var("RUSTOK_INSTALL_LOCK_TTL_SECS")
                .ok()
                .and_then(|value| value.parse::<i64>().ok())
                .unwrap_or(900),
        };

        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--environment" => {
                    options.environment = parse_environment(&take_value(args, &mut index)?)?;
                }
                "--profile" => {
                    options.profile = parse_profile(&take_value(args, &mut index)?)?;
                }
                "--database-engine" => {
                    options.database_engine =
                        parse_database_engine(&take_value(args, &mut index)?)?;
                }
                "--database-url" => {
                    options.database_url = take_value(args, &mut index)?;
                    options.database_secret_ref = None;
                }
                "--database-secret-ref" => {
                    options.database_secret_ref =
                        Some(parse_secret_ref(&take_value(args, &mut index)?)?);
                }
                "--create-database" => options.create_database = true,
                "--pg-admin-url" => {
                    options.pg_admin_url = Some(take_value(args, &mut index)?);
                }
                "--admin-email" => {
                    options.admin_email = take_value(args, &mut index)?;
                }
                "--admin-password" => {
                    options.admin_password = Some(take_value(args, &mut index)?);
                    options.admin_password_ref = None;
                }
                "--admin-password-ref" => {
                    options.admin_password_ref =
                        Some(parse_secret_ref(&take_value(args, &mut index)?)?);
                }
                "--tenant-slug" => {
                    options.tenant_slug = take_value(args, &mut index)?;
                }
                "--tenant-name" => {
                    options.tenant_name = take_value(args, &mut index)?;
                }
                "--seed-profile" => {
                    options.seed_profile = parse_seed_profile(&take_value(args, &mut index)?)?;
                }
                "--secrets-mode" => {
                    options.secrets_mode = parse_secret_mode(&take_value(args, &mut index)?)?;
                }
                "--enable-module" => {
                    options.enable_modules.push(take_value(args, &mut index)?);
                }
                "--disable-module" => {
                    options.disable_modules.push(take_value(args, &mut index)?);
                }
                "--lock-owner" => {
                    options.lock_owner = take_value(args, &mut index)?;
                }
                "--lock-ttl-secs" => {
                    options.lock_ttl_secs = take_value(args, &mut index)?
                        .parse::<i64>()
                        .map_err(|error| eyre!("invalid --lock-ttl-secs: {error}"))?;
                }
                "--help" | "-h" => {
                    print_usage();
                    std::process::exit(0);
                }
                unknown => bail!("unknown install option `{unknown}`"),
            }
            index += 1;
        }

        Ok(options)
    }

    fn into_plan(self) -> Result<InstallPlan> {
        let admin_password = match (self.admin_password_ref, self.admin_password) {
            (Some(reference), _) => SecretValue::Reference { reference },
            (None, Some(value)) => SecretValue::Plaintext { value },
            (None, None) => {
                if self.environment.is_production() {
                    bail!("production install requires --admin-password-ref or SUPERADMIN_PASSWORD")
                }
                SecretValue::Plaintext {
                    value: "admin12345".to_string(),
                }
            }
        };

        let database_url = match self.database_secret_ref {
            Some(reference) => SecretValue::Reference { reference },
            None => SecretValue::Plaintext {
                value: self.database_url,
            },
        };

        Ok(InstallPlan {
            environment: self.environment,
            profile: self.profile,
            database: DatabaseConfig {
                engine: self.database_engine,
                url: database_url,
                create_if_missing: self.create_database,
            },
            tenant: TenantBootstrap {
                slug: self.tenant_slug,
                name: self.tenant_name,
            },
            admin: AdminBootstrap {
                email: self.admin_email,
                password: admin_password,
            },
            modules: ModuleSelection {
                enable: self.enable_modules,
                disable: self.disable_modules,
            },
            seed_profile: self.seed_profile,
            secrets_mode: self.secrets_mode,
        })
    }
}

async fn handle_apply(options: InstallCliOptions) -> Result<()> {
    let apply_options = InstallerApplyOptions {
        lock_owner: options.lock_owner.clone(),
        lock_ttl_secs: options.lock_ttl_secs,
        pg_admin_url: options.pg_admin_url.clone(),
    };
    let plan = options.into_plan()?;
    let output = apply_plan(plan, apply_options).await?;
    println!("{}", serde_json::to_string_pretty(&output)?);

    Ok(())
}

pub async fn apply_plan(
    plan: InstallPlan,
    options: InstallerApplyOptions,
) -> Result<InstallerApplyOutput> {
    let lock_owner = options.lock_owner;
    let lock_ttl_secs = options.lock_ttl_secs.max(1);
    let pg_admin_url = options.pg_admin_url;
    let report = evaluate_preflight(&plan);
    if !report.passed() {
        bail!("installer preflight failed")
    }

    let database_url = resolve_database_url(&plan)?;
    let database_ready = prepare_database(&plan, &database_url, pg_admin_url.as_deref()).await?;
    apply_schema_migrations(&database_ready.connection).await?;
    let plan_snapshot = redact_install_plan(&plan);
    let db = database_ready.connection.clone();
    let persistence = InstallerPersistenceService::new(db);
    let session = persistence
        .create_session(&plan, None, None)
        .await
        .map_err(|error| eyre!("failed to create installer session: {error}"))?;
    let session = persistence
        .acquire_lock(
            session,
            &lock_owner,
            chrono::Duration::seconds(lock_ttl_secs),
        )
        .await
        .map_err(|error| eyre!("failed to acquire installer lock: {error}"))?;
    let receipt = InstallReceipt::success(
        session.id.to_string(),
        InstallStep::Preflight,
        &plan_snapshot,
        serde_json::json!({
            "report": report,
            "mode": "apply",
            "note": "preflight receipt recorded after database and schema bootstrap"
        }),
    )?;
    let receipt = persistence
        .record_receipt(&receipt)
        .await
        .map_err(|error| eyre!("failed to record installer preflight receipt: {error}"))?;
    let session = persistence
        .set_state(session.id, rustok_installer::InstallState::PreflightPassed)
        .await
        .map_err(|error| eyre!("failed to update installer session state: {error}"))?;
    let config_receipt = InstallReceipt::success(
        session.id.to_string(),
        InstallStep::Config,
        &plan_snapshot,
        serde_json::json!({
            "source": "cli",
            "secrets_mode": plan.secrets_mode,
            "redacted": true
        }),
    )?;
    let config_receipt = persistence
        .record_receipt(&config_receipt)
        .await
        .map_err(|error| eyre!("failed to record installer config receipt: {error}"))?;
    let session = persistence
        .set_state(session.id, InstallState::ConfigPrepared)
        .await
        .map_err(|error| eyre!("failed to update installer session state: {error}"))?;
    let database_receipt = InstallReceipt::success(
        session.id.to_string(),
        InstallStep::Database,
        &plan_snapshot,
        serde_json::json!({
            "database_engine": plan.database.engine,
            "database_name": database_ready.database_name,
            "create_if_missing": plan.database.create_if_missing,
            "created_database": database_ready.created_database,
            "checked": true
        }),
    )?;
    let database_receipt = persistence
        .record_receipt(&database_receipt)
        .await
        .map_err(|error| eyre!("failed to record installer database receipt: {error}"))?;
    let session = persistence
        .set_state(session.id, InstallState::DatabaseReady)
        .await
        .map_err(|error| eyre!("failed to update installer session state: {error}"))?;
    let migrate_receipt = InstallReceipt::success(
        session.id.to_string(),
        InstallStep::Migrate,
        &plan_snapshot,
        serde_json::json!({
            "migrator": "apps/server/migration::Migrator",
            "limit": null,
            "applied": "up_to_latest"
        }),
    )?;
    let migrate_receipt = persistence
        .record_receipt(&migrate_receipt)
        .await
        .map_err(|error| eyre!("failed to record installer migrate receipt: {error}"))?;
    let session = persistence
        .set_state(session.id, InstallState::SchemaApplied)
        .await
        .map_err(|error| eyre!("failed to update installer session state: {error}"))?;
    let seed_outcome = apply_seed_profile(&database_ready.connection, &plan).await?;
    let session = persistence
        .set_tenant_id(session.id, seed_outcome.tenant_id)
        .await
        .map_err(|error| eyre!("failed to attach tenant to installer session: {error}"))?;
    let seed_receipt = InstallReceipt::success(
        session.id.to_string(),
        InstallStep::Seed,
        &plan_snapshot,
        serde_json::json!({
            "seed_profile": plan.seed_profile,
            "tenant_id": seed_outcome.tenant_id,
            "tenant_slug": seed_outcome.tenant_slug,
            "tenant_created": seed_outcome.tenant_created,
            "enabled_modules": seed_outcome.enabled_modules,
            "disabled_modules": seed_outcome.disabled_modules,
            "demo_customer_created": seed_outcome.demo_customer_created
        }),
    )?;
    let seed_receipt = persistence
        .record_receipt(&seed_receipt)
        .await
        .map_err(|error| eyre!("failed to record installer seed receipt: {error}"))?;
    let session = persistence
        .set_state(session.id, InstallState::SeedApplied)
        .await
        .map_err(|error| eyre!("failed to update installer session state: {error}"))?;
    let admin_password = resolve_admin_password(&plan)?;
    let admin_outcome = provision_admin(
        &database_ready.connection,
        &plan,
        seed_outcome.tenant_id,
        &admin_password,
    )
    .await?;
    let admin_receipt = InstallReceipt::success(
        session.id.to_string(),
        InstallStep::Admin,
        &plan_snapshot,
        serde_json::json!({
            "tenant_id": seed_outcome.tenant_id,
            "admin_email": admin_outcome.email,
            "admin_user_id": admin_outcome.user_id,
            "admin_created": admin_outcome.created,
            "role": "super_admin"
        }),
    )?;
    let admin_receipt = persistence
        .record_receipt(&admin_receipt)
        .await
        .map_err(|error| eyre!("failed to record installer admin receipt: {error}"))?;
    let session = persistence
        .set_state(session.id, InstallState::AdminProvisioned)
        .await
        .map_err(|error| eyre!("failed to update installer session state: {error}"))?;
    let verify_outcome =
        verify_installation(&database_ready.connection, &plan, seed_outcome.tenant_id).await?;
    let verify_receipt = InstallReceipt::success(
        session.id.to_string(),
        InstallStep::Verify,
        &plan_snapshot,
        serde_json::json!({
            "tenant_id": verify_outcome.tenant_id,
            "tenant_slug": verify_outcome.tenant_slug,
            "admin_user_id": verify_outcome.admin_user_id,
            "enabled_modules": verify_outcome.enabled_modules
        }),
    )?;
    let verify_receipt = persistence
        .record_receipt(&verify_receipt)
        .await
        .map_err(|error| eyre!("failed to record installer verify receipt: {error}"))?;
    let session = persistence
        .set_state(session.id, InstallState::Verified)
        .await
        .map_err(|error| eyre!("failed to update installer session state: {error}"))?;
    let finalize_receipt = InstallReceipt::success(
        session.id.to_string(),
        InstallStep::Finalize,
        &plan_snapshot,
        serde_json::json!({
            "completed": true,
            "tenant_id": verify_outcome.tenant_id,
            "tenant_slug": verify_outcome.tenant_slug
        }),
    )?;
    let finalize_receipt = persistence
        .record_receipt(&finalize_receipt)
        .await
        .map_err(|error| eyre!("failed to record installer finalize receipt: {error}"))?;
    let session = persistence
        .set_state(session.id, InstallState::Completed)
        .await
        .map_err(|error| eyre!("failed to update installer session state: {error}"))?;

    Ok(InstallerApplyOutput {
        status: "completed".to_string(),
        session_id: session.id,
        tenant_id: session.tenant_id,
        lock_owner: session.lock_owner,
        lock_expires_at: session.lock_expires_at,
        preflight_receipt_id: receipt.id,
        preflight_receipt_checksum: receipt.input_checksum,
        config_receipt_id: config_receipt.id,
        config_receipt_checksum: config_receipt.input_checksum,
        database_receipt_id: database_receipt.id,
        database_receipt_checksum: database_receipt.input_checksum,
        migrate_receipt_id: migrate_receipt.id,
        migrate_receipt_checksum: migrate_receipt.input_checksum,
        seed_receipt_id: seed_receipt.id,
        seed_receipt_checksum: seed_receipt.input_checksum,
        admin_receipt_id: admin_receipt.id,
        admin_receipt_checksum: admin_receipt.input_checksum,
        verify_receipt_id: verify_receipt.id,
        verify_receipt_checksum: verify_receipt.input_checksum,
        finalize_receipt_id: finalize_receipt.id,
        finalize_receipt_checksum: finalize_receipt.input_checksum,
        next: None,
    })
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
    let existing_tenant = tenants::Entity::find_by_slug(db, &plan.tenant.slug)
        .await
        .map_err(|error| eyre!("failed to inspect installer tenant: {error}"))?;
    let tenant = tenants::Entity::find_or_create(db, &plan.tenant.name, &plan.tenant.slug, None)
        .await
        .map_err(|error| eyre!("failed to create installer tenant: {error}"))?;
    let mut enabled_modules = default_modules_for_seed(plan.seed_profile);
    enabled_modules.extend(plan.modules.enable.iter().cloned());
    enabled_modules.sort();
    enabled_modules.dedup();

    let mut disabled_modules = plan.modules.disable.clone();
    disabled_modules.sort();
    disabled_modules.dedup();
    enabled_modules.retain(|module| !disabled_modules.contains(module));

    let registry = build_registry();
    for module in &enabled_modules {
        ModuleLifecycleService::toggle_module_with_actor(
            db,
            &registry,
            tenant.id,
            module,
            true,
            Some("installer".to_string()),
        )
        .await
        .map_err(|error| eyre!("failed to enable module `{module}`: {error}"))?;
    }
    for module in &disabled_modules {
        ModuleLifecycleService::toggle_module_with_actor(
            db,
            &registry,
            tenant.id,
            module,
            false,
            Some("installer".to_string()),
        )
        .await
        .map_err(|error| eyre!("failed to disable module `{module}`: {error}"))?;
    }

    let demo_customer_created = if plan.seed_profile == SeedProfile::Dev {
        ensure_user_with_role(
            db,
            tenant.id,
            "customer@demo.local",
            "Demo Customer",
            "dev-password-123",
            rustok_core::UserRole::Customer,
        )
        .await?
        .created
    } else {
        false
    };

    Ok(SeedOutcome {
        tenant_id: tenant.id,
        tenant_slug: tenant.slug,
        tenant_created: existing_tenant.is_none(),
        enabled_modules,
        disabled_modules,
        demo_customer_created,
    })
}

fn default_modules_for_seed(seed_profile: SeedProfile) -> Vec<String> {
    match seed_profile {
        SeedProfile::Dev => ["content", "commerce", "pages", "blog", "forum", "index"]
            .into_iter()
            .map(ToString::to_string)
            .collect(),
        SeedProfile::Minimal | SeedProfile::None => Vec::new(),
    }
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
    if let Some(user) = users::Entity::find_by_email(db, tenant_id, email)
        .await
        .map_err(|error| eyre!("failed to inspect installer user `{email}`: {error}"))?
    {
        RbacService::assign_role_permissions(db, &user.id, &tenant_id, role)
            .await
            .map_err(|error| eyre!("failed to synchronize installer user role: {error}"))?;
        return Ok(AdminOutcome {
            user_id: user.id,
            email: user.email,
            created: false,
        });
    }

    let password_hash = hash_password(password)
        .map_err(|error| eyre!("failed to hash installer user password: {error}"))?;
    let mut user = users::ActiveModel::new(tenant_id, email, &password_hash);
    user.name = Set(Some(name.to_string()));
    let user = user
        .insert(db)
        .await
        .map_err(|error| eyre!("failed to create installer user `{email}`: {error}"))?;

    RbacService::assign_role_permissions(db, &user.id, &tenant_id, role)
        .await
        .map_err(|error| eyre!("failed to assign installer user role: {error}"))?;

    Ok(AdminOutcome {
        user_id: user.id,
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
    let tenant = tenants::Entity::find_by_slug(db, &plan.tenant.slug)
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

fn resolve_database_url(plan: &InstallPlan) -> Result<String> {
    resolve_secret_value(&plan.database.url, "database URL")
}

fn resolve_admin_password(plan: &InstallPlan) -> Result<String> {
    resolve_secret_value(&plan.admin.password, "admin password")
}

fn resolve_secret_value(secret: &SecretValue, label: &str) -> Result<String> {
    match secret {
        SecretValue::Plaintext { value } => Ok(value.clone()),
        SecretValue::Reference { reference } => resolve_secret_ref(reference, label),
    }
}

fn resolve_secret_ref(reference: &SecretRef, label: &str) -> Result<String> {
    match normalize(&reference.backend).as_str() {
        "env" => std::env::var(&reference.key)
            .map_err(|_| eyre!("{label} env secret `{}` is not set", reference.key)),
        "file" | "mounted_file" | "mounted-file" => read_secret_file(&reference.key, label),
        "dotenv" | "dotenv_file" | "dotenv-file" => read_dotenv_secret(&reference.key, label),
        "external_secret" | "external-secret" | "vault" | "kubernetes" | "k8s" | "aws" | "gcp"
        | "azure" => bail!(
            "{label} secret backend `{}` requires an external secret resolver, which is not implemented yet",
            reference.backend
        ),
        backend => bail!("{label} secret backend `{backend}` is not supported by install apply"),
    }
}

fn read_secret_file(path: &str, label: &str) -> Result<String> {
    let value = std::fs::read_to_string(path)
        .map_err(|error| eyre!("failed to read {label} secret file `{path}`: {error}"))?;
    let value = strip_secret_newline(value);
    if value.is_empty() {
        bail!("{label} secret file `{path}` is empty");
    }
    Ok(value)
}

fn read_dotenv_secret(reference_key: &str, label: &str) -> Result<String> {
    let (path, key) = reference_key
        .split_once('#')
        .unwrap_or((".env", reference_key));
    if path.trim().is_empty() || key.trim().is_empty() {
        bail!("{label} dotenv secret ref must use `dotenv:<path>#<KEY>` or `dotenv:<KEY>`");
    }

    let contents = std::fs::read_to_string(path)
        .map_err(|error| eyre!("failed to read {label} dotenv file `{path}`: {error}"))?;
    let Some(value) = parse_dotenv_value(&contents, key.trim()) else {
        bail!(
            "{label} dotenv key `{}` was not found in `{path}`",
            key.trim()
        );
    };
    if value.is_empty() {
        bail!("{label} dotenv key `{}` in `{path}` is empty", key.trim());
    }
    Ok(value)
}

fn parse_dotenv_value(contents: &str, key: &str) -> Option<String> {
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line).trim_start();
        let Some((candidate, value)) = line.split_once('=') else {
            continue;
        };
        if candidate.trim() != key {
            continue;
        }
        return Some(unquote_dotenv_value(value.trim()));
    }
    None
}

fn unquote_dotenv_value(value: &str) -> String {
    if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        value[1..value.len() - 1].to_string()
    } else {
        strip_secret_newline(value.to_string())
    }
}

fn strip_secret_newline(mut value: String) -> String {
    while value.ends_with('\n') || value.ends_with('\r') {
        value.pop();
    }
    value
}

fn parse_env_var<T>(key: &str, parser: fn(&str) -> Result<T>) -> Option<T> {
    std::env::var(key)
        .ok()
        .and_then(|value| parser(&value).ok())
}

fn take_value(args: &[String], index: &mut usize) -> Result<String> {
    *index += 1;
    args.get(*index)
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .ok_or_else(|| eyre!("install option requires a value"))
}

fn parse_environment(value: &str) -> Result<InstallEnvironment> {
    match normalize(value).as_str() {
        "local" => Ok(InstallEnvironment::Local),
        "demo" => Ok(InstallEnvironment::Demo),
        "test" => Ok(InstallEnvironment::Test),
        "production" | "prod" => Ok(InstallEnvironment::Production),
        _ => bail!("unknown install environment `{value}`"),
    }
}

fn parse_profile(value: &str) -> Result<InstallProfile> {
    match normalize(value).as_str() {
        "dev_local" | "dev-local" | "dev" => Ok(InstallProfile::DevLocal),
        "monolith" => Ok(InstallProfile::Monolith),
        "hybrid_admin" | "hybrid-admin" => Ok(InstallProfile::HybridAdmin),
        "headless_next" | "headless-next" => Ok(InstallProfile::HeadlessNext),
        "headless_leptos" | "headless-leptos" => Ok(InstallProfile::HeadlessLeptos),
        _ => bail!("unknown install profile `{value}`"),
    }
}

fn parse_database_engine(value: &str) -> Result<DatabaseEngine> {
    match normalize(value).as_str() {
        "postgres" | "postgresql" => Ok(DatabaseEngine::Postgres),
        "sqlite" => Ok(DatabaseEngine::Sqlite),
        _ => bail!("unknown database engine `{value}`"),
    }
}

fn parse_seed_profile(value: &str) -> Result<SeedProfile> {
    match normalize(value).as_str() {
        "none" => Ok(SeedProfile::None),
        "minimal" => Ok(SeedProfile::Minimal),
        "dev" => Ok(SeedProfile::Dev),
        _ => bail!("unknown seed profile `{value}`"),
    }
}

fn parse_secret_mode(value: &str) -> Result<SecretMode> {
    match normalize(value).as_str() {
        "env" => Ok(SecretMode::Env),
        "dotenv_file" | "dotenv-file" => Ok(SecretMode::DotenvFile),
        "mounted_file" | "mounted-file" => Ok(SecretMode::MountedFile),
        "external_secret" | "external-secret" => Ok(SecretMode::ExternalSecret),
        _ => bail!("unknown secret mode `{value}`"),
    }
}

fn parse_secret_ref(value: &str) -> Result<SecretRef> {
    let (backend, key) = value
        .split_once(':')
        .ok_or_else(|| eyre!("secret ref must use `<backend>:<key>` format"))?;
    if backend.trim().is_empty() || key.trim().is_empty() {
        bail!("secret ref must include non-empty backend and key");
    }
    Ok(SecretRef {
        backend: backend.trim().to_string(),
        key: key.trim().to_string(),
    })
}

fn normalize(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn print_usage() {
    println!("Usage:");
    println!("  rustok-server install preflight [options]");
    println!("  rustok-server install plan [options]");
    println!("  rustok-server install apply [options]");
    println!();
    println!("Options:");
    println!("  --environment <local|demo|test|production>");
    println!("  --profile <dev-local|monolith|hybrid-admin|headless-next|headless-leptos>");
    println!("  --database-engine <postgres|sqlite>");
    println!("  --database-url <url>");
    println!("  --database-secret-ref <backend:key>");
    println!("  --create-database");
    println!("  --pg-admin-url <url>");
    println!("  --admin-email <email>");
    println!("  --admin-password <value>");
    println!("  --admin-password-ref <backend:key>");
    println!("  --tenant-slug <slug>");
    println!("  --tenant-name <name>");
    println!("  --seed-profile <none|minimal|dev>");
    println!("  --secrets-mode <env|dotenv-file|mounted-file|external-secret>");
    println!("  --enable-module <slug>");
    println!("  --disable-module <slug>");
    println!("  --lock-owner <value>");
    println!("  --lock-ttl-secs <seconds>");
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
