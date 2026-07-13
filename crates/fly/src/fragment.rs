use crate::{
    ComponentNode, EditorCommand, FlyEditor, FlyError, FlyResult, IdGenerator, ProjectDocument,
    ProjectFormat, ValidationDiagnostic, FLY_FRAGMENT_V1, RICH_TEXT_PAYLOAD_V1,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderRequirement {
    pub provider: String,
    pub schema_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectFragment {
    pub format: String,
    pub source_project_format: ProjectFormat,
    pub components: Vec<ComponentNode>,
    #[serde(default)]
    pub styles: Vec<Value>,
    #[serde(default)]
    pub assets: Vec<Value>,
    #[serde(default)]
    pub provider_requirements: Vec<ProviderRequirement>,
    #[serde(default)]
    pub migration_diagnostics: Vec<ValidationDiagnostic>,
    #[serde(flatten)]
    pub extensions: Map<String, Value>,
}

impl ProjectFragment {
    pub fn from_component(document: &ProjectDocument, component_id: &str) -> FlyResult<Self> {
        let component = document
            .component(component_id)
            .ok_or_else(|| FlyError::ComponentNotFound(component_id.to_string()))?;
        let node = ComponentNode::Object(component.clone());
        let mut requirements = BTreeMap::<String, Option<String>>::new();
        node.visit(0, "fragment.components[0]", &mut |component, _, _| {
            if let Some(provider) = component.provider.as_ref() {
                requirements
                    .entry(provider.clone())
                    .or_insert_with(|| component.schema_version.clone());
            }
        });
        Ok(Self {
            format: FLY_FRAGMENT_V1.to_string(),
            source_project_format: document.format,
            components: vec![node],
            styles: document.project.styles.clone(),
            assets: document.project.assets.clone(),
            provider_requirements: requirements
                .into_iter()
                .map(|(provider, schema_version)| ProviderRequirement {
                    provider,
                    schema_version,
                })
                .collect(),
            migration_diagnostics: Vec::new(),
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
            .map(|source| {
                let target = generator.next_id("paste");
                (source, target)
            })
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
    pub format: String,
    pub capability: String,
    pub schema_version: String,
    pub payload: Value,
    #[serde(flatten)]
    pub extensions: Map<String, Value>,
}

impl RichTextPayload {
    pub fn opaque(
        capability: impl Into<String>,
        schema_version: impl Into<String>,
        payload: Value,
    ) -> Self {
        Self {
            format: RICH_TEXT_PAYLOAD_V1.to_string(),
            capability: capability.into(),
            schema_version: schema_version.into(),
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
        Value::Object(object) => replace_map_references(object, mapping),
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GrapesJsV1Codec, RegistrySet};
    use serde_json::json;

    #[test]
    fn multi_component_fragment_uses_one_history_entry() {
        let document = GrapesJsV1Codec::decode_value(json!({
            "pages": [{
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": []
                }
            }]
        }))
        .expect("document");
        let mut editor = FlyEditor::new(document, RegistrySet::with_builtins());
        let fragment = ProjectFragment {
            format: FLY_FRAGMENT_V1.to_string(),
            source_project_format: ProjectFormat::GrapesJsV1,
            components: vec![
                serde_json::from_value(json!({ "id": "a", "type": "text" }))
                    .expect("component a"),
                serde_json::from_value(json!({ "id": "b", "type": "text" }))
                    .expect("component b"),
            ],
            styles: Vec::new(),
            assets: Vec::new(),
            provider_requirements: Vec::new(),
            migration_diagnostics: Vec::new(),
            extensions: Map::new(),
        };
        let inserted = fragment
            .insert(&mut editor, Some("root".to_string()), 0)
            .expect("insert fragment");
        assert_eq!(inserted.len(), 2);
        assert_eq!(editor.history().undo_len(), 1);
        editor.undo().expect("undo fragment");
        assert_eq!(editor.document().component_child_count("root"), Some(0));
    }
}
