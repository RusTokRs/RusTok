//! Site symbols: shared component definitions referenced by thin instances.
//!
//! A symbol definition is a named component subtree (plus its component style
//! rules and asset descriptors) stored in the project under
//! [`FLY_SYMBOLS_FIELD`]. An instance is a component carrying `symbolId` and
//! an empty child list at rest. [`resolve_symbol_instances`] expands every
//! instance deterministically: the instance shell keeps its id, tag, and
//! attributes as a wrapper, and receives a deep clone of the definition whose
//! ids are reissued from the instance identity. Unknown references and cycles
//! fail closed.

use crate::fragment::reset_remapped_style_rule_identity;
use crate::id_reference::remap_value_ids;
use crate::{
    ComponentNode, ComponentObject, FlyError, FlyResult, ProjectDocument, validate_identifier,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const FLY_SYMBOLS_FIELD: &str = "flySymbols";

/// One named definition body referenced by symbol instances.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SymbolDescriptor {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub components: Vec<ComponentNode>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub styles: Vec<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assets: Vec<Value>,
}

impl SymbolDescriptor {
    pub fn from_value(raw: &Value) -> Option<Self> {
        serde_json::from_value(raw.clone()).ok().and_then(|descriptor: Self| {
            (!descriptor.id.is_empty()).then_some(descriptor)
        })
    }

    pub fn into_value(self) -> FlyResult<Value> {
        serde_json::to_value(self).map_err(|error| FlyError::Encode(error.to_string()))
    }

    /// Raw definition entries in document order with their `flySymbols` path.
    pub fn entries_from_document(document: &ProjectDocument) -> Vec<(String, Value)> {
        let Some(Value::Array(entries)) = document.project.extensions.get(FLY_SYMBOLS_FIELD) else {
            return Vec::new();
        };
        entries
            .iter()
            .enumerate()
            .map(|(index, raw)| (format!("{FLY_SYMBOLS_FIELD}[{index}]"), raw.clone()))
            .collect()
    }

    /// Parses every well-formed definition; malformed entries are skipped here
    /// and reported by project validation.
    pub fn catalog_from_document(document: &ProjectDocument) -> BTreeMap<String, SymbolDescriptor> {
        Self::entries_from_document(document)
            .into_iter()
            .filter_map(|(_, entry)| Self::from_value(&entry))
            .map(|descriptor| (descriptor.id.clone(), descriptor))
            .collect()
    }
}

/// Replaces the stored definition list with `symbols` (used by command application).
pub fn set_symbol_descriptors(
    document: &mut ProjectDocument,
    symbols: Vec<Value>,
) {
    if symbols.is_empty() {
        document.project.extensions.remove(FLY_SYMBOLS_FIELD);
        return;
    }
    document
        .project
        .extensions
        .insert(FLY_SYMBOLS_FIELD.to_string(), Value::Array(symbols));
}

