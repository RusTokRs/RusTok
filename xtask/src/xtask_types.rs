use super::*;

#[derive(Debug, Deserialize)]
pub(crate) struct Manifest {
    pub(crate) schema: u32,
    #[allow(dead_code)]
    pub(crate) app: String,
    #[allow(dead_code)]
    pub(crate) build: Option<BuildConfig>,
    pub(crate) modules: HashMap<String, ModuleSpec>,
    pub(crate) settings: Option<Settings>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct BuildConfig {
    #[allow(dead_code)]
    pub(crate) target: Option<String>,
    #[allow(dead_code)]
    pub(crate) profile: Option<String>,
    #[allow(dead_code)]
    pub(crate) deployment_profile: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ModuleSpec {
    #[serde(rename = "crate")]
    pub(crate) crate_name: String,
    pub(crate) source: String,
    pub(crate) path: Option<String>,
    #[serde(default)]
    pub(crate) required: bool,
    pub(crate) version: Option<String>,
    pub(crate) git: Option<String>,
    #[allow(dead_code)]
    pub(crate) rev: Option<String>,
    #[allow(dead_code)]
    pub(crate) depends_on: Option<Vec<String>>,
    #[allow(dead_code)]
    pub(crate) features: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Settings {
    pub(crate) default_enabled: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Default)]
pub(crate) struct ModulePackageManifest {
    #[serde(default)]
    pub(crate) module: ModulePackageMetadata,
    #[serde(default)]
    pub(crate) marketplace: ModulePackageMarketplaceMetadata,
    #[serde(default)]
    pub(crate) dependencies: HashMap<String, ModulePackageDependency>,
    #[serde(rename = "crate", default)]
    pub(crate) crate_contract: ModulePackageCrateContract,
    #[serde(default)]
    pub(crate) provides: ModulePackageProvides,
}

#[derive(Debug, Deserialize, Default)]
pub(crate) struct ModulePackageMetadata {
    #[serde(default)]
    pub(crate) slug: String,
    #[serde(default)]
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) version: String,
    #[serde(default)]
    pub(crate) description: String,
    #[serde(default)]
    pub(crate) ownership: String,
    #[serde(default)]
    pub(crate) trust_level: String,
    #[serde(default)]
    pub(crate) ui_classification: String,
    #[serde(default)]
    pub(crate) recommended_admin_surfaces: Vec<String>,
    #[serde(default)]
    pub(crate) showcase_admin_surfaces: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
pub(crate) struct ModulePackageDependency {
    #[allow(dead_code)]
    #[serde(default)]
    pub(crate) version_req: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub(crate) struct ModulePackageMarketplaceMetadata {
    #[serde(default)]
    pub(crate) category: Option<String>,
    #[serde(default)]
    pub(crate) tags: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
pub(crate) struct ModulePackageCrateContract {
    #[serde(default)]
    pub(crate) entry_type: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub(crate) struct ModulePackageProvides {
    #[serde(default)]
    pub(crate) graphql: Option<ModuleGraphqlProvides>,
    #[serde(default)]
    pub(crate) http: Option<ModuleHttpProvides>,
    #[serde(default)]
    pub(crate) admin_ui: Option<ModuleUiProvides>,
    #[serde(default)]
    pub(crate) storefront_ui: Option<ModuleUiProvides>,
}

#[derive(Debug, Deserialize, Default)]
pub(crate) struct ModuleGraphqlProvides {
    #[serde(default)]
    pub(crate) query: Option<String>,
    #[serde(default)]
    pub(crate) mutation: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub(crate) struct ModuleHttpProvides {
    #[serde(default)]
    pub(crate) routes: Option<String>,
    #[serde(default)]
    pub(crate) webhook_routes: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub(crate) struct ModuleUiProvides {
    #[serde(default)]
    pub(crate) leptos_crate: Option<String>,
    #[serde(default)]
    pub(crate) route_segment: Option<String>,
    #[serde(default)]
    pub(crate) nav_label: Option<String>,
    #[serde(default)]
    pub(crate) slot: Option<String>,
    #[serde(default)]
    pub(crate) page_title: Option<String>,
    #[serde(default)]
    pub(crate) i18n: Option<ModuleUiI18nProvides>,
}

#[derive(Debug, Deserialize, Default)]
pub(crate) struct ModuleUiI18nProvides {
    #[serde(default)]
    pub(crate) default_locale: Option<String>,
    #[serde(default)]
    pub(crate) supported_locales: Vec<String>,
    #[serde(default)]
    pub(crate) leptos_locales_path: Option<String>,
}

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
}

#[derive(Debug, Serialize)]
pub(crate) struct ModuleUiPackagePreview {
    pub(crate) crate_name: String,
    pub(crate) manifest_path: String,
}
