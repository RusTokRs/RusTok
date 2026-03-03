use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ModuleInfo {
    #[serde(rename = "moduleSlug")]
    pub module_slug: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub kind: String,
    pub dependencies: Vec<String>,
    pub enabled: bool,
}

impl ModuleInfo {
    pub fn is_core(&self) -> bool {
        self.kind == "core"
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ToggleModuleResult {
    #[serde(rename = "moduleSlug")]
    pub module_slug: String,
    pub enabled: bool,
    pub settings: String,
}