/// Applies [`SymbolCommand`] to the document's definition catalog.
///
/// Removal fails closed while any page instance still references the
/// definition; upserts fail closed on malformed definitions and on reference
/// cycles they would introduce.
pub fn apply_symbol_command(
    document: &mut ProjectDocument,
    command: &crate::SymbolCommand,
) -> FlyResult<()> {
    use crate::SymbolCommand;

    match command {
        SymbolCommand::Upsert { symbol } => {
            let descriptor = SymbolDescriptor::from_value(symbol).ok_or_else(|| {
                FlyError::InvalidSymbolReference(
                    "symbol must be an object with a non-empty id and components".to_string(),
                )
            })?;
            validate_definition(&descriptor)?;
            for asset in &descriptor.assets {
                let asset_descriptor = crate::AssetDescriptor::from_value(asset.clone())
                    .ok_or_else(|| {
                        FlyError::InvalidAssetReference(
                            "symbol asset must be an object with src, source, or url".to_string(),
                        )
                    })?;
                if let Some(index) = document.project.assets.iter().position(|candidate| {
                    crate::AssetDescriptor::from_value(candidate.clone())
                        .is_some_and(|candidate| candidate.id == asset_descriptor.id)
                }) {
                    document.project.assets[index] = asset.clone();
                } else {
                    document.project.assets.push(asset.clone());
                }
            }
            let mut entries: Vec<Value> = SymbolDescriptor::entries_from_document(document)
                .into_iter()
                .map(|(_, entry)| entry)
                .collect();
            if let Some(index) = entries.iter().position(|entry| {
                SymbolDescriptor::from_value(entry)
                    .is_some_and(|existing| existing.id == descriptor.id)
            }) {
                entries[index] = symbol.clone();
            } else {
                entries.push(symbol.clone());
            }
            set_symbol_descriptors(document, entries);
            let cycles = detect_symbol_cycles(&SymbolDescriptor::catalog_from_document(document));
            if let Some(cycle) = cycles.first() {
                return Err(FlyError::SymbolCycle(cycle.clone()));
            }
            Ok(())
        }
        SymbolCommand::Remove { symbol_id } => {
            let entries: Vec<Value> = SymbolDescriptor::entries_from_document(document)
                .into_iter()
                .map(|(_, entry)| entry)
                .collect();
            let before = entries.len();
            let mut referenced = 0usize;
            document.project.visit_components(|component, _, _| {
                if component.symbol_id.as_deref() == Some(symbol_id.as_str()) {
                    referenced += 1;
                }
            });
            if referenced > 0 {
                return Err(FlyError::InvalidSymbolReference(format!(
                    "cannot remove symbol `{symbol_id}`; {referenced} instance(s) still reference it"
                )));
            }
            let retained: Vec<Value> = entries
                .into_iter()
                .filter(|entry| {
                    SymbolDescriptor::from_value(entry)
                        .is_none_or(|descriptor| descriptor.id != *symbol_id)
                })
                .collect();
            if retained.len() == before {
                return Err(FlyError::SymbolNotFound(symbol_id.clone()));
            }
            set_symbol_descriptors(document, retained);
            Ok(())
        }
    }
}

fn validate_definition(descriptor: &SymbolDescriptor) -> FlyResult<()> {
    if let Err(reason) = validate_identifier(&descriptor.id) {
        return Err(FlyError::InvalidSymbolReference(format!(
            "symbol id `{}` is invalid: {reason}",
            descriptor.id
        )));
    }
    let mut ids = BTreeSet::new();
    for component in &descriptor.components {
        let mut collected = Vec::new();
        component.collect_ids(&mut collected);
        for id in collected {
            if let Err(reason) = validate_identifier(&id) {
                return Err(FlyError::InvalidSymbolReference(format!(
                    "symbol `{}` component id `{id}` is invalid: {reason}",
                    descriptor.id
                )));
            }
            if !ids.insert(id.clone()) {
                return Err(FlyError::InvalidSymbolReference(format!(
                    "symbol `{}` contains duplicate component id `{id}`",
                    descriptor.id
                )));
            }
        }
    }
    Ok(())
}

/// Expands every symbol instance into its definition content.
///
/// The returned document is a pure content projection: instance shells keep
/// their own id, tag, and attributes as wrappers, children become a cloned and
/// re-identified definition body, and the `symbolId` marker is removed so the
/// result is idempotent under re-resolution. Ids are reissued deterministically
/// as `{instance_id}-{definition_id}` (with a deterministic numeric fallback on
/// collision or length overflow), so equal inputs always produce equal output
/// and therefore equal published hashes.
pub fn resolve_symbol_instances(document: &ProjectDocument) -> FlyResult<ProjectDocument> {
    let catalog = strict_catalog(document)?;
    if catalog.is_empty()
        && !document_has_instances(document)
    {
        return Ok(document.clone());
    }
    let mut resolved = document.clone();
    let mut used_ids = BTreeSet::new();
    resolved.project.visit_components(|component, _, _| {
        if let Some(id) = component.id.as_ref() {
            used_ids.insert(id.clone());
        }
    });
    let mut pending_styles = Vec::new();
    for page in &mut resolved.project.pages {
        if let Some(root) = page.component.as_mut() {
            let mut stack = Vec::new();
            expand_node(root, &catalog, &mut used_ids, &mut stack, &mut pending_styles)?;
        }
    }
    resolved.project.styles.extend(pending_styles);
    Ok(resolved)
}

