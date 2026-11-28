//! One-pass typed `id -> location` index plus opaque component identity tracking.
//!
//! # Why
//!
//! [`ProjectDocument::component_location`] and [`ProjectDocument::contains_component`] each walk
//! every page from the top. That is fine for a one-off lookup and quietly quadratic in a loop,
//! which is how most callers actually use them: validating style rules calls
//! `contains_component` once per rule, so a project with 5 000 rules over 10 000 nodes performs
//! tens of millions of node visits to answer a question that one traversal answers for all of
//! them.
//!
//! Build an index once, query it many times. The index is a *snapshot*: it borrows nothing and
//! must be invalidated when the document changes. The document caches it until its mutation
//! boundary invalidates the cache.

use crate::{
    ComponentChildren, ComponentLocation, ComponentNode, ComponentObject, ProjectDocument,
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Snapshot of where every identified component sits in a document.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ComponentIndex {
    pub(crate) locations: BTreeMap<String, ComponentLocation>,
    pub(crate) paths: BTreeMap<String, (usize, Vec<usize>)>,
    pub(crate) duplicates: BTreeSet<String>,
    identities: BTreeSet<String>,
}

impl ComponentIndex {
    /// Index every component in `document` in a single pre-order traversal.
    ///
    /// Resolution matches [`ProjectDocument::component_location`] exactly: pages in order, each
    /// page root before its children, and the first occurrence of a repeated id wins. Repeated
    /// ids are additionally recorded, because "first one wins" is a tolerable lookup rule but a
    /// terrible thing to leave undiagnosed.
    pub fn build(document: &ProjectDocument) -> Self {
        let mut index = Self::default();
        for (page_index, page) in document.project().pages.iter().enumerate() {
            let Some(root) = page.component.as_ref() else {
                continue;
            };
            if let Some(id) = root.id() {
                index.insert(
                    id,
                    ComponentLocation {
                        page_index,
                        parent_component_id: None,
                        index: 0,
                        depth: 0,
                    },
                    (page_index, Vec::new()),
                );
            }
            let mut path = Vec::new();
            index.visit(
                root,
                page_index,
                root.id().map(ToString::to_string),
                1,
                &mut path,
            );
        }
        index
    }

    fn insert(&mut self, id: &str, location: ComponentLocation, path: (usize, Vec<usize>)) {
        self.identities.insert(id.to_string());
        if self.locations.contains_key(id) {
            self.duplicates.insert(id.to_string());
            return;
        }
        self.locations.insert(id.to_string(), location);
        self.paths.insert(id.to_string(), path);
    }

    fn visit(
        &mut self,
        parent: &ComponentNode,
        page_index: usize,
        parent_component_id: Option<String>,
        depth: usize,
        path: &mut Vec<usize>,
    ) {
        let Some(object) = parent.as_object() else {
            if let ComponentNode::Opaque(raw) = parent {
                collect_opaque_component_ids(raw, &mut self.identities);
            }
            return;
        };
        for (index, child) in object.children().iter().enumerate() {
            path.push(index);
            if let Some(id) = child.id() {
                self.insert(
                    id,
                    ComponentLocation {
                        page_index,
                        parent_component_id: parent_component_id.clone(),
                        index,
                        depth,
                    },
                    (page_index, path.clone()),
                );
            }
            self.visit(
                child,
                page_index,
                child.id().map(ToString::to_string),
                depth + 1,
                path,
            );
            path.pop();
        }
        if let ComponentChildren::Opaque(raw) = &object.components {
            collect_opaque_component_ids(raw, &mut self.identities);
        }
    }

    /// Whether a typed, addressable component with this id exists.
    pub fn contains(&self, component_id: &str) -> bool {
        self.locations.contains_key(component_id)
    }

    /// Whether an identity exists anywhere in the typed or preserved opaque component tree.
    ///
    /// This is for lossless references such as style rules; it does not make opaque nodes
    /// addressable through typed component editing APIs.
    pub(crate) fn contains_identity(&self, component_id: &str) -> bool {
        self.identities.contains(component_id)
    }

    /// Where the component sits, or `None` if it does not exist.
    pub fn location(&self, component_id: &str) -> Option<&ComponentLocation> {
        self.locations.get(component_id)
    }

    /// Test strict ancestry using structural paths instead of parent IDs.
    ///
    /// An intermediate node need not have an ID, and a repeated parent ID may
    /// resolve to a different branch under the first-occurrence lookup rule.
    pub(crate) fn is_descendant_of(&self, candidate_id: &str, ancestor_id: &str) -> bool {
        let (Some((candidate_page, candidate_path)), Some((ancestor_page, ancestor_path))) =
            (self.paths.get(candidate_id), self.paths.get(ancestor_id))
        else {
            return false;
        };
        candidate_page == ancestor_page
            && candidate_path.len() > ancestor_path.len()
            && candidate_path.starts_with(ancestor_path)
    }

    /// Retrieve the component directly via its indexed path in O(depth) steps.
    pub fn get_component<'a>(
        &self,
        document: &'a ProjectDocument,
        id: &str,
    ) -> Option<&'a ComponentObject> {
        let indexed = self.paths.get(id).and_then(|(page_index, path)| {
            let page = document.project().pages.get(*page_index)?;
            let root = page.component.as_ref()?.as_object()?;
            Some((root, path))
        });

        if let Some((root, path)) = indexed {
            let mut current = root;
            let mut ok = true;
            for &idx in path {
                if let Some(child) = current
                    .children()
                    .get(idx)
                    .and_then(ComponentNode::as_object)
                {
                    current = child;
                } else {
                    ok = false;
                    break;
                }
            }
            if ok && current.id() == Some(id) {
                return Some(current);
            }
        }
        document.project().component(id)
    }

    /// Retrieve a mutable reference to the component via its indexed path in O(depth) steps.
    pub fn get_component_mut<'a>(
        &self,
        document: &'a mut ProjectDocument,
        id: &str,
    ) -> Option<&'a mut ComponentObject> {
        let (page_index, path) = self.paths.get(id)?;
        let page = document.project_mut().pages.get_mut(*page_index)?;
        let root = page.component.as_mut()?.as_object_mut()?;
        let target = navigate_mut(root, path)?;
        if target.id() == Some(id) {
            Some(target)
        } else {
            None
        }
    }
}

