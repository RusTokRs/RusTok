use eyre::{bail, eyre, Result};
use rustok_auth::{AuthUserBootstrapDbWriter, AuthUserBootstrapRequest};
use rustok_installer::{
    execute_seed_profile, InstallAdminOutcome, InstallAdminPort, InstallApplyOptions,
    InstallApplyOutput, InstallDatabasePort, InstallDatabaseReady, InstallExecutionError,
    InstallExecutor, InstallPersistencePort, InstallPlan, InstallReceipt, InstallReceiptRecord,
    InstallSchemaPort, InstallSeedOutcome, InstallSeedPort, InstallSessionRecord, InstallState,
    InstallVerificationOutcome, InstallVerificationPort, SeedExecutionError, SeedExecutionRequest,
    SeedIdentityPort, SeedModulePort, SeedProfile, SeedRolePort, SeedTenant, SeedTenantPort,
    SeedTenantRequest, SeedUser, SeedUserRequest, TenantBootstrap,
};
use rustok_installer_persistence::SeaOrmInstallerPorts;
use rustok_migrations::Migrator;
use rustok_modules::ModuleLifecycleDbWriter;
use rustok_rbac::RbacRoleAssignmentDbWriter;
use rustok_tenant::{
    PortActor, PortContext, PortErrorKind, TenantReadPort, TenantReadProjection, TenantReadRequest,
    TenantReadSelector, TenantService,
};
use sea_orm::DatabaseConnection;
use std::time::Duration;
use uuid::Uuid;

use crate::models::users;
use crate::modules::build_registry;
use crate::services::effective_module_policy::EffectiveModulePolicyService;
use crate::services::rbac_service::RbacService;

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
        InstallDatabasePort::prepare_database(&SeaOrmInstallerPorts, plan, database_url, options)
            .await
    }
}

#[async_trait::async_trait]
impl InstallSchemaPort<DatabaseConnection> for ServerInstallerPorts {
    async fn apply_schema(
        &self,
        runtime: &DatabaseConnection,
    ) -> std::result::Result<(), InstallExecutionError> {
        InstallSchemaPort::apply_schema(&SeaOrmInstallerPorts, runtime).await
    }
}

#[async_trait::async_trait]
impl InstallPersistencePort<DatabaseConnection> for ServerInstallerPorts {
    async fn create_session(
        &self,
        runtime: &DatabaseConnection,
        plan: &InstallPlan,
    ) -> std::result::Result<InstallSessionRecord, InstallExecutionError> {
        InstallPersistencePort::create_session(&SeaOrmInstallerPorts, runtime, plan).await
    }

    async fn acquire_lock(
        &self,
        runtime: &DatabaseConnection,
        session: InstallSessionRecord,
        owner: &str,
        ttl_secs: i64,
    ) -> std::result::Result<InstallSessionRecord, InstallExecutionError> {
        InstallPersistencePort::acquire_lock(
            &SeaOrmInstallerPorts,
            runtime,
            session,
            owner,
            ttl_secs,
        )
        .await
    }

    async fn record_receipt(
        &self,
        runtime: &DatabaseConnection,
        receipt: &InstallReceipt,
    ) -> std::result::Result<InstallReceiptRecord, InstallExecutionError> {
        InstallPersistencePort::record_receipt(&SeaOrmInstallerPorts, runtime, receipt).await
    }

    async fn set_state(
        &self,
        runtime: &DatabaseConnection,
        session_id: Uuid,
        state: InstallState,
    ) -> std::result::Result<InstallSessionRecord, InstallExecutionError> {
        InstallPersistencePort::set_state(&SeaOrmInstallerPorts, runtime, session_id, state).await
    }

    async fn set_tenant_id(
        &self,
        runtime: &DatabaseConnection,
        session_id: Uuid,
        tenant_id: Uuid,
    ) -> std::result::Result<InstallSessionRecord, InstallExecutionError> {
        InstallPersistencePort::set_tenant_id(&SeaOrmInstallerPorts, runtime, session_id, tenant_id)
            .await
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
    defaults: Vec<String>,
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
        ModuleLifecycleDbWriter::new(self.db.clone(), &self.registry, self.defaults.clone())
            .toggle(tenant_id, module_slug, enabled, actor)
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
        defaults: plan.seed_profile.default_enabled_modules(),
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
