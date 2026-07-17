use crate::{ComponentNode, FlyError, FlyResult};
use serde::{Deserialize, Serialize};
use serde_json::json;
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
        "wrapper",
        "section",
        "container",
        "row",
        "column",
        "grid",
        "text",
        "heading",
        "list",
        "list_item",
        "link",
        "image",
        "video",
        "media",
        "button",
        "divider",
        "spacer",
        "form",
        "label",
        "input",
        "textarea",
        "select",
        "option",
        "checkbox",
        "submit",
        "raw_html",
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
pub struct StyleDefinition {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub applies_to: Vec<String>,
}

impl RegistryItem for StyleDefinition {
    fn registry_id(&self) -> &str {
        &self.id
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SelectorDefinition {
    pub id: String,
    pub selector: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

impl RegistryItem for SelectorDefinition {
    fn registry_id(&self) -> &str {
        &self.id
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssetProviderDefinition {
    pub id: String,
    #[serde(default)]
    pub supported_kinds: Vec<String>,
}

impl RegistryItem for AssetProviderDefinition {
    fn registry_id(&self) -> &str {
        &self.id
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommandDefinition {
    pub id: String,
    pub label: String,
    pub mutates_project: bool,
}

impl RegistryItem for CommandDefinition {
    fn registry_id(&self) -> &str {
        &self.id
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PluginRequirement {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PluginDescriptor {
    pub id: String,
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
    pub styles: Registry<StyleDefinition>,
    pub selectors: Registry<SelectorDefinition>,
    pub asset_providers: Registry<AssetProviderDefinition>,
    pub commands: Registry<CommandDefinition>,
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
        for command in builtin_commands() {
            registries
                .commands
                .register(command)
                .expect("built-in command ids are valid and unique");
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
        .map(|id| {
            let allowed_children = match id {
                "list" => vec!["list_item".to_string()],
                "select" => vec!["option".to_string()],
                _ => Vec::new(),
            };
            let accepts_any_child = matches!(
                id,
                "wrapper" | "section" | "container" | "row" | "column" | "grid" | "media" | "form"
            );
            ComponentDefinition {
                id: id.to_string(),
                provider: "fly.builtin".to_string(),
                allowed_children,
                accepts_any_child,
                is_container: accepts_any_child || matches!(id, "list" | "select"),
            }
        })
        .collect()
}

pub fn builtin_blocks() -> Vec<BlockDefinition> {
    let mut blocks = builtin_component_ids()
        .into_iter()
        .filter(|id| !matches!(*id, "wrapper" | "list_item" | "label" | "option"))
        .map(|id| BlockDefinition {
            id: id.to_string(),
            label: humanize_id(id),
            category: builtin_category(id).to_string(),
            component: builtin_component_template(id),
        })
        .collect::<Vec<_>>();
    blocks.extend(landing_templates());
    blocks
}

pub fn builtin_commands() -> Vec<CommandDefinition> {
    [
        ("fly.insert", "Insert component", true),
        ("fly.remove", "Remove component", true),
        ("fly.move", "Move component", true),
        ("fly.patch", "Update component", true),
        ("fly.select", "Select component", false),
        ("fly.undo", "Undo", true),
        ("fly.redo", "Redo", true),
        ("fly.copy", "Copy component", false),
        ("fly.cut", "Cut component", true),
        ("fly.paste", "Paste component", true),
    ]
    .into_iter()
    .map(|(id, label, mutates_project)| CommandDefinition {
        id: id.to_string(),
        label: label.to_string(),
        mutates_project,
    })
    .collect()
}

fn builtin_component_ids() -> Vec<&'static str> {
    vec![
        "wrapper",
        "section",
        "container",
        "row",
        "column",
        "grid",
        "text",
        "heading",
        "list",
        "list_item",
        "link",
        "image",
        "video",
        "media",
        "button",
        "divider",
        "spacer",
        "form",
        "label",
        "input",
        "textarea",
        "select",
        "option",
        "checkbox",
        "submit",
        "raw_html",
    ]
}

fn builtin_component_template(id: &str) -> ComponentNode {
    let value = match id {
        "section" => json!({
            "type": "section",
            "style": { "padding": "64px 24px", "min-height": "160px" },
            "components": []
        }),
        "container" => json!({
            "type": "container",
            "style": { "max-width": "1120px", "margin": "0 auto", "padding": "0 20px" },
            "components": []
        }),
        "row" => json!({
            "type": "row",
            "style": { "display": "flex", "gap": "24px", "align-items": "stretch", "flex-wrap": "wrap" },
            "components": []
        }),
        "column" => json!({
            "type": "column",
            "style": { "flex": "1 1 280px", "min-width": "0" },
            "components": []
        }),
        "grid" => json!({
            "type": "grid",
            "style": { "display": "grid", "grid-template-columns": "repeat(auto-fit,minmax(220px,1fr))", "gap": "24px" },
            "components": []
        }),
        "heading" => json!({
            "type": "heading",
            "tagName": "h2",
            "content": "A clear, compelling headline",
            "style": { "font-size": "40px", "line-height": "1.1", "margin": "0 0 16px" }
        }),
        "text" => json!({
            "type": "text",
            "content": "Add concise supporting copy for this section.",
            "style": { "font-size": "18px", "line-height": "1.6", "margin": "0 0 20px" }
        }),
        "list" => json!({
            "type": "list",
            "tagName": "ul",
            "components": [
                { "type": "list_item", "tagName": "li", "content": "First benefit" },
                { "type": "list_item", "tagName": "li", "content": "Second benefit" },
                { "type": "list_item", "tagName": "li", "content": "Third benefit" }
            ]
        }),
        "link" => json!({
            "type": "link",
            "tagName": "a",
            "attributes": { "href": "#" },
            "content": "Learn more"
        }),
        "image" => json!({
            "type": "image",
            "tagName": "img",
            "attributes": {
                "src": "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='960' height='540'%3E%3Crect width='100%25' height='100%25' fill='%23e2e8f0'/%3E%3Ctext x='50%25' y='50%25' text-anchor='middle' dominant-baseline='middle' fill='%23475569' font-family='sans-serif' font-size='32'%3EImage%3C/text%3E%3C/svg%3E",
                "alt": "Placeholder image"
            },
            "style": { "display": "block", "width": "100%", "height": "auto", "border-radius": "16px" }
        }),
        "video" => json!({
            "type": "video",
            "tagName": "video",
            "attributes": { "controls": true },
            "style": { "display": "block", "width": "100%", "min-height": "240px", "background": "#0f172a", "border-radius": "16px" }
        }),
        "media" => json!({
            "type": "media",
            "tagName": "figure",
            "style": { "margin": "0" },
            "components": [
                builtin_component_template("image"),
                { "type": "text", "tagName": "figcaption", "content": "Media caption", "style": { "margin-top": "8px", "color": "#64748b" } }
            ]
        }),
        "button" => json!({
            "type": "button",
            "tagName": "a",
            "attributes": { "href": "#" },
            "content": "Get started",
            "style": { "display": "inline-block", "padding": "12px 20px", "border-radius": "10px", "background": "#2563eb", "color": "#ffffff", "text-decoration": "none", "font-weight": "600" }
        }),
        "divider" => json!({
            "type": "divider",
            "tagName": "hr",
            "style": { "border": "0", "border-top": "1px solid #cbd5e1", "margin": "32px 0" }
        }),
        "spacer" => json!({
            "type": "spacer",
            "style": { "height": "48px" }
        }),
        "form" => json!({
            "type": "form",
            "tagName": "form",
            "style": { "display": "grid", "gap": "16px" },
            "components": []
        }),
        "input" => json!({
            "type": "input",
            "tagName": "input",
            "attributes": { "type": "text", "placeholder": "Your name" },
            "style": { "width": "100%", "padding": "12px", "border": "1px solid #cbd5e1", "border-radius": "8px" }
        }),
        "textarea" => json!({
            "type": "textarea",
            "tagName": "textarea",
            "attributes": { "placeholder": "Your message", "rows": 5 },
            "style": { "width": "100%", "padding": "12px", "border": "1px solid #cbd5e1", "border-radius": "8px" }
        }),
        "select" => json!({
            "type": "select",
            "tagName": "select",
            "style": { "width": "100%", "padding": "12px", "border": "1px solid #cbd5e1", "border-radius": "8px" },
            "components": [
                { "type": "option", "tagName": "option", "attributes": { "value": "" }, "content": "Choose an option" }
            ]
        }),
        "checkbox" => json!({
            "type": "checkbox",
            "tagName": "input",
            "attributes": { "type": "checkbox" }
        }),
        "submit" => json!({
            "type": "submit",
            "tagName": "button",
            "attributes": { "type": "submit" },
            "content": "Submit",
            "style": { "padding": "12px 20px", "border": "0", "border-radius": "8px", "background": "#2563eb", "color": "#ffffff", "font-weight": "600" }
        }),
        "raw_html" => json!({
            "type": "raw_html",
            "content": "Restricted raw HTML placeholder"
        }),
        _ => json!({ "type": id }),
    };
    serde_json::from_value(value).expect("built-in block templates use valid component JSON")
}

fn landing_templates() -> Vec<BlockDefinition> {
    [
        (
            "fly.hero",
            "Hero section",
            "landing",
            json!({
                "type": "section",
                "flyLandingSection": "hero",
                "style": { "padding": "96px 24px", "background": "#f8fafc" },
                "components": [{
                    "type": "container",
                    "style": { "max-width": "960px", "margin": "0 auto", "text-align": "center" },
                    "components": [
                        { "type": "heading", "tagName": "h1", "content": "Build your next idea with confidence", "style": { "font-size": "56px", "line-height": "1.05", "margin": "0 0 20px" } },
                        { "type": "text", "content": "A stable Rust-powered landing page that is easy to compose and evolve.", "style": { "font-size": "20px", "line-height": "1.6", "margin": "0 auto 28px", "max-width": "720px", "color": "#475569" } },
                        builtin_component_template("button")
                    ]
                }]
            }),
        ),
        (
            "fly.two_columns",
            "Two columns",
            "landing",
            json!({
                "type": "section",
                "flyLandingSection": "two_columns",
                "style": { "padding": "72px 24px" },
                "components": [{
                    "type": "container",
                    "style": { "max-width": "1120px", "margin": "0 auto" },
                    "components": [{
                        "type": "row",
                        "style": { "display": "flex", "gap": "48px", "align-items": "center", "flex-wrap": "wrap" },
                        "components": [
                            { "type": "column", "style": { "flex": "1 1 420px" }, "components": [builtin_component_template("heading"), builtin_component_template("text"), builtin_component_template("button")] },
                            { "type": "column", "style": { "flex": "1 1 420px" }, "components": [builtin_component_template("image")] }
                        ]
                    }]
                }]
            }),
        ),
        (
            "fly.feature_grid",
            "Feature grid",
            "landing",
            json!({
                "type": "section",
                "flyLandingSection": "feature_grid",
                "style": { "padding": "72px 24px", "background": "#f8fafc" },
                "components": [{
                    "type": "container",
                    "style": { "max-width": "1120px", "margin": "0 auto" },
                    "components": [
                        { "type": "heading", "content": "Everything you need", "style": { "text-align": "center", "font-size": "40px", "margin": "0 0 40px" } },
                        { "type": "grid", "style": { "display": "grid", "grid-template-columns": "repeat(auto-fit,minmax(220px,1fr))", "gap": "24px" }, "components": [
                            feature_card("Fast composition", "Build pages from stable reusable components."),
                            feature_card("Safe persistence", "Keep project data canonical and lossless."),
                            feature_card("Rust runtime", "Share editor behavior across framework adapters.")
                        ] }
                    ]
                }]
            }),
        ),
        (
            "fly.cta",
            "Call to action",
            "landing",
            json!({
                "type": "section",
                "flyLandingSection": "call_to_action",
                "style": { "padding": "72px 24px" },
                "components": [{
                    "type": "container",
                    "style": { "max-width": "960px", "margin": "0 auto", "padding": "48px", "border-radius": "20px", "background": "#0f172a", "color": "#ffffff", "text-align": "center" },
                    "components": [
                        { "type": "heading", "content": "Ready to launch?", "style": { "font-size": "40px", "margin": "0 0 16px" } },
                        { "type": "text", "content": "Turn this page into your own production-ready landing experience.", "style": { "font-size": "18px", "margin": "0 0 24px", "color": "#cbd5e1" } },
                        { "type": "button", "tagName": "a", "attributes": { "href": "#" }, "content": "Start now", "style": { "display": "inline-block", "padding": "12px 20px", "border-radius": "10px", "background": "#ffffff", "color": "#0f172a", "text-decoration": "none", "font-weight": "700" } }
                    ]
                }]
            }),
        ),
        (
            "fly.contact_form",
            "Contact form",
            "landing",
            json!({
                "type": "section",
                "flyLandingSection": "contact_form",
                "style": { "padding": "72px 24px" },
                "components": [{
                    "type": "container",
                    "style": { "max-width": "720px", "margin": "0 auto" },
                    "components": [
                        { "type": "heading", "content": "Talk to us", "style": { "font-size": "40px", "margin": "0 0 12px" } },
                        { "type": "text", "content": "Tell us what you are building and we will get back to you.", "style": { "color": "#475569", "margin": "0 0 28px" } },
                        { "type": "form", "tagName": "form", "style": { "display": "grid", "gap": "16px" }, "components": [
                            { "type": "input", "tagName": "input", "attributes": { "type": "text", "placeholder": "Your name" }, "style": { "padding": "12px", "border": "1px solid #cbd5e1", "border-radius": "8px" } },
                            { "type": "input", "tagName": "input", "attributes": { "type": "email", "placeholder": "Email address" }, "style": { "padding": "12px", "border": "1px solid #cbd5e1", "border-radius": "8px" } },
                            { "type": "textarea", "tagName": "textarea", "attributes": { "placeholder": "Your message", "rows": 5 }, "style": { "padding": "12px", "border": "1px solid #cbd5e1", "border-radius": "8px" } },
                            builtin_component_template("submit")
                        ] }
                    ]
                }]
            }),
        ),
    ]
    .into_iter()
    .map(|(id, label, category, component)| BlockDefinition {
        id: id.to_string(),
        label: label.to_string(),
        category: category.to_string(),
        component: serde_json::from_value(component)
            .expect("landing templates use valid component JSON"),
    })
    .collect()
}

fn feature_card(title: &str, body: &str) -> serde_json::Value {
    json!({
        "type": "column",
        "style": { "padding": "24px", "border": "1px solid #e2e8f0", "border-radius": "16px", "background": "#ffffff" },
        "components": [
            { "type": "heading", "tagName": "h3", "content": title, "style": { "font-size": "22px", "margin": "0 0 10px" } },
            { "type": "text", "content": body, "style": { "margin": "0", "color": "#475569" } }
        ]
    })
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
        "section" | "container" | "row" | "column" | "grid" | "divider" | "spacer" => "layout",
        "image" | "video" | "media" => "media",
        "form" | "input" | "textarea" | "select" | "checkbox" | "submit" => "forms",
        "raw_html" => "advanced",
        _ => "content",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn landing_templates_have_visible_nested_content() {
        let blocks = builtin_blocks();
        let hero = blocks
            .iter()
            .find(|block| block.id == "fly.hero")
            .expect("hero block");
        let root = hero.component.as_object().expect("hero root");
        assert_eq!(root.component_type(), "section");
        assert!(!root.children().is_empty());
    }

    #[test]
    fn form_and_list_children_are_explicitly_allowed() {
        let registries = RegistrySet::with_builtins();
        assert!(registries.accepts_child_type(Some("list"), "list_item"));
        assert!(registries.accepts_child_type(Some("select"), "option"));
        assert!(registries.accepts_child_type(Some("form"), "input"));
    }
}
