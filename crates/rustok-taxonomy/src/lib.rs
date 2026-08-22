use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

mod category_hierarchy;
pub mod dto;
pub mod entities;
pub mod error;
pub mod migrations;
mod module_term_lookup;
pub mod module_term_mutation;
mod normalization;
mod owner_read;
mod route_key_registry;
pub mod services;
mod translation_evidence;
pub mod translation_target;

pub use category_hierarchy::MAX_TAXONOMY_CATEGORY_DEPTH;
pub use dto::{
    ApplyExactTaxonomyTranslationInput, CreateTaxonomyTermInput, ListTaxonomyTermsFilter,
    ResolveTaxonomyTermInput, SetTaxonomyCategoryPlacementInput, TaxonomyCategoryPlacement,
    TaxonomyScopeType, TaxonomyTermKind, TaxonomyTermListItem, TaxonomyTermResponse,
    TaxonomyTranslationApplyResult, UpdateTaxonomyTermInput,
};
pub use error::{TaxonomyError, TaxonomyResult};
pub use module_term_mutation::{
    ModuleTermMutationResult, ModuleTermUpdateInput, delete_module_term_in_tx,
    update_module_term_in_tx,
};
pub use normalization::{normalize_term_locale, normalize_term_route_key};
pub use owner_read::{TaxonomyOwnerReader, TaxonomyOwnerTerm, TaxonomyOwnerTermNames};
pub use services::TaxonomyService;
pub use translation_target::TaxonomyTranslationTargetProvider;

#[cfg(test)]
mod translation_target_tests;

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
        &["content", "outbox"]
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

    fn migration_dependencies(&self) -> Vec<rustok_core::MigrationDependencyDescriptor> {
        migrations::migration_dependencies()
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
        assert_eq!(module.dependencies(), &["content", "outbox"]);
    }

    #[test]
    fn module_permissions_cover_term_crud() {
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