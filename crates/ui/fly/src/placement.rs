use crate::{ComponentNode, ProjectDocument, RegistrySet};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComponentLocation {
    pub page_index: usize,
    pub parent_component_id: Option<String>,
    pub index: usize,
    pub depth: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlacementDecision {
    pub legal: bool,
    pub reason: Option<String>,
}

impl PlacementDecision {
    pub fn allowed() -> Self {
        Self {
            legal: true,
            reason: None,
        }
    }

    pub fn rejected(reason: impl Into<String>) -> Self {
        Self {
            legal: false,
            reason: Some(reason.into()),
        }
    }
}

impl ProjectDocument {
    pub fn component_location(&self, component_id: &str) -> Option<ComponentLocation> {
        self.component_index().location(component_id).cloned()
    }

    pub fn component_parent_id(&self, component_id: &str) -> Option<String> {
        self.component_location(component_id)
            .and_then(|location| location.parent_component_id)
    }

    pub fn component_child_count(&self, component_id: &str) -> Option<usize> {
        self.component(component_id)
            .map(|component| component.children().len())
    }

    pub fn root_child_count(&self) -> Option<usize> {
        self.project
            .pages
            .iter()
            .find_map(|page| page.component.as_ref())
            .and_then(ComponentNode::as_object)
            .map(|root| root.children().len())
    }

    pub fn child_count_for_parent(&self, parent_component_id: Option<&str>) -> Option<usize> {
        match parent_component_id {
            Some(parent_component_id) => self.component_child_count(parent_component_id),
            None => self.root_child_count(),
        }
    }

    pub fn is_component_descendant_of(&self, candidate_id: &str, ancestor_id: &str) -> bool {
        self.component_index()
            .is_descendant_of(candidate_id, ancestor_id)
    }

    pub fn component_type_for_id(&self, component_id: &str) -> Option<&str> {
        self.component(component_id)
            .map(|component| component.component_type())
    }
}

impl RegistrySet {
    pub fn accepts_child_type(&self, parent_type: Option<&str>, child_type: &str) -> bool {
        let Some(parent_type) = parent_type else {
            return true;
        };
        let Some(parent) = self.components.get(parent_type) else {
            return false;
        };
        parent.accepts_any_child
            || parent
                .allowed_children
                .iter()
                .any(|allowed| allowed == child_type)
    }

    pub fn evaluate_placement(
        &self,
        document: &ProjectDocument,
        moving_component_id: Option<&str>,
        child_type: &str,
        parent_component_id: Option<&str>,
        index: usize,
    ) -> PlacementDecision {
        let Some(child_count) = document.child_count_for_parent(parent_component_id) else {
            return PlacementDecision::rejected("drop parent does not exist or is opaque");
        };
        if index > child_count {
            return PlacementDecision::rejected(format!(
                "drop index {index} exceeds parent child count {child_count}"
            ));
        }

        if let Some(moving_component_id) = moving_component_id {
            if parent_component_id == Some(moving_component_id) {
                return PlacementDecision::rejected("a component cannot contain itself");
            }
            if parent_component_id.is_some_and(|parent_id| {
                document.is_component_descendant_of(parent_id, moving_component_id)
            }) {
                return PlacementDecision::rejected(
                    "a component cannot be moved into one of its descendants",
                );
            }
        }

        let parent_type =
            parent_component_id.and_then(|parent_id| document.component_type_for_id(parent_id));
        if !self.accepts_child_type(parent_type, child_type) {
            let parent_name = parent_type.unwrap_or("project root");
            return PlacementDecision::rejected(format!(
                "component type `{parent_name}` does not accept `{child_type}` children"
            ));
        }

        PlacementDecision::allowed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GrapesJsCodec, RegistrySet};
    use serde_json::json;

    fn document() -> ProjectDocument {
        GrapesJsCodec::decode_value(json!({
            "pages": [{
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "section-a",
                        "type": "section",
                        "components": [{ "id": "text-a", "type": "text" }]
                    }, {
                        "id": "section-b",
                        "type": "section",
                        "components": []
                    }]
                }
            }]
        }))
        .expect("document")
    }

    #[test]
    fn location_tracks_parent_and_index() {
        let document = document();
        let location = document.component_location("text-a").expect("location");
        assert_eq!(location.parent_component_id.as_deref(), Some("section-a"));
        assert_eq!(location.index, 0);
        assert_eq!(location.depth, 2);
    }

    #[test]
    fn placement_rejects_recursive_move() {
        let document = document();
        let registries = RegistrySet::with_builtins();
        let decision = registries.evaluate_placement(
            &document,
            Some("section-a"),
            "section",
            Some("text-a"),
            0,
        );
        assert!(!decision.legal);
        assert!(decision.reason.unwrap_or_default().contains("descendants"));
    }

    #[test]
    fn ancestry_and_placement_cross_anonymous_intermediate_nodes() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "section",
                        "type": "section",
                        "components": [{
                            "type": "section",
                            "components": [{ "id": "nested", "type": "section" }]
                        }]
                    }, { "id": "sibling", "type": "section" }]
                }
            }]
        }))
        .expect("document");

        assert!(document.is_component_descendant_of("nested", "section"));
        assert!(document.is_component_descendant_of("nested", "root"));
        assert!(!document.is_component_descendant_of("nested", "nested"));
        assert!(!document.is_component_descendant_of("nested", "sibling"));
        assert!(!document.is_component_descendant_of("missing", "root"));
        let decision = RegistrySet::with_builtins().evaluate_placement(
            &document,
            Some("section"),
            "section",
            Some("nested"),
            0,
        );
        assert!(!decision.legal);
        assert!(decision.reason.unwrap_or_default().contains("descendants"));
    }

    #[test]
    fn ancestry_uses_actual_branch_not_first_lookup_of_repeated_parent() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "first",
                "component": {
                    "id": "first-root",
                    "type": "wrapper",
                    "components": [{ "id": "repeated", "type": "section" }]
                }
            }, {
                "id": "second",
                "component": {
                    "id": "second-root",
                    "type": "wrapper",
                    "components": [{
                        "id": "repeated",
                        "type": "section",
                        "components": [{ "id": "nested", "type": "section" }]
                    }]
                }
            }]
        }))
        .expect("document");

        // Repeated IDs remain invalid document identities. For a diagnostic
        // lookup the first occurrence wins; it is not the actual ancestor here.
        assert!(document.is_component_descendant_of("nested", "second-root"));
        assert!(!document.is_component_descendant_of("nested", "first-root"));
        assert!(!document.is_component_descendant_of("nested", "repeated"));
    }

    #[test]
    fn placement_rejects_leaf_parent() {
        let document = document();
        let registries = RegistrySet::with_builtins();
        let decision = registries.evaluate_placement(&document, None, "text", Some("text-a"), 0);
        assert!(!decision.legal);
        assert!(
            decision
                .reason
                .unwrap_or_default()
                .contains("does not accept")
        );
    }

    #[test]
    fn placement_allows_builtin_inside_container() {
        let document = document();
        let registries = RegistrySet::with_builtins();
        let decision = registries.evaluate_placement(&document, None, "text", Some("section-b"), 0);
        assert!(decision.legal, "{:?}", decision.reason);
    }
}
