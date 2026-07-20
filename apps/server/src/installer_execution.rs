use eyre::{Result, bail, eyre};
use rustok_installer::{
    InstallAdminOutcome, InstallAdminPort, InstallApplyOptions, InstallApplyOutput,
    InstallDatabasePort, InstallDatabaseReady, InstallExecutionError, InstallExecutor,
    InstallPersistencePort, InstallPlan, InstallReceipt, InstallReceiptRecord, InstallSchemaPort,
    InstallSeedOutcome, InstallSeedPort, InstallSessionRecord, InstallState,
    InstallVerificationOutcome, InstallVerificationPort,
};
use rustok_installer_persistence::{SeaOrmInstallerBootstrapPorts, SeaOrmInstallerPorts};
use rustok_tenant::{
    PortActor, PortContext, PortErrorKind, TenantReadPort, TenantReadProjection, TenantReadRequest,
    TenantReadSelector, TenantService,
};
use sea_orm::DatabaseConnection;
use std::time::Duration;
use uuid::Uuid;

use crate::installer_deployment::ServerInstallerDeploymentAdapter;
use crate::models::users;
use crate::services::effective_module_policy::EffectiveModulePolicyService;

const INSTALLER_TENANT_READ_DEADLINE: Duration = Duration::from_secs(15);

pub async fn apply_plan(
    plan: InstallPlan,
    options: InstallApplyOptions,
    build_settings: crate::common::settings::BuildRuntimeSettings,
    registry: rustok_core::ModuleRegistry,
) -> Result<InstallApplyOutput> {
    let ports = ServerInstallerPorts {
        registry,
        deployment: ServerInstallerDeploymentAdapter::new(build_settings),
    };
    rustok_installer::execute_install_apply(&ports, plan, options)
        .await
        .map_err(|error| eyre!(error.to_string()))
}

/// HTTP-host adapter for the portable installer executor contract.
#[derive(Clone)]
pub struct ServerInstallExecutor {
    build_settings: crate::common::settings::BuildRuntimeSettings,
    registry: rustok_core::ModuleRegistry,
}

impl ServerInstallExecutor {
    pub fn new(
        build_settings: crate::common::settings::BuildRuntimeSettings,
        registry: rustok_core::ModuleRegistry,
    ) -> Self {
        Self {
            build_settings,
            registry,
        }
    }
}

#[async_trait::async_trait]
impl InstallExecutor for ServerInstallExecutor {
    async fn apply(
        &self,
        plan: InstallPlan,
        options: InstallApplyOptions,
    ) -> std::result::Result<InstallApplyOutput, InstallExecutionError> {
        apply_plan(
            plan,
            options,
            self.build_settings.clone(),
            self.registry.clone(),
        )
        .await
        .map_err(|error| InstallExecutionError::new(error.to_string()))
    }
}

/// Host composition supplies the module registry; every SeaORM mutation is
/// delegated to the reusable installer persistence adapter.
struct ServerInstallerPorts {
    registry: rustok_core::ModuleRegistry,
    deployment: ServerInstallerDeploymentAdapter,
}

#[async_trait::async_trait]
impl rustok_installer::InstallDeploymentPort<DatabaseConnection> for ServerInstallerPorts {
    fn supports_distributed_deployment(&self) -> bool {
        rustok_installer::InstallDeploymentPort::supports_distributed_deployment(&self.deployment)
    }

    async fn deploy_role(
        &self,
        runtime: &DatabaseConnection,
        request: rustok_installer::InstallRoleDeploymentRequest,
    ) -> std::result::Result<rustok_installer::InstallRoleDeployment, InstallExecutionError> {
        rustok_installer::InstallDeploymentPort::deploy_role(&self.deployment, runtime, request)
            .await
    }
}

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
        SeaOrmInstallerBootstrapPorts::new(
            runtime.clone(),
            &self.registry,
            plan.seed_profile.default_enabled_modules(),
        )
        .apply_seed_profile(plan, "installer")
        .await
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
        SeaOrmInstallerBootstrapPorts::new(
            runtime.clone(),
            &self.registry,
            plan.seed_profile.default_enabled_modules(),
        )
        .provision_admin(plan, tenant_id, password)
        .await
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
        verify_installation(runtime, plan, tenant_id, &self.registry)
            .await
            .map_err(|error| InstallExecutionError::new(error.to_string()))
    }
}

async fn verify_installation(
    db: &DatabaseConnection,
    plan: &InstallPlan,
    tenant_id: Uuid,
    registry: &rustok_core::ModuleRegistry,
) -> Result<InstallVerificationOutcome> {
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
    let enabled_modules = EffectiveModulePolicyService::list_enabled(db, registry, tenant.id)
        .await
        .map_err(|error| eyre!("failed to verify installer module enablement: {error}"))?;

    Ok(InstallVerificationOutcome {
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
        Ok(tenant) => Ok(Some(tenant)),
        // treat missing tenant as create candidate; all other port errors surface.
        Err(error) if error.kind == PortErrorKind::NotFound => Ok(None),
        Err(error) => Err(eyre!(
            "tenant read projection `{slug}` failed through TenantReadPort ({}): {}",
            error.code,
            error.message
        )),
    }
}
