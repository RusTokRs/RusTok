use crate::id_reference::{remap_attribute_ids, remap_map_ids, remap_value_ids};
use crate::{FlyError, FlyResult, IdGenerator, ProjectHash, validate_identifier};
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

    /// Give every component a unique, syntactically valid id, minting new ones where needed.
    ///
    /// Ids that fail [`validate_identifier`] are replaced rather than preserved. An id is
    /// interpolated into HTML attributes and into CSS selectors inside a raw `<style>` element, so
    /// a malformed one is a liability; keeping it would also deadlock the editor, because
    /// validation rejects it and every command runs validation. Self-healing here mirrors how
    /// duplicate ids have always been handled.
    pub fn ensure_stable_ids(&mut self, generator: &mut impl IdGenerator) {
        let mut reserved = BTreeSet::new();
        self.project.visit_components(|component, _, _| {
            if let Some(id) = component.id()
                && validate_identifier(id).is_ok()
            {
                reserved.insert(id.to_string());
            }
        });
        let mut seen = BTreeSet::new();
        self.project
            .ensure_stable_ids(generator, &mut reserved, &mut seen);
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

    fn ensure_stable_ids(
        &mut self,
        generator: &mut impl IdGenerator,
        reserved: &mut BTreeSet<String>,
        seen: &mut BTreeSet<String>,
    ) {
        for page in &mut self.pages {
            if let Some(root) = page.component.as_mut() {
                root.ensure_stable_ids(generator, reserved, seen);
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

    fn ensure_stable_ids(
        &mut self,
        generator: &mut impl IdGenerator,
        reserved: &mut BTreeSet<String>,
        seen: &mut BTreeSet<String>,
    ) {
        let Some(object) = self.as_object_mut() else {
            return;
        };
        let current_id = object
            .id
            .as_deref()
            .filter(|id| validate_identifier(id).is_ok());
        let needs_new_id = current_id.is_none_or(|id| !seen.insert(id.to_string()));
        if needs_new_id {
            let hint = object.component_type.as_deref().unwrap_or("node");
            let id = loop {
                let candidate = generator.next_id(hint);
                if reserved.insert(candidate.clone()) && seen.insert(candidate.clone()) {
                    break candidate;
                }
            };
            object.id = Some(id);
        }
        if let Some(children) = object.children_mut() {
            for child in children {
                child.ensure_stable_ids(generator, reserved, seen);
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
            Self::Opaque(value) => remap_value_ids(value, mapping),
            Self::Object(object) => {
                if let Some(id) = object.id.as_mut()
                    && let Some(replacement) = mapping.get(id)
                {
                    *id = replacement.clone();
                }
                remap_attribute_ids(&mut object.attributes, mapping);
                if let Some(style) = object.style.as_mut() {
                    remap_value_ids(style, mapping);
                }
                for trait_value in &mut object.traits {
                    remap_value_ids(trait_value, mapping);
                }
                remap_map_ids(&mut object.extensions, mapping);
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

    pub fn text_content(&self) -> Option<String> {
        let mut output = String::new();
        append_component_text(self, &mut output);
        let output = output.trim();
        (!output.is_empty()).then(|| output.to_string())
    }

    pub(crate) fn set_extension_field(&mut self, name: String, value: Value) {
        if name == "content" {
            self.clear_direct_text_children();
        }
        self.extensions.insert(name, value);
    }

    pub(crate) fn remove_extension_field(&mut self, name: &str) {
        if name == "content" {
            self.clear_direct_text_children();
        }
        self.extensions.remove(name);
    }

    fn clear_direct_text_children(&mut self) {
        if let Some(children) = self.children_mut() {
            children.retain(|child| !is_scalar_text_node(child));
        }
    }
}

fn append_component_text(component: &ComponentObject, output: &mut String) {
    append_scalar_text(component.extensions.get("content"), output);
    for child in component.children() {
        append_node_text(child, output);
    }
}

fn append_node_text(node: &ComponentNode, output: &mut String) {
    match node {
        ComponentNode::Object(component) => append_component_text(component, output),
        ComponentNode::Opaque(value) => append_scalar_text(Some(value), output),
    }
}

fn append_scalar_text(value: Option<&Value>, output: &mut String) {
    match value {
        Some(Value::String(value)) => {
            output.push_str(value);
            output.push(' ');
        }
        Some(Value::Number(value)) => {
            output.push_str(&value.to_string());
            output.push(' ');
        }
        Some(Value::Bool(value)) => {
            output.push_str(if *value { "true" } else { "false" });
            output.push(' ');
        }
        Some(Value::Null | Value::Array(_) | Value::Object(_)) | None => {}
    }
}

fn is_scalar_text_node(node: &ComponentNode) -> bool {
    matches!(
        node,
        ComponentNode::Opaque(Value::String(_) | Value::Number(_) | Value::Bool(_))
    )
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

