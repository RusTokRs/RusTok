use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{MigrationDependencyDescriptor, MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

use crate::migrations;

pub struct DocsModule;

#[async_trait]
impl RusToKModule for DocsModule {
    fn slug(&self) -> &'static str {
        "docs"
    }

    fn name(&self) -> &'static str {
        "Collaborative Documents & Wiki"
    }

    fn description(&self) -> &'static str {
        "Block-based collaborative documents, Wiki and knowledge base powered by Loro CRDT"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn dependencies(&self) -> &[&'static str] {
        &["outbox", "flex"]
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::PAGES_CREATE,
            Permission::PAGES_READ,
            Permission::PAGES_UPDATE,
            Permission::PAGES_DELETE,
            Permission::PAGES_LIST,
            Permission::PAGES_MANAGE,
        ]
    }

    fn register_runtime_extensions(
        &self,
        extensions: &mut rustok_core::ModuleRuntimeExtensions,
    ) -> rustok_core::Result<()> {
        rustok_core::search_projection::register_search_projection_source(
            extensions,
            crate::search_projection::DocsSearchProjectionSourceFactory,
        )
        .map_err(|error| {
            rustok_core::Error::Validation(format!(
                "docs Search projection source registration failed: {error}"
            ))
        })
    }
}

impl MigrationSource for DocsModule {
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
    use rustok_api::{Action, Resource};

    #[test]
    fn module_metadata() {
        let module = DocsModule;
        assert_eq!(module.slug(), "docs");
        assert_eq!(module.name(), "Collaborative Documents & Wiki");
        assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
        assert_eq!(module.dependencies(), &["outbox", "flex"]);
    }

    #[test]
    fn module_permissions() {
        let module = DocsModule;
        let perms = module.permissions();
        assert!(perms.contains(&Permission::new(Resource::Pages, Action::Create)));
        assert!(perms.contains(&Permission::new(Resource::Pages, Action::Read)));
        assert!(perms.contains(&Permission::new(Resource::Pages, Action::Update)));
        assert!(perms.contains(&Permission::new(Resource::Pages, Action::Delete)));
        assert!(perms.contains(&Permission::new(Resource::Pages, Action::List)));
        assert!(perms.contains(&Permission::new(Resource::Pages, Action::Manage)));
    }

    #[test]
    fn module_has_owned_migrations() {
        let module = DocsModule;
        assert!(!module.migrations().is_empty());
    }
}
