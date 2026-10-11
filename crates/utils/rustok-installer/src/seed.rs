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
    /// Optional seed data path for content import (e.g., "seeds")
    pub seed_data_path: Option<String>,
    /// Continue seed execution even if content loading fails for some modules
    pub continue_on_content_error: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedExecutionOutcome {
    pub tenant: SeedTenant,
    pub enabled_modules: Vec<String>,
    pub disabled_modules: Vec<String>,
    pub admin: Option<SeedUser>,
    pub demo_customer: Option<SeedUser>,
    /// Content import outcomes per module
    pub content_outcomes: Vec<SeedContentOutcome>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SeedExecutionError {
    #[error("seed request is invalid: {0}")]
    Validation(String),
    #[error("seed dependency failed: {0}")]
    Dependency(String),
    #[error("seed content loading failed: {0}")]
    ContentLoading(String),
}

/// Request to load seed content for a specific module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedContentRequest {
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub module_slug: String,
    pub seed_data_path: String,
}

/// Outcome of seed content loading for a single module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedContentOutcome {
    pub module_slug: String,
    pub imported_count: usize,
    pub failed_count: usize,
    pub errors: Vec<String>,
}

impl SeedContentOutcome {
    pub fn success(module_slug: String, imported_count: usize) -> Self {
        Self {
            module_slug,
            imported_count,
            failed_count: 0,
            errors: Vec::new(),
        }
    }

    pub fn is_success(&self) -> bool {
        self.failed_count == 0
    }

    pub fn total_count(&self) -> usize {
        self.imported_count + self.failed_count
    }