fn document_has_instances(document: &ProjectDocument) -> bool {
    let mut found = false;
    document.project.visit_components(|component, _, _| {
        found = found || component.symbol_id.is_some();
    });
    found
}

fn strict_catalog(
    document: &ProjectDocument,
) -> FlyResult<BTreeMap<String, SymbolDescriptor>> {
    let mut catalog = BTreeMap::new();
    for (path, entry) in SymbolDescriptor::entries_from_document(document) {
        let descriptor = SymbolDescriptor::from_value(&entry)
            .ok_or_else(|| FlyError::InvalidSymbolReference(format!("{path} is not a valid symbol definition")))?;
        if catalog.contains_key(&descriptor.id) {
            return Err(FlyError::InvalidSymbolReference(format!(
                "symbol `{}` is defined more than once",
                descriptor.id
            )));
        }
        catalog.insert(descriptor.id.clone(), descriptor);
    }
    Ok(catalog)
}

fn expand_node(
    node: &mut ComponentNode,
    catalog: &BTreeMap<String, SymbolDescriptor>,
    used_ids: &mut BTreeSet<String>,
    stack: &mut Vec<String>,
    pending_styles: &mut Vec<Value>,
) -> FlyResult<()> {
    let ComponentNode::Object(object) = node else {
        return Ok(());
    };
    if let Some(symbol_id) = object.symbol_id.clone() {
        expand_instance(object, &symbol_id, catalog, used_ids, stack, pending_styles)?;
    }
    if let Some(children) = object.children_mut() {
        for child in children.iter_mut() {
            expand_node(child, catalog, used_ids, stack, pending_styles)?;
        }
    }
    Ok(())
}

fn expand_instance(
    object: &mut ComponentObject,
    symbol_id: &str,
    catalog: &BTreeMap<String, SymbolDescriptor>,
    used_ids: &mut BTreeSet<String>,
    stack: &mut Vec<String>,
    pending_styles: &mut Vec<Value>,
) -> FlyResult<()> {
    let Some(descriptor) = catalog.get(symbol_id) else {
        return Err(FlyError::SymbolNotFound(symbol_id.to_string()));
    };
    if stack.iter().any(|active| active == symbol_id) {
        let mut cycle = stack.clone();
        cycle.push(symbol_id.to_string());
        return Err(FlyError::SymbolCycle(cycle.join(" -> ")));
    }
    let Some(instance_id) = object.id.clone() else {
        return Err(FlyError::InvalidSymbolReference(format!(
            "symbol instance of `{symbol_id}` must carry a component id"
        )));
    };

    let mut source_ids = Vec::new();
    for component in &descriptor.components {
        component.collect_ids(&mut source_ids);
    }
    let mut mapping = BTreeMap::new();
    for source in &source_ids {
        let derived = derived_id(&instance_id, source, used_ids);
        used_ids.insert(derived.clone());
        mapping.insert(source.clone(), derived);
    }

    let mut cloned: Vec<ComponentNode> = descriptor.components.clone();
    for component in &mut cloned {
        component.remap_ids(&mapping);
    }
    for style in &descriptor.styles {
        let mut style = style.clone();
        remap_value_ids(&mut style, &mapping);
        reset_remapped_style_rule_identity(&mut style, &mapping);
        pending_styles.push(style);
    }

    object.components = crate::ComponentChildren::Nodes(cloned);
    object.symbol_id = None;

    stack.push(symbol_id.to_string());
    if let Some(children) = object.children_mut() {
        for child in children.iter_mut() {
            expand_node(child, catalog, used_ids, stack, pending_styles)?;
        }
    }
    stack.pop();
    Ok(())
}

