use crate::{
    ComponentNode, EditorCommand, FlyEditor, FlyError, FlyResult, IdGenerator, ProjectDocument,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderRequirement {
    pub provider: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectFragment {
    pub components: Vec<ComponentNode>,
    #[serde(default)]
    pub styles: Vec<Value>,
    #[serde(default)]
    pub assets: Vec<Value>,
    #[serde(default)]
    pub provider_requirements: Vec<ProviderRequirement>,
    #[serde(flatten)]
    pub extensions: Map<String, Value>,
}

impl ProjectFragment {
    pub fn from_component(document: &ProjectDocument, component_id: &str) -> FlyResult<Self> {
        let component = document
            .component(component_id)
            .ok_or_else(|| FlyError::ComponentNotFound(component_id.to_string()))?;
        let node = ComponentNode::Object(Box::new(component.clone()));
        let mut requirements = BTreeSet::new();
        node.visit(0, "fragment.components[0]", &mut |component, _, _| {
            if let Some(provider) = component.provider.as_ref() {
                requirements.insert(provider.clone());
            }
        });
        Ok(Self {
            components: vec![node],
            styles: document.project.styles.clone(),
            assets: document.project.assets.clone(),
            provider_requirements: requirements
                .into_iter()
                .map(|provider| ProviderRequirement { provider })
                .collect(),
            extensions: Map::new(),
        })
    }

    pub fn remap_ids(&mut self, generator: &mut impl IdGenerator) -> BTreeMap<String, String> {
        let mut source_ids = Vec::new();
        for component in &self.components {
            component.collect_ids(&mut source_ids);
        }
        let mapping = source_ids
            .into_iter()
            .map(|source| (source, generator.next_id("paste")))
            .collect::<BTreeMap<_, _>>();
        for component in &mut self.components {
            component.remap_ids(&mapping);
        }
        for style in &mut self.styles {
            replace_value_references(style, &mapping);
        }
        for asset in &mut self.assets {
            replace_value_references(asset, &mapping);
        }
        replace_map_references(&mut self.extensions, &mapping);
        mapping
    }

    pub fn insert(
        mut self,
        editor: &mut FlyEditor,
        parent_id: Option<String>,
        index: usize,
    ) -> FlyResult<Vec<String>> {
        let mut staged = editor.clone();
        self.remap_ids(&mut staged.id_generator);
        let mut inserted_ids = Vec::new();
        let commands = self
            .components
            .into_iter()
            .enumerate()
            .map(|(offset, component)| {
                if let Some(id) = component.id() {
                    inserted_ids.push(id.to_string());
                }
                EditorCommand::Insert {
                    parent_id: parent_id.clone(),
                    index: index + offset,
                    component,
                }
            })
            .collect::<Vec<_>>();
        if commands.is_empty() {
            return Ok(inserted_ids);
        }
        staged.apply(EditorCommand::batch(commands))?;
        *editor = staged;
        Ok(inserted_ids)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RichTextPayload {
    pub capability: String,
    pub payload: Value,
    #[serde(flatten)]
    pub extensions: Map<String, Value>,
}

impl RichTextPayload {
    pub fn opaque(capability: impl Into<String>, payload: Value) -> Self {
        Self {
            capability: capability.into(),
            payload,
            extensions: Map::new(),
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
        Value::Object(map) => replace_map_references(map, mapping),
        _ => {}
    }
}
