mod alloy;
mod manifest;

use rustok_blog::BlogModule;
use rustok_commerce::CommerceModule;
use rustok_content::ContentModule;
use rustok_core::ModuleRegistry;
use rustok_forum::ForumModule;
use rustok_index::IndexModule;
use rustok_pages::PagesModule;
use rustok_rbac::RbacModule;
use rustok_tenant::TenantModule;

pub use alloy::AlloyModule;
pub use manifest::{
    validate_registry_vs_manifest, InstalledManifestModule, ManifestDiff, ManifestError,
    ManifestManager, ManifestModuleSpec, ModulesManifest,
};

pub fn build_registry() -> ModuleRegistry {
    ModuleRegistry::new()
        .register(IndexModule)
        .register(TenantModule)
        .register(RbacModule)
        .register(ContentModule)
        .register(CommerceModule)
        .register(BlogModule)
        .register(ForumModule)
        .register(PagesModule)
        .register(AlloyModule::new())
}
