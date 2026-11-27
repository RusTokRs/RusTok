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

use crate::style_rule::remap_component_style_rule;
use crate::{
    ComponentNode, ComponentObject, EditorCommand, FlyError, FlyResult, ProjectDocument,
    ProjectFragment, SymbolCommand, validate_identifier,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub const FLY_SYMBOLS_FIELD: &str = "flySymbols";
/// Explicit caps on authoring definitions independent of per-page render caps.
pub const MAX_SYMBOL_DEFINITIONS: usize = 128;
pub const MAX_SYMBOL_COMPONENTS: usize = 512;
pub const MAX_SYMBOL_DEFINITION_BYTES: usize = 262_144;

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
        crate::codec::ensure_depth_within_limit(raw, crate::MAXIMUM_DECODE_DEPTH).ok()?;
        if symbol_definition_size_exceeds_limit(raw) {
            return None;
        }
        serde_json::from_value(raw.clone())
            .ok()
            .and_then(|descriptor: Self| (!descriptor.id.is_empty()).then_some(descriptor))
    }

    pub fn into_value(self) -> FlyResult<Value> {
        serde_json::to_value(self).map_err(|error| FlyError::Encode(error.to_string()))
    }

    /// Raw definition entries in document order with their `flySymbols` path.
    pub fn entries_from_document(document: &ProjectDocument) -> Vec<(String, Value)> {
        let Some(block) = document.project().extensions.get(FLY_SYMBOLS_FIELD) else {
            return Vec::new();
        };
        if crate::codec::ensure_depth_within_limit(block, crate::MAXIMUM_DECODE_DEPTH).is_err() {
            return Vec::new();
        }
        let Value::Array(entries) = block else {
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
pub fn set_symbol_descriptors(document: &mut ProjectDocument, symbols: Vec<Value>) {
    if symbols.is_empty() {
        document.project_mut().extensions.remove(FLY_SYMBOLS_FIELD);
    } else {
        document
            .project_mut()
            .extensions
            .insert(FLY_SYMBOLS_FIELD.to_string(), Value::Array(symbols));
    }
    document.invalidate_caches();
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
    let mut candidate = document.clone();
    apply_symbol_command_inner(&mut candidate, command)?;
    candidate.invalidate_caches();
    *document = candidate;
    Ok(())
}

fn apply_symbol_command_inner(
    document: &mut ProjectDocument,
    command: &crate::SymbolCommand,
) -> FlyResult<()> {
    use crate::SymbolCommand;

    if let Some(block) = document.project().extensions.get(FLY_SYMBOLS_FIELD) {
        crate::codec::ensure_depth_within_limit(block, crate::MAXIMUM_DECODE_DEPTH).map_err(
            |error| {
                FlyError::InvalidSymbolReference(format!(
                    "symbol catalog exceeds safe JSON depth: {error}"
                ))
            },
        )?;
    }
    // Preserve malformed catalog source instead of treating a non-array block as empty.
    if document
        .project()
        .extensions
        .get(FLY_SYMBOLS_FIELD)
        .is_some_and(|block| !block.is_array())
    {
        return Err(FlyError::InvalidSymbolReference(format!(
            "{FLY_SYMBOLS_FIELD} must be an array"
        )));
    }

    match command {
        SymbolCommand::Upsert { symbol } => {
            let descriptor = SymbolDescriptor::from_value(symbol).ok_or_else(|| {
                FlyError::InvalidSymbolReference(
                    "symbol must be an object with a non-empty id and components".to_string(),
                )
            })?;
            validate_definition(&descriptor)?;

            // A symbol asset id shares the page-wide asset namespace. Never overwrite a
            // different page asset or capture a dangling page reference when importing a
            // definition; remap the symbol and its component references as one unit.
            let mut symbol_assets = descriptor.assets.clone();
            let mut symbol_components = descriptor.components.clone();
            let mut existing_asset_references = BTreeSet::new();
            for page in &document.project().pages {
                if let Some(root) = page.component.as_ref() {
                    crate::fragment::collect_component_asset_ids(
                        root,
                        &mut existing_asset_references,
                    );
                }
            }
            for (_, entry) in SymbolDescriptor::entries_from_document(document) {
                if let Some(existing) = SymbolDescriptor::from_value(&entry)
                    && existing.id != descriptor.id
                {
                    existing_asset_references
                        .extend(symbol_asset_dangling_references(&existing));
                }
            }
            let mut asset_id_generator =
                crate::SequentialIdGenerator::new("fly-symbol-asset");
            crate::fragment::remap_fragment_asset_ids(
                &mut symbol_assets,
                &mut symbol_components,
                &document.project().assets,
                &existing_asset_references,
                &mut asset_id_generator,
            )?;

            let mut stored_symbol = symbol.clone();
            let stored_object = stored_symbol.as_object_mut().ok_or_else(|| {
                FlyError::InvalidSymbolReference("symbol definition must be an object".to_string())
            })?;
            stored_object.insert(
                "assets".to_string(),
                Value::Array(symbol_assets.clone()),
            );
            stored_object.insert(
                "components".to_string(),
                serde_json::to_value(&symbol_components)
                    .map_err(|error| FlyError::Encode(error.to_string()))?,
            );
            let stored_descriptor = SymbolDescriptor::from_value(&stored_symbol).ok_or_else(|| {
                FlyError::InvalidSymbolReference("remapped symbol definition is invalid".to_string())
            })?;
            validate_definition(&stored_descriptor)?;

            let mut merged_assets = document.project().assets.clone();
            for raw in &symbol_assets {
                let asset_descriptor = crate::AssetDescriptor::from_value(raw.clone()).ok_or_else(
                    || {
                        FlyError::InvalidAssetReference(
                            "symbol asset must be an object with src, source, or url".to_string(),
                        )
                    },
                )?;
                let existing = merged_assets.iter().find(|candidate| {
                    crate::AssetDescriptor::from_value((*candidate).clone())
                        .is_some_and(|candidate| candidate.id == asset_descriptor.id)
                });
                if let Some(existing) = existing {
                    if existing != raw {
                        return Err(FlyError::InvalidAssetReference(format!(
                            "symbol asset id \`{}\` conflicts with an existing project asset",
                            asset_descriptor.id
                        )));
                    }
                } else {
                    merged_assets.push(raw.clone());
                }
            }
            document.project_mut().assets = merged_assets;

            let mut entries: Vec<Value> = SymbolDescriptor::entries_from_document(document)
                .into_iter()
                .map(|(_, entry)| entry)
                .collect();
            if let Some(index) = entries.iter().position(|entry| {
                entry.get("id").and_then(Value::as_str) == Some(descriptor.id.as_str())
            }) {
                entries[index] = stored_symbol;
            } else {
                entries.push(stored_symbol);
            }
            if entries.len() > MAX_SYMBOL_DEFINITIONS {
                return Err(FlyError::InvalidSymbolReference(format!(
                    "site symbol catalog exceeds {MAX_SYMBOL_DEFINITIONS} definitions"
                )));
            }
            set_symbol_descriptors(document, entries);
            // Validate the complete catalog, not only cycles: a direct caller of this
            // public command API must not be able to commit a dangling nested reference.
            strict_catalog(document)?;
            Ok(())
        }
        SymbolCommand::Remove { symbol_id } => {
            let entries: Vec<Value> = SymbolDescriptor::entries_from_document(document)
                .into_iter()
                .map(|(_, entry)| entry)
                .collect();
            let before = entries.len();
            let referenced = symbol_instances_in_document(document)
                .iter()
                .filter(|instance| instance.symbol_id.as_deref() == Some(symbol_id.as_str()))
                .count();
            if referenced > 0 {
                return Err(FlyError::InvalidSymbolReference(format!(
                    "cannot remove symbol `{symbol_id}`; {referenced} instance(s) still reference it"
                )));
            }
            for definition in entries.iter().filter_map(SymbolDescriptor::from_value) {
                if definition.id != *symbol_id
                    && symbol_references(&definition.components)
                        .iter()
                        .any(|id| id == symbol_id)
                {
                    return Err(FlyError::InvalidSymbolReference(format!(
                        "cannot remove symbol `{symbol_id}`; definition `{}` still references it",
                        definition.id
                    )));
                }
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
            strict_catalog(document)?;
            Ok(())
        }
    }
}

pub(crate) fn symbol_definition_size_exceeds_limit(value: &Value) -> bool {
    struct Counter(usize);
    impl std::io::Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 = self.0.saturating_add(bytes.len());
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let mut counter = Counter(0);
    serde_json::to_writer(&mut counter, value)
        .map(|()| counter.0 > MAX_SYMBOL_DEFINITION_BYTES)
        .unwrap_or(true)
}

enum SymbolComponentRef<'a> {
    Typed(&'a ComponentNode),
    Opaque(&'a Value),
}

fn push_opaque_symbol_children<'a>(
    value: &'a Value,
    pending: &mut Vec<SymbolComponentRef<'a>>,
) {
    match value {
        Value::Array(children) => {
            pending.extend(children.iter().map(SymbolComponentRef::Opaque));
        }
        Value::Null => {}
        _ => pending.push(SymbolComponentRef::Opaque(value)),
    }
}

fn push_opaque_symbol_node_children<'a>(
    value: &'a Value,
    pending: &mut Vec<SymbolComponentRef<'a>>,
) {
    if let Some(children) = value
        .as_object()
        .and_then(|object| object.get("components"))
    {
        push_opaque_symbol_children(children, pending);
    }
}

/// A symbol marker found on a typed or opaque component node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SymbolInstanceInfo {
    pub(crate) path: String,
    pub(crate) component_id: Option<String>,
    pub(crate) symbol_id: Option<String>,
    pub(crate) has_children: bool,
    pub(crate) malformed_symbol_id: bool,
}

fn collect_symbol_instances_from_components(
    components: &[ComponentNode],
    path: &str,
    instances: &mut Vec<SymbolInstanceInfo>,
) {
    for (index, component) in components.iter().enumerate() {
        collect_symbol_instances_from_node(
            component,
            &format!("{path}[{index}]"),
            instances,
        );
    }
}

fn collect_symbol_instances_from_node(
    component: &ComponentNode,
    path: &str,
    instances: &mut Vec<SymbolInstanceInfo>,
) {
    match component {
        ComponentNode::Object(object) => {
            if let Some(symbol_id) = object.symbol_id.as_ref() {
                instances.push(SymbolInstanceInfo {
                    path: path.to_string(),
                    component_id: object.id.clone(),
                    symbol_id: Some(symbol_id.clone()),
                    has_children: !object.components.is_empty(),
                    malformed_symbol_id: false,
                });
            }
            match &object.components {
                crate::ComponentChildren::Nodes(children) => {
                    collect_symbol_instances_from_components(
                        children,
                        &format!("{path}.components"),
                        instances,
                    );
                }
                crate::ComponentChildren::Opaque(children) => {
                    collect_opaque_symbol_instances(
                        children,
                        &format!("{path}.components"),
                        instances,
                    );
                }
            }
        }
        ComponentNode::Opaque(raw) => {
            collect_opaque_symbol_instances(raw, path, instances);
        }
    }
}

fn collect_opaque_symbol_instances(
    value: &Value,
    path: &str,
    instances: &mut Vec<SymbolInstanceInfo>,
) {
    match value {
        Value::Array(children) => {
            for (index, child) in children.iter().enumerate() {
                collect_opaque_symbol_instances(
                    child,
                    &format!("{path}[{index}]"),
                    instances,
                );
            }
        }
        Value::Object(object) => {
            if let Some(marker) = object.get("symbolId").filter(|value| !value.is_null()) {
                let symbol_id = marker.as_str().map(ToString::to_string);
                let has_children = object.get("components").is_some_and(|children| match children {
                    Value::Array(children) => !children.is_empty(),
                    Value::Null => false,
                    _ => true,
                });
                instances.push(SymbolInstanceInfo {
                    path: path.to_string(),
                    component_id: object
                        .get("id")
                        .and_then(Value::as_str)
                        .map(ToString::to_string),
                    malformed_symbol_id: symbol_id.is_none(),
                    symbol_id,
                    has_children,
                });
            }
            if let Some(children) = object.get("components") {
                collect_opaque_symbol_instances(
                    children,
                    &format!("{path}.components"),
                    instances,
                );
            }
        }
        _ => {}
    }
}

pub(crate) fn symbol_instances_in_document(
    document: &ProjectDocument,
) -> Vec<SymbolInstanceInfo> {
    let mut instances = Vec::new();
    for (page_index, page) in document.project().pages.iter().enumerate() {
        if let Some(root) = page.component.as_ref() {
            collect_symbol_instances_from_node(
                root,
                &format!("pages[{page_index}].component"),
                &mut instances,
            );
        }
    }
    instances
}

fn symbol_instance_facts_in_components(
    components: &[ComponentNode],
) -> Vec<SymbolInstanceInfo> {
    let mut instances = Vec::new();
    collect_symbol_instances_from_components(components, "components", &mut instances);
    instances
}

fn symbol_asset_dangling_references(
    descriptor: &SymbolDescriptor,
) -> BTreeSet<String> {
    let mut references = BTreeSet::new();
    for component in &descriptor.components {
        crate::fragment::collect_component_asset_ids(component, &mut references);
    }
    for raw in &descriptor.assets {
        if let Some(asset) = crate::AssetDescriptor::from_value(raw.clone()) {
            references.remove(&asset.id);
        }
    }
    references
}

fn count_component_node(component: &ComponentNode) -> usize {
    match component {
        ComponentNode::Object(object) => {
            1usize.saturating_add(count_component_children(&object.components))
        }
        ComponentNode::Opaque(value) => count_opaque_component_node(value),
    }
}

fn count_component_children(children: &crate::ComponentChildren) -> usize {
    match children {
        crate::ComponentChildren::Nodes(children) => children
            .iter()
            .map(count_component_node)
            .fold(0usize, usize::saturating_add),
        crate::ComponentChildren::Opaque(value) => count_opaque_component_children(value),
    }
}

fn count_opaque_component_children(value: &Value) -> usize {
    match value {
        Value::Array(children) => children
            .iter()
            .map(count_opaque_component_node)
            .fold(0usize, usize::saturating_add),
        Value::Object(_) => count_opaque_component_node(value),
        Value::Null => 0,
        _ => 1,
    }
}

fn count_opaque_component_node(value: &Value) -> usize {
    match value {
        Value::Array(children) => children
            .iter()
            .map(count_opaque_component_node)
            .fold(0usize, usize::saturating_add),
        Value::Object(object) => {
            1usize.saturating_add(
                object
                    .get("components")
                    .map(count_opaque_component_children)
                    .unwrap_or_default(),
            )
        }
        _ => 1,
    }
}

fn count_document_component_nodes(document: &ProjectDocument) -> usize {
    document
        .project()
        .pages
        .iter()
        .filter_map(|page| page.component.as_ref())
        .map(count_component_node)
        .fold(0usize, usize::saturating_add)
}

pub(crate) fn validate_definition(descriptor: &SymbolDescriptor) -> FlyResult<()> {
    let encoded = serde_json::to_vec(descriptor)
        .map_err(|error| FlyError::InvalidSymbolReference(error.to_string()))?;
    if encoded.len() > MAX_SYMBOL_DEFINITION_BYTES {
        return Err(FlyError::InvalidSymbolReference(format!(
            "symbol `{}` exceeds {MAX_SYMBOL_DEFINITION_BYTES} bytes",
            descriptor.id
        )));
    }
    let mut pending = descriptor
        .components
        .iter()
        .map(SymbolComponentRef::Typed)
        .collect::<Vec<_>>();
    let mut count = 0;
    while let Some(component) = pending.pop() {
        count += 1;
        if count > MAX_SYMBOL_COMPONENTS {
            return Err(FlyError::InvalidSymbolReference(format!(
                "symbol `{}` exceeds {MAX_SYMBOL_COMPONENTS} components",
                descriptor.id
            )));
        }
        match component {
            SymbolComponentRef::Typed(ComponentNode::Object(object)) => {
                match &object.components {
                    crate::ComponentChildren::Nodes(children) => {
                        pending.extend(children.iter().map(SymbolComponentRef::Typed));
                    }
                    crate::ComponentChildren::Opaque(children) => {
                        push_opaque_symbol_children(children, &mut pending);
                    }
                }
            }
            SymbolComponentRef::Typed(ComponentNode::Opaque(raw))
            | SymbolComponentRef::Opaque(raw) => {
                push_opaque_symbol_node_children(raw, &mut pending);
            }
        }
    }
    if let Err(reason) = validate_identifier(&descriptor.id) {
        return Err(FlyError::InvalidSymbolReference(format!(
            "symbol id `{}` is invalid: {reason}",
            descriptor.id
        )));
    }
    if let Some(id) = crate::id_reference::conflicting_form_dom_id(descriptor.components.iter()) {
        return Err(FlyError::InvalidSymbolReference(format!(
            "symbol `{}` form id `{id}` belongs to a different HTML element",
            descriptor.id
        )));
    }
    let mut ids = BTreeSet::new();
    let mut dom_ids = BTreeSet::new();
    let mut form_ids = BTreeSet::new();
    for component in &descriptor.components {
        let mut collected_dom = Vec::new();
        let mut collected_forms = Vec::new();
        component.collect_dom_ids(&mut collected_dom);
        component.collect_form_ids(&mut collected_forms);
        for id in collected_dom {
            if !dom_ids.insert(id.clone()) {
                return Err(FlyError::InvalidSymbolReference(format!(
                    "symbol `{}` contains duplicate HTML id `{id}`",
                    descriptor.id
                )));
            }
        }
        for id in collected_forms {
            if !form_ids.insert(id.clone()) {
                return Err(FlyError::InvalidSymbolReference(format!(
                    "symbol `{}` contains duplicate form id `{id}`",
                    descriptor.id
                )));
            }
        }
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
    for instance in symbol_instance_facts_in_components(&descriptor.components) {
        if instance.malformed_symbol_id {
            return Err(FlyError::InvalidSymbolReference(format!(
                "symbol `{}` has a non-string symbolId at {}",
                descriptor.id, instance.path
            )));
        }
        let Some(symbol_id) = instance.symbol_id.as_deref() else {
            continue;
        };
        validate_identifier(symbol_id).map_err(|reason| {
            FlyError::InvalidSymbolReference(format!(
                "symbol `{}` reference id `{symbol_id}` is invalid: {reason}",
                descriptor.id
            ))
        })?;
        if instance.component_id.is_none() {
            return Err(FlyError::InvalidSymbolReference(format!(
                "symbol `{}` instance at {} must carry a component id",
                descriptor.id, instance.path
            )));
        }
        if instance.has_children {
            return Err(FlyError::InvalidSymbolReference(format!(
                "symbol `{}` instance at {} must be child-free",
                descriptor.id, instance.path
            )));
        }
    }
    let asset_policy = crate::AssetPolicy::default();
    if descriptor.assets.len() > asset_policy.maximum_assets {
        return Err(FlyError::InvalidAssetReference(format!(
            "symbol `{}` exceeds the maximum of {} assets",
            descriptor.id, asset_policy.maximum_assets
        )));
    }
    let mut asset_ids = BTreeSet::new();
    for raw in &descriptor.assets {
        let asset = crate::AssetDescriptor::from_value(raw.clone()).ok_or_else(|| {
            FlyError::InvalidAssetReference(format!(
                "symbol `{}` contains an invalid asset descriptor",
                descriptor.id
            ))
        })?;
        if asset.id.trim().is_empty()
            || asset.id.len() > 256
            || asset.id.chars().any(char::is_control)
        {
            return Err(FlyError::InvalidAssetReference(format!(
                "symbol `{}` contains an invalid asset id",
                descriptor.id
            )));
        }
        if !asset_ids.insert(asset.id.clone()) {
            return Err(FlyError::InvalidAssetReference(format!(
                "symbol `{}` contains duplicate asset id `{}`",
                descriptor.id, asset.id
            )));
        }
        if !crate::source_allowed(&asset.source, asset.kind, &asset_policy) {
            return Err(FlyError::InvalidAssetReference(format!(
                "symbol `{}` asset `{}` uses a source rejected by policy",
                descriptor.id, asset.id
            )));
        }
    }
    Ok(())
}

/// Converts a selected non-root component into a shared symbol in one editor
/// history entry: save its subtree as a definition, then replace the original
/// component at the same position with a reference wrapper of the same id.
pub fn convert_component_to_symbol(
    document: &ProjectDocument,
    component_id: &str,
    symbol_id: &str,
    name: Option<String>,
) -> FlyResult<EditorCommand> {
    validate_identifier(symbol_id).map_err(|reason| {
        FlyError::InvalidSymbolReference(format!("symbol id `{symbol_id}` is invalid: {reason}"))
    })?;
    let location = document
        .component_location(component_id)
        .ok_or_else(|| FlyError::ComponentNotFound(component_id.to_string()))?;
    let parent_id = location.parent_component_id.ok_or_else(|| {
        FlyError::InvalidSymbolReference("the page root cannot become a symbol".to_string())
    })?;
    let fragment = ProjectFragment::from_component(document, component_id)?;
    if !fragment.extensions.is_empty() {
        return Err(FlyError::InvalidSymbolReference(
            "symbol definitions cannot yet preserve component runtime bindings, conditions, or repeaters"
                .to_string(),
        ));
    }
    let definition = SymbolDescriptor {
        id: symbol_id.to_string(),
        name,
        components: fragment.components,
        styles: fragment.styles,
        assets: fragment.assets,
    };
    let wrapper = ComponentNode::Object(Box::new(ComponentObject {
        id: Some(component_id.to_string()),
        component_type: Some("symbol".to_string()),
        symbol_id: Some(symbol_id.to_string()),
        ..ComponentObject::default()
    }));
    Ok(EditorCommand::batch([
        EditorCommand::Symbol {
            command: SymbolCommand::Upsert {
                symbol: definition.into_value()?,
            },
        },
        EditorCommand::Remove {
            component_id: component_id.to_string(),
        },
        EditorCommand::Insert {
            parent_id: Some(parent_id),
            index: location.index,
            component: wrapper,
        },
    ]))
}

/// Appends an instance of a catalog symbol to the document's first page root.
pub fn insert_symbol_instance(
    document: &ProjectDocument,
    symbol_id: &str,
) -> FlyResult<EditorCommand> {
    if !SymbolDescriptor::catalog_from_document(document).contains_key(symbol_id) {
        return Err(FlyError::SymbolNotFound(symbol_id.to_string()));
    }
    let index = document
        .root_child_count()
        .ok_or(FlyError::MissingProjectRoot)?;
    Ok(EditorCommand::Insert {
        parent_id: None,
        index,
        component: ComponentNode::Object(Box::new(ComponentObject {
            component_type: Some("symbol".to_string()),
            symbol_id: Some(symbol_id.to_string()),
            ..ComponentObject::default()
        })),
    })
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
    if !document_has_symbol_instances(document) {
        return Ok(document.clone());
    }
    let mut resolved = document.clone();
    let node_count = count_document_component_nodes(&resolved);
    if node_count > crate::DEFAULT_MAXIMUM_NODES {
        return Err(FlyError::MaximumNodesExceeded {
            maximum: crate::DEFAULT_MAXIMUM_NODES,
        });
    }
    if resolved.project().assets.len() > crate::DEFAULT_MAXIMUM_ASSETS {
        return Err(FlyError::MaximumAssetsExceeded {
            maximum: crate::DEFAULT_MAXIMUM_ASSETS,
        });
    }
    if resolved.project().styles.len() > crate::DEFAULT_MAXIMUM_STYLE_RULES {
        return Err(FlyError::MaximumStyleRulesExceeded {
            maximum: crate::DEFAULT_MAXIMUM_STYLE_RULES,
        });
    }
    let mut asset_references = BTreeSet::new();
    for page in &resolved.project().pages {
        if let Some(root) = page.component.as_ref() {
            crate::fragment::collect_component_asset_ids(root, &mut asset_references);
        }
    }
    let symbol_asset_dangling_references = catalog
        .iter()
        .map(|(id, descriptor)| {
            (id.clone(), symbol_asset_dangling_references(descriptor))
        })
        .collect();
    let mut identities = SymbolIdentityReservations {
        generated: crate::id_reference::project_identity_ids(&resolved),
        forms: crate::id_reference::project_form_ids(&resolved),
        page_dom: BTreeSet::new(),
        assets: resolved.project().assets.clone(),
        asset_references,
        asset_id_generator: crate::SequentialIdGenerator::new("fly-symbol-asset"),
        symbol_asset_mappings: BTreeMap::new(),
        symbol_asset_dangling_references,
        node_count,
        style_rule_count: resolved.project().styles.len(),
    };
    let mut pending_styles = Vec::new();
    for page in &mut resolved.project_mut().pages {
        if let Some(root) = page.component.as_mut() {
            identities.page_dom =
                crate::id_reference::materialized_dom_ids(std::iter::once(&*root));
            let mut stack = Vec::new();
            expand_node(
                root,
                &catalog,
                &mut identities,
                &mut stack,
                &mut pending_styles,
            )?;
        }
    }
    resolved.project_mut().assets = identities.assets;
    resolved.project_mut().styles.extend(pending_styles);
    Ok(resolved)
}

/// True when a page contains at least one unresolved symbol instance.
pub fn document_has_symbol_instances(document: &ProjectDocument) -> bool {
    symbol_instances_in_document(document)
        .iter()
        .any(|instance| instance.symbol_id.is_some() || instance.malformed_symbol_id)
}

fn strict_catalog(document: &ProjectDocument) -> FlyResult<BTreeMap<String, SymbolDescriptor>> {
    if let Some(block) = document.project().extensions.get(FLY_SYMBOLS_FIELD) {
        crate::codec::ensure_depth_within_limit(block, crate::MAXIMUM_DECODE_DEPTH).map_err(
            |error| {
                FlyError::InvalidSymbolReference(format!(
                    "symbol catalog exceeds safe JSON depth: {error}"
                ))
            },
        )?;
    }
    if document
        .project()
        .extensions
        .get(FLY_SYMBOLS_FIELD)
        .is_some_and(|block| !block.is_array())
    {
        return Err(FlyError::InvalidSymbolReference(format!(
            "{FLY_SYMBOLS_FIELD} must be an array"
        )));
    }
    let mut catalog = BTreeMap::new();
    let entries = SymbolDescriptor::entries_from_document(document);
    if entries.len() > MAX_SYMBOL_DEFINITIONS {
        return Err(FlyError::InvalidSymbolReference(format!(
            "site symbol catalog exceeds {MAX_SYMBOL_DEFINITIONS} definitions"
        )));
    }
    for (path, entry) in entries {
        let descriptor = SymbolDescriptor::from_value(&entry).ok_or_else(|| {
            FlyError::InvalidSymbolReference(format!("{path} is not a valid symbol definition"))
        })?;
        validate_definition(&descriptor)?;
        if catalog.contains_key(&descriptor.id) {
            return Err(FlyError::InvalidSymbolReference(format!(
                "symbol `{}` is defined more than once",
                descriptor.id
            )));
        }
        catalog.insert(descriptor.id.clone(), descriptor);
    }
    for (id, descriptor) in &catalog {
        for reference in symbol_references(&descriptor.components) {
            if !catalog.contains_key(&reference) {
                return Err(FlyError::SymbolNotFound(format!(
                    "{reference} (referenced by symbol {id})"
                )));
            }
        }
    }
    if let Some(cycle) = detect_symbol_cycles(&catalog).first() {
        return Err(FlyError::SymbolCycle(cycle.clone()));
    }
    Ok(catalog)
}

/// Generated identities avoid every existing name, but conflicts for unchanged form IDs
/// must be checked against actual form/DOM owners, not unrelated component IDs.
struct SymbolIdentityReservations {
    generated: BTreeSet<String>,
    forms: BTreeSet<String>,
    page_dom: BTreeSet<String>,
    assets: Vec<Value>,
    asset_references: BTreeSet<String>,
    asset_id_generator: crate::SequentialIdGenerator,
    symbol_asset_mappings: BTreeMap<String, BTreeMap<String, String>>,
    symbol_asset_dangling_references: BTreeMap<String, BTreeSet<String>>,
    node_count: usize,
    style_rule_count: usize,
}

fn expand_node(
    node: &mut ComponentNode,
    catalog: &BTreeMap<String, SymbolDescriptor>,
    identities: &mut SymbolIdentityReservations,
    stack: &mut Vec<String>,
    pending_styles: &mut Vec<Value>,
) -> FlyResult<()> {
    match node {
        ComponentNode::Object(object) => {
            if let Some(symbol_id) = object.symbol_id.clone() {
                // `expand_instance` recursively resolves the cloned definition while its
                // symbol is on the cycle-detection stack. Do not walk the expanded subtree twice.
                return expand_instance(
                    object,
                    &symbol_id,
                    catalog,
                    identities,
                    stack,
                    pending_styles,
                );
            }
            match &mut object.components {
                crate::ComponentChildren::Nodes(children) => {
                    for child in children {
                        expand_node(child, catalog, identities, stack, pending_styles)?;
                    }
                }
                crate::ComponentChildren::Opaque(children) => {
                    expand_opaque_symbol_children(
                        children,
                        catalog,
                        identities,
                        stack,
                        pending_styles,
                    )?;
                }
            }
            Ok(())
        }
        ComponentNode::Opaque(value) => expand_opaque_symbol_children(
            value,
            catalog,
            identities,
            stack,
            pending_styles,
        ),
    }
}

fn expand_opaque_symbol_children(
    value: &mut Value,
    catalog: &BTreeMap<String, SymbolDescriptor>,
    identities: &mut SymbolIdentityReservations,
    stack: &mut Vec<String>,
    pending_styles: &mut Vec<Value>,
) -> FlyResult<()> {
    match value {
        Value::Array(children) => {
            for child in children {
                expand_opaque_symbol_node(
                    child,
                    catalog,
                    identities,
                    stack,
                    pending_styles,
                )?;
            }
            Ok(())
        }
        Value::Object(_) => expand_opaque_symbol_node(
            value,
            catalog,
            identities,
            stack,
            pending_styles,
        ),
        _ => Ok(()),
    }
}

fn expand_opaque_symbol_node(
    value: &mut Value,
    catalog: &BTreeMap<String, SymbolDescriptor>,
    identities: &mut SymbolIdentityReservations,
    stack: &mut Vec<String>,
    pending_styles: &mut Vec<Value>,
) -> FlyResult<()> {
    if !value.is_object() {
        return Ok(());
    }
    let symbol_marker = value
        .as_object()
        .and_then(|object| object.get("symbolId"))
        .cloned();
    match symbol_marker {
        Some(Value::String(symbol_id)) => {
            let component = serde_json::from_value::<ComponentObject>(value.clone()).map_err(
                |error| {
                    FlyError::InvalidSymbolReference(format!(
                        "opaque symbol instance `{symbol_id}` cannot be decoded: {error}"
                    ))
                },
            )?;
            let mut node = ComponentNode::Object(Box::new(component));
            expand_node(&mut node, catalog, identities, stack, pending_styles)?;
            *value = serde_json::to_value(node)
                .map_err(|error| FlyError::Encode(error.to_string()))?;
            Ok(())
        }
        Some(Value::Null) | None => {
            if let Some(children) = value
                .as_object_mut()
                .and_then(|object| object.get_mut("components"))
            {
                expand_opaque_symbol_children(
                    children,
                    catalog,
                    identities,
                    stack,
                    pending_styles,
                )?;
            }
            Ok(())
        }
        Some(_) => Err(FlyError::InvalidSymbolReference(
            "opaque component symbolId must be a string".to_string(),
        )),
    }
}

fn expand_instance(
    object: &mut ComponentObject,
    symbol_id: &str,
    catalog: &BTreeMap<String, SymbolDescriptor>,
    identities: &mut SymbolIdentityReservations,
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
    if !object.components.is_empty() {
        return Err(FlyError::InvalidSymbolReference(format!(
            "symbol instance `{instance_id}` must be child-free at rest"
        )));
    }
    validate_identifier(&instance_id).map_err(|reason| {
        FlyError::InvalidSymbolReference(format!(
            "symbol instance id `{instance_id}` is invalid: {reason}"
        ))
    })?;

    let replacement_nodes = descriptor
        .components
        .iter()
        .map(count_component_node)
        .fold(0usize, usize::saturating_add);
    let next_node_count = identities
        .node_count
        .saturating_sub(1)
        .saturating_add(replacement_nodes);
    if next_node_count > crate::DEFAULT_MAXIMUM_NODES {
        return Err(FlyError::MaximumNodesExceeded {
            maximum: crate::DEFAULT_MAXIMUM_NODES,
        });
    }
    let next_style_rule_count = identities
        .style_rule_count
        .saturating_add(descriptor.styles.len());
    if next_style_rule_count > crate::DEFAULT_MAXIMUM_STYLE_RULES {
        return Err(FlyError::MaximumStyleRulesExceeded {
            maximum: crate::DEFAULT_MAXIMUM_STYLE_RULES,
        });
    }
    identities.node_count = next_node_count;
    identities.style_rule_count = next_style_rule_count;

    let mut source_ids = Vec::new();
    let mut dom_ids = Vec::new();
    let mut form_ids = Vec::new();
    let mut matching_ids = BTreeSet::new();
    for component in &descriptor.components {
        component.collect_ids(&mut source_ids);
        component.collect_dom_ids(&mut dom_ids);
        component.collect_form_ids(&mut form_ids);
        component.collect_matching_dom_ids(&mut matching_ids);
    }
    // A form ID is also an external provider identity; changing it would silently redirect
    // submissions. Form IDs are document-wide; ordinary HTML ids collide only within this page.
    // A same-named Fly component (such as the conversion wrapper) is not itself an HTML owner.
    for id in &form_ids {
        if identities.forms.contains(id) || identities.page_dom.contains(id) {
            return Err(FlyError::InvalidSymbolReference(format!(
                "symbol `{symbol_id}` form id `{id}` collides with the page"
            )));
        }
    }
    for id in &form_ids {
        identities.forms.insert(id.clone());
        identities.page_dom.insert(id.clone());
        identities.generated.insert(id.clone());
    }
    let mut mapping = BTreeMap::new();
    for source in &source_ids {
        let preferred = format!("{instance_id}-{source}");
        let derived = crate::id_reference::allocate_derived_id(
            &preferred,
            &instance_id,
            &mut identities.generated,
        );
        mapping.insert(source.clone(), derived);
    }
    let mut dom_mapping = form_ids
        .into_iter()
        .map(|id| (id.clone(), id))
        .collect::<BTreeMap<_, _>>();
    for source in dom_ids {
        if dom_mapping.contains_key(&source) {
            continue;
        }
        let derived = if matching_ids.contains(&source) {
            mapping.get(&source).cloned()
        } else {
            None
        }
        .unwrap_or_else(|| {
            let preferred = format!("{instance_id}-{source}");
            crate::id_reference::allocate_derived_id(
                &preferred,
                &instance_id,
                &mut identities.generated,
            )
        });
        dom_mapping.insert(source, derived);
    }

    let mut cloned: Vec<ComponentNode> = descriptor.components.clone();
    for component in &mut cloned {
        component.remap_ids_with_dom(&mapping, &dom_mapping);
    }
    let mut symbol_assets = descriptor.assets.clone();
    if let Some(asset_mapping) = identities.symbol_asset_mappings.get(symbol_id) {
        crate::fragment::apply_fragment_asset_id_mapping(
            &mut symbol_assets,
            &mut cloned,
            asset_mapping,
        )?;
    } else {
        let mut existing_asset_references = identities.asset_references.clone();
        for (other_symbol_id, references) in &identities.symbol_asset_dangling_references {
            if other_symbol_id != symbol_id {
                existing_asset_references.extend(references.iter().cloned());
            }
        }
        let asset_mapping = crate::fragment::remap_fragment_asset_ids(
            &mut symbol_assets,
            &mut cloned,
            &identities.assets,
            &existing_asset_references,
            &mut identities.asset_id_generator,
        )?;
        identities
            .symbol_asset_mappings
            .insert(symbol_id.to_string(), asset_mapping);
    }
    for raw in symbol_assets {
        let asset = crate::AssetDescriptor::from_value(raw.clone()).ok_or_else(|| {
            FlyError::InvalidAssetReference(format!(
                "symbol `{symbol_id}` contains an invalid asset descriptor"
            ))
        })?;
        if let Some(existing) = identities.assets.iter().find(|candidate| {
            crate::AssetDescriptor::from_value((*candidate).clone())
                .is_some_and(|candidate| candidate.id == asset.id)
        }) {
            if existing != &raw {
                return Err(FlyError::InvalidAssetReference(format!(
                    "symbol `{symbol_id}` asset id `{}` conflicts with an existing asset",
                    asset.id
                )));
            }
        } else {
            identities.assets.push(raw);
            if identities.assets.len() > crate::DEFAULT_MAXIMUM_ASSETS {
                return Err(FlyError::MaximumAssetsExceeded {
                    maximum: crate::DEFAULT_MAXIMUM_ASSETS,
                });
            }
        }
    }
    for component in &cloned {
        crate::fragment::collect_component_asset_ids(
            component,
            &mut identities.asset_references,
        );
    }
    // A later instance must also see the actual HTML ids minted by this one.
    identities
        .page_dom
        .extend(crate::id_reference::materialized_dom_ids(cloned.iter()));
    for style in &descriptor.styles {
        let mut style = style.clone();
        remap_component_style_rule(&mut style, &mapping);
        pending_styles.push(style);
    }

    object.components = crate::ComponentChildren::Nodes(cloned);
    object.symbol_id = None;

    stack.push(symbol_id.to_string());
    if let Some(children) = object.children_mut() {
        for child in children.iter_mut() {
            expand_node(child, catalog, identities, stack, pending_styles)?;
        }
    }
    stack.pop();
    Ok(())
}

/// Ids of symbol definitions referenced by instances inside `components`.
pub fn symbol_references(components: &[ComponentNode]) -> Vec<String> {
    symbol_instance_facts_in_components(components)
        .into_iter()
        .filter_map(|instance| instance.symbol_id)
        .collect()
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
    for reference in symbol_references(&descriptor.components) {
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
        let mut project = GrapesProject {
            pages: vec![ProjectPage {
                id: Some("page-1".to_string()),
                component: Some(root),
                ..ProjectPage::default()
            }],
            ..GrapesProject::default()
        };
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
        assert_eq!(
            expanded_root.children()[0]
                .as_object()
                .expect("text")
                .id
                .as_deref(),
            Some("slot-a-cta-text")
        );

        let second = resolved.component("slot-b").expect("slot-b");
        let second_child = second.children()[0].as_object().expect("object");
        assert_eq!(second_child.id.as_deref(), Some("slot-b-cta-root"));

        // Style rules were re-attached with remapped component references and
        // without stale rule identity, so ids derive from the new component ids.
        let styles = &resolved.project().styles;
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
    fn symbol_instances_retarget_selector_only_styles_without_touching_provider_data() {
        let definition = json!({
            "id": "card", "components": [{"id": "title", "type": "text"}],
            "styles": [{"id": "title-style", "selectors": [{"name": "title", "type": 2}],
                        "style": {"color": "red"}, "providerData": {"component_id": "title"}}]
        });
        let document = document_with(
            object(
                "root",
                "wrapper",
                vec![instance("one", "card"), instance("two", "card")],
            ),
            vec![definition],
        );
        let resolved = resolve_symbol_instances(&document).expect("resolve");
        let catalog = crate::StyleRuleCatalog::from_document(&resolved);
        let one = catalog
            .component_rule("one-title", &crate::StyleRuleScope::Base)
            .expect("first style");
        let two = catalog
            .component_rule("two-title", &crate::StyleRuleScope::Base)
            .expect("second style");
        assert_ne!(one.id, two.id);
        assert_eq!(one.raw["selectors"][0]["name"], "one-title");
        assert_eq!(two.raw["selectors"][0]["name"], "two-title");
        assert_eq!(one.raw["providerData"]["component_id"], "title");
        let css = crate::render_page(
            &resolved,
            &crate::PageSelection::First,
            &crate::RenderPolicy::default(),
        )
        .unwrap()
        .css;
        assert!(
            css.contains("[data-fly-style-id=\"one-title\"]{color:red}"),
            "{css}"
        );
        assert!(
            css.contains("[data-fly-style-id=\"two-title\"]{color:red}"),
            "{css}"
        );
    }

    #[test]
    fn symbol_instances_reidentify_html_panels_and_actions_deterministically() {
        let definition = symbol_json(
            "faq",
            vec![json!({
                "id": "faq-root", "type": "container", "components": [
                    {"id": "trigger", "type": "button", "attributes": {"aria-controls": "answer"},
                     "flyAction": {"kind": "toggle_accordion", "target_id": "answer", "exclusive": true}},
                    {"id": "panel", "type": "container", "attributes": {"id": "answer"}}
                ]
            })],
        );
        let document = document_with(
            object(
                "root",
                "wrapper",
                vec![instance("one", "faq"), instance("two", "faq")],
            ),
            vec![definition],
        );
        let resolved = resolve_symbol_instances(&document).expect("resolve");
        let mut ids = Vec::new();
        for name in ["one", "two"] {
            let group = resolved.component(name).unwrap().children()[0]
                .as_object()
                .unwrap();
            let trigger = group.children()[0].as_object().unwrap();
            let panel = group.children()[1].as_object().unwrap();
            let id = panel.attributes["id"].as_str().unwrap();
            assert_eq!(trigger.extensions["flyAction"]["target_id"], id);
            assert_eq!(trigger.attributes["aria-controls"], id);
            ids.push(id.to_string());
        }
        assert_ne!(ids[0], ids[1]);
        assert_eq!(resolve_symbol_instances(&document).unwrap(), resolved);
    }

    #[test]
    fn symbol_resolution_terminates_for_long_instance_ids_and_existing_names() {
        let long = "x".repeat(crate::MAXIMUM_IDENTIFIER_LENGTH);
        let definition = symbol_json("title", vec![json!({"id": "child", "type": "text"})]);
        let document = document_with(
            object("root", "wrapper", vec![instance(&long, "title")]),
            vec![definition],
        );
        let resolved = resolve_symbol_instances(&document).expect("resolve");
        let child = resolved.component(&long).unwrap().children()[0]
            .as_object()
            .unwrap();
        assert!(validate_identifier(child.id.as_deref().unwrap()).is_ok());
        assert_ne!(child.id.as_deref(), Some(long.as_str()));
        assert_eq!(resolve_symbol_instances(&document).unwrap(), resolved);

        let short = document_with(
            object(
                "root",
                "wrapper",
                vec![
                    instance("slot", "title"),
                    object("slot-child", "text", vec![]),
                ],
            ),
            vec![symbol_json(
                "title",
                vec![json!({"id": "child", "type": "text"})],
            )],
        );
        let resolved = resolve_symbol_instances(&short).expect("resolve collision");
        let child = resolved.component("slot").unwrap().children()[0]
            .as_object()
            .unwrap();
        assert_ne!(child.id.as_deref(), Some("slot-child"));
        assert!(resolved.component("slot-child").is_some());
    }

    #[test]
    fn symbol_rejects_a_form_id_owned_by_another_html_node() {
        let definition = symbol_json(
            "form",
            vec![
                json!({"id": "form-node", "type": "form", "flyForm": {"id": "contact", "provider": "forms", "action": "submit"}}),
                json!({"id": "other", "type": "section", "attributes": {"id": "contact"}}),
            ],
        );
        let document = document_with(
            object("root", "wrapper", vec![instance("one", "form")]),
            vec![definition],
        );
        assert!(
            matches!(resolve_symbol_instances(&document), Err(FlyError::InvalidSymbolReference(message)) if message.contains("contact"))
        );
    }

    #[test]
    fn converting_a_form_with_matching_component_and_form_ids_still_resolves() {
        let document = crate::GrapesJsCodec::decode_value(json!({
            "pages": [{"component": {"id": "root", "type": "wrapper", "components": [
                {"id": "contact", "type": "form", "flyForm": {"id": "contact", "action_url": "/contact"}}
            ]}}]
        })).unwrap();
        let command =
            convert_component_to_symbol(&document, "contact", "contact-symbol", None).unwrap();
        let mut editor = crate::FlyEditor::new(document, crate::RegistrySet::with_builtins());
        editor.apply(command).expect("convert");
        let resolved = resolve_symbol_instances(editor.document())
            .expect("the wrapper component id is not a DOM collision");
        let materialized = crate::materialize_project_with_runtime_context(&resolved, &json!({}));
        assert!(
            materialized
                .diagnostics
                .iter()
                .all(|d| d.severity != crate::ValidationSeverity::Error),
            "{:?}",
            materialized.diagnostics
        );
        let html = crate::render_page(
            &materialized.document,
            &crate::PageSelection::First,
            &crate::RenderPolicy::default(),
        )
        .unwrap()
        .html;
        assert_eq!(html.matches(" id=\"contact\"").count(), 1, "{html}");
    }

    #[test]
    fn symbol_form_checks_effective_dom_ids_per_page() {
        let definition = symbol_json(
            "form",
            vec![json!({
                "id": "form-node", "type": "form", "flyForm": {"id": "contact", "action_url": "/contact"}
            })],
        );
        let mut separate = document_with(instance("contact", "form"), vec![definition.clone()]);
        separate.project_mut().pages.push(serde_json::from_value(json!({
            "id": "second", "component": {"id": "other", "type": "section", "attributes": {"id": "contact"}}
        })).unwrap());
        resolve_symbol_instances(&separate).expect("a DOM id on another page does not collide");

        let same = document_with(
            object(
                "root",
                "wrapper",
                vec![
                    instance("slot", "form"),
                    serde_json::from_value(
                        json!({"id": "other", "type": "section", "attributes": {"id": "contact"}}),
                    )
                    .unwrap(),
                ],
            ),
            vec![definition],
        );
        assert!(
            matches!(resolve_symbol_instances(&same), Err(FlyError::InvalidSymbolReference(message)) if message.contains("form id `contact`"))
        );
    }

    #[test]
    fn symbol_form_detects_dom_ids_generated_by_an_earlier_instance() {
        let document = document_with(
            object(
                "root",
                "wrapper",
                vec![instance("one", "section"), instance("two", "form")],
            ),
            vec![
                symbol_json(
                    "section",
                    vec![json!({"id": "body", "type": "section", "attributes": {"id": "anchor"}})],
                ),
                symbol_json(
                    "form",
                    vec![
                        json!({"id": "form-node", "type": "form", "flyForm": {"id": "one-anchor", "action_url": "/contact"}}),
                    ],
                ),
            ],
        );
        assert!(
            matches!(resolve_symbol_instances(&document), Err(FlyError::InvalidSymbolReference(message)) if message.contains("form id `one-anchor`"))
        );
    }

    #[test]
    fn symbol_form_identity_is_never_rewritten_or_duplicated() {
        let definition = symbol_json(
            "form",
            vec![json!({
                "id": "form-node", "type": "form",
                "flyForm": {"id": "contact", "method": "post", "provider": "forms", "action": "submit"}
            })],
        );
        let single = document_with(
            object("root", "wrapper", vec![instance("one", "form")]),
            vec![definition.clone()],
        );
        let resolved = resolve_symbol_instances(&single).expect("one form");
        let form = resolved.component("one").unwrap().children()[0]
            .as_object()
            .unwrap();
        assert_eq!(form.extensions["flyForm"]["id"], "contact");
        let duplicate = document_with(
            object(
                "root",
                "wrapper",
                vec![instance("one", "form"), instance("two", "form")],
            ),
            vec![definition],
        );
        assert!(
            matches!(resolve_symbol_instances(&duplicate), Err(FlyError::InvalidSymbolReference(message)) if message.contains("form id `contact`"))
        );
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
                symbol_json(
                    "a",
                    vec![json!({ "id": "a-1", "type": "symbol", "symbolId": "b" })],
                ),
                symbol_json(
                    "b",
                    vec![json!({ "id": "b-1", "type": "symbol", "symbolId": "a" })],
                ),
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
                symbol_json(
                    "inner",
                    vec![json!({ "id": "inner-1", "type": "text", "content": "in" })],
                ),
            ],
        );
        let resolved = resolve_symbol_instances(&nested).expect("resolve");
        let outer = resolved.component("slot").expect("slot");
        let outer_child = outer.children()[0].as_object().expect("outer-1");
        assert!(outer_child.symbol_id.is_none());
        assert_eq!(
            outer_child.children()[0]
                .as_object()
                .expect("inner-1")
                .id
                .as_deref(),
            Some("slot-outer-1-inner-1")
        );
    }

    #[test]
    fn resolves_symbol_instances_nested_in_object_valued_opaque_children() {
        let document = GrapesJsCodec::decode_value(json!({
            "flySymbols": [{
                "id": "cta",
                "components": [{
                    "id": "cta-root",
                    "type": "section",
                    "components": [{ "id": "cta-text", "type": "text", "content": "Buy" }]
                }]
            }],
            "pages": [{
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": {
                        "id": "opaque-parent",
                        "type": "custom-wrapper",
                        "components": {
                            "id": "slot",
                            "type": "symbol",
                            "symbolId": "cta",
                            "components": []
                        }
                    }
                }
            }]
        }))
        .expect("document");

        let resolved = resolve_symbol_instances(&document).expect("resolve");
        let root = resolved.project().pages[0]
            .component
            .as_ref()
            .and_then(ComponentNode::as_object)
            .expect("root");
        let crate::ComponentChildren::Opaque(parent) = &root.components else {
            panic!("object-valued children must remain opaque")
        };
        let expanded = &parent["components"];
        assert!(expanded.get("symbolId").is_none());
        assert_eq!(expanded["components"][0]["id"], "slot-cta-root");
        assert_eq!(expanded["components"][0]["components"][0]["id"], "slot-cta-text");
    }

    #[test]
    fn resolves_symbol_instances_inside_opaque_array_component_roots() {
        let document = GrapesJsCodec::decode_value(json!({
            "flySymbols": [{
                "id": "cta",
                "components": [{ "id": "cta-root", "type": "section" }]
            }],
            "pages": [{
                "component": [{
                    "id": "slot",
                    "type": "symbol",
                    "symbolId": "cta",
                    "components": []
                }]
            }]
        }))
        .expect("document");

        let resolved = resolve_symbol_instances(&document).expect("resolve");
        let Some(ComponentNode::Opaque(Value::Array(children))) =
            resolved.project().pages[0].component.as_ref()
        else {
            panic!("opaque array root must remain an array")
        };
        assert!(children[0].get("symbolId").is_none());
        assert_eq!(children[0]["components"][0]["id"], "slot-cta-root");
    }

    #[test]
    fn symbol_cycles_include_references_inside_opaque_children() {
        let document = GrapesJsCodec::decode_value(json!({
            "flySymbols": [
                {
                    "id": "outer",
                    "components": [{
                        "id": "opaque-parent",
                        "type": "custom-wrapper",
                        "components": {
                            "id": "inner-slot",
                            "type": "symbol",
                            "symbolId": "inner",
                            "components": []
                        }
                    }]
                },
                {
                    "id": "inner",
                    "components": [{
                        "id": "outer-slot",
                        "type": "symbol",
                        "symbolId": "outer",
                        "components": []
                    }]
                }
            ],
            "pages": [{
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "slot",
                        "type": "symbol",
                        "symbolId": "outer",
                        "components": []
                    }]
                }
            }]
        }))
        .expect("document");

        assert!(matches!(
            resolve_symbol_instances(&document),
            Err(FlyError::SymbolCycle(_))
        ));
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
            document_with(
                object("root", "wrapper", vec![instance("slot", "cta")]),
                vec![symbol_json("cta", vec![plain_component_json("cta-1")])],
            ),
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
    fn remove_fails_closed_for_instances_inside_opaque_children() {
        let document = GrapesJsCodec::decode_value(json!({
            "flySymbols": [{
                "id": "cta",
                "components": [{ "id": "cta-1", "type": "text" }]
            }],
            "pages": [{
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": {
                        "id": "opaque-parent",
                        "type": "custom-wrapper",
                        "components": {
                            "id": "nested-slot",
                            "type": "symbol",
                            "symbolId": "cta",
                            "components": []
                        }
                    }
                }
            }]
        }))
        .expect("document");
        let mut editor = FlyEditor::new(document, RegistrySet::with_builtins());

        let error = editor
            .apply(EditorCommand::Symbol {
                command: SymbolCommand::Remove {
                    symbol_id: "cta".to_string(),
                },
            })
            .expect_err("opaque instance must prevent definition removal");

        assert!(matches!(
            error,
            FlyError::InvalidSymbolReference(message)
                if message.contains("cannot remove symbol `cta`")
        ));
    }

    #[test]
    fn conversion_replaces_selection_with_shared_instance_and_is_undoable() {
        let document = document_with(
            object(
                "root",
                "wrapper",
                vec![object(
                    "cta",
                    "section",
                    vec![object("label", "text", vec![])],
                )],
            ),
            Vec::new(),
        );
        let mut editor = FlyEditor::new(document.clone(), RegistrySet::with_builtins());
        let command = convert_component_to_symbol(
            &document,
            "cta",
            "site-cta",
            Some("Call to action".into()),
        )
        .expect("convert command");
        editor.apply(command).expect("convert");
        assert_eq!(
            editor
                .document()
                .component("cta")
                .unwrap()
                .symbol_id
                .as_deref(),
            Some("site-cta")
        );
        assert!(editor.document().component("label").is_none());
        assert_eq!(
            SymbolDescriptor::catalog_from_document(editor.document())["site-cta"]
                .name
                .as_deref(),
            Some("Call to action")
        );
        let resolved = resolve_symbol_instances(editor.document()).expect("resolve");
        assert!(resolved.component("cta-cta").is_some());
        assert!(resolved.component("cta-label").is_some());
        editor.undo().expect("undo");
        assert_eq!(editor.document(), &document);
    }

    #[test]
    fn invalid_catalog_block_fails_closed() {
        let mut document = document_with(object("root", "wrapper", vec![]), Vec::new());
        document.project_mut().extensions.insert(
            FLY_SYMBOLS_FIELD.to_string(),
            serde_json::json!({"bad": true}),
        );
        assert!(matches!(
            resolve_symbol_instances(&document),
            Err(FlyError::InvalidSymbolReference(_))
        ));
        let report = validate_project(
            &document,
            &RegistrySet::with_builtins(),
            ValidationLimits::default(),
        );
        assert!(
            report
                .diagnostics
                .iter()
                .any(|item| item.code == "malformed_symbol_catalog")
        );
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

    #[test]
    fn symbol_conversion_rejects_runtime_catalogs_it_cannot_preserve() {
        let document = GrapesJsCodec::decode_value(json!({
            "flyRuntimeBindings": [{
                "id": "binding-1",
                "component_id": "hero",
                "path": "user.name",
                "target": "attribute",
                "name": "aria-label"
            }],
            "pages": [{
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{ "id": "hero", "type": "section" }]
                }
            }]
        }))
        .expect("document");
        let before = document.clone();

        let error = convert_component_to_symbol(&document, "hero", "cta", None)
            .expect_err("conversion must not silently discard runtime bindings");

        assert!(matches!(
            error,
            FlyError::InvalidSymbolReference(message) if message.contains("runtime bindings")
        ));
        assert_eq!(document, before);
    }

    #[test]
    fn symbol_expansion_rejects_opaque_instance_children() {
        let instance = ComponentNode::Object(Box::new(ComponentObject {
            id: Some("slot".to_string()),
            component_type: Some("symbol".to_string()),
            symbol_id: Some("cta".to_string()),
            components: crate::ComponentChildren::Opaque(json!({
                "id": "opaque-child"
            })),
            ..ComponentObject::default()
        }));
        let document = document_with(
            object("root", "wrapper", vec![instance]),
            vec![symbol_json("cta", vec![plain_component_json("cta-1")])],
        );

        let error = resolve_symbol_instances(&document).expect_err(
            "opaque children must not be silently overwritten by symbol expansion",
        );
        assert!(matches!(
            error,
            FlyError::InvalidSymbolReference(message) if message.contains("child-free")
        ));
    }

    #[test]
    fn symbol_definition_byte_limit_includes_unknown_extensions() {
        let raw = json!({
            "id": "large-symbol",
            "components": [{ "id": "root", "type": "wrapper" }],
            "unknownExtension": "x".repeat(MAX_SYMBOL_DEFINITION_BYTES + 1)
        });

        assert!(SymbolDescriptor::from_value(&raw).is_none());
        assert!(symbol_definition_size_exceeds_limit(&raw));
    }

    #[test]
    fn symbol_definition_rejects_malformed_opaque_symbol_markers() {
        let raw = json!({
            "id": "outer",
            "components": [{
                "id": "opaque-parent",
                "type": "custom-wrapper",
                "components": {
                    "id": "nested-slot",
                    "type": "symbol",
                    "symbolId": 42,
                    "components": []
                }
            }]
        });
        let descriptor = SymbolDescriptor::from_value(&raw).expect("descriptor");

        assert!(matches!(
            validate_definition(&descriptor),
            Err(FlyError::InvalidSymbolReference(message))
                if message.contains("non-string symbolId")
        ));
    }

    #[test]
    fn symbol_assets_are_materialized_and_shared_by_all_instances() {
        let document = GrapesJsCodec::decode_value(json!({
            "flySymbols": [{
                "id": "gallery",
                "assets": [{ "id": "gallery-image", "src": "/media/gallery.webp" }],
                "components": [{
                    "id": "image",
                    "type": "image",
                    "attributes": {
                        "src": "/media/gallery.webp",
                        "data-fly-asset-id": "gallery-image"
                    }
                }]
            }],
            "pages": [{
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [
                        { "id": "slot-a", "type": "symbol", "symbolId": "gallery", "components": [] },
                        { "id": "slot-b", "type": "symbol", "symbolId": "gallery", "components": [] }
                    ]
                }
            }]
        }))
        .expect("document");

        let resolved = resolve_symbol_instances(&document).expect("resolve");
        let assets = resolved.project().assets.iter()
            .filter_map(|raw| AssetDescriptor::from_value(raw.clone()))
            .collect::<Vec<_>>();
        assert_eq!(
            assets.iter().filter(|asset| asset.source == "/media/gallery.webp").count(),
            1
        );
        let first = resolved.component("slot-a").expect("first instance");
        let second = resolved.component("slot-b").expect("second instance");
        assert_eq!(
            first.children()[0].as_object().expect("first image").attributes["data-fly-asset-id"],
            "gallery-image"
        );
        assert_eq!(
            second.children()[0].as_object().expect("second image").attributes["data-fly-asset-id"],
            "gallery-image"
        );
    }

    #[test]
    fn symbol_asset_collisions_preserve_page_assets_and_dangling_references() {
        let document = GrapesJsCodec::decode_value(json!({
            "assets": [{ "id": "shared", "src": "/media/page.webp" }],
            "flySymbols": [{
                "id": "gallery",
                "assets": [
                    { "id": "shared", "src": "/media/symbol.webp" },
                    { "id": "dangling", "src": "/media/dangling.webp" }
                ],
                "components": [
                    {
                        "id": "shared-image",
                        "type": "image",
                        "attributes": {
                            "src": "/media/symbol.webp",
                            "data-fly-asset-id": "shared"
                        }
                    },
                    {
                        "id": "dangling-image",
                        "type": "image",
                        "attributes": {
                            "src": "/media/dangling.webp",
                            "data-fly-asset-id": "dangling"
                        }
                    }
                ]
            }],
            "pages": [{
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [
                        {
                            "id": "legacy-shared",
                            "type": "image",
                            "attributes": { "data-fly-asset-id": "shared" }
                        },
                        {
                            "id": "legacy-dangling",
                            "type": "image",
                            "attributes": { "data-fly-asset-id": "dangling" }
                        },
                        { "id": "slot-a", "type": "symbol", "symbolId": "gallery", "components": [] },
                        { "id": "slot-b", "type": "symbol", "symbolId": "gallery", "components": [] }
                    ]
                }
            }]
        }))
        .expect("document");

        let resolved = resolve_symbol_instances(&document).expect("resolve");
        let page_asset = AssetDescriptor::from_value(resolved.project().assets[0].clone())
            .expect("page asset");
        assert_eq!(page_asset.id, "shared");
        assert_eq!(page_asset.source, "/media/page.webp");

        let first = resolved.component("slot-a").expect("first instance");
        let second = resolved.component("slot-b").expect("second instance");
        let first_shared = first.children()[0].as_object().expect("first shared image")
            .attributes["data-fly-asset-id"].as_str().expect("asset id");
        let first_dangling = first.children()[1].as_object().expect("first dangling image")
            .attributes["data-fly-asset-id"].as_str().expect("asset id");
        let second_shared = second.children()[0].as_object().expect("second shared image")
            .attributes["data-fly-asset-id"].as_str().expect("asset id");
        let second_dangling = second.children()[1].as_object().expect("second dangling image")
            .attributes["data-fly-asset-id"].as_str().expect("asset id");

        assert_ne!(first_shared, "shared");
        assert_ne!(first_dangling, "dangling");
        assert_eq!(first_shared, second_shared);
        assert_eq!(first_dangling, second_dangling);
        assert_eq!(
            resolved.component("legacy-shared").and_then(|component| {
                component
                    .attributes
                    .get("data-fly-asset-id")
                    .and_then(Value::as_str)
            }),
            Some("shared")
        );
        assert_eq!(
            resolved.component("legacy-dangling").and_then(|component| {
                component
                    .attributes
                    .get("data-fly-asset-id")
                    .and_then(Value::as_str)
            }),
            Some("dangling")
        );
        assert!(
            resolved.project().assets.iter()
                .filter_map(|raw| AssetDescriptor::from_value(raw.clone()))
                .any(|asset| asset.id == first_shared && asset.source == "/media/symbol.webp")
        );
        assert!(
            resolved.project().assets.iter()
                .filter_map(|raw| AssetDescriptor::from_value(raw.clone()))
                .any(|asset| asset.id == first_dangling && asset.source == "/media/dangling.webp")
        );
    }

    #[test]
    fn symbol_assets_do_not_claim_dangling_ids_from_other_definitions() {
        let document = GrapesJsCodec::decode_value(json!({
            "flySymbols": [
                {
                    "id": "gallery",
                    "assets": [{ "id": "shared", "src": "/media/gallery.webp" }],
                    "components": [{
                        "id": "gallery-image",
                        "type": "image",
                        "attributes": {
                            "src": "/media/gallery.webp",
                            "data-fly-asset-id": "shared"
                        }
                    }]
                },
                {
                    "id": "legacy",
                    "components": [{
                        "id": "legacy-image",
                        "type": "image",
                        "attributes": { "data-fly-asset-id": "shared" }
                    }]
                }
            ],
            "pages": [{
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [
                        { "id": "gallery-slot", "type": "symbol", "symbolId": "gallery", "components": [] },
                        { "id": "legacy-slot", "type": "symbol", "symbolId": "legacy", "components": [] }
                    ]
                }
            }]
        }))
        .expect("document");

        let resolved = resolve_symbol_instances(&document).expect("resolve");
        let gallery = resolved.component("gallery-slot").expect("gallery");
        let gallery_asset_id = gallery.children()[0].as_object().expect("image")
            .attributes["data-fly-asset-id"].as_str().expect("asset id");
        let legacy = resolved.component("legacy-slot").expect("legacy");
        let legacy_asset_id = legacy.children()[0].as_object().expect("image")
            .attributes["data-fly-asset-id"].as_str().expect("asset id");

        assert_ne!(gallery_asset_id, "shared");
        assert_eq!(legacy_asset_id, "shared");
        assert!(
            resolved.project().assets.iter()
                .filter_map(|raw| AssetDescriptor::from_value(raw.clone()))
                .any(|asset| asset.id == gallery_asset_id && asset.source == "/media/gallery.webp")
        );
        assert!(
            !resolved.project().assets.iter()
                .filter_map(|raw| AssetDescriptor::from_value(raw.clone()))
                .any(|asset| asset.id == "shared")
        );
    }

    #[test]
    fn symbol_expansion_respects_the_project_wide_node_budget() {
        let mut children = (0..crate::DEFAULT_MAXIMUM_NODES - 2)
            .map(|index| {
                json!({
                    "id": format!("existing-{index}"),
                    "type": "section"
                })
            })
            .collect::<Vec<_>>();
        children.push(json!({
            "id": "slot",
            "type": "symbol",
            "symbolId": "large",
            "components": []
        }));
        let document = GrapesJsCodec::decode_value(json!({
            "flySymbols": [{
                "id": "large",
                "components": [
                    { "id": "added-a", "type": "section" },
                    { "id": "added-b", "type": "section" }
                ]
            }],
            "pages": [{
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": children
                }
            }]
        }))
        .expect("document");

        assert!(matches!(
            resolve_symbol_instances(&document),
            Err(FlyError::MaximumNodesExceeded {
                maximum: crate::DEFAULT_MAXIMUM_NODES
            })
        ));
    }

    #[test]
    fn symbol_component_limit_counts_opaque_children() {
        let opaque_children = (0..MAX_SYMBOL_COMPONENTS)
            .map(|index| json!({ "id": format!("opaque-{index}") }))
            .collect::<Vec<_>>();
        let descriptor = SymbolDescriptor {
            id: "large-symbol".to_string(),
            name: None,
            components: vec![ComponentNode::Object(Box::new(ComponentObject {
                id: Some("root".to_string()),
                component_type: Some("wrapper".to_string()),
                components: crate::ComponentChildren::Opaque(Value::Object(
                    serde_json::Map::from_iter([
                        ("id".to_string(), json!("opaque-parent")),
                        ("components".to_string(), json!(opaque_children)),
                    ]),
                )),
                ..ComponentObject::default()
            }))],
            styles: Vec::new(),
            assets: Vec::new(),
        };

        assert!(matches!(
            validate_definition(&descriptor),
            Err(FlyError::InvalidSymbolReference(message))
                if message.contains("exceeds 512 components")
        ));
    }

    #[test]
    fn deeply_nested_symbol_values_are_rejected_before_deserialization() {
        let mut raw = json!({ "id": "deep", "components": [] });
        for _ in 0..=crate::MAXIMUM_DECODE_DEPTH {
            raw = json!({ "nested": raw });
        }
        assert!(SymbolDescriptor::from_value(&raw).is_none());
    }

    #[test]
    fn direct_symbol_upsert_does_not_overwrite_a_malformed_catalog() {
        let mut document = document_with(object("root", "wrapper", vec![]), Vec::new());
        document
            .project_mut()
            .extensions
            .insert(FLY_SYMBOLS_FIELD.to_string(), json!({"unexpected": true}));
        let before = document.clone();

        let error = apply_symbol_command(
            &mut document,
            &SymbolCommand::Upsert {
                symbol: symbol_json("cta", vec![plain_component_json("cta-1")]),
            },
        )
        .expect_err("malformed source catalog must be preserved");

        assert!(matches!(error, FlyError::InvalidSymbolReference(_)));
        assert_eq!(document, before);
    }

    #[test]
    fn symbol_upsert_does_not_overwrite_colliding_page_assets() {
        let mut document = document_with(object("root", "wrapper", vec![]), Vec::new());
        document.project_mut().assets = vec![
            json!({ "id": "shared", "src": "/media/page.webp" }),
        ];
        let symbol = json!({
            "id": "gallery",
            "assets": [{ "id": "shared", "src": "/media/symbol.webp" }],
            "components": [{
                "id": "image",
                "type": "image",
                "attributes": {
                    "src": "/media/symbol.webp",
                    "data-fly-asset-id": "shared"
                }
            }]
        });

        apply_symbol_command(
            &mut document,
            &SymbolCommand::Upsert { symbol },
        )
        .expect("upsert");

        let page_asset = AssetDescriptor::from_value(document.project().assets[0].clone())
            .expect("page asset");
        assert_eq!(page_asset.id, "shared");
        assert_eq!(page_asset.source, "/media/page.webp");

        let descriptor = SymbolDescriptor::catalog_from_document(&document)
            .remove("gallery")
            .expect("stored symbol");
        let symbol_asset = AssetDescriptor::from_value(descriptor.assets[0].clone())
            .expect("symbol asset");
        assert_ne!(symbol_asset.id, "shared");
        assert_eq!(symbol_asset.source, "/media/symbol.webp");
        assert_eq!(
            descriptor.components[0]
                .as_object()
                .expect("image")
                .attributes
                .get("data-fly-asset-id")
                .and_then(Value::as_str),
            Some(symbol_asset.id.as_str())
        );
        assert!(
            document.project().assets.iter()
                .filter_map(|raw| AssetDescriptor::from_value(raw.clone()))
                .any(|asset| asset.id == symbol_asset.id && asset.source == "/media/symbol.webp")
        );
    }

    #[test]
    fn direct_symbol_upsert_can_repair_a_missing_nested_definition() {
        let mut document = document_with(object("root", "wrapper", vec![]), Vec::new());
        document.project_mut().extensions.insert(
            FLY_SYMBOLS_FIELD.to_string(),
            json!([{
                "id": "outer",
                "components": [{
                    "id": "inner-slot",
                    "type": "symbol",
                    "symbolId": "inner",
                    "components": []
                }]
            }]),
        );

        apply_symbol_command(
            &mut document,
            &SymbolCommand::Upsert {
                symbol: symbol_json("inner", vec![plain_component_json("inner-copy")]),
            },
        )
        .expect("upsert should repair the missing definition");

        let catalog = SymbolDescriptor::catalog_from_document(&document);
        assert!(catalog.contains_key("outer"));
        assert!(catalog.contains_key("inner"));
        assert!(symbol_references(&catalog["outer"].components).contains(&"inner".to_string()));
    }

    #[test]
    fn direct_symbol_upsert_rejects_missing_nested_references_atomically() {
        let mut document = document_with(object("root", "wrapper", vec![]), Vec::new());
        let before = document.clone();

        let error = apply_symbol_command(
            &mut document,
            &SymbolCommand::Upsert {
                symbol: symbol_json(
                    "outer",
                    vec![json!({
                        "id": "nested-slot",
                        "type": "symbol",
                        "symbolId": "missing",
                        "components": []
                    })],
                ),
            },
        )
        .expect_err("direct upsert must reject dangling nested references");

        assert!(matches!(error, FlyError::SymbolNotFound(message) if message.contains("missing")));
        assert_eq!(document, before);
    }

    #[test]
    fn direct_symbol_commands_invalidate_cached_hashes() {
        let mut document = document_with(object("root", "wrapper", vec![]), Vec::new());
        let before = document.hash();
        apply_symbol_command(
            &mut document,
            &SymbolCommand::Upsert {
                symbol: symbol_json("cta", vec![plain_component_json("cta-1")]),
            },
        )
        .expect("upsert symbol");
        assert_ne!(document.hash(), before);
    }

    #[test]
    fn failed_symbol_upsert_does_not_leave_asset_or_definition_mutations() {
        let existing = json!({
            "id": "symbol-b",
            "components": [{
                "id": "symbol-b-root",
                "type": "wrapper",
                "symbolId": "symbol-a"
            }]
        });
        let mut document = document_with(object("root", "wrapper", vec![]), vec![existing]);
        let before = document.clone();
        let before_hash = document.hash();
        let mut symbol = symbol_json(
            "symbol-a",
            vec![json!({
                "id": "symbol-a-root",
                "type": "wrapper",
                "symbolId": "symbol-b"
            })],
        );
        symbol["assets"] = json!([{
            "id": "symbol-asset",
            "src": "/assets/symbol.png"
        }]);

        let error = apply_symbol_command(&mut document, &SymbolCommand::Upsert { symbol })
            .expect_err("cycle must be rejected");
        assert!(matches!(error, FlyError::SymbolCycle(_)));
        assert_eq!(document, before);
        assert_eq!(document.hash(), before_hash);
        assert!(document.project().assets.is_empty());
        assert!(!SymbolDescriptor::catalog_from_document(&document).contains_key("symbol-a"));
    }
}
