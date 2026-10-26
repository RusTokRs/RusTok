use super::*;

#[allow(unused_imports)]
pub(crate) use rustok_manifest::{
    BuildConfig, Manifest, ModuleGraphqlProvides, ModuleHttpProvides, ModulePackageCliProvides,
    ModulePackageCrateContract, ModulePackageDependency, ModulePackageManifest,
    ModulePackageMarketplaceMetadata, ModulePackageMetadata, ModulePackageProvides, ModuleSpec,
    ModuleUiComponentProvides, ModuleUiI18nProvides, ModuleUiProvides, ModuleUiSupportProvides,
    Settings,
};

#[derive(Debug, Serialize)]
pub(crate) struct ModulePublishDryRunPreview {
    pub(crate) slug: String,
    pub(crate) version: String,
    pub(crate) crate_name: String,
    pub(crate) module_default_locale: String,
    pub(crate) module_name: String,
    pub(crate) module_description: String,
    pub(crate) ownership: String,
    pub(crate) trust_level: String,
    pub(crate) license: String,
    pub(crate) manifest_path: String,
    pub(crate) package_manifest_path: String,
    pub(crate) module_entry_type: Option<String>,
    pub(crate) marketplace: ModuleMarketplacePreview,
    pub(crate) ui_packages: ModuleUiPackagesPreview,
}

#[derive(Debug, Serialize)]
pub(crate) struct ModuleMarketplacePreview {
    pub(crate) category: Option<String>,
    pub(crate) tags: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ModuleUiPackagesPreview {
    pub(crate) admin: Option<ModuleUiPackagePreview>,
    pub(crate) storefront: Option<ModuleUiPackagePreview>,
    pub(crate) admin_support: Option<ModuleUiPackagePreview>,
    pub(crate) storefront_support: Option<ModuleUiPackagePreview>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ModuleUiPackagePreview {
    pub(crate) crate_name: String,
    pub(crate) manifest_path: String,
}
