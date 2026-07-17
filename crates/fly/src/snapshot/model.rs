use crate::{FlyError, FlyResult, GrapesJsV1Codec};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectSnapshot {
    pub id: String,
    pub label: String,
    pub project_hash: String,
    pub project_data: Value,
    #[serde(default)]
    pub metadata: Map<String, Value>,
}

impl ProjectSnapshot {
    pub fn restore(&self) -> FlyResult<crate::ProjectDocument> {
        let document = GrapesJsV1Codec::decode_value(self.project_data.clone())?;
        let actual = document.hash().hex();
        if actual != self.project_hash {
            return Err(FlyError::SnapshotHashMismatch {
                snapshot_id: self.id.clone(),
                declared: self.project_hash.clone(),
                actual,
            });
        }
        Ok(document)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ProjectDiffSummary {
    pub before_hash: String,
    pub after_hash: String,
    pub added_pages: Vec<String>,
    pub removed_pages: Vec<String>,
    pub changed_pages: Vec<String>,
    pub added_components: Vec<String>,
    pub removed_components: Vec<String>,
    pub changed_components: Vec<String>,
    pub added_assets: Vec<String>,
    pub removed_assets: Vec<String>,
    pub changed_assets: Vec<String>,
    pub added_style_rules: Vec<String>,
    pub removed_style_rules: Vec<String>,
    pub changed_style_rules: Vec<String>,
    pub project_extensions_changed: bool,
}

impl ProjectDiffSummary {
    pub fn is_empty(&self) -> bool {
        self.before_hash == self.after_hash
            && self.added_pages.is_empty()
            && self.removed_pages.is_empty()
            && self.changed_pages.is_empty()
            && self.added_components.is_empty()
            && self.removed_components.is_empty()
            && self.changed_components.is_empty()
            && self.added_assets.is_empty()
            && self.removed_assets.is_empty()
            && self.changed_assets.is_empty()
            && self.added_style_rules.is_empty()
            && self.removed_style_rules.is_empty()
            && self.changed_style_rules.is_empty()
            && !self.project_extensions_changed
    }

    pub fn change_count(&self) -> usize {
        self.added_pages.len()
            + self.removed_pages.len()
            + self.changed_pages.len()
            + self.added_components.len()
            + self.removed_components.len()
            + self.changed_components.len()
            + self.added_assets.len()
            + self.removed_assets.len()
            + self.changed_assets.len()
            + self.added_style_rules.len()
            + self.removed_style_rules.len()
            + self.changed_style_rules.len()
            + usize::from(self.project_extensions_changed)
    }
}