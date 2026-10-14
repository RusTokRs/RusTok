//! Shared editor view-models for framework adapters.
//!
//! Adapter crates (`fly-leptos`, `fly-dioxus`, and host editors) all need the same flattened
//! layer tree and palette listing. That derivation is pure data work, so it lives here —
//! verified without a UI runtime — and adapters only render it and forward intents.

use crate::PaletteBlockAccess;
use fly::{ComponentNode, ProjectDocument, RegistrySet};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EditorLayerItem {
    pub id: String,
    pub component_type: String,
    pub label: String,
    pub depth: usize,
    pub child_count: usize,
    pub opaque: bool,
    /// Parent component id, for hosts that render move/reparent affordances per row.
    pub parent_id: Option<String>,
    /// Index within the parent's child list (0 for page roots).
    pub index: usize,
}

/// Flatten every page tree into layer rows in document order. Components without an id are
/// skipped: they cannot be selected or addressed by commands, and `FlyEditor` assigns ids
/// before mutation anyway.
pub fn editor_layer_items(document: &ProjectDocument) -> Vec<EditorLayerItem> {
    let mut items = Vec::new();
    for page in &document.project.pages {
        if let Some(root) = page.component.as_ref() {
            collect_layer_items(root, 0, None, 0, &mut items);
        }
    }
    items
}

/// Which facet of a component a property edit targets. Mirrors the
/// `patch_component_property` browser-intent contract so adapters, hosts, and
/// the SSR inspector forms all speak one shape.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ComponentPropertyKind {
    Field,
    Attribute,
    Style,
}

impl ComponentPropertyKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Field => "field",
            Self::Attribute => "attribute",
            Self::Style => "style",
        }
    }
}

/// One property edit against the selected component: set (`remove = false`)
/// or clear (`remove = true`) one field, attribute, or style property.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentPropertyChange {
    pub component_id: String,
    pub kind: ComponentPropertyKind,
    pub name: String,
    pub value: String,
    pub remove: bool,
}

/// Presentation model of the selected component for property panels: identity,
/// tag, text content, attributes, and inline style — everything the property
/// forms edit, derived from the document without a UI runtime.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentPropertyView {
    pub id: String,
    pub component_type: String,
    pub tag_name: Option<String>,
    pub content: Option<String>,
    pub attributes: Vec<(String, Value)>,
    pub style: Map<String, Value>,
}

/// Derives the [`ComponentPropertyView`] for `component_id`, or `None` when the
/// document has no addressable component with that id.
pub fn component_property_view(
    document: &ProjectDocument,
    component_id: &str,
) -> Option<ComponentPropertyView> {
    let component = document.component(component_id)?;
    let content = component
        .extensions
        .get("content")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let style = component
        .style
        .as_ref()
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    Some(ComponentPropertyView {
        id: component_id.to_string(),
        component_type: component.component_type().to_string(),
        tag_name: component.tag_name.clone(),
        content,
        attributes: component
            .attributes
            .iter()
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect(),
        style,
    })
}

/// One scoped style edit against the selected component: a CSS declaration
/// under a media-query scope or a pseudo-class state scope — the responsive /
/// interaction-state counterpart of [`ComponentPropertyChange`].
#[derive(Debug, Clone, PartialEq)]
pub struct ScopedStyleChange {
    pub component_id: String,
    pub scope: fly::StyleRuleScope,
    pub property: String,
    pub value: String,
    pub remove: bool,
}

/// Current value of one declaration of a component's scoped rule, for seeding
/// style forms: catalog in, scalar string out (empty when unset).
pub fn scoped_style_value(
    catalog: &fly::StyleRuleCatalog,
    component_id: &str,
    scope: &fly::StyleRuleScope,
    property: &str,
) -> String {
    catalog
        .component_rule(component_id, scope)
        .and_then(|rule| rule.declarations.get(property))
        .and_then(|value| match value {
            Value::String(value) => Some(value.clone()),
            Value::Number(value) => Some(value.to_string()),
            Value::Bool(value) => Some(value.to_string()),
            _ => None,
        })
        .unwrap_or_default()
}

/// Resolves a responsive breakpoint id to its preview viewport preset, so
/// hosts can switch their preview surface to the breakpoint's device width.
pub fn preview_viewport_for_breakpoint(
    breakpoint_id: &str,
    current: crate::ViewportState,
) -> Option<crate::ViewportState> {
    let breakpoint = crate::responsive_breakpoint(breakpoint_id)?;
    crate::viewport_preset(&breakpoint.preview_preset_id).map(|preset| preset.apply(current))
}

