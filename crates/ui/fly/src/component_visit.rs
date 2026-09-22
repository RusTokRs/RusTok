use crate::{ComponentNode, ComponentObject, GrapesProject};

/// Stable metadata supplied while walking the canonical component tree.
///
/// The path is diagnostic-only. Component identity continues to use stable component ids;
/// consumers must not persist the path as an editor reference because sibling insertions can
/// change indexes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentVisit {
    page_index: usize,
    page_id: Option<String>,
    depth: usize,
    path: String,
}

impl ComponentVisit {
    fn root(page_index: usize, page_id: Option<String>) -> Self {
        Self {
            page_index,
            page_id,
            depth: 0,
            path: format!("project.pages[{page_index}].component"),
        }
    }

    fn child(&self, index: usize) -> Self {
        Self {
            page_index: self.page_index,
            page_id: self.page_id.clone(),
            depth: self.depth.saturating_add(1),
            path: format!("{}.components[{index}]", self.path),
        }
    }

    pub const fn page_index(&self) -> usize {
        self.page_index
    }

    pub fn page_id(&self) -> Option<&str> {
        self.page_id.as_deref()
    }

    pub const fn depth(&self) -> usize {
        self.depth
    }

    pub fn path(&self) -> &str {
        &self.path
    }
}

/// Walks every typed component in deterministic page/pre-order sequence.
///
/// Opaque provider nodes remain lossless in the document but are intentionally skipped because
/// Fly cannot safely expose them as `ComponentObject` values. Mutation stays crate-private so
/// external adapters cannot bypass editor commands, validation, revision tracking, or history.
pub fn visit_project_components(
    project: &GrapesProject,
    mut visitor: impl FnMut(&ComponentObject, &ComponentVisit),
) {
    for (page_index, page) in project.pages.iter().enumerate() {
        let Some(root) = page.component.as_ref() else {
            continue;
        };
        visit_node(
            root,
            ComponentVisit::root(page_index, page.id.clone()),
            &mut visitor,
        );
    }
}

pub(crate) fn visit_project_components_mut(
    project: &mut GrapesProject,
    mut visitor: impl FnMut(&mut ComponentObject, &ComponentVisit),
) {
    for (page_index, page) in project.pages.iter_mut().enumerate() {
        let page_id = page.id.clone();
        let Some(root) = page.component.as_mut() else {
            continue;
        };
        visit_node_mut(
            root,
            ComponentVisit::root(page_index, page_id),
            &mut visitor,
        );
    }
}

fn visit_node(
    node: &ComponentNode,
    visit: ComponentVisit,
    visitor: &mut impl FnMut(&ComponentObject, &ComponentVisit),
) {
    let Some(component) = node.as_object() else {
        return;
    };
    visitor(component, &visit);
    for (index, child) in component.children().iter().enumerate() {
        visit_node(child, visit.child(index), visitor);
    }
}

fn visit_node_mut(
    node: &mut ComponentNode,
    visit: ComponentVisit,
    visitor: &mut impl FnMut(&mut ComponentObject, &ComponentVisit),
) {
    let Some(component) = node.as_object_mut() else {
        return;
    };
    visitor(component, &visit);
    let Some(children) = component.children_mut() else {
        return;
    };
    for (index, child) in children.iter_mut().enumerate() {
        visit_node_mut(child, visit.child(index), visitor);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GrapesJsCodec;
    use serde_json::{Value, json};

    #[test]
    fn immutable_and_mutable_walks_share_page_depth_and_path_contract() {
        let mut document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [
                        "opaque-provider-node",
                        {
                            "id": "section",
                            "type": "section",
                            "components": [{ "id": "copy", "type": "text" }]
                        }
                    ]
                }
            }, {
                "id": "empty"
            }]
        }))
        .expect("document");

        let mut immutable = Vec::new();
        visit_project_components(&document.project, |component, visit| {
            immutable.push((
                component.id.clone().unwrap_or_default(),
                visit.page_index(),
                visit.page_id().map(ToString::to_string),
                visit.depth(),
                visit.path().to_string(),
            ));
        });

        let mut mutable = Vec::new();
        visit_project_components_mut(&mut document.project, |component, visit| {
            mutable.push((
                component.id.clone().unwrap_or_default(),
                visit.page_index(),
                visit.page_id().map(ToString::to_string),
                visit.depth(),
                visit.path().to_string(),
            ));
            component
                .attributes
                .insert("data-visited".to_string(), Value::Bool(true));
        });

        assert_eq!(immutable, mutable);
        assert_eq!(
            immutable,
            vec![
                (
                    "root".to_string(),
                    0,
                    Some("home".to_string()),
                    0,
                    "project.pages[0].component".to_string(),
                ),
                (
                    "section".to_string(),
                    0,
                    Some("home".to_string()),
                    1,
                    "project.pages[0].component.components[1]".to_string(),
                ),
                (
                    "copy".to_string(),
                    0,
                    Some("home".to_string()),
                    2,
                    "project.pages[0].component.components[1].components[0]".to_string(),
                ),
            ]
        );
        assert_eq!(
            document.component("copy").unwrap().attributes["data-visited"],
            Value::Bool(true)
        );
    }
}
