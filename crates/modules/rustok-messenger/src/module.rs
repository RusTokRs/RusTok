use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{MigrationDependencyDescriptor, MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

use crate::migrations;

pub struct MessengerModule;

#[async_trait]
impl RusToKModule for MessengerModule {
    fn slug(&self) -> &'static str {
        "messenger"
    }

    fn name(&self) -> &'static str {
        "Messenger & Team Chat"
    }

    fn description(&self) -> &'static str {
        "Highload distributed team messaging, channels, direct messages, monotonic sequence cursors and E2EE"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn dependencies(&self) -> &[&'static str] {
        &["outbox"]
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::CONVERSATIONS_CREATE,
            Permission::CONVERSATIONS_READ,
            Permission::CONVERSATIONS_UPDATE,
            Permission::CONVERSATIONS_DELETE,
            Permission::CONVERSATIONS_LIST,
            Permission::CONVERSATIONS_MANAGE,
            Permission::MESSAGES_CREATE,
            Permission::MESSAGES_READ,
            Permission::MESSAGES_DELETE,
            Permission::MESSAGES_LIST,
        ]
    }
}

impl MigrationSource for MessengerModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<MigrationDependencyDescriptor> {
        migrations::migration_dependencies()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_metadata() {
        let module = MessengerModule;
        assert_eq!(module.slug(), "messenger");
        assert_eq!(module.name(), "Messenger & Team Chat");
        assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
        assert_eq!(module.dependencies(), &["outbox"]);
    }

    #[test]
    fn module_permissions() {
        let module = MessengerModule;
        let perms = module.permissions();
        assert!(perms.contains(&Permission::CONVERSATIONS_CREATE));
        assert!(perms.contains(&Permission::CONVERSATIONS_READ));
        assert!(perms.contains(&Permission::MESSAGES_CREATE));
    }

    #[test]
    fn module_has_owned_migrations() {
        let module = MessengerModule;
        assert!(!module.migrations().is_empty());
    }
}
