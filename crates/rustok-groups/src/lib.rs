use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{MigrationSource, ModuleRuntimeExtensions, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub mod domain;
pub mod dto;
pub mod entities;
pub mod error;
#[cfg(feature = "graphql")]
pub mod graphql;
pub mod governance;
pub mod governance_entities;
pub mod migrations;
pub mod ports;
pub mod service;

pub use domain::*;
pub use dto::*;
pub use error::{GroupsError, GroupsResult};
pub use governance::*;
pub use ports::*;
pub use service::GroupsService;

/// Social group identity, membership, privacy, and modular feature owner.
pub struct GroupsModule;

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

    fn register_runtime_extensions(&self, extensions: &mut ModuleRuntimeExtensions) {
        extensions.insert(GroupCapabilityDescriptor::default());
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
        let module = GroupsModule;
        assert_eq!(module.slug(), "groups");
        assert_eq!(module.name(), "Groups");
        assert!(module.dependencies().is_empty());
        assert_eq!(module.migrations().len(), 2);
        assert_eq!(module.permissions().len(), 7);
    }

    #[test]
    fn capability_descriptor_fails_closed() {
        let descriptor = GroupCapabilityDescriptor::default();
        assert_eq!(descriptor.contract_version, "groups.access.v1");
        assert_eq!(descriptor.private_content_fallback, "deny");
        assert!(!descriptor.implicit_transport_fallback);
    }
}