fn derived_id(instance_id: &str, definition_id: &str, used_ids: &BTreeSet<String>) -> String {
    let preferred = format!("{instance_id}-{definition_id}");
    if !used_ids.contains(&preferred) && validate_identifier(&preferred).is_ok() {
        return preferred;
    }
    let mut counter = 2u32;
    loop {
        let candidate = format!("{instance_id}-{counter}");
        if !used_ids.contains(&candidate) && validate_identifier(&candidate).is_ok() {
            return candidate;
        }
        counter += 1;
    }
}

/// Ids of symbol definitions referenced by instances inside `components`.
pub(crate) fn instance_references(components: &[ComponentNode]) -> Vec<String> {
    let mut references = Vec::new();
    for component in components {
        collect_instance_references(component, &mut references);
    }
    references
}

fn collect_instance_references(node: &ComponentNode, references: &mut Vec<String>) {
    let ComponentNode::Object(object) = node else {
        return;
    };
    if let Some(symbol_id) = object.symbol_id.as_ref() {
        references.push(symbol_id.clone());
    }
    for child in object.children() {
        collect_instance_references(child, references);
    }
}

fn detect_symbol_cycles(catalog: &BTreeMap<String, SymbolDescriptor>) -> Vec<String> {
    let mut cycles = Vec::new();
    for id in catalog.keys() {
        let mut stack = Vec::new();
        walk_symbol_graph(id, catalog, &mut stack, &mut cycles);
    }
    cycles.sort();
    cycles.dedup();
    cycles
}

/// Reports every reference cycle among definitions as `"a -> b -> a"` chains.
pub fn symbol_reference_cycles(document: &ProjectDocument) -> Vec<String> {
    detect_symbol_cycles(&SymbolDescriptor::catalog_from_document(document))
}

