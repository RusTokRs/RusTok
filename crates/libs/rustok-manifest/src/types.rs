use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};

/// Root composition manifest (`modules.toml`).
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ModulesManifest {
    pub schema: u32,
    #[serde(default = "default_app_name")]
    pub app: String,
    #[serde(default)]
    pub build: Option<BuildConfig>,
    #[serde(default)]
    pub modules: BTreeMap<String, ModuleSpec>,
    #[serde(default)]
    pub settings: Option<Settings>,
}

pub type Manifest = ModulesManifest;

fn default_app_name() -> String {
    "rustok-server".to_string()
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Default)]
pub struct BuildConfig {
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default)]
    pub deployment_profile: Option<String>,
    #[serde(default)]
    pub server: Option<ServerBuildConfig>,
    #[serde(default)]
    pub admin: Option<AdminBuildConfig>,
    #[serde(default)]
    pub storefront: Option<Vec<StorefrontBuildConfig>>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Default)]
pub struct ServerBuildConfig {
    #[serde(default)]
    pub embed_admin: Option<bool>,
    #[serde(default)]
    pub embed_storefront: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Default)]
pub struct AdminBuildConfig {
    #[serde(default)]
    pub stack: Option<String>,
    #[serde(default)]
    pub public_url: Option<String>,
    #[serde(default)]
    pub redirect_uris: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Default)]
pub struct StorefrontBuildConfig {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub stack: Option<String>,
    #[serde(default)]
    pub public_url: Option<String>,
    #[serde(default)]
    pub redirect_uris: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ModuleSpec {
    #[serde(rename = "crate")]
    pub crate_name: String,
    #[serde(default = "default_module_source")]
    pub source: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub git: Option<String>,
    #[serde(default)]
    pub rev: Option<String>,
    #[serde(default)]
    pub depends_on: Option<Vec<String>>,
    #[serde(default)]
    pub features: Option<Vec<String>>,
    #[serde(default = "default_module_runtime")]
    pub runtime: String,
}

fn default_module_source() -> String {
    "path".to_string()
}

fn default_module_runtime() -> String {
    "module".to_string()
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Default)]
pub struct Settings {
    #[serde(default)]
    pub default_enabled: Option<Vec<String>>,
}

/// Module package contract manifest (`rustok-module.toml`).
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Default)]
pub struct ModulePackageManifest {
    #[serde(default)]
    pub module: ModulePackageMetadata,
    #[serde(default)]
    pub marketplace: ModulePackageMarketplaceMetadata,
    #[serde(default)]
    pub dependencies: BTreeMap<String, ModulePackageDependency>,
    #[serde(rename = "crate", default)]
    pub crate_contract: ModulePackageCrateContract,
    #[serde(default)]
    pub provides: ModulePackageProvides,
    #[serde(default)]
    pub settings: Option<BTreeMap<String, toml::Value>>,
    #[serde(default)]
    pub fba: Option<toml::Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Default)]
pub struct ModulePackageMetadata {
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_ownership")]
    pub ownership: String,
    #[serde(default = "default_trust_level")]
    pub trust_level: String,
    #[serde(default = "default_ui_classification")]
    pub ui_classification: String,
    #[serde(default)]
    pub recommended_admin_surfaces: Vec<String>,
    #[serde(default)]
    pub showcase_admin_surfaces: Vec<String>,
    #[serde(default = "default_module_runtime")]
    pub runtime: String,
}

fn default_ownership() -> String {
    "first_party".to_string()
}

fn default_trust_level() -> String {
    "verified".to_string()
}

fn default_ui_classification() -> String {
    "no_ui".to_string()
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Default)]
pub struct ModulePackageMarketplaceMetadata {
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub publisher: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Default)]
pub struct ModulePackageDependency {
    #[serde(default)]
    pub version_req: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Default)]
pub struct ModulePackageCrateContract {
    #[serde(default)]
    pub entry_type: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Default)]
