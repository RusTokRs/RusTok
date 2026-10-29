#[cfg(feature = "server")]
use rustok_core::{
    MigrationSource, ModuleKind, ModuleRuntimeExtensions, Result as RusToKResult, RusToKModule,
};
#[cfg(feature = "server")]
use sea_orm_migration::MigrationTrait;

/// Native module definition for AI capability.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AiModule;

#[cfg(feature = "server")]
impl MigrationSource for AiModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        crate::migrations::migrations()
    }
}

#[cfg(feature = "server")]
#[async_trait::async_trait]
impl RusToKModule for AiModule {
    fn slug(&self) -> &'static str {
        "ai"
    }

    fn name(&self) -> &'static str {
        "AI"
    }

    fn description(&self) -> &'static str {
        "Rig-based AI orchestration capability"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    /// AI is composed at deployment scope. Tenant profiles and principals are
    /// still tenant-scoped, but a tenant-module toggle must never remove the
    /// generic runtime handles or durable worker from a running deployment.
    fn kind(&self) -> ModuleKind {
        ModuleKind::Core
    }

    fn register_runtime_extensions(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
    ) -> RusToKResult<()> {
        let deployment =
            crate::runtime_extensions::AiDeploymentRuntime::from_environment().map_err(|error| {
                rustok_core::Error::Validation(format!(
                    "invalid deployment-owned AI runtime configuration: {error}"
                ))
            })?;
        extensions.insert(deployment.secret_registry);
        extensions.insert(deployment.egress_policy);
        extensions.insert(deployment.provider_targets);
        if let Some(result_keyring) = deployment.result_keyring {
            extensions.insert(result_keyring);
        }
        extensions
            .get_or_insert_with::<rustok_runtime::ModuleWorkRegistrations, _>(Default::default)
            .register(std::sync::Arc::new(
                crate::scheduler::AiAgentWorkflowWorkRegistration,
            ));
        Ok(())
    }
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use rustok_core::{ModuleKind, RusToKModule};

    #[test]
    fn ai_module_is_deployment_scoped_and_globally_active() {
        let module = super::AiModule;
        assert_eq!(module.slug(), "ai");
        assert_eq!(module.kind(), ModuleKind::Core);
    }
}
