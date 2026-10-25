
//! # RusToK Taxonomy Module
//!
//! Provides scope-aware hierarchical taxonomy dictionaries, shared and module-local terms,
//! slug normalization, SEO targets, and multi-locale term translations.

mod category_delete;
mod category_hierarchy;
mod category_presentation;
pub mod dto;
pub mod entities;
pub mod error;
pub mod migrations;
mod module_term_lookup;
pub mod module_term_mutation;
mod normalization;
mod owner_category_hierarchy_mutation;
mod owner_category_read;
mod owner_category_revision;
mod owner_category_route_sync;
mod owner_category_sync;
mod owner_identity;
mod owner_read;
mod route_key_registry;
pub mod seo_targets;
pub mod services;
mod translation_evidence;
pub mod translation_target;

pub use seo_targets::TaxonomyCategorySeoTargetProvider;

pub use category_delete::TaxonomyCategoryDeleteCleanupPort;
pub use category_hierarchy::{MAX_TAXONOMY_CATEGORY_DEPTH, lock_category_hierarchy_writer_in_tx};
pub use category_presentation::{
    TAXONOMY_CATEGORY_ICON_KEY_MAX_BYTES, TaxonomyCategoryMediaReferenceValidator,
    normalize_taxonomy_category_color, normalize_taxonomy_category_icon_key,
};
pub use dto::{
    ApplyExactTaxonomyTranslationInput, CreateTaxonomyTermInput, ListTaxonomyTermsFilter,
    ResolveTaxonomyTermInput, SetTaxonomyCategoryPlacementInput,
    SetTaxonomyCategoryPresentationInput, TaxonomyCategoryMediaId, TaxonomyCategoryPlacement,
    TaxonomyCategoryPresentation, TaxonomyScopeType, TaxonomyTermKind, TaxonomyTermListItem,
    TaxonomyTermResponse, TaxonomyTranslationApplyResult, UpdateTaxonomyTermInput,
};
pub use error::{TaxonomyError, TaxonomyResult};
pub use module_term_lookup::TaxonomyModuleRouteMatch;
pub use module_term_mutation::{
    ModuleTermCreateInput, ModuleTermMutationResult, ModuleTermUpdateInput,
    delete_module_term_in_tx, lock_module_term_in_tx, update_module_term_in_tx,
};
pub use normalization::{normalize_term_locale, normalize_term_route_key};
pub use owner_category_hierarchy_mutation::{
    delete_module_category_placement_and_compact_in_tx, move_module_category_in_tx,
    reorder_module_category_siblings_in_tx, shift_module_category_siblings_for_insert_in_tx,
};
pub use owner_category_read::{TaxonomyOwnerCategory, TaxonomyOwnerCategoryReader};
pub use owner_category_revision::{
    MAX_TAXONOMY_CATEGORY_OWNER_PAGE, TaxonomyCategoryOwnerRevision,
    TaxonomyCategoryOwnerRevisionPage, advance_category_owner_revision_in_tx,
    list_category_owner_revisions_in, load_category_owner_revisions_in,
    lock_category_owner_revision_in_tx,
};
pub use owner_category_route_sync::{
    TaxonomyModuleCategoryLocaleCopy, load_module_category_locale_copy_in_tx,
    sync_module_category_structure_with_owned_copy_in_tx,
    sync_module_category_with_owned_aliases_in_tx,
};
pub use owner_category_sync::{
    SyncModuleCategoryInput, SyncModuleCategoryResult, sync_module_category_in_tx,
};
pub use owner_identity::taxonomy_term_identity_exists;
pub use owner_read::{TaxonomyOwnerReader, TaxonomyOwnerTerm, TaxonomyOwnerTermNames};
pub use services::TaxonomyService;
pub use translation_target::{
    TaxonomyModuleTermTranslationOwner, TaxonomyModuleTermTranslationOwnerRegistry,
    TaxonomyTranslationTargetProvider,
};

#[cfg(test)]
mod translation_target_tests;

pub mod module;
pub use module::TaxonomyModule;

#[cfg(test)]
mod contract_tests;

