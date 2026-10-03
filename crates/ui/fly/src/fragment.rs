use crate::id_reference::{remap_map_ids, remap_value_ids};
use crate::{
    AssetCommand, AssetDescriptor, ComponentNode, EditorCommand, FLY_COMPONENT_RULE_FIELD,
    FLY_RULE_ID_FIELD, FlyEditor, FlyError, FlyResult, IdGenerator, ProjectDocument,
    StyleRuleCommand, StyleRuleDescriptor,
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
        let mut component_ids = BTreeSet::new();
        let mut asset_references = BTreeSet::new();
        node.visit(0, "fragment.components[0]", &mut |component, _, _| {
            if let Some(provider) = component.provider.as_ref() {
                requirements.insert(provider.clone());
            }
            if let Some(id) = component.id.as_ref() {
                component_ids.insert(id.clone());
            }
            collect_component_asset_references(component, &mut asset_references);
        });
        let styles = document
            .project
            .styles
            .iter()
            .filter(|raw| {
                StyleRuleDescriptor::from_value((*raw).clone())
                    .and_then(|rule| rule.component_id)
                    .is_some_and(|component_id| component_ids.contains(&component_id))
            })
            .cloned()
            .collect();
        let assets = document
            .project
            .assets
            .iter()
            .filter(|raw| {
                AssetDescriptor::from_value((*raw).clone()).is_some_and(|asset| {
                    asset_references.contains(&asset.id)
                        || asset_references.contains(&asset.source)
                        || asset
                            .provider_asset_id
                            .as_ref()
                            .is_some_and(|id| asset_references.contains(id))
                })
            })
            .cloned()
            .collect();
        Ok(Self {
            components: vec![node],
            styles,
            assets,
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
            remap_value_ids(style, &mapping);
            reset_remapped_style_rule_identity(style, &mapping);
        }
        for asset in &mut self.assets {
            remap_value_ids(asset, &mapping);
        }
        remap_map_ids(&mut self.extensions, &mapping);
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
        let mut commands = Vec::new();
        commands.extend(
            self.assets
                .into_iter()
                .map(|asset| EditorCommand::Asset {
                    command: AssetCommand::Upsert { asset },
                }),
        );
        commands.extend(
            self.components
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
                }),
        );
        commands.extend(
            self.styles
                .into_iter()
                .map(|rule| EditorCommand::StyleRule {
                    command: StyleRuleCommand::UpsertRaw { rule },
                }),
        );
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

fn collect_component_asset_references(
    component: &crate::ComponentObject,
    references: &mut BTreeSet<String>,
) {
    for key in [
        "data-fly-asset-id",
        "data-fly-provider-asset-id",
        "src",
        "poster",
        "href",
    ] {
        if let Some(value) = component.attributes.get(key).and_then(Value::as_str) {
            references.insert(value.to_string());
        }
    }
}

fn reset_remapped_style_rule_identity(style: &mut Value, mapping: &BTreeMap<String, String>) {
    let Some(object) = style.as_object_mut() else {
        return;
    };
    let Some(component_id) = object
        .get(FLY_COMPONENT_RULE_FIELD)
        .and_then(Value::as_str)
        .map(ToString::to_string)
    else {
        return;
    };
    if mapping.values().any(|target_id| target_id == &component_id) {
        object.remove(FLY_RULE_ID_FIELD);
        object.remove("id");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AssetDescriptor, FlyEditor, GrapesJsCodec, RegistrySet, StyleRuleCatalog, StyleRuleScope,
    };
    use serde_json::json;

    #[test]
    fn insert_carries_referenced_assets_and_component_styles() {
        let source = GrapesJsCodec::decode_value(json!({
            "assets": [{ "id": "hero-asset", "src": "/media/hero.webp" }],
            "styles": [{
                "selectors": [{ "name": "hero", "type": 2 }],
                "style": { "padding": "24px" },
                "flyComponentId": "hero",
                "flyRuleId": "source-hero-rule"
            }],
            "pages": [{
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "hero",
                        "type": "image",
                        "attributes": {
                            "src": "/media/hero.webp",
                            "data-fly-asset-id": "hero-asset"
                        }
                    }]
                }
            }]
        }))
        .expect("source");
        let target = GrapesJsCodec::decode_value(json!({
            "assets": [],
            "styles": [],
            "pages": [{
                "component": {
                    "id": "target-root",
                    "type": "wrapper",
                    "components": []
                }
            }]
        }))
        .expect("target");
        let fragment = ProjectFragment::from_component(&source, "hero").expect("fragment");
        let mut editor = FlyEditor::new(target, RegistrySet::with_builtins());

        let inserted = fragment
            .insert(&mut editor, Some("target-root".to_string()), 0)
            .expect("insert fragment");

        assert_eq!(inserted, vec!["fly-paste-1".to_string()]);
        assert!(editor.document().project.assets.iter().any(|asset| {
            AssetDescriptor::from_value(asset.clone())
                .is_some_and(|asset| asset.id == "hero-asset")
        }));
        assert!(
            StyleRuleCatalog::from_document(editor.document())
                .component_rule("fly-paste-1", &StyleRuleScope::Base)
                .is_some()
        );
    }
}
