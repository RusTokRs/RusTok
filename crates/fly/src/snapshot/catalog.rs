use super::diff::compare_projects;
use super::model::{ProjectDiffSummary, ProjectSnapshot};
use crate::{FlyError, FlyResult, GrapesJsV1Codec, ProjectDocument};
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
        let hash = document.hash().hex();
        let id = format!("snapshot-{}-{hash}", self.next_sequence);
        self.next_sequence = self.next_sequence.saturating_add(1);
        self.snapshots.push_back(ProjectSnapshot {
            id,
            label: if label.trim().is_empty() {
                format!("Snapshot {}", self.next_sequence.saturating_sub(1))
            } else {
                label.trim().to_string()
            },
            project_hash: hash,
            project_data: GrapesJsV1Codec::encode_value(document)?,
            metadata,
        });
        while self.snapshots.len() > self.maximum_snapshots {
            self.snapshots.pop_front();
        }
        Ok(self.snapshots.back().expect("captured snapshot"))
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
