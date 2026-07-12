//! Consumer-owned ports for applying an installer seed profile.

use async_trait::async_trait;
use rustok_core::UserRole;
use thiserror::Error;
use uuid::Uuid;

use crate::SeedProfile;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedTenantRequest {
    pub name: String,
    pub slug: String,
    pub domain: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedTenant {
    pub id: Uuid,
    pub slug: String,
    pub created: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedUserRequest {
    pub tenant_id: Uuid,
    pub email: String,
    pub name: String,
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedUser {
    pub id: Uuid,
    pub email: String,
    pub created: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedExecutionRequest {
    pub profile: SeedProfile,
    pub tenant: SeedTenantRequest,
    pub enabled_modules: Vec<String>,
    pub disabled_modules: Vec<String>,
    pub admin: Option<SeedUserRequest>,
    pub demo_customer_password: Option<String>,
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedExecutionOutcome {
    pub tenant: SeedTenant,
    pub enabled_modules: Vec<String>,
    pub disabled_modules: Vec<String>,
    pub admin: Option<SeedUser>,
    pub demo_customer: Option<SeedUser>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SeedExecutionError {
    #[error("seed request is invalid: {0}")]
    Validation(String),
    #[error("seed dependency failed: {0}")]
    Dependency(String),
}

#[async_trait]
pub trait SeedTenantPort: Send + Sync {
    async fn ensure_seed_tenant(
        &self,
        request: SeedTenantRequest,
    ) -> Result<SeedTenant, SeedExecutionError>;
}

#[async_trait]
pub trait SeedIdentityPort: Send + Sync {
    async fn ensure_seed_user(
        &self,
        request: SeedUserRequest,
    ) -> Result<SeedUser, SeedExecutionError>;
}

#[async_trait]
pub trait SeedRolePort: Send + Sync {
    async fn assign_seed_role(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        role: UserRole,
    ) -> Result<(), SeedExecutionError>;
}

#[async_trait]
pub trait SeedModulePort: Send + Sync {
    async fn set_seed_module_enabled(
        &self,
        tenant_id: Uuid,
        module_slug: &str,
        enabled: bool,
        actor: &str,
    ) -> Result<(), SeedExecutionError>;
}

pub async fn execute_seed_profile(
    request: SeedExecutionRequest,
    tenant_port: &dyn SeedTenantPort,
    identity_port: &dyn SeedIdentityPort,
    role_port: &dyn SeedRolePort,
    module_port: &dyn SeedModulePort,
) -> Result<SeedExecutionOutcome, SeedExecutionError> {
    validate_request(&request)?;
    let tenant = tenant_port.ensure_seed_tenant(request.tenant).await?;

    let mut enabled_modules = request.enabled_modules;
    enabled_modules.sort();
    enabled_modules.dedup();
    let mut disabled_modules = request.disabled_modules;
    disabled_modules.sort();
    disabled_modules.dedup();
    enabled_modules.retain(|module| !disabled_modules.contains(module));

    for module in &enabled_modules {
        module_port
            .set_seed_module_enabled(tenant.id, module, true, &request.actor)
            .await?;
    }
    for module in &disabled_modules {
        module_port
            .set_seed_module_enabled(tenant.id, module, false, &request.actor)
            .await?;
    }

    let admin = if let Some(mut admin) = request.admin {
        admin.tenant_id = tenant.id;
        let user = identity_port.ensure_seed_user(admin).await?;
        role_port
            .assign_seed_role(tenant.id, user.id, UserRole::SuperAdmin)
            .await?;
        Some(user)
    } else {
        None
    };

    let demo_customer = if request.profile == SeedProfile::Dev {
        let password = request.demo_customer_password.ok_or_else(|| {
            SeedExecutionError::Validation(
                "development seed profile requires a demo customer password".to_string(),
            )
        })?;
        let user = identity_port
            .ensure_seed_user(SeedUserRequest {
                tenant_id: tenant.id,
                email: "customer@demo.local".to_string(),
                name: "Demo Customer".to_string(),
                password,
            })
            .await?;
        role_port
            .assign_seed_role(tenant.id, user.id, UserRole::Customer)
            .await?;
        Some(user)
    } else {
        None
    };

    Ok(SeedExecutionOutcome {
        tenant,
        enabled_modules,
        disabled_modules,
        admin,
        demo_customer,
    })
}

fn validate_request(request: &SeedExecutionRequest) -> Result<(), SeedExecutionError> {
    if request.tenant.name.trim().is_empty() || request.tenant.slug.trim().is_empty() {
        return Err(SeedExecutionError::Validation(
            "seed tenant requires non-empty name and slug".to_string(),
        ));
    }
    if request.actor.trim().is_empty() {
        return Err(SeedExecutionError::Validation(
            "seed request requires a non-empty actor".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use uuid::Uuid;

    use super::{
        execute_seed_profile, SeedExecutionError, SeedExecutionRequest, SeedIdentityPort,
        SeedModulePort, SeedRolePort, SeedTenant, SeedTenantPort, SeedTenantRequest, SeedUser,
        SeedUserRequest,
    };
    use crate::SeedProfile;

    struct TenantPort;
    struct IdentityPort;
    struct RolePort;
    struct ModulePort;

    #[async_trait]
    impl SeedTenantPort for TenantPort {
        async fn ensure_seed_tenant(
            &self,
            request: SeedTenantRequest,
        ) -> Result<SeedTenant, SeedExecutionError> {
            Ok(SeedTenant {
                id: Uuid::nil(),
                slug: request.slug,
                created: true,
            })
        }
    }

    #[async_trait]
    impl SeedIdentityPort for IdentityPort {
        async fn ensure_seed_user(
            &self,
            request: SeedUserRequest,
        ) -> Result<SeedUser, SeedExecutionError> {
            Ok(SeedUser {
                id: Uuid::nil(),
                email: request.email,
                created: true,
            })
        }
    }

    #[async_trait]
    impl SeedRolePort for RolePort {
        async fn assign_seed_role(
            &self,
            _tenant_id: Uuid,
            _user_id: Uuid,
            _role: rustok_core::UserRole,
        ) -> Result<(), SeedExecutionError> {
            Ok(())
        }
    }

    #[async_trait]
    impl SeedModulePort for ModulePort {
        async fn set_seed_module_enabled(
            &self,
            _tenant_id: Uuid,
            _module_slug: &str,
            _enabled: bool,
            _actor: &str,
        ) -> Result<(), SeedExecutionError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn development_profile_deduplicates_module_selection_and_creates_demo_customer() {
        let outcome = execute_seed_profile(
            SeedExecutionRequest {
                profile: SeedProfile::Dev,
                tenant: SeedTenantRequest {
                    name: "Demo".to_string(),
                    slug: "demo".to_string(),
                    domain: None,
                },
                enabled_modules: vec!["pages".to_string(), "blog".to_string(), "pages".to_string()],
                disabled_modules: vec!["blog".to_string()],
                admin: None,
                demo_customer_password: Some("password".to_string()),
                actor: "installer".to_string(),
            },
            &TenantPort,
            &IdentityPort,
            &RolePort,
            &ModulePort,
        )
        .await
        .unwrap();

        assert_eq!(outcome.enabled_modules, vec!["pages".to_string()]);
        assert_eq!(outcome.disabled_modules, vec!["blog".to_string()]);
        assert_eq!(outcome.demo_customer.unwrap().email, "customer@demo.local");
    }
}
