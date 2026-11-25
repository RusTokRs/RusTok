use super::diff::compare_projects;
use super::model::{ProjectDiffSummary, ProjectSnapshot};
use crate::{ContentDigest, FlyError, FlyResult, GrapesJsCodec, ProjectDocument, ProjectHash};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::VecDeque;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SnapshotCatalog {
    maximum_snapshots: usize,
    next_sequence: u64,
    snapshots: VecDeque<ProjectSnapshot>,
}

impl Default for SnapshotCatalog {
    fn default() -> Self {
        Self::new(25)
    }
}

impl SnapshotCatalog {
    pub fn new(maximum_snapshots: usize) -> Self {
        Self {
            maximum_snapshots: maximum_snapshots.max(1),
            next_sequence: 1,
            snapshots: VecDeque::new(),
        }
    }

    pub fn capture(
        &mut self,
        label: impl Into<String>,
        document: &ProjectDocument,
        metadata: Map<String, Value>,
    ) -> FlyResult<&ProjectSnapshot> {
        let label = label.into();
        let sequence = self.next_sequence;
        let next_sequence = sequence.checked_add(1).ok_or_else(|| {
            FlyError::Encode("snapshot sequence exhausted".to_string())
        })?;
        if self.maximum_snapshots == 0 {
            return Err(FlyError::Encode("snapshot catalog has zero capacity".to_string()));
        }
        // Hash the actual canonical payload, not a possibly stale document cache.
        // Validate that the payload can be decoded before committing the capture.
        let project_data = GrapesJsCodec::encode_value(document)?;
        let restored = GrapesJsCodec::decode_value(project_data.clone())?;
        if &restored != document {
            return Err(FlyError::Encode(
                "snapshot payload does not round-trip to the source project".to_string(),
            ));
        }
        let hash = ProjectHash::try_from_document(&restored)
            .ok_or_else(|| FlyError::Encode("snapshot fingerprint unavailable".to_string()))?
            .hex();
        let id = format!("snapshot-{sequence}-{hash}");
        if self.get(&id).is_some() {
            return Err(FlyError::Encode(format!("duplicate snapshot id `{id}`")));
        }
        let content_digest = ContentDigest::from_json(&project_data)?;
        self.snapshots.push_back(ProjectSnapshot {
            id: id.clone(),
            label: if label.trim().is_empty() {
                format!("Snapshot {sequence}")
            } else {
                label.trim().to_string()
            },
            project_hash: hash,
            content_digest: Some(content_digest),
            project_data,
            metadata,
        });
        self.next_sequence = next_sequence;
        while self.snapshots.len() > self.maximum_snapshots {
            self.snapshots.pop_front();
        }
        self.snapshots
            .back()
            .ok_or_else(|| FlyError::SnapshotNotFound(id))
    }

    pub fn get(&self, id: &str) -> Option<&ProjectSnapshot> {
        self.snapshots.iter().find(|snapshot| snapshot.id == id)
    }

    pub fn remove(&mut self, id: &str) -> Option<ProjectSnapshot> {
        let index = self
            .snapshots
            .iter()
            .position(|snapshot| snapshot.id == id)?;
        self.snapshots.remove(index)
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &ProjectSnapshot> {
        self.snapshots.iter()
    }

    pub fn len(&self) -> usize {
        self.snapshots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }

    pub fn clear(&mut self) {
        self.snapshots.clear();
    }

    pub fn compare_with_current(
        &self,
        id: &str,
        current: &ProjectDocument,
    ) -> FlyResult<ProjectDiffSummary> {
        let snapshot = self
            .get(id)
            .ok_or_else(|| FlyError::SnapshotNotFound(id.to_string()))?;
        let previous = snapshot.restore()?;
        Ok(compare_projects(&previous, current))
    }
}