/// Layer rows for one page by index — the shape single-page editors (including the
/// RusTok page-builder admin) render. Only addressable components appear: opaque
/// payloads have no id to select, so they are omitted rather than shown as dead rows.
pub fn editor_layer_items_for_page(
    document: &ProjectDocument,
    page_index: usize,
) -> Vec<EditorLayerItem> {
    let mut items = Vec::new();
    if let Some(root) = document
        .project
        .pages
        .get(page_index)
        .and_then(|page| page.component.as_ref())
    {
        collect_layer_items(root, 0, None, 0, &mut items);
    }
    items.retain(|item| !item.opaque && !item.id.is_empty());
    items
}

fn collect_layer_items(
    node: &ComponentNode,
    depth: usize,
    parent_id: Option<String>,
    index: usize,
    items: &mut Vec<EditorLayerItem>,
) {
    match node {
        ComponentNode::Object(component) => {
            let children = component.children();
            let own_id = component.id().map(ToString::to_string);
            if let Some(id) = own_id.clone() {
                items.push(EditorLayerItem {
                    id: id.clone(),
                    component_type: component.component_type().to_string(),
                    label: layer_label(component.component_type(), &id),
                    depth,
                    child_count: children.len(),
                    opaque: false,
                    parent_id: parent_id.clone(),
                    index,
                });
            }
            for (child_index, child) in children.iter().enumerate() {
                collect_layer_items(child, depth + 1, own_id.clone(), child_index, items);
            }
        }
        ComponentNode::Opaque(_) => {
            items.push(EditorLayerItem {
                id: String::new(),
                component_type: "opaque".to_string(),
                label: "Opaque component (preserved)".to_string(),
                depth,
                child_count: 0,
                opaque: true,
                parent_id,
                index,
            });
        }
    }
}

