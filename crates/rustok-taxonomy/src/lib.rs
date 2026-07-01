use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub mod dto;
pub mod entities;
pub mod error;
pub mod migrations;
pub mod services;

pub use dto::{
    CreateTaxonomyTermInput, ListTaxonomyTermsFilter, TaxonomyScopeType, TaxonomyTermKind,
    TaxonomyTermListItem, TaxonomyTermResponse, TaxonomyTermStatus, UpdateTaxonomyTermInput,
};
pub use error::{TaxonomyError, TaxonomyResult};
pub use services::TaxonomyService;

pub struct TaxonomyModule;

#[async_trait]
impl RusToKModule for TaxonomyModule {
    fn slug(&self) -> &'static str {
        "taxonomy"
    }

    fn name(&self) -> &'static str {
        "Taxonomy"
    }

    fn description(&self) -> &'static str {
        "Scope-aware taxonomy dictionary for shared and module-local terms"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn dependencies(&self) -> &[&'static str] {
        &["content"]
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::TAXONOMY_CREATE,
            Permission::TAXONOMY_READ,
            Permission::TAXONOMY_UPDATE,
            Permission::TAXONOMY_DELETE,
            Permission::TAXONOMY_LIST,
            Permission::TAXONOMY_MANAGE,
        ]
    }
}

impl MigrationSource for TaxonomyModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::{Action, Resource};

    #[test]
    fn module_metadata() {
        let module = TaxonomyModule;

        assert_eq!(module.slug(), "taxonomy");
        assert_eq!(module.name(), "Taxonomy");
        assert_eq!(
            module.description(),
            "Scope-aware taxonomy dictionary for shared and module-local terms"
        );
        assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
        assert_eq!(module.dependencies(), &["content"]);
    }

    #[test]
    fn module_permissions_cover_term_lifecycle() {
        let module = TaxonomyModule;
        let permissions = module.permissions();

        assert!(permissions.contains(&Permission::new(Resource::Taxonomy, Action::Create)));
        assert!(permissions.contains(&Permission::new(Resource::Taxonomy, Action::Read)));
        assert!(permissions.contains(&Permission::new(Resource::Taxonomy, Action::Update)));
        assert!(permissions.contains(&Permission::new(Resource::Taxonomy, Action::Delete)));
        assert!(permissions.contains(&Permission::new(Resource::Taxonomy, Action::List)));
        assert!(permissions.contains(&Permission::new(Resource::Taxonomy, Action::Manage)));
    }
}