fn collect_opaque_component_ids(value: &Value, identities: &mut BTreeSet<String>) {
    match value {
        Value::Array(children) => {
            for child in children {
                collect_opaque_component_ids(child, identities);
            }
        }
        Value::Object(object) => {
            if let Some(id) = object.get("id").and_then(Value::as_str) {
                identities.insert(id.to_string());
            }
            if let Some(children) = object.get("components") {
                collect_opaque_component_ids(children, identities);
            }
        }
        _ => {}
    }
}

pub(crate) fn navigate_mut<'a>(
    root: &'a mut ComponentObject,
    path: &[usize],
) -> Option<&'a mut ComponentObject> {
    let mut current = root;
    for &idx in path {
        let children = current.children_mut()?;
        let child = children.get_mut(idx)?;
        current = child.as_object_mut()?;
    }
    Some(current)
}

impl ComponentIndex {
    /// Canonical diagnostic path string for the component, e.g. `pages[0].component.components[2]`.
    pub fn diagnostic_path(&self, id: &str) -> Option<String> {
        let (page_index, path) = self.paths.get(id)?;
        let mut result = format!("pages[{page_index}].component");
        for idx in path {
            result.push_str(&format!(".components[{idx}]"));
        }
        Some(result)
    }

    /// Ids that appear more than once, in sorted order.
    ///
    /// Sorted rather than discovery-ordered so that diagnostics are reproducible across runs.
    pub fn duplicate_ids(&self) -> impl Iterator<Item = &str> {
        self.duplicates.iter().map(String::as_str)
    }

    /// Number of distinct identified components.
    pub fn len(&self) -> usize {
        self.locations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.locations.is_empty()
    }

    /// Every indexed id, in sorted order.
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.locations.keys().map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GrapesJsCodec;
    use serde_json::json;

