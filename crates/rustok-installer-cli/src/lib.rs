use rustok_auth::{AuthUserBootstrapDbWriter, AuthUserBootstrapRequest};
use rustok_cli_core::{
    CliCoreError, CliCoreResult, CommandDescriptor, CommandOutcome, CommandProvider, CommandRequest,
};
use rustok_installer::{
    execute_seed_profile, SeedExecutionError, SeedExecutionRequest, SeedIdentityPort,
    SeedModulePort, SeedProfile, SeedRolePort, SeedTenant, SeedTenantPort, SeedTenantRequest,
    SeedUser, SeedUserRequest,
};
use rustok_modules::ModuleLifecycleDbWriter;
use rustok_rbac::RbacRoleAssignmentDbWriter;
use rustok_runtime::{db_clone, RuntimeComposition};
use rustok_tenant::{CreateTenantInput, TenantService};

pub struct InstallerCommandProvider {
    runtime: RuntimeComposition,
}

pub fn command_provider(runtime: &RuntimeComposition) -> Box<dyn CommandProvider> {
    Box::new(InstallerCommandProvider {
        runtime: runtime.clone(),
    })
}

#[async_trait::async_trait]
impl CommandProvider for InstallerCommandProvider {
    fn commands(&self) -> Vec<CommandDescriptor> {
        vec![CommandDescriptor::new(
            "seed",
            "apply",
            "Apply a typed tenant seed profile",
        )]
    }

    async fn execute(&self, request: CommandRequest) -> CliCoreResult<CommandOutcome> {
        if (request.namespace.as_str(), request.name.as_str()) != ("seed", "apply") {
            return Err(CliCoreError::UnknownCommand {
                namespace: request.namespace,
                name: request.name,
            });
        }
        if request.dry_run {
            return Ok(CommandOutcome::success(
                "Seed profile validated; dry run does not mutate state.",
            ));
        }
        let db = db_clone(
            self.runtime
                .require_host()
                .map_err(|error| failed(error.to_string()))?,
        );
        let options = &request.args["options"];
        let profile = option(options, "profile")
            .as_deref()
            .map(SeedProfile::parse_cli_value)
            .transpose()
            .map_err(input)?
            .unwrap_or(SeedProfile::Dev);
        let password = option(options, "password")
            .or_else(|| environment("SEED_ADMIN_PASSWORD"))
            .or_else(|| environment("SUPERADMIN_PASSWORD"))
            .ok_or_else(|| {
                input("seed apply requires --password, SEED_ADMIN_PASSWORD, or SUPERADMIN_PASSWORD")
            })?;
        let tenant = SeedTenantRequest {
            name: option(options, "tenant_name").unwrap_or_else(|| "Demo Workspace".to_string()),
            slug: option(options, "tenant_slug").unwrap_or_else(|| "demo".to_string()),
            domain: option(options, "tenant_domain"),
        };
        let admin = Some(SeedUserRequest {
            tenant_id: uuid::Uuid::nil(),
            email: option(options, "email").unwrap_or_else(|| "admin@demo.local".to_string()),
            name: option(options, "name").unwrap_or_else(|| "Super Admin".to_string()),
            password: password.clone(),
        });
        let registry = rustok_distribution::build_registry();
        let tenant_port = TenantPort { db: db.clone() };
        let identity_port = IdentityPort { db: db.clone() };
        let role_port = RolePort { db: db.clone() };
        let module_port = ModulePort {
            db: db.clone(),
            registry: &registry,
            defaults: profile.default_enabled_modules(),
        };
        let result = execute_seed_profile(
            SeedExecutionRequest {
                profile,
                tenant,
                enabled_modules: profile.default_enabled_modules(),
                disabled_modules: Vec::new(),
                admin,
                demo_customer_password: Some(password),
                actor: "rustok-cli seed apply".to_string(),
            },
            &tenant_port,
            &identity_port,
            &role_port,
            &module_port,
        )
        .await
        .map_err(failed)?;
        Ok(CommandOutcome::success("Seed profile applied").with_data(serde_json::json!({ "tenant_id": result.tenant.id, "tenant_slug": result.tenant.slug, "enabled_modules": result.enabled_modules })))
    }
}

struct TenantPort {
    db: sea_orm::DatabaseConnection,
}
struct IdentityPort {
    db: sea_orm::DatabaseConnection,
}
struct RolePort {
    db: sea_orm::DatabaseConnection,
}
struct ModulePort<'a> {
    db: sea_orm::DatabaseConnection,
    registry: &'a rustok_core::ModuleRegistry,
    defaults: Vec<String>,
}

#[async_trait::async_trait]
impl SeedTenantPort for TenantPort {
    async fn ensure_seed_tenant(
        &self,
        request: SeedTenantRequest,
    ) -> Result<SeedTenant, SeedExecutionError> {
        let (tenant, created) = TenantService::new(self.db.clone())
            .ensure_tenant(CreateTenantInput {
                name: request.name,
                slug: request.slug,
                domain: request.domain,
            })
            .await
            .map_err(seed_error)?;
        Ok(SeedTenant {
            id: tenant.id,
            slug: tenant.slug,
            created,
        })
    }
}
#[async_trait::async_trait]
impl SeedIdentityPort for IdentityPort {
    async fn ensure_seed_user(
        &self,
        request: SeedUserRequest,
    ) -> Result<SeedUser, SeedExecutionError> {
        let user = AuthUserBootstrapDbWriter::new(self.db.clone())
            .ensure_user(AuthUserBootstrapRequest {
                tenant_id: request.tenant_id,
                email: request.email,
                name: request.name,
                password: request.password,
            })
            .await
            .map_err(seed_error)?;
        Ok(SeedUser {
            id: user.id,
            email: user.email,
            created: user.created,
        })
    }
}
#[async_trait::async_trait]
impl SeedRolePort for RolePort {
    async fn assign_seed_role(
        &self,
        tenant_id: uuid::Uuid,
        user_id: uuid::Uuid,
        role: rustok_core::UserRole,
    ) -> Result<(), SeedExecutionError> {
        RbacRoleAssignmentDbWriter::new(self.db.clone())
            .assign_role_permissions(tenant_id, user_id, role)
            .await
            .map_err(seed_error)
    }
}
#[async_trait::async_trait]
impl SeedModulePort for ModulePort<'_> {
    async fn set_seed_module_enabled(
        &self,
        tenant_id: uuid::Uuid,
        module_slug: &str,
        enabled: bool,
        actor: &str,
    ) -> Result<(), SeedExecutionError> {
        ModuleLifecycleDbWriter::new(self.db.clone(), self.registry, self.defaults.clone())
            .toggle(tenant_id, module_slug, enabled, actor)
            .await
            .map_err(seed_error)
    }
}

fn option(options: &serde_json::Value, name: &str) -> Option<String> {
    options
        .get(name)
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string)
        .filter(|value| !value.trim().is_empty())
}
fn environment(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}
fn input(message: impl Into<String>) -> CliCoreError {
    CliCoreError::InvalidInput {
        message: message.into(),
    }
}
fn failed(error: impl std::fmt::Display) -> CliCoreError {
    CliCoreError::CommandFailed {
        message: error.to_string(),
    }
}
fn seed_error(error: impl std::fmt::Display) -> SeedExecutionError {
    SeedExecutionError::Dependency(error.to_string())
}
