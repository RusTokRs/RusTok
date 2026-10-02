use crate::{ContentDigest, FlyError, FlyResult, GrapesJsCodec};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectSnapshot {
    pub id: String,
    pub label: String,
    /// Cheap change-detection fingerprint. Not an integrity guarantee — see `content_digest`.
    pub project_hash: String,
    /// Collision-resistant digest of `project_data`.
    ///
    /// Optional only so that snapshots captured before digests existed stay readable. Everything
    /// Fly captures today populates it, and [`ProjectSnapshot::restore`] refuses to restore a
    /// snapshot whose digest is present and wrong.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_digest: Option<ContentDigest>,
    pub project_data: Value,
    #[serde(default)]
    pub metadata: Map<String, Value>,
}

impl ProjectSnapshot {
    /// Digest of the payload as it is stored right now.
    pub fn compute_content_digest(&self) -> FlyResult<ContentDigest> {
        ContentDigest::from_json(&self.project_data)
    }

    /// True when this snapshot carries a collision-resistant integrity digest.
    pub fn has_content_digest(&self) -> bool {
        self.content_digest.is_some()
    }

    /// Verify the integrity digest, then the cheap fingerprint, then decode.
    ///
    /// The digest is checked first and in constant time: it is the only check that actually
    /// resists a crafted payload.
    pub fn restore(&self) -> FlyResult<crate::ProjectDocument> {
        self.verify_content_digest()?;
        let document = GrapesJsCodec::decode_value(self.project_data.clone())?;
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

    /// Restore while requiring a digest to be present, for callers that must not accept
    /// pre-digest snapshots (publishing, rollback, cross-tenant import).
    pub fn restore_verified(&self) -> FlyResult<crate::ProjectDocument> {
        if self.content_digest.is_none() {
            return Err(FlyError::SnapshotDigestMissing(self.id.clone()));
        }
        self.restore()
    }

    fn verify_content_digest(&self) -> FlyResult<()> {
        let Some(declared) = self.content_digest.as_ref() else {
            return Ok(());
        };
        let actual = self.compute_content_digest()?;
        if declared.matches(&actual) {
            return Ok(());
        }
        Err(FlyError::SnapshotDigestMismatch {
            snapshot_id: self.id.clone(),
            declared: declared.to_string(),
            actual: actual.to_string(),
        })
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
