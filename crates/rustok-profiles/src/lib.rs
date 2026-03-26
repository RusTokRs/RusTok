use async_trait::async_trait;
use rustok_core::permissions::Permission;
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub mod dto;
pub mod entities;
pub mod error;
pub mod graphql;
pub mod migrations;
pub mod reader;
pub mod services;

pub use dto::{ProfileStatus, ProfileSummary, ProfileVisibility, UpsertProfileInput};
pub use entities::ProfileRecord;
pub use error::{ProfileError, ProfileResult};
pub use reader::ProfilesReader;
pub use services::ProfileService;

pub struct ProfilesModule;

#[async_trait]
impl RusToKModule for ProfilesModule {
    fn slug(&self) -> &'static str {
        "profiles"
    }

    fn name(&self) -> &'static str {
        "Profiles"
    }

    fn description(&self) -> &'static str {
        "Universal public profile domain for platform users"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::PROFILES_CREATE,
            Permission::PROFILES_READ,
            Permission::PROFILES_UPDATE,
            Permission::PROFILES_DELETE,
            Permission::PROFILES_LIST,
            Permission::PROFILES_MANAGE,
        ]
    }
}

impl MigrationSource for ProfilesModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_core::permissions::{Action, Resource};

    #[test]
    fn module_metadata() {
        let module = ProfilesModule;

        assert_eq!(module.slug(), "profiles");
        assert_eq!(module.name(), "Profiles");
        assert_eq!(
            module.description(),
            "Universal public profile domain for platform users"
        );
        assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
        assert!(module.dependencies().is_empty());
    }

    #[test]
    fn module_permissions() {
        let module = ProfilesModule;
        let permissions = module.permissions();

        assert!(permissions
            .iter()
            .any(|permission| permission.resource == Resource::Profiles
                && permission.action == Action::Read));
        assert!(permissions
            .iter()
            .any(|permission| permission.resource == Resource::Profiles
                && permission.action == Action::Manage));
    }

    #[test]
    fn module_has_profile_migrations() {
        let module = ProfilesModule;
        assert!(!module.migrations().is_empty());
    }
}
