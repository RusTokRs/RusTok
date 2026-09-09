//! Browser-safe projections for the active static module composition.

use serde::{Deserialize, Serialize};

/// Immutable composition revision required by static module-set commands.
///
/// The manifest and its digest remain owner-private; clients only need this
/// value for optimistic concurrency.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ModuleCompositionSnapshotView {
    pub revision: i64,
}

/// Browser-safe description of one module present in the active static
/// platform composition.
///
/// Source-control and filesystem locator data stays inside the host manifest;
/// clients receive only the identity and dependency facts they need to render
/// the installed-module projection.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct StaticInstalledModuleView {
    pub slug: String,
    pub source: String,
    #[serde(rename = "crateName")]
    pub crate_name: String,
    pub version: Option<String>,
    pub required: bool,
    pub dependencies: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::{ModuleCompositionSnapshotView, StaticInstalledModuleView};

    #[test]
    fn composition_view_uses_the_graphql_field_contract() {
        let encoded = serde_json::to_value(ModuleCompositionSnapshotView { revision: 12 })
            .expect("composition view serializes");
        assert_eq!(encoded["revision"], 12);
    }

    #[test]
    fn installed_module_view_uses_the_browser_safe_field_contract() {
        let encoded = serde_json::to_value(StaticInstalledModuleView {
            slug: "catalog".to_string(),
            source: "path".to_string(),
            crate_name: "rustok-catalog".to_string(),
            version: Some("1.2.3".to_string()),
            required: false,
            dependencies: vec!["content".to_string()],
        })
        .expect("installed module view serializes");

        assert_eq!(
            encoded,
            serde_json::json!({
                "slug": "catalog",
                "source": "path",
                "crateName": "rustok-catalog",
                "version": "1.2.3",
                "required": false,
                "dependencies": ["content"],
            })
        );
    }
}
