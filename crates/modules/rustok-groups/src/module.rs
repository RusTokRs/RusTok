use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::ModuleRuntimeExtensions;
use rustok_core::module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};
use rustok_moderation::register_moderation_subject_adapter_factory;
use rustok_notifications::register_notification_source_provider_factory;
use sea_orm_migration::MigrationTrait;

use crate::migrations;
use crate::moderation_subject;
use crate::notification_source;
use crate::ports::GroupCapabilityDescriptor;

/// Social group identity, membership, privacy, and modular feature owner.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GroupsModule;

impl GroupsModule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl RusToKModule for GroupsModule {
    fn slug(&self) -> &'static str {
        "groups"
    }

    fn name(&self) -> &'static str {
        "Groups"
    }

    fn description(&self) -> &'static str {
        "Social groups, memberships, local roles, privacy, and feature bindings"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Optional
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::GROUPS_CREATE,
            Permission::GROUPS_READ,
            Permission::GROUPS_UPDATE,
            Permission::GROUPS_DELETE,
            Permission::GROUPS_LIST,
            Permission::GROUPS_MODERATE,
            Permission::GROUPS_MANAGE,
        ]
    }

    fn register_runtime_extensions(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
    ) -> rustok_core::Result<()> {
        extensions.insert(GroupCapabilityDescriptor::default());
        register_notification_source_provider_factory(
            extensions,
            notification_source::GroupsNotificationSourceProviderFactory,
        )
        .map_err(|error| {
            rustok_core::Error::Validation(format!(
                "groups notification source factory registration failed: {error}"
            ))
        })?;
        register_moderation_subject_adapter_factory(
            extensions,
            moderation_subject::GroupsModerationSubjectAdapterFactory,
        )
        .map_err(|error| {
            rustok_core::Error::Validation(format!(
                "groups moderation subject factory registration failed: {error}"
            ))
        })?;
        Ok(())
    }

    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }
}

impl MigrationSource for GroupsModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_metadata_matches_manifest_contract() {
        let module = GroupsModule::new();
        assert_eq!(module.slug(), "groups");
        assert_eq!(module.name(), "Groups");
        assert_eq!(
            module.description(),
            "Social groups, memberships, local roles, privacy, and feature bindings"
        );
        assert!(!module.version().is_empty());
        assert_eq!(module.kind(), ModuleKind::Optional);
        assert!(module.dependencies().is_empty());
        assert_eq!(module.migrations().len(), 9);
        assert_eq!(module.permissions().len(), 7);
    }
}
