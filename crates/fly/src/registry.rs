use crate::{ComponentNode, FlyError, FlyResult};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub trait RegistryItem {
    fn registry_id(&self) -> &str;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Registry<T> {
    items: BTreeMap<String, T>,
}

impl<T> Default for Registry<T> {
    fn default() -> Self {
        Self {
            items: BTreeMap::new(),
        }
    }
}

impl<T: RegistryItem> Registry<T> {
    pub fn register(&mut self, item: T) -> FlyResult<()> {
        let id = item.registry_id().to_string();
        validate_registry_id(&id)?;
        if self.items.contains_key(&id) {
            return Err(FlyError::DuplicateRegistryItem(id));
        }
        self.items.insert(id, item);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&T> {
        self.items.get(id)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.items.contains_key(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &T)> {
        self.items.iter().map(|(id, item)| (id.as_str(), item))
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

fn validate_registry_id(id: &str) -> FlyResult<()> {
    const BUILT_INS: &[&str] = &[
        "wrapper", "section", "container", "row", "column", "grid", "text", "heading",
        "list", "link", "image", "video", "media", "button", "divider", "spacer", "form",
        "input", "textarea", "select", "checkbox", "submit", "raw_html",
    ];
    if id.is_empty() || (!id.contains('.') && !BUILT_INS.contains(&id)) {
        return Err(FlyError::InvalidRegistryId(id.to_string()));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComponentDefinition {
    pub id: String,
    pub provider: String,
    pub schema_version: String,
    #[serde(default)]
    pub allowed_children: Vec<String>,
    #[serde(default)]
    pub accepts_any_child: bool,
    #[serde(default)]
    pub is_container: bool,
}

impl RegistryItem for ComponentDefinition {
    fn registry_id(&self) -> &str {
        &self.id
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BlockDefinition {
    pub id: String,
    pub label: String,
    pub category: String,
    pub component: ComponentNode,
}

impl RegistryItem for BlockDefinition {
    fn registry_id(&self) -> &str {
        &self.id
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TraitDefinition {
    pub id: String,
    pub label: String,
    pub value_type: String,
    #[serde(default)]
    pub required: bool,
}

impl RegistryItem for TraitDefinition {
    fn registry_id(&self) -> &str {
        &self.id
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PluginRequirement {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PluginDescriptor {
    pub id: String,
    pub version: String,
    #[serde(default)]
    pub dependencies: Vec<PluginRequirement>,
}

impl RegistryItem for PluginDescriptor {
    fn registry_id(&self) -> &str {
        &self.id
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct RegistrySet {
    pub components: Registry<ComponentDefinition>,
    pub blocks: Registry<BlockDefinition>,
    pub traits: Registry<TraitDefinition>,
    pub plugins: Registry<PluginDescriptor>,
}

impl RegistrySet {
    pub fn with_builtins() -> Self {
        let mut registries = Self::default();
        for definition in builtin_component_definitions() {
            registries
                .components
                .register(definition)
                .expect("built-in component ids are valid and unique");
        }
        for block in builtin_blocks() {
            registries
                .blocks
                .register(block)
                .expect("built-in block ids are valid and unique");
        }
        registries
    }

    pub fn validate_plugin_dependencies(&self) -> FlyResult<()> {
        for (id, plugin) in self.plugins.iter() {
            for dependency in &plugin.dependencies {
                if !self.plugins.contains(&dependency.id) {
                    return Err(FlyError::MissingPluginDependency {
                        plugin: id.to_string(),
                        dependency: dependency.id.clone(),
                    });
                }
            }
        }

        let mut visiting = BTreeSet::new();
        let mut visited = BTreeSet::new();
        for (id, _) in self.plugins.iter() {
            validate_plugin_node(id, &self.plugins, &mut visiting, &mut visited)?;
        }
        Ok(())
    }
}

fn validate_plugin_node(
    id: &str,
    plugins: &Registry<PluginDescriptor>,
    visiting: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
) -> FlyResult<()> {
    if visited.contains(id) {
        return Ok(());
    }
    if !visiting.insert(id.to_string()) {
        return Err(FlyError::PluginDependencyCycle(id.to_string()));
    }
    if let Some(plugin) = plugins.get(id) {
        for dependency in &plugin.dependencies {
            validate_plugin_node(&dependency.id, plugins, visiting, visited)?;
        }
    }
    visiting.remove(id);
    visited.insert(id.to_string());
    Ok(())
}

pub fn builtin_component_definitions() -> Vec<ComponentDefinition> {
    builtin_component_ids()
        .into_iter()
        .map(|id| ComponentDefinition {
            id: id.to_string(),
            provider: "fly.builtin".to_string(),
            schema_version: "1".to_string(),
            allowed_children: Vec::new(),
            accepts_any_child: matches!(
                id,
                "wrapper" | "section" | "container" | "row" | "column" | "grid" | "form"
            ),
            is_container: matches!(
                id,
                "wrapper" | "section" | "container" | "row" | "column" | "grid" | "form"
            ),
        })
        .collect()
}

pub fn builtin_blocks() -> Vec<BlockDefinition> {
    builtin_component_ids()
        .into_iter()
        .filter(|id| *id != "wrapper")
        .map(|id| BlockDefinition {
            id: id.to_string(),
            label: humanize_id(id),
            category: builtin_category(id).to_string(),
            component: ComponentNode::object(id),
        })
        .collect()
}

fn builtin_component_ids() -> Vec<&'static str> {
    vec![
        "wrapper", "section", "container", "row", "column", "grid", "text", "heading",
        "list", "link", "image", "video", "media", "button", "divider", "spacer", "form",
        "input", "textarea", "select", "checkbox", "submit", "raw_html",
    ]
}

fn humanize_id(id: &str) -> String {
    let mut characters = id.replace('_', " ").chars().collect::<Vec<_>>();
    if let Some(first) = characters.first_mut() {
        first.make_ascii_uppercase();
    }
    characters.into_iter().collect()
}

fn builtin_category(id: &str) -> &'static str {
    match id {
        "section" | "container" | "row" | "column" | "grid" | "divider" | "spacer" => {
            "layout"
        }
        "image" | "video" | "media" => "media",
        "form" | "input" | "textarea" | "select" | "checkbox" | "submit" => "forms",
        "raw_html" => "advanced",
        _ => "content",
    }
}