    fn document() -> ProjectDocument {
        GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [
                        { "id": "hero", "type": "section", "components": [
                            { "id": "title", "type": "heading" }
                        ]},
                        { "id": "footer", "type": "section" }
                    ]
                }
            }, {
                "id": "about",
                "component": { "id": "about-root", "type": "wrapper", "components": [
                    { "id": "about-hero", "type": "section" }
                ]}
            }]
        }))
        .expect("document")
    }

    #[test]
    fn the_index_agrees_with_the_walking_implementation_for_every_id() {
        // The index is only safe to substitute for `component_location` if it resolves
        // identically, including `parent_component_id`, `index` and `depth`.
        let document = document();
        let index = ComponentIndex::build(&document);

        for id in [
            "root",
            "hero",
            "title",
            "footer",
            "about-root",
            "about-hero",
        ] {
            assert_eq!(
                index.location(id),
                document.component_location(id).as_ref(),
                "disagreement for `{id}`"
            );
            assert_eq!(index.contains(id), document.contains_component(id));
        }
    }

    #[test]
    fn unknown_ids_are_absent_from_both() {
        let document = document();
        let index = ComponentIndex::build(&document);
        assert!(!index.contains("nope"));
        assert_eq!(index.location("nope"), None);
        assert_eq!(document.component_location("nope"), None);
    }

    #[test]
    fn repeated_ids_resolve_to_the_first_occurrence_and_are_reported() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": { "id": "root", "type": "wrapper", "components": [
                    { "id": "dup", "type": "section" },
                    { "id": "other", "type": "section", "components": [
                        { "id": "dup", "type": "span" }
                    ]}
                ]}
            }]
        }))
        .expect("document");
        let index = ComponentIndex::build(&document);

        assert_eq!(index.duplicate_ids().collect::<Vec<_>>(), vec!["dup"]);
        // First in pre-order wins, matching `component_location`.
        assert_eq!(index.location("dup").map(|l| l.index), Some(0));
        assert_eq!(
            index.location("dup"),
            document.component_location("dup").as_ref()
        );
    }

    #[test]
    fn counts_distinct_ids_only() {
        let document = document();
        let index = ComponentIndex::build(&document);
        assert_eq!(index.len(), 6);
        assert!(!index.is_empty());
        assert_eq!(
            index.ids().collect::<Vec<_>>(),
            vec![
                "about-hero",
                "about-root",
                "footer",
                "hero",
                "root",
                "title"
            ]
        );
    }

    #[test]
    fn indexed_mutation_invalidates_hash_and_location_cache() {
        let mut document = document();
        let index = ComponentIndex::build(&document);
        let before_hash = document.hash();
        assert!(document.component_index().contains("title"));

        index
            .get_component_mut(&mut document, "title")
            .expect("title")
            .id = Some("renamed-title".to_string());

        assert_ne!(document.hash(), before_hash);
        assert!(document.component_location("title").is_none());
        assert!(document.component_location("renamed-title").is_some());
        assert!(document.component_index().contains("renamed-title"));
    }

    #[test]
    fn raw_project_mutable_borrow_invalidates_both_caches() {
        let mut document = document();
        let before_hash = document.hash();
        assert!(document.component_index().contains("about-root"));

        document.project_mut().pages.remove(1);

        assert_ne!(document.hash(), before_hash);
        assert!(document.component_location("about-root").is_none());
        assert!(!document.component_index().contains("about-hero"));
    }

    #[test]
    fn component_and_diagnostic_path_resolution() {
        let mut document = document();
        let index = ComponentIndex::build(&document);

        assert_eq!(
            index.diagnostic_path("title"),
            Some("pages[0].component.components[0].components[0]".to_string())
        );
        assert_eq!(
            index.diagnostic_path("about-hero"),
            Some("pages[1].component.components[0]".to_string())
        );
        assert_eq!(
            index.diagnostic_path("root"),
            Some("pages[0].component".to_string())
        );

        let title = index.get_component(&document, "title").expect("title");
        assert_eq!(title.id(), Some("title"));
        assert_eq!(title.component_type(), "heading");

        let title_mut = index
            .get_component_mut(&mut document, "title")
            .expect("title mut");
        title_mut.component_type = Some("custom_heading".to_string());
        assert_eq!(
            document
                .project()
                .component("title")
                .and_then(|c| c.component_type.as_deref()),
            Some("custom_heading")
        );
    }
}