fn layer_label(component_type: &str, id: &str) -> String {
    format!("{} · {}", component_type, id)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaletteBlockItem {
    pub id: String,
    pub label: String,
    pub category: String,
    /// Contributions that declared this block in the active assembly, in registry
    /// order. Empty for built-in primitives and on unrestricted surfaces.
    #[serde(default)]
    pub contribution_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PaletteBlock {
    pub id: String,
    pub label: String,
    pub category: String,
    /// The component inserted when the block is chosen (drag source or click-insert).
    pub component: fly::ComponentNode,
}

/// Palette listing with insert templates, for hosts that insert on selection (the
/// page-builder admin controller). Adapters that only notify the host use
/// [`palette_block_items`] instead.
pub fn palette_blocks(registries: &RegistrySet) -> Vec<PaletteBlock> {
    registries
        .blocks
        .iter()
        .map(|(_, block)| PaletteBlock {
            id: block.id.clone(),
            label: block.label.clone(),
            category: block.category.clone(),
            component: block.component.clone(),
        })
        .collect()
}

/// One palette block by id, with its insert template.
pub fn palette_block(registries: &RegistrySet, block_id: &str) -> Option<PaletteBlock> {
    registries.blocks.get(block_id).map(|block| PaletteBlock {
        id: block.id.clone(),
        label: block.label.clone(),
        category: block.category.clone(),
        component: block.component.clone(),
    })
}

/// Palette listing in registry order, minus blocks whose template cannot be decoded
/// (registries are host-extensible; a broken provider block must not break the palette).
pub fn palette_block_items(registries: &RegistrySet) -> Vec<PaletteBlockItem> {
    palette_block_items_with_access(registries, &PaletteBlockAccess::unrestricted())
}

/// Palette listing restricted by contribution access: namespaced blocks appear only
/// when an active contribution declares them, and each item carries its contribution
/// provenance. This is the listing adapter workspaces render when the host assembles
/// provider contributions (audit P0-1: contribution access in the workspaces).
pub fn palette_block_items_with_access(
    registries: &RegistrySet,
    access: &PaletteBlockAccess,
) -> Vec<PaletteBlockItem> {
    registries
        .blocks
        .iter()
        .filter(|(_, block)| access.allows(&block.id))
        .map(|(_, block)| PaletteBlockItem {
            id: block.id.clone(),
            label: block.label.clone(),
            category: block.category.clone(),
            contribution_ids: access
                .contribution_ids(&block.id)
                .map(ToString::to_string)
                .collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use fly::GrapesJsCodec;
    use serde_json::json;

    #[test]
    fn component_property_view_reads_the_selected_component() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": {
                    "id": "hero",
                    "type": "section",
                    "tagName": "section",
                    "content": "Welcome",
                    "attributes": { "id": "top", "data-kind": "hero" },
                    "style": { "color": "red" }
                }
            }]
        }))
        .expect("document");
        let view = component_property_view(&document, "hero").expect("view");
        assert_eq!(view.component_type, "section");
        assert_eq!(view.tag_name.as_deref(), Some("section"));
        assert_eq!(view.content.as_deref(), Some("Welcome"));
        assert_eq!(view.attributes.len(), 2);
        assert_eq!(view.style.get("color"), Some(&json!("red")));
        assert!(component_property_view(&document, "missing").is_none());
        assert_eq!(ComponentPropertyKind::Style.as_str(), "style");
    }

    #[test]
    fn layer_items_flatten_pages_in_document_order() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [
                        { "id": "hero", "type": "section", "components": [
                            { "id": "title", "type": "heading", "content": "Hi" }
                        ]},
                        { "id": "cta", "type": "button", "content": "Go" }
                    ]
                }
            }]
        }))
        .expect("document");
        let items = editor_layer_items(&document);
        let ids: Vec<&str> = items.iter().map(|item| item.id.as_str()).collect();
        assert_eq!(ids, ["root", "hero", "title", "cta"]);
        assert_eq!(items[1].depth, 1);
        assert_eq!(items[1].child_count, 1);
        assert_eq!(items[2].depth, 2);
    }

    #[test]
    fn page_layer_items_carry_parent_and_index() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [
                        { "id": "a", "type": "section" },
                        { "id": "b", "type": "section", "components": [
                            { "id": "b1", "type": "text" }
                        ]}
                    ]
                }
            }, {
                "id": "other",
                "component": { "id": "other-root", "type": "wrapper" }
            }]
        }))
        .expect("document");
        let items = editor_layer_items_for_page(&document, 0);
        let ids: Vec<&str> = items.iter().map(|item| item.id.as_str()).collect();
        assert_eq!(ids, ["root", "a", "b", "b1"]);
        assert_eq!(items[2].parent_id.as_deref(), Some("root"));
        assert_eq!(items[2].index, 1);
        assert_eq!(items[3].parent_id.as_deref(), Some("b"));
        assert!(editor_layer_items_for_page(&document, 9).is_empty());
    }

    #[test]
    fn palette_items_include_the_rich_text_and_embed_blocks() {
        let items = palette_block_items(&RegistrySet::with_builtins());
        assert!(items.iter().any(|item| item.id == "rich_text"));
        assert!(items.iter().any(|item| item.id == "embed"));
    }

    #[test]
    fn palette_items_with_access_filter_namespaced_blocks_and_report_provenance() {
        use crate::{ContributionAssemblyResult, ContributionDescriptor, ContributionRegistry};
        use std::collections::{BTreeMap, BTreeSet};

        let mut registries = RegistrySet::with_builtins();
        for block_id in ["shop.product_card", "blog.post_list"] {
            let template = registries
                .blocks
                .get("text")
                .expect("text template")
                .component
                .clone();
            registries
                .blocks
                .register(fly::BlockDefinition {
                    id: block_id.to_string(),
                    label: block_id.to_string(),
                    category: "provider".to_string(),
                    component: template,
                })
                .expect("provider block");
        }

        let mut registry = ContributionRegistry::default();
        registry
            .register(ContributionDescriptor {
                id: "shop.blocks".to_string(),
                provider: "rustok-commerce".to_string(),
                required_capabilities: BTreeSet::new(),
                blocks: vec!["shop.product_card".to_string()],
                renderers: Vec::new(),
                property_editors: Vec::new(),
                messages: BTreeMap::new(),
                metadata: serde_json::Map::new(),
            })
            .expect("registry");
        let assembly = ContributionAssemblyResult {
            registry,
            registered_contributions: 1,
            ..ContributionAssemblyResult::default()
        };
        let access = PaletteBlockAccess::from_assembly(&assembly);

        let items = palette_block_items_with_access(&registries, &access);
        assert!(items.iter().any(|item| item.id == "text"));
        let card = items
            .iter()
            .find(|item| item.id == "shop.product_card")
            .expect("declared provider block stays visible");
        assert_eq!(card.contribution_ids, vec!["shop.blocks".to_string()]);
        assert!(
            !items.iter().any(|item| item.id == "blog.post_list"),
            "undeclared namespaced blocks are filtered out"
        );
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ThemeTokenItem {
    pub name: String,
    pub value: String,
    pub css_variable: String,
}

/// Design-token listing for adapter theme panels. A malformed theme yields no items:
/// validation owns the error report, panels never render a partial theme.
pub fn theme_token_items(document: &ProjectDocument) -> Vec<ThemeTokenItem> {
    fly::decode_theme_tokens(document)
        .map(|tokens| {
            tokens
                .into_iter()
                .map(|token| ThemeTokenItem {
                    css_variable: token.css_variable(),
                    name: token.name,
                    value: token.value,
                })
                .collect()
        })
        .unwrap_or_default()
}
