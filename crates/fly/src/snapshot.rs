use crate::{
    AssetCatalog, FlyError, FlyResult, GrapesJsV1Codec, ProjectDocument, StyleRuleCatalog,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

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
    pub fn restore(&self) -> FlyResult<ProjectDocument> {
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
        let index = self.snapshots.iter().position(|snapshot| snapshot.id == id)?;
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

pub fn compare_projects(
    before: &ProjectDocument,
    after: &ProjectDocument,
) -> ProjectDiffSummary {
    let before_pages = page_map(before);
    let after_pages = page_map(after);
    let before_components = component_map(before);
    let after_components = component_map(after);
    let before_assets = asset_map(before);
    let after_assets = asset_map(after);
    let before_styles = style_rule_map(before);
    let after_styles = style_rule_map(after);

    let (added_pages, removed_pages, changed_pages) = diff_maps(&before_pages, &after_pages);
    let (added_components, removed_components, changed_components) =
        diff_maps(&before_components, &after_components);
    let (added_assets, removed_assets, changed_assets) = diff_maps(&before_assets, &after_assets);
    let (added_style_rules, removed_style_rules, changed_style_rules) =
        diff_maps(&before_styles, &after_styles);

    ProjectDiffSummary {
        before_hash: before.hash().hex(),
        after_hash: after.hash().hex(),
        added_pages,
        removed_pages,
        changed_pages,
        added_components,
        removed_components,
        changed_components,
        added_assets,
        removed_assets,
        changed_assets,
        added_style_rules,
        removed_style_rules,
        changed_style_rules,
        project_extensions_changed: before.project.extensions != after.project.extensions,
    }
}

fn page_map(document: &ProjectDocument) -> BTreeMap<String, Value> {
    document
        .project
        .pages
        .iter()
        .enumerate()
        .map(|(index, page)| {
            let key = page
                .id
                .clone()
                .unwrap_or_else(|| format!("@index:{index}"));
            (
                key,
                serde_json::to_value(page).unwrap_or(Value::Null),
            )
        })
        .collect()
}

fn component_map(document: &ProjectDocument) -> BTreeMap<String, Value> {
    let mut components = BTreeMap::new();
    document.project.visit_components(|component, _, path| {
        let key = component
            .id()
            .map(ToString::to_string)
            .unwrap_or_else(|| format!("@path:{path}"));
        components.insert(
            key,
            serde_json::to_value(component).unwrap_or(Value::Null),
        );
    });
    components
}

fn asset_map(document: &ProjectDocument) -> BTreeMap<String, Value> {
    let catalog = AssetCatalog::from_document(document);
    let mut assets = BTreeMap::new();
    for asset in catalog.assets {
        assets.insert(asset.id, asset.raw);
    }
    for (index, asset) in catalog.unknown_entries.into_iter().enumerate() {
        assets.insert(format!("@opaque:{index}"), asset);
    }
    assets
}

fn style_rule_map(document: &ProjectDocument) -> BTreeMap<String, Value> {
    let catalog = StyleRuleCatalog::from_document(document);
    let mut rules = BTreeMap::new();
    for rule in catalog.rules {
        rules.insert(rule.id, rule.raw);
    }
    for (index, rule) in catalog.unknown_entries.into_iter().enumerate() {
        rules.insert(format!("@opaque:{index}"), rule);
    }
    rules
}

fn diff_maps(
    before: &BTreeMap<String, Value>,
    after: &BTreeMap<String, Value>,
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let before_keys = before.keys().cloned().collect::<BTreeSet<_>>();
    let after_keys = after.keys().cloned().collect::<BTreeSet<_>>();
    let added = after_keys.difference(&before_keys).cloned().collect();
    let removed = before_keys.difference(&after_keys).cloned().collect();
    let changed = before_keys
        .intersection(&after_keys)
        .filter(|key| before.get(*key) != after.get(*key))
        .cloned()
        .collect();
    (added, removed, changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn document(content: &str) -> ProjectDocument {
        GrapesJsV1Codec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "text",
                        "type": "text",
                        "content": content
                    }]
                }
            }]
        }))
        .expect("document")
    }

    #[test]
    fn snapshots_restore_and_verify_hash() {
        let mut catalog = SnapshotCatalog::new(2);
        let snapshot = catalog
            .capture("Initial", &document("Hello"), Map::new())
            .expect("snapshot")
            .clone();
        assert_eq!(snapshot.restore().expect("restore").hash().hex(), snapshot.project_hash);

        let mut tampered = snapshot;
        tampered.project_data["pages"][0]["id"] = json!("changed");
        assert!(matches!(
            tampered.restore(),
            Err(FlyError::SnapshotHashMismatch { .. })
        ));
    }

    #[test]
    fn catalog_evicts_old_snapshots() {
        let mut catalog = SnapshotCatalog::new(2);
        catalog.capture("One", &document("1"), Map::new()).unwrap();
        catalog.capture("Two", &document("2"), Map::new()).unwrap();
        catalog.capture("Three", &document("3"), Map::new()).unwrap();
        assert_eq!(catalog.len(), 2);
        assert_eq!(catalog.iter().next().unwrap().label, "Two");
    }

    #[test]
    fn structural_diff_tracks_component_changes() {
        let before = document("Hello");
        let mut after = document("Hello world");
        after.project.assets.push(json!({ "id": "hero", "src": "/hero.png" }));
        let diff = compare_projects(&before, &after);
        assert!(diff.changed_components.contains(&"text".to_string()));
        assert!(diff.changed_pages.contains(&"home".to_string()));
        assert!(diff.added_assets.contains(&"hero".to_string()));
        assert!(diff.change_count() >= 3);
    }

    #[test]
    fn catalog_compares_snapshot_with_current() {
        let mut catalog = SnapshotCatalog::default();
        let id = catalog
            .capture("Initial", &document("Hello"), Map::new())
            .unwrap()
            .id
            .clone();
        let diff = catalog
            .compare_with_current(&id, &document("Updated"))
            .expect("diff");
        assert!(!diff.is_empty());
    }
}
