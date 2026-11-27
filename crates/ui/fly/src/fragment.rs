use crate::{
    AssetCommand, AssetDescriptor, ComponentNode, EditorCommand, FlyEditor, FlyError, FlyResult,
    IdGenerator, ProjectDocument, StyleRuleCommand, StyleRuleDescriptor,
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
            .project()
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
            .project()
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

    /// Reissue identities and local references. Reject ambiguous source identities without
    /// mutating the fragment or consuming the generator.
    pub fn remap_ids(
        &mut self,
        generator: &mut impl IdGenerator,
    ) -> FlyResult<BTreeMap<String, String>> {
        self.remap_ids_avoiding(generator, BTreeSet::new())
    }

    fn remap_ids_avoiding(
        &mut self,
        generator: &mut impl IdGenerator,
        mut reserved: BTreeSet<String>,
    ) -> FlyResult<BTreeMap<String, String>> {
        if let Some(reason) = crate::id_reference::ambiguous_tree_identity(self.components.iter()) {
            return Err(FlyError::Decode(format!(
                "cannot remap an ambiguous fragment: {reason}"
            )));
        }
        let mut source_ids = Vec::new();
        let mut dom_ids = Vec::new();
        let mut form_ids = Vec::new();
        let mut matching_ids = BTreeSet::new();
        for component in &self.components {
            component.collect_ids(&mut source_ids);
            component.collect_dom_ids(&mut dom_ids);
            component.collect_form_ids(&mut form_ids);
            component.collect_matching_dom_ids(&mut matching_ids);
        }
        reserved.extend(source_ids.iter().chain(&dom_ids).chain(&form_ids).cloned());
        let mapping = crate::id_reference::fresh_component_id_mapping(
            source_ids,
            "paste",
            &mut reserved,
            generator,
        );
        let form_ids = form_ids.into_iter().collect();
        let dom_mapping = crate::id_reference::fresh_dom_id_mapping(
            dom_ids,
            &matching_ids,
            &form_ids,
            &mapping,
            &mut reserved,
            generator,
        );
        for component in &mut self.components {
            component.remap_ids_with_dom(&mapping, &dom_mapping);
        }
        for style in &mut self.styles {
            crate::style_rule::remap_component_style_rule(style, &mapping);
        }
        Ok(mapping)
    }

    pub fn insert(
        mut self,
        editor: &mut FlyEditor,
        parent_id: Option<String>,
        index: usize,
    ) -> FlyResult<Vec<String>> {
        // Validate and remap all roots together so cross-root references remain coherent.
        let reserved = crate::id_reference::project_identity_ids(editor.document());
        let mut staged = editor.clone();
        self.remap_ids_avoiding(&mut staged.id_generator, reserved)?;
        let existing_assets = staged.document().project().assets.clone();
        let mut existing_asset_references = BTreeSet::new();
        for page in &staged.document().project().pages {
            if let Some(root) = page.component.as_ref() {
                collect_component_asset_ids(root, &mut existing_asset_references);
            }
        }
        remap_fragment_asset_ids(
            &mut self.assets,
            &mut self.components,
            &existing_assets,
            &existing_asset_references,
            &mut staged.id_generator,
        )?;
        let mut inserted_ids = Vec::new();
        let mut commands = Vec::new();
        commands.extend(self.assets.into_iter().map(|asset| EditorCommand::Asset {
            command: AssetCommand::Upsert { asset },
        }));
        for (offset, component) in self.components.into_iter().enumerate() {
            if let Some(id) = component.id() {
                inserted_ids.push(id.to_string());
            }
            let insertion_index = index.checked_add(offset).ok_or_else(|| {
                FlyError::Decode("fragment insertion index overflow".to_string())
            })?;
            commands.push(EditorCommand::Insert {
                parent_id: parent_id.clone(),
                index: insertion_index,
                component,
            });
        }
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

fn remap_fragment_asset_ids(
    assets: &mut [Value],
    components: &mut [ComponentNode],
    existing_assets: &[Value],
    existing_asset_references: &BTreeSet<String>,
    generator: &mut impl IdGenerator,
) -> FlyResult<()> {
    let mut existing_by_id = BTreeMap::new();
    for raw in existing_assets {
        if let Some(asset) = AssetDescriptor::from_value(raw.clone()) {
            existing_by_id.entry(asset.id).or_insert_with(|| raw.clone());
        }
    }

    let mut reserved = existing_by_id.keys().cloned().collect::<BTreeSet<_>>();
    reserved.extend(existing_asset_references.iter().cloned());
    let mut seen = BTreeSet::new();
    let mut mapping = BTreeMap::new();
    for raw in assets {
        let Some(asset) = AssetDescriptor::from_value(raw.clone()) else {
            continue;
        };
        if !seen.insert(asset.id.clone()) {
            return Err(FlyError::InvalidAssetReference(format!(
                "fragment contains duplicate asset id `{}`",
                asset.id
            )));
        }
        reserved.insert(asset.id.clone());
        let conflicts_with_existing_record = existing_by_id
            .get(&asset.id)
            .is_some_and(|existing| existing != raw);
        let conflicts_with_dangling_reference = !existing_by_id.contains_key(&asset.id)
            && existing_asset_references.contains(&asset.id);
        if conflicts_with_existing_record || conflicts_with_dangling_reference {
            let replacement =
                crate::id_reference::allocate_generated_id(generator, "asset", &mut reserved);
            set_asset_identity(raw, &replacement)?;
            mapping.insert(asset.id, replacement);
        }
    }

    if !mapping.is_empty() {
        for component in components {
            remap_component_asset_references(component, &mapping);
        }
    }
    Ok(())
}

fn collect_component_asset_ids(
    component: &ComponentNode,
    asset_ids: &mut BTreeSet<String>,
) {
    match component {
        ComponentNode::Object(object) => {
            if let Some(Value::String(asset_id)) =
                object.attributes.get("data-fly-asset-id")
                && !asset_id.trim().is_empty()
            {
                asset_ids.insert(asset_id.clone());
            }
            if let Some(children) = object.children() {
                for child in children {
                    collect_component_asset_ids(child, asset_ids);
                }
            }
        }
        ComponentNode::Opaque(raw) => collect_opaque_asset_ids(raw, asset_ids),
    }
}

fn collect_opaque_asset_ids(raw: &Value, asset_ids: &mut BTreeSet<String>) {
    let Some(object) = raw.as_object() else {
        return;
    };
    if let Some(attributes) = object.get("attributes").and_then(Value::as_object)
        && let Some(asset_id) = attributes
            .get("data-fly-asset-id")
            .and_then(Value::as_str)
        && !asset_id.trim().is_empty()
    {
        asset_ids.insert(asset_id.to_string());
    }
    if let Some(children) = object.get("components").and_then(Value::as_array) {
        for child in children {
            collect_opaque_asset_ids(child, asset_ids);
        }
    }
}

fn set_asset_identity(raw: &mut Value, id: &str) -> FlyResult<()> {
    let Some(object) = raw.as_object_mut() else {
        return Err(FlyError::InvalidAssetReference(
            "asset must be a JSON object".to_string(),
        ));
    };
    let mut updated = false;
    for key in ["id", "assetId"] {
        if object.contains_key(key) {
            object.insert(key.to_string(), Value::String(id.to_string()));
            updated = true;
        }
    }
    if !updated {
        object.insert("id".to_string(), Value::String(id.to_string()));
    }
    Ok(())
}

fn remap_component_asset_references(
    component: &mut ComponentNode,
    mapping: &BTreeMap<String, String>,
) {
    match component {
        ComponentNode::Object(object) => {
            remap_asset_attribute(&mut object.attributes, mapping);
            if let Some(children) = object.children_mut() {
                for child in children {
                    remap_component_asset_references(child, mapping);
                }
            }
        }
        ComponentNode::Opaque(raw) => remap_opaque_asset_references(raw, mapping),
    }
}

fn remap_opaque_asset_references(raw: &mut Value, mapping: &BTreeMap<String, String>) {
    let Some(object) = raw.as_object_mut() else {
        return;
    };
    if let Some(attributes) = object.get_mut("attributes").and_then(Value::as_object_mut) {
        remap_asset_attribute(attributes, mapping);
    }
    if let Some(children) = object.get_mut("components").and_then(Value::as_array_mut) {
        for child in children {
            remap_opaque_asset_references(child, mapping);
        }
    }
}

fn remap_asset_attribute(
    attributes: &mut Map<String, Value>,
    mapping: &BTreeMap<String, String>,
) {
    let Some(Value::String(asset_id)) = attributes.get_mut("data-fly-asset-id") else {
        return;
    };
    if let Some(replacement) = mapping.get(asset_id) {
        *asset_id = replacement.clone();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AssetDescriptor, FlyEditor, GrapesJsCodec, RegistrySet, StyleRuleCatalog, StyleRuleScope,
    };
    use serde_json::json;

    #[test]
    fn paste_remaps_html_ids_and_actions_without_changing_opaque_content() {
        let source = GrapesJsCodec::decode_value(json!({
            "pages": [{"component": {"id": "root", "type": "wrapper", "components": [{
                "id": "group", "type": "container", "components": [
                    {"id": "trigger", "type": "button", "attributes": {"aria-controls": "answer", "data-fly-tab-target": "answer", "placeholder": "answer"},
                     "flyAction": {"kind": "switch_tab", "panel_id": "answer"}},
                    {"id": "panel", "type": "container", "attributes": {"id": "answer", "href": "#answer"}, "content": "answer"},
                    {"id": "external", "type": "button", "flyAction": {"kind": "scroll_to", "target_id": "outside"}},
                    {"id": "event", "type": "button", "flyAction": {"kind": "emit_event", "event": "track", "payload": {"component_id": "panel"}}}
                ]
            }]}}]
        })).expect("source");
        let mut fragment = ProjectFragment::from_component(&source, "group").expect("fragment");
        let mapping = fragment
            .remap_ids(&mut crate::SequentialIdGenerator::new("copy"))
            .expect("remap");
        let group = fragment.components[0].as_object().unwrap();
        let trigger = group.children()[0].as_object().unwrap();
        let panel = group.children()[1].as_object().unwrap();
        let new_dom = panel.attributes["id"].as_str().unwrap();
        assert_ne!(new_dom, "answer");
        assert_eq!(trigger.extensions["flyAction"]["panel_id"], new_dom);
        assert_eq!(trigger.attributes["aria-controls"], new_dom);
        assert_eq!(trigger.attributes["data-fly-tab-target"], new_dom);
        assert_eq!(panel.attributes["href"], format!("#{new_dom}"));
        assert_eq!(panel.id.as_ref(), mapping.get("panel"));
        assert_eq!(trigger.attributes["placeholder"], "answer");
        assert_eq!(panel.extensions["content"], "answer");
        assert_eq!(
            group.children()[2].as_object().unwrap().extensions["flyAction"]["target_id"],
            "outside"
        );
        assert_eq!(
            group.children()[3].as_object().unwrap().extensions["flyAction"]["payload"]["component_id"],
            "panel"
        );
    }

    #[test]
    fn opaque_component_identity_slots_are_remapped_without_changing_provider_content() {
        let source = GrapesJsCodec::decode_value(json!({
            "pages": [{"component": {"id": "root", "type": "wrapper", "components": [{
                "id": "group", "type": "container", "components": []
            }]}}]
        }))
        .unwrap();
        let mut fragment = ProjectFragment::from_component(&source, "group").unwrap();
        fragment.components[0]
            .as_object_mut()
            .unwrap()
            .children_mut()
            .unwrap()
            .push(ComponentNode::Opaque(json!({
                "id": "opaque", "attributes": {"id": "panel", "aria-controls": "inside"},
                "flyAction": {"kind": "scroll_to", "target_id": "inside"},
                "providerData": {"label": "opaque", "value": "inside", "component_id": "inside", "flyComponentId": "opaque"},
                "components": [{"id": "inside", "attributes": {"id": "inside"}}]
            })));
        let mapping = fragment
            .remap_ids(&mut crate::SequentialIdGenerator::new("copy"))
            .expect("remap");
        let ComponentNode::Opaque(opaque) =
            &fragment.components[0].as_object().unwrap().children()[0]
        else {
            panic!("opaque node was replaced")
        };
        assert_eq!(opaque["id"], mapping["opaque"]);
        assert_eq!(opaque["components"][0]["id"], mapping["inside"]);
        assert_eq!(
            opaque["components"][0]["attributes"]["id"],
            mapping["inside"]
        );
        assert_eq!(opaque["attributes"]["aria-controls"], mapping["inside"]);
        assert_eq!(opaque["flyAction"]["target_id"], mapping["inside"]);
        assert_eq!(
            opaque["providerData"],
            json!({"label": "opaque", "value": "inside", "component_id": "inside", "flyComponentId": "opaque"})
        );
        assert_ne!(opaque["attributes"]["id"], "panel");
    }

    #[test]
    fn fragment_id_allocator_reserves_the_destination_and_keeps_external_targets() {
        let source = GrapesJsCodec::decode_value(json!({
            "pages": [{"component": {"id": "root", "type": "wrapper", "components": [{
                "id": "source", "type": "container", "components": [
                    {"id": "link", "type": "button", "flyAction": {"kind": "scroll_to", "target_id": "copy-paste-1"}},
                    {"id": "panel", "type": "section", "attributes": {"id": "copy-dom-4"}}
                ]
            }]}}]
        })).unwrap();
        let mut fragment = ProjectFragment::from_component(&source, "source").unwrap();
        let reserved = BTreeSet::from(["copy-paste-1".to_string(), "copy-dom-4".to_string()]);
        let mapping = fragment
            .remap_ids_avoiding(
                &mut crate::SequentialIdGenerator::new("copy"),
                reserved.clone(),
            )
            .expect("remap");
        assert!(mapping.values().all(|id| !reserved.contains(id)));
        let group = fragment.components[0].as_object().unwrap();
        assert_eq!(
            group.children()[0].as_object().unwrap().extensions["flyAction"]["target_id"],
            "copy-paste-1"
        );
        let panel = group.children()[1].as_object().unwrap();
        assert!(!reserved.contains(panel.attributes["id"].as_str().unwrap()));
    }

    #[test]
    fn fragment_rejects_cross_root_form_html_collision_atomically() {
        let source = GrapesJsCodec::decode_value(json!({
            "pages": [{"component": {"id": "root", "type": "wrapper"}}]
        }))
        .unwrap();
        let mut editor = FlyEditor::new(source, RegistrySet::with_builtins());
        let before = editor.clone();
        let fragment = ProjectFragment {
            components: vec![
                serde_json::from_value(json!({"id": "form-node", "type": "form", "flyForm": {"id": "contact", "provider": "forms", "action": "submit"}})).unwrap(),
                serde_json::from_value(json!({"id": "other", "type": "section", "attributes": {"id": "contact"}})).unwrap(),
            ],
            styles: Vec::new(), assets: Vec::new(), provider_requirements: Vec::new(), extensions: Map::new(),
        };
        let mut to_remap = fragment.clone();
        let mut generator = crate::SequentialIdGenerator::new("copy");
        let original_generator = generator.clone();
        assert!(to_remap.remap_ids(&mut generator).is_err());
        assert_eq!(to_remap, fragment);
        assert_eq!(generator, original_generator);
        let error = fragment
            .insert(&mut editor, Some("root".into()), 0)
            .expect_err("ambiguous form identity");
        assert!(error.to_string().contains("contact"), "{error}");
        assert_eq!(editor, before);
    }

    #[test]
    fn pasting_selector_only_rules_keeps_both_original_and_copy_styled() {
        let source = GrapesJsCodec::decode_value(json!({
            "pages": [{"component": {"id": "root", "type": "wrapper", "components": [
                {"id": "hero", "type": "section"}
            ]}}],
            "styles": [
                {"id": "base-rule", "selectors": [{"name": "hero", "type": 2}, {"name": "hero", "type": 1}],
                 "style": {"color": "red"}, "providerData": {"component_id": "hero"}},
                {"flyRuleId": "hover-rule", "selectors": "#hero", "state": "hover", "style": {"color": "blue"}}
            ]
        })).unwrap();
        let fragment = ProjectFragment::from_component(&source, "hero").unwrap();
        let mut editor = FlyEditor::new(source, RegistrySet::with_builtins());
        let inserted = fragment
            .insert(&mut editor, Some("root".into()), 1)
            .expect("paste");
        let copy = &inserted[0];
        let catalog = StyleRuleCatalog::from_document(editor.document());
        let original = catalog
            .component_rule("hero", &StyleRuleScope::Base)
            .unwrap();
        let copied = catalog.component_rule(copy, &StyleRuleScope::Base).unwrap();
        assert_ne!(original.id, copied.id);
        assert_eq!(copied.raw["selectors"][0]["name"], *copy);
        assert_eq!(copied.raw["selectors"][1]["name"], "hero"); // a class, not an ID
        assert_eq!(copied.raw["providerData"]["component_id"], "hero");
        let hover = StyleRuleScope::State {
            pseudo: "hover".into(),
        };
        assert!(catalog.component_rule("hero", &hover).is_some());
        assert_eq!(
            catalog.component_rule(copy, &hover).unwrap().raw["selectors"],
            format!("#{copy}")
        );
        let rendered = crate::render_page(
            editor.document(),
            &crate::PageSelection::First,
            &crate::RenderPolicy::default(),
        )
        .unwrap();
        assert!(
            rendered
                .css
                .contains("[data-fly-style-id=\"hero\"]{color:red}"),
            "{}",
            rendered.css
        );
        assert!(
            rendered
                .css
                .contains(&format!("[data-fly-style-id=\"{copy}\"]{{color:red}}")),
            "{}",
            rendered.css
        );
    }

    #[test]
    fn pasting_asset_id_collisions_does_not_replace_destination_assets() {
        let source = GrapesJsCodec::decode_value(json!({
            "assets": [{ "id": "shared", "src": "/source.webp", "name": "source" }],
            "pages": [{
                "component": {
                    "id": "source-root",
                    "type": "wrapper",
                    "components": [{
                        "id": "hero",
                        "type": "image",
                        "attributes": {
                            "src": "/source.webp",
                            "data-fly-asset-id": "shared"
                        }
                    }]
                }
            }]
        }))
        .expect("source");
        let target = GrapesJsCodec::decode_value(json!({
            "assets": [{ "id": "shared", "src": "/target.webp", "name": "target" }],
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
            .expect("paste");

        let catalog = AssetDescriptor::from_value;
        let assets = editor.document().project().assets.iter()
            .filter_map(|raw| catalog(raw.clone()))
            .collect::<Vec<_>>();
        assert_eq!(
            assets.iter().find(|asset| asset.id == "shared").map(|asset| asset.source.as_str()),
            Some("/target.webp")
        );
        let inserted_component = editor.document().component(&inserted[0]).expect("inserted");
        let pasted_asset_id = inserted_component.attributes["data-fly-asset-id"]
            .as_str()
            .expect("asset id");
        assert_ne!(pasted_asset_id, "shared");
        assert_eq!(
            assets.iter().find(|asset| asset.id == pasted_asset_id).map(|asset| asset.source.as_str()),
            Some("/source.webp")
        );
        assert_eq!(
            inserted_component.attributes["src"],
            "/source.webp"
        );
    }

    #[test]
    fn pasted_assets_do_not_claim_dangling_asset_ids_used_by_the_target() {
        let source = GrapesJsCodec::decode_value(json!({
            "assets": [{ "id": "shared", "src": "/source.webp" }],
            "pages": [{
                "component": {
                    "id": "source-root",
                    "type": "wrapper",
                    "components": [{
                        "id": "hero",
                        "type": "image",
                        "attributes": {
                            "src": "/source.webp",
                            "data-fly-asset-id": "shared"
                        }
                    }]
                }
            }]
        }))
        .expect("source");
        let target = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "component": {
                    "id": "target-root",
                    "type": "wrapper",
                    "components": [{
                        "id": "existing",
                        "type": "section",
                        "attributes": { "data-fly-asset-id": "shared" }
                    }]
                }
            }]
        }))
        .expect("target");
        let fragment = ProjectFragment::from_component(&source, "hero").expect("fragment");
        let mut editor = FlyEditor::new(target, RegistrySet::with_builtins());

        let inserted = fragment
            .insert(&mut editor, Some("target-root".to_string()), 1)
            .expect("paste");

        assert_eq!(
            editor
                .document()
                .component("existing")
                .expect("existing component")
                .attributes["data-fly-asset-id"],
            "shared"
        );
        let pasted = editor.document().component(&inserted[0]).expect("pasted");
        let pasted_asset_id = pasted.attributes["data-fly-asset-id"]
            .as_str()
            .expect("pasted asset id");
        assert_ne!(pasted_asset_id, "shared");
        assert!(
            editor
                .document()
                .project()
                .assets
                .iter()
                .filter_map(|raw| AssetDescriptor::from_value(raw.clone()))
                .any(|asset| asset.id == pasted_asset_id && asset.source == "/source.webp")
        );
    }

    #[test]
    fn fragment_insert_rejects_index_overflow_without_mutating_the_editor() {
        let target = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "component": {
                    "id": "target-root",
                    "type": "wrapper",
                    "components": []
                }
            }]
        }))
        .expect("target");
        let mut editor = FlyEditor::new(target, RegistrySet::with_builtins());
        let before = editor.clone();
        let fragment = ProjectFragment {
            components: vec![
                serde_json::from_value(json!({"id": "first", "type": "section"}))
                    .expect("first component"),
                serde_json::from_value(json!({"id": "second", "type": "section"}))
                    .expect("second component"),
            ],
            styles: Vec::new(),
            assets: Vec::new(),
            provider_requirements: Vec::new(),
            extensions: Map::new(),
        };

        let error = fragment
            .insert(&mut editor, Some("target-root".to_string()), usize::MAX)
            .expect_err("overflowing fragment index");

        assert!(matches!(error, FlyError::Decode(message) if message.contains("index overflow")));
        assert_eq!(editor, before);
    }

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

        assert_eq!(inserted.len(), 1);
        let inserted_id = inserted[0].clone();
        assert!(
            inserted_id.starts_with("fly-paste-"),
            "editor-minted ids come from the unique generator: {inserted_id}"
        );
        assert!(editor.document().component(&inserted_id).is_some());
        assert!(editor.document().project().assets.iter().any(|asset| {
            AssetDescriptor::from_value(asset.clone()).is_some_and(|asset| asset.id == "hero-asset")
        }));
        assert!(
            StyleRuleCatalog::from_document(editor.document())
                .component_rule(&inserted_id, &StyleRuleScope::Base)
                .is_some()
        );
    }
}