fn walk_symbol_graph(
    id: &str,
    catalog: &BTreeMap<String, SymbolDescriptor>,
    stack: &mut Vec<String>,
    cycles: &mut Vec<String>,
) {
    if let Some(position) = stack.iter().position(|active| active == id) {
        let mut cycle = stack[position..].to_vec();
        cycle.push(id.to_string());
        cycles.push(cycle.join(" -> "));
        return;
    }
    let Some(descriptor) = catalog.get(id) else {
        return;
    };
    stack.push(id.to_string());
    for reference in instance_references(&descriptor.components) {
        walk_symbol_graph(&reference, catalog, stack, cycles);
    }
    stack.pop();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ComponentChildren, EditorCommand, FlyEditor, GrapesProject, ProjectPage, RegistrySet,
        SymbolCommand, ValidationLimits, validate_project,
    };
    use serde_json::json;

    fn object(id: &str, component_type: &str, children: Vec<ComponentNode>) -> ComponentNode {
        ComponentNode::Object(Box::new(ComponentObject {
            id: Some(id.to_string()),
            component_type: Some(component_type.to_string()),
            components: ComponentChildren::Nodes(children),
            ..ComponentObject::default()
        }))
    }

    fn instance(id: &str, symbol_id: &str) -> ComponentNode {
        ComponentNode::Object(Box::new(ComponentObject {
            id: Some(id.to_string()),
            component_type: Some("symbol".to_string()),
            symbol_id: Some(symbol_id.to_string()),
            ..ComponentObject::default()
        }))
    }

    fn document_with(root: ComponentNode, symbols: Vec<Value>) -> ProjectDocument {
        let mut project = GrapesProject::default();
        project.pages = vec![ProjectPage {
            id: Some("page-1".to_string()),
            component: Some(root),
            ..ProjectPage::default()
        }];
        if !symbols.is_empty() {
            project
                .extensions
                .insert(FLY_SYMBOLS_FIELD.to_string(), Value::Array(symbols));
        }
        ProjectDocument::new(project)
    }

    fn symbol_json(id: &str, components: Vec<Value>) -> Value {
        json!({ "id": id, "name": id, "components": components })
    }

    fn plain_component_json(id: &str) -> Value {
        json!({ "id": id, "type": "text", "content": "hello" })
    }

    #[test]
    fn descriptor_round_trips_through_value() {
        let entry = json!({
            "id": "hero",
            "name": "Hero",
            "components": [{ "id": "hero-title", "type": "heading", "content": "Welcome" }],
            "styles": [{ "flyRuleId": "rule-1", "flyComponentId": "hero-title", "style": { "color": "red" } }],
            "assets": [{ "id": "img-1", "src": "/img.png" }]
        });
        let descriptor = SymbolDescriptor::from_value(&entry).expect("descriptor");
        assert_eq!(descriptor.id, "hero");
        assert_eq!(descriptor.components.len(), 1);
        assert_eq!(descriptor.styles.len(), 1);
        assert_eq!(descriptor.assets.len(), 1);
        let value = descriptor.into_value().expect("encode");
        assert_eq!(value["id"], "hero");
        assert_eq!(value["name"], "Hero");
    }

    #[test]
    fn resolve_expands_instances_deterministically() {
        let definition = json!({
            "id": "cta",
            "components": [
                {
                    "id": "cta-root",
                    "type": "section",
                    "components": [{ "id": "cta-text", "type": "text", "content": "Buy" }]
                }
            ],
            "styles": [
                {
                    "flyRuleId": "rule-cta",
                    "flyComponentId": "cta-text",
                    "style": { "color": "blue" }
                }
            ]
        });
        let document = document_with(
            object(
                "root",
                "wrapper",
                vec![instance("slot-a", "cta"), instance("slot-b", "cta")],
            ),
            vec![definition],
        );

        let resolved = resolve_symbol_instances(&document).expect("resolve");
        let first = resolved.component("slot-a").expect("slot-a");
        assert!(first.symbol_id.is_none());
        assert_eq!(first.id.as_deref(), Some("slot-a"));
        assert_eq!(first.children().len(), 1);
        let expanded_root = first.children()[0].as_object().expect("object");
        assert_eq!(expanded_root.id.as_deref(), Some("slot-a-cta-root"));
        assert_eq!(expanded_root.children()[0].as_object().expect("text").id.as_deref(), Some("slot-a-cta-text"));

        let second = resolved.component("slot-b").expect("slot-b");
        let second_child = second.children()[0].as_object().expect("object");
        assert_eq!(second_child.id.as_deref(), Some("slot-b-cta-root"));

        // Style rules were re-attached with remapped component references and
        // without stale rule identity, so ids derive from the new component ids.
        let styles = &resolved.project.styles;
        assert_eq!(styles.len(), 2);
        let mut targets: Vec<&str> = styles
            .iter()
            .map(|style| style["flyComponentId"].as_str().expect("flyComponentId"))
            .collect();
        targets.sort();
        assert_eq!(targets, vec!["slot-a-cta-text", "slot-b-cta-text"]);
        assert!(styles.iter().all(|style| style.get("flyRuleId").is_none()));

        // Idempotent: re-resolving the resolved document changes nothing.
        let again = resolve_symbol_instances(&resolved).expect("resolve again");
        assert_eq!(again, resolved);

        // Deterministic: resolving the source again reproduces the same tree.
        let repeat = resolve_symbol_instances(&document).expect("resolve repeat");
        assert_eq!(repeat, resolved);
    }

    #[test]
    fn resolve_fails_closed_on_missing_symbol_and_cycles() {
        let missing = document_with(instance("slot", "ghost"), Vec::new());
        assert!(matches!(
            resolve_symbol_instances(&missing),
            Err(FlyError::SymbolNotFound(id)) if id == "ghost"
        ));

        let cyclic = document_with(
            object("root", "wrapper", vec![instance("slot", "a")]),
            vec![
                symbol_json("a", vec![json!({ "id": "a-1", "type": "symbol", "symbolId": "b" })]),
                symbol_json("b", vec![json!({ "id": "b-1", "type": "symbol", "symbolId": "a" })]),
            ],
        );
        assert!(matches!(
            resolve_symbol_instances(&cyclic),
            Err(FlyError::SymbolCycle(_))
        ));
    }

    #[test]
    fn resolve_expands_nested_instances_inside_definitions() {
        let nested = document_with(
            object("root", "wrapper", vec![instance("slot", "outer")]),
            vec![
                symbol_json(
                    "outer",
                    vec![json!({
                        "id": "outer-1",
                        "type": "symbol",
                        "symbolId": "inner",
                        "components": []
                    })],
                ),
                symbol_json("inner", vec![json!({ "id": "inner-1", "type": "text", "content": "in" })]),
            ],
        );
        let resolved = resolve_symbol_instances(&nested).expect("resolve");
        let outer = resolved.component("slot").expect("slot");
        let outer_child = outer.children()[0].as_object().expect("outer-1");
        assert!(outer_child.symbol_id.is_none());
        assert_eq!(
            outer_child.children()[0].as_object().expect("inner-1").id.as_deref(),
            Some("slot-outer-1-inner-1")
        );
    }

    #[test]
    fn commands_upsert_and_remove_definitions_transactionally() {
        let mut editor = FlyEditor::new(
            document_with(object("root", "wrapper", vec![]), Vec::new()),
            RegistrySet::with_builtins(),
        );

        editor
            .apply(EditorCommand::Symbol {
                command: SymbolCommand::Upsert {
                    symbol: symbol_json("cta", vec![plain_component_json("cta-1")]),
                },
            })
            .expect("upsert definition");
        assert!(SymbolDescriptor::catalog_from_document(editor.document()).contains_key("cta"));

        editor
            .apply(EditorCommand::Insert {
                parent_id: Some("root".to_string()),
                index: 0,
                component: instance("slot", "cta"),
            })
            .expect("insert instance");

        let removal = editor.apply(EditorCommand::Symbol {
            command: SymbolCommand::Remove {
                symbol_id: "cta".to_string(),
            },
        });
        assert!(matches!(
            removal,
            Err(FlyError::InvalidSymbolReference(message))
                if message.contains("cannot remove symbol `cta`")
        ));

        editor
            .apply(EditorCommand::Remove {
                component_id: "slot".to_string(),
            })
            .expect("remove instance");
        editor
            .apply(EditorCommand::Symbol {
                command: SymbolCommand::Remove {
                    symbol_id: "cta".to_string(),
                },
            })
            .expect("remove unreferenced definition");
        assert!(SymbolDescriptor::catalog_from_document(editor.document()).is_empty());
    }

    #[test]
    fn remove_fails_closed_while_instances_reference_the_definition() {
        let mut editor = FlyEditor::new(
            document_with(object("root", "wrapper", vec![instance("slot", "cta")]), vec![
                symbol_json("cta", vec![plain_component_json("cta-1")]),
            ]),
            RegistrySet::with_builtins(),
        );
        let removal = editor.apply(EditorCommand::Symbol {
            command: SymbolCommand::Remove {
                symbol_id: "cta".to_string(),
            },
        });
        assert!(matches!(
            removal,
            Err(FlyError::InvalidSymbolReference(message))
                if message.contains("cannot remove symbol `cta`")
        ));
    }

    #[test]
    fn validation_flags_orphan_references_and_instance_children() {
        let document = document_with(
            object(
                "root",
                "wrapper",
                vec![ComponentNode::Object(Box::new(ComponentObject {
                    id: Some("slot".to_string()),
                    component_type: Some("symbol".to_string()),
                    symbol_id: Some("ghost".to_string()),
                    components: ComponentChildren::Nodes(vec![object("stray", "text", vec![])]),
                    ..ComponentObject::default()
                }))],
            ),
            Vec::new(),
        );
        let report = validate_project(
            &document,
            &RegistrySet::with_builtins(),
            ValidationLimits::default(),
        );
        let codes: Vec<&str> = report
            .diagnostics
            .iter()
            .map(|error| error.code.as_str())
            .collect();
        assert!(codes.contains(&"symbol_reference_missing"));
        assert!(codes.contains(&"symbol_instance_with_children"));
    }
}
