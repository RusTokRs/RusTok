#[cfg(feature = "ssr")]
use serde::{Deserialize, Serialize};

/// Minimal transport adapter for the owner-owned active composition snapshot.
/// Unknown composition fields are intentionally ignored; admin does not own
/// build planning, hashing, manifest discovery, or marketplace projection.
#[cfg(feature = "ssr")]
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct RuntimeModulesManifest {
    #[serde(default)]
    pub modules: std::collections::HashMap<String, RuntimeManifestModuleSpec>,
    #[serde(default)]
    pub settings: RuntimeSettingsManifest,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct RuntimeSettingsManifest {
    #[serde(default)]
    pub default_enabled: Vec<String>,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct RuntimeManifestModuleSpec {
    pub source: String,
    #[serde(rename = "crate", default)]
    pub crate_name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Clone)]
pub struct RuntimePlatformSnapshot {
    #[allow(dead_code)]
    pub revision: i64,
    pub manifest: RuntimeModulesManifest,
}
