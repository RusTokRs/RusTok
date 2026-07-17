use crate::{FlyError, FlyResult, IdGenerator, ProjectHash};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectDocument {
    pub project: GrapesProject,
}

impl ProjectDocument {
    pub fn new(project: GrapesProject) -> Self {
        Self { project }
    }

    pub fn hash(&self) -> ProjectHash {
        ProjectHash::from_document(self)
    }

    pub fn component(&self, id: &str) -> Option<&ComponentObject> {
        self.project.component(id)
    }

    pub fn component_mut(&mut self, id: &str) -> Option<&mut ComponentObject> {
        self.project.component_mut(id)
    }

    pub fn contains_component(&self, id: &str) -> bool {
        self.component(id).is_some()
    }

    pub fn ensure_stable_ids(&mut self, generator: &mut impl IdGenerator) {
        let mut used = BTreeSet::new();
        self.project.visit_components(|component, _, _| {
            if let Some(id) = component.id() {
                if !id.is_empty() {
                    used.insert(id.to_string());
                }
            }
        });
        self.project.ensure_stable_ids(generator, &mut used);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct GrapesProject {
    #[serde(default)]
    pub assets: Vec<Value>,
    #[serde(default)]
    pub styles: Vec<Value>,
    #[serde(default)]
    pub pages: Vec<ProjectPage>,
    #[serde(flatten)]
    pub extensions: Map<String, Value>,
}

impl GrapesProject {
    pub fn component(&self, id: &str) -> Option<&ComponentObject> {
        self.pages
            .iter()
            .filter_map(|page| page.component.as_ref())
            .find_map(|root| root.find(id))
    }

    pub fn component_mut(&mut self, id: &str) -> Option<&mut ComponentObject> {
        self.pages
            .iter_mut()
            .filter_map(|page| page.component.as_mut())
            .find_map(|root| root.find_mut(id))
    }

    pub fn visit_components(&self, mut visitor: impl FnMut(&ComponentObject, usize, &str)) {
        for (page_index, page) in self.pages.iter().enumerate() {
            if let Some(component) = page.component.as_ref() {
                let path = format!("pages[{page_index}].component");
                component.visit(0, &path, &mut visitor);
            }
        }
    }

    fn ensure_stable_ids(&mut self, generator: &mut impl IdGenerator, used: &mut BTreeSet<String>) {
        for page in &mut self.pages {
            if let Some(root) = page.component.as_mut() {
                root.ensure_stable_ids(generator, used);
            }
        }
    }

    fn first_root_mut(&mut self) -> FlyResult<&mut ComponentNode> {
        self.pages
            .iter_mut()
            .find_map(|page| page.component.as_mut())
            .ok_or(FlyError::MissingProjectRoot)
    }

    pub fn insert_component(
        &mut self,
        parent_id: Option<&str>,
        index: usize,
        component: ComponentNode,
    ) -> FlyResult<()> {
        let children = match parent_id {
            Some(parent_id) => self
                .component_mut(parent_id)
                .ok_or_else(|| FlyError::ParentNotFound(parent_id.to_string()))?
                .children_mut()
                .ok_or_else(|| FlyError::OpaqueComponent(parent_id.to_string()))?,
            None => self
                .first_root_mut()?
                .as_object_mut()
                .ok_or_else(|| FlyError::OpaqueComponent("project-root".to_string()))?
                .children_mut()
                .ok_or_else(|| FlyError::OpaqueComponent("project-root".to_string()))?,
        };

        if index > children.len() {
            return Err(FlyError::InvalidInsertionIndex {
                index,
                len: children.len(),
            });
        }
        children.insert(index, component);
        Ok(())
    }

    pub fn remove_component(&mut self, id: &str) -> FlyResult<ComponentNode> {
        for page in &mut self.pages {
            if let Some(root) = page.component.as_mut() {
                if root.id() == Some(id) {
                    return Err(FlyError::OpaqueComponent(
                        "removing a page root is not supported".to_string(),
                    ));
                }
                if let Some(component) = root.remove_descendant(id) {
                    return Ok(component);
                }
            }
        }
        Err(FlyError::ComponentNotFound(id.to_string()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ProjectPage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component: Option<ComponentNode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frames: Option<Value>,
    #[serde(flatten)]
    pub extensions: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ComponentNode {
    Object(Box<ComponentObject>),
    Opaque(Value),
}

impl ComponentNode {
    pub fn object(component_type: impl Into<String>) -> Self {
        Self::Object(Box::new(ComponentObject {
            component_type: Some(component_type.into()),
            ..ComponentObject::default()
        }))
    }

    pub fn as_object(&self) -> Option<&ComponentObject> {
        match self {
            Self::Object(value) => Some(value),
            Self::Opaque(_) => None,
        }
    }

    pub fn as_object_mut(&mut self) -> Option<&mut ComponentObject> {
        match self {
            Self::Object(value) => Some(value),
            Self::Opaque(_) => None,
        }
    }

    pub fn id(&self) -> Option<&str> {
        self.as_object().and_then(ComponentObject::id)
    }

    pub fn find(&self, id: &str) -> Option<&ComponentObject> {
        let object = self.as_object()?;
        if object.id() == Some(id) {
            return Some(object);
        }
        object.children().iter().find_map(|child| child.find(id))
    }

    pub fn find_mut(&mut self, id: &str) -> Option<&mut ComponentObject> {
        let object = self.as_object_mut()?;
        if object.id() == Some(id) {
            return Some(object);
        }
        object
            .children_mut()?
            .iter_mut()
            .find_map(|child| child.find_mut(id))
    }

    pub(crate) fn visit(
        &self,
        depth: usize,
        path: &str,
        visitor: &mut impl FnMut(&ComponentObject, usize, &str),
    ) {
        let Some(object) = self.as_object() else {
            return;
        };
        visitor(object, depth, path);
        for (index, child) in object.children().iter().enumerate() {
            child.visit(depth + 1, &format!("{path}.components[{index}]"), visitor);
        }
    }

    fn ensure_stable_ids(&mut self, generator: &mut impl IdGenerator, used: &mut BTreeSet<String>) {
        let Some(object) = self.as_object_mut() else {
            return;
        };
        if object.id.as_deref().is_none_or(str::is_empty) {
            let hint = object.component_type.as_deref().unwrap_or("node");
            let id = loop {
                let candidate = generator.next_id(hint);
                if used.insert(candidate.clone()) {
                    break candidate;
                }
            };
            object.id = Some(id);
        }
        if let Some(children) = object.children_mut() {
            for child in children {
                child.ensure_stable_ids(generator, used);
            }
        }
    }

    fn remove_descendant(&mut self, id: &str) -> Option<ComponentNode> {
        let object = self.as_object_mut()?;
        let children = object.children_mut()?;
        if let Some(index) = children.iter().position(|child| child.id() == Some(id)) {
            return Some(children.remove(index));
        }
        children
            .iter_mut()
            .find_map(|child| child.remove_descendant(id))
    }

    pub(crate) fn collect_ids(&self, ids: &mut Vec<String>) {
        if let Some(object) = self.as_object() {
            if let Some(id) = object.id.clone() {
                ids.push(id);
            }
            for child in object.children() {
                child.collect_ids(ids);
            }
        }
    }

    pub(crate) fn remap_ids(&mut self, mapping: &BTreeMap<String, String>) {
        match self {
            Self::Opaque(value) => replace_value_references(value, mapping),
            Self::Object(object) => {
                if let Some(id) = object.id.as_mut() {
                    if let Some(replacement) = mapping.get(id) {
                        *id = replacement.clone();
                    }
                }
                replace_map_references(&mut object.attributes, mapping);
                if let Some(style) = object.style.as_mut() {
                    replace_value_references(style, mapping);
                }
                for trait_value in &mut object.traits {
                    replace_value_references(trait_value, mapping);
                }
                replace_map_references(&mut object.extensions, mapping);
                if let Some(children) = object.children_mut() {
                    for child in children {
                        child.remap_ids(mapping);
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComponentObject {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub component_type: Option<String>,
    #[serde(rename = "tagName", default, skip_serializing_if = "Option::is_none")]
    pub tag_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub attributes: Map<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub traits: Vec<Value>,
    #[serde(default)]
    pub components: ComponentChildren,
    #[serde(flatten)]
    pub extensions: Map<String, Value>,
}

impl Default for ComponentObject {
    fn default() -> Self {
        Self {
            id: None,
            component_type: None,
            tag_name: None,
            provider: None,
            attributes: Map::new(),
            style: None,
            traits: Vec::new(),
            components: ComponentChildren::Nodes(Vec::new()),
            extensions: Map::new(),
        }
    }
}

impl ComponentObject {
    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    pub fn component_type(&self) -> &str {
        self.component_type.as_deref().unwrap_or("default")
    }

    pub fn children(&self) -> &[ComponentNode] {
        self.components.as_nodes().unwrap_or(&[])
    }

    pub fn children_mut(&mut self) -> Option<&mut Vec<ComponentNode>> {
        self.components.as_nodes_mut()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ComponentChildren {
    Nodes(Vec<ComponentNode>),
    Opaque(Value),
}

impl Default for ComponentChildren {
    fn default() -> Self {
        Self::Nodes(Vec::new())
    }
}

impl ComponentChildren {
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Nodes(nodes) => nodes.is_empty(),
            Self::Opaque(Value::Null) => true,
            Self::Opaque(_) => false,
        }
    }

    pub fn as_nodes(&self) -> Option<&[ComponentNode]> {
        match self {
            Self::Nodes(nodes) => Some(nodes),
            Self::Opaque(_) => None,
        }
    }

    pub fn as_nodes_mut(&mut self) -> Option<&mut Vec<ComponentNode>> {
        match self {
            Self::Nodes(nodes) => Some(nodes),
            Self::Opaque(_) => None,
        }
    }
}

fn replace_map_references(map: &mut Map<String, Value>, mapping: &BTreeMap<String, String>) {
    for value in map.values_mut() {
        replace_value_references(value, mapping);
    }
}

fn replace_value_references(value: &mut Value, mapping: &BTreeMap<String, String>) {
    match value {
        Value::String(string) => {
            if let Some(replacement) = mapping.get(string) {
                *string = replacement.clone();
            }
        }
        Value::Array(values) => {
            for value in values {
                replace_value_references(value, mapping);
            }
        }
        Value::Object(object) => replace_map_references(object, mapping),
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}