pub struct ModulePackageProvides {
    #[serde(default)]
    pub graphql: Option<ModuleGraphqlProvides>,
    #[serde(default)]
    pub http: Option<ModuleHttpProvides>,
    #[serde(default)]
    pub cli: Option<ModulePackageCliProvides>,
    #[serde(default)]
    pub admin_ui: Option<ModuleUiProvides>,
    #[serde(default)]
    pub storefront_ui: Option<ModuleUiProvides>,
    #[serde(default)]
    pub admin_support: Option<ModuleUiSupportProvides>,
    #[serde(default)]
    pub storefront_support: Option<ModuleUiSupportProvides>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Default)]
pub struct ModuleGraphqlProvides {
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub mutation: Option<String>,
    #[serde(default)]
    pub subscription: Option<String>,
    #[serde(default)]
    pub runtime_data_factory: Option<String>,
}

pub type ModulePackageGraphqlProvides = ModuleGraphqlProvides;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Default)]
pub struct ModuleHttpProvides {
    #[serde(default)]
    pub routes: Option<String>,
    #[serde(default)]
    pub axum_router: Option<String>,
    #[serde(default)]
    pub webhook_routes: Option<String>,
    #[serde(default)]
    pub axum_webhook_router: Option<String>,
}

pub type ModulePackageHttpProvides = ModuleHttpProvides;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Default)]
pub struct ModulePackageCliProvides {
    #[serde(default)]
    pub namespace: Option<String>,
    #[serde(default)]
    pub factory: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Default)]
pub struct ModuleUiProvides {
    #[serde(default)]
    pub leptos_crate: Option<String>,
    #[serde(default)]
    pub route_segment: Option<String>,
    #[serde(default)]
    pub nav_label: Option<String>,
    #[serde(default)]
    pub nav_group: Option<String>,
    #[serde(default)]
    pub nav_order: Option<i32>,
    #[serde(default)]
    pub slot: Option<String>,
    #[serde(default)]
    pub page_title: Option<String>,
    #[serde(default)]
    pub next_package: Option<String>,
    #[serde(default)]
    pub components: Vec<ModuleUiComponentProvides>,
    #[serde(default)]
    pub child_pages: Vec<ModulePackageAdminUiChildPage>,
    #[serde(default)]
    pub i18n: Option<ModuleUiI18nProvides>,
}

pub type ModulePackageAdminUiProvides = ModuleUiProvides;
pub type ModulePackageStorefrontUiProvides = ModuleUiProvides;

/// Declares that the module's `admin/` or `storefront/` sub-crate is a
/// support crate: reusable Leptos components composed into other modules'
/// surfaces or host compositions. A support crate owns no route, slot, or
/// navigation entry, is not registered by host codegen, and does not count
/// as a UI surface for `module.ui_classification`. Exactly one of the
/// surface section (`admin_ui`/`storefront_ui`) or the support section may
/// be declared per host, because both describe the same single sub-crate.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Default)]
pub struct ModuleUiSupportProvides {
    #[serde(default)]
    pub leptos_crate: Option<String>,
    #[serde(default)]
    pub i18n: Option<ModuleUiI18nProvides>,
}

pub type ModulePackageAdminSupportProvides = ModuleUiSupportProvides;
pub type ModulePackageStorefrontSupportProvides = ModuleUiSupportProvides;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Default)]
pub struct ModulePackageAdminUiChildPage {
    #[serde(default)]
    pub subpath: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub nav_label: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Default)]
pub struct ModuleUiComponentProvides {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub component: String,
    #[serde(default)]
    pub slot: String,
    #[serde(default)]
    pub order: Option<usize>,
}

pub type ModulePackageStorefrontUiComponent = ModuleUiComponentProvides;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Default)]
pub struct ModuleUiI18nProvides {
    #[serde(default)]
    pub default_locale: Option<String>,
    #[serde(default)]
    pub supported_locales: Vec<String>,
    #[serde(default)]
    pub leptos_locales_path: Option<String>,
}

pub type ModulePackageI18nProvides = ModuleUiI18nProvides;