    pub fn success_rate(&self) -> f64 {
        let total = self.total_count();
        if total == 0 {
            0.0
        } else {
            (self.imported_count as f64 / total as f64) * 100.0
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SeedContentError {
    #[error("seed content loading failed: {0}")]
    Loading(String),
    #[error("seed content import failed: {0}")]
    Import(String),
    #[error("seed data not found: {0}")]
    NotFound(String),
}

/// Port for loading seed content into a specific module.
///
/// Each module that wants to provide demo data should implement this trait.
/// The implementation should load seed data from files and use ContentImporter
/// to import it into the database.
#[async_trait]
pub trait SeedContentPort: Send + Sync {
    /// Load seed content for the module
    async fn load_seed_content(
        &self,
        request: SeedContentRequest,
    ) -> Result<SeedContentOutcome, SeedContentError>;

    /// Check if this loader has seed data for the given module
    fn has_seed_data(&self, module_slug: &str) -> bool;
}

#[async_trait]
pub trait SeedTenantPort: Send + Sync {
    async fn ensure_seed_tenant(
        &self,
        request: SeedTenantRequest,
    ) -> Result<SeedTenant, SeedExecutionError>;
}

/// Atomic identity and role provisioning boundary for installer seed users.
///
/// Implementations must not return success unless both the identity and its
/// requested RBAC role are durable. Database adapters should use one transaction.
#[async_trait]
pub trait SeedPrincipalPort: Send + Sync {
    async fn ensure_seed_principal(
        &self,
        request: SeedUserRequest,
        role: UserRole,
    ) -> Result<SeedUser, SeedExecutionError>;
}

/// Legacy split identity port retained for adapters outside the typed executor.
#[async_trait]
pub trait SeedIdentityPort: Send + Sync {
    async fn ensure_seed_user(
        &self,
        request: SeedUserRequest,
    ) -> Result<SeedUser, SeedExecutionError>;
}

/// Legacy split role port retained for adapters outside the typed executor.
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
    principal_port: &dyn SeedPrincipalPort,
    module_port: &dyn SeedModulePort,
    content_port: Option<&dyn SeedContentPort>,
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
        Some(
            principal_port
                .ensure_seed_principal(admin, UserRole::SuperAdmin)
                .await?,
        )
    } else {
        None
    };

    let demo_customer = if request.profile == SeedProfile::Dev {
        let password = request.demo_customer_password.ok_or_else(|| {
            SeedExecutionError::Validation(
                "development seed profile requires a demo customer password".to_string(),
            )
        })?;
        Some(
            principal_port
                .ensure_seed_principal(
                    SeedUserRequest {
                        tenant_id: tenant.id,
                        email: "customer@demo.local".to_string(),
                        name: "Demo Customer".to_string(),
                        password,
                    },
                    UserRole::Customer,
                )
                .await?,
        )
    } else {
        None
    };

    // Load seed content for Dev profile
    let mut content_outcomes = Vec::new();
    if request.profile == SeedProfile::Dev
        && let (Some(content_port), Some(seed_data_path)) = (content_port, &request.seed_data_path)
    {
        // Validate that admin user exists for content import
        let user_id = match &admin {
            Some(admin_user) => admin_user.id,
            None => {
                return Err(SeedExecutionError::Validation(
                    "seed content loading requires an admin user to be created".to_string(),
                ));
            }
        };

        for module_slug in &enabled_modules {
            if !content_port.has_seed_data(module_slug) {
                continue;
            }

            let content_request = SeedContentRequest {
                tenant_id: tenant.id,
                user_id,
                module_slug: module_slug.clone(),
                seed_data_path: format!("{}/{}", seed_data_path, module_slug),
            };

            match content_port.load_seed_content(content_request).await {
                Ok(outcome) => content_outcomes.push(outcome),
                Err(e) => {
                    let error_msg =
                        format!("failed to load seed content for {}: {}", module_slug, e);

                    if request.continue_on_content_error {
                        // Log error but continue with other modules
                        content_outcomes.push(SeedContentOutcome {
                            module_slug: module_slug.clone(),
                            imported_count: 0,
                            failed_count: 0,
                            errors: vec![error_msg],
                        });
                    } else {
                        return Err(SeedExecutionError::ContentLoading(error_msg));
                    }
                }
            }
        }
    }

    Ok(SeedExecutionOutcome {
        tenant,
        enabled_modules,
        disabled_modules,
        admin,
        demo_customer,
        content_outcomes,
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
        SeedContentError, SeedContentOutcome, SeedContentPort, SeedContentRequest,
        SeedExecutionError, SeedExecutionRequest, SeedModulePort, SeedPrincipalPort, SeedTenant,
        SeedTenantPort, SeedTenantRequest, SeedUser, SeedUserRequest, execute_seed_profile,
    };
    use crate::SeedProfile;

    struct TenantPort;
    struct PrincipalPort;
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
    impl SeedPrincipalPort for PrincipalPort {
        async fn ensure_seed_principal(
            &self,
            request: SeedUserRequest,
            _role: rustok_core::UserRole,
        ) -> Result<SeedUser, SeedExecutionError> {
            Ok(SeedUser {
                id: Uuid::nil(),
                email: request.email,
                created: true,
            })
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
                seed_data_path: None,
                continue_on_content_error: false,
            },
            &TenantPort,
            &PrincipalPort,
            &ModulePort,
            None,
        )
        .await
        .unwrap();

        assert_eq!(outcome.enabled_modules, vec!["pages".to_string()]);
        assert_eq!(outcome.disabled_modules, vec!["blog".to_string()]);
        assert_eq!(outcome.demo_customer.unwrap().email, "customer@demo.local");
        assert!(outcome.content_outcomes.is_empty());
    }

    #[tokio::test]
    async fn development_profile_loads_seed_content_when_port_provided() {
        struct MockContentPort;

        #[async_trait]
        impl SeedContentPort for MockContentPort {
            async fn load_seed_content(
                &self,
                request: SeedContentRequest,
            ) -> Result<SeedContentOutcome, SeedContentError> {
                Ok(SeedContentOutcome::success(request.module_slug, 10))
            }

            fn has_seed_data(&self, module_slug: &str) -> bool {
                module_slug == "blog"
            }
        }

        let outcome = execute_seed_profile(
            SeedExecutionRequest {
                profile: SeedProfile::Dev,
                tenant: SeedTenantRequest {
                    name: "Demo".to_string(),
                    slug: "demo".to_string(),
                    domain: None,
                },
                enabled_modules: vec!["blog".to_string(), "newsletter".to_string()],
                disabled_modules: vec![],
                admin: Some(SeedUserRequest {
                    tenant_id: Uuid::nil(),
                    email: "admin@demo.local".to_string(),
                    name: "Admin".to_string(),
                    password: "password".to_string(),
                }),
                demo_customer_password: Some("password".to_string()),
                actor: "installer".to_string(),
                seed_data_path: Some("seeds".to_string()),
                continue_on_content_error: false,
            },
            &TenantPort,
            &PrincipalPort,
            &ModulePort,
            Some(&MockContentPort),
        )
        .await
        .unwrap();

        assert_eq!(outcome.content_outcomes.len(), 1);
        assert_eq!(outcome.content_outcomes[0].module_slug, "blog");
        assert_eq!(outcome.content_outcomes[0].imported_count, 10);
        assert!(outcome.content_outcomes[0].is_success());
    }

    #[tokio::test]
    async fn development_profile_fails_content_loading_without_admin() {
        struct MockContentPort;

        #[async_trait]
        impl SeedContentPort for MockContentPort {
            async fn load_seed_content(
                &self,
                _request: SeedContentRequest,
            ) -> Result<SeedContentOutcome, SeedContentError> {
                Ok(SeedContentOutcome::success("test".to_string(), 10))
            }

            fn has_seed_data(&self, _module_slug: &str) -> bool {
                true
            }
        }

        let result = execute_seed_profile(
            SeedExecutionRequest {
                profile: SeedProfile::Dev,
                tenant: SeedTenantRequest {
                    name: "Demo".to_string(),
                    slug: "demo".to_string(),
                    domain: None,
                },
                enabled_modules: vec!["blog".to_string()],
                disabled_modules: vec![],
                admin: None, // No admin user
                demo_customer_password: Some("password".to_string()),
                actor: "installer".to_string(),
                seed_data_path: Some("seeds".to_string()),
                continue_on_content_error: false,
            },
            &TenantPort,
            &PrincipalPort,
            &ModulePort,
            Some(&MockContentPort),
        )
        .await;

        assert!(result.is_err());
        match result {
            Err(SeedExecutionError::Validation(msg)) => {
                assert!(msg.contains("admin user"));
            }
            _ => panic!("Expected Validation error"),
        }
    }

    #[tokio::test]
    async fn development_profile_continues_on_content_error_when_enabled() {
        struct FailingContentPort;

        #[async_trait]
        impl SeedContentPort for FailingContentPort {
            async fn load_seed_content(
                &self,
                _request: SeedContentRequest,
            ) -> Result<SeedContentOutcome, SeedContentError> {
                Err(SeedContentError::Loading("mock failure".to_string()))
            }

            fn has_seed_data(&self, _module_slug: &str) -> bool {
                true
            }
        }

        let outcome = execute_seed_profile(
            SeedExecutionRequest {
                profile: SeedProfile::Dev,
                tenant: SeedTenantRequest {
                    name: "Demo".to_string(),
                    slug: "demo".to_string(),
                    domain: None,
                },
                enabled_modules: vec!["blog".to_string(), "newsletter".to_string()],
                disabled_modules: vec![],
                admin: Some(SeedUserRequest {
                    tenant_id: Uuid::nil(),
                    email: "admin@demo.local".to_string(),
                    name: "Admin".to_string(),
                    password: "password".to_string(),
                }),
                demo_customer_password: Some("password".to_string()),
                actor: "installer".to_string(),
                seed_data_path: Some("seeds".to_string()),
                continue_on_content_error: true, // Continue on error
            },
            &TenantPort,
            &PrincipalPort,
            &ModulePort,
            Some(&FailingContentPort),
        )
        .await
        .unwrap();

        // Should have 2 outcomes (one per enabled module), both with errors
        assert_eq!(outcome.content_outcomes.len(), 2);
        assert!(!outcome.content_outcomes[0].errors.is_empty());
        assert!(!outcome.content_outcomes[1].errors.is_empty());
    }
}
