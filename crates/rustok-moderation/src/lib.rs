pub mod domain;
pub mod migrations;
pub mod ports;

use async_trait::async_trait;
use rustok_api::{Action, Permission, Resource};
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub use domain::*;
pub use ports::*;

pub struct ModerationModule;

#[async_trait]
impl RusToKModule for ModerationModule {
    fn slug(&self) -> &'static str {
        "moderation"
    }

    fn name(&self) -> &'static str {
        "Moderation"
    }

    fn description(&self) -> &'static str {
        "Cross-domain moderation owner for reports, cases, decisions, and auditable enforcement"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::new(Resource::Moderation, Action::Create),
            Permission::new(Resource::Moderation, Action::Read),
            Permission::new(Resource::Moderation, Action::List),
            Permission::new(Resource::Moderation, Action::Update),
            Permission::new(Resource::Moderation, Action::Moderate),
            Permission::new(Resource::Moderation, Action::Resolve),
            Permission::new(Resource::Moderation, Action::Manage),
        ]
    }
}

impl MigrationSource for ModerationModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_boundary_is_owner_neutral() {
        let module = ModerationModule;
        assert_eq!(module.slug(), "moderation");
        assert!(module.dependencies().is_empty());
        assert_eq!(module.migrations().len(), 1);
    }

    #[test]
    fn permissions_cover_case_lifecycle() {
        let permissions = ModerationModule.permissions();
        assert!(permissions.contains(&Permission::new(
            Resource::Moderation,
            Action::Moderate,
        )));
        assert!(permissions.contains(&Permission::new(
            Resource::Moderation,
            Action::Resolve,
        )));
        assert!(permissions.contains(&Permission::new(
            Resource::Moderation,
            Action::Manage,
        )));
    }
}
