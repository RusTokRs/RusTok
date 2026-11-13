use crate::id_reference::{remap_action_ids, remap_attribute_ids, remap_runtime_catalogs};
use crate::{ComponentIndex, FlyError, FlyResult, IdGenerator, ProjectHash, validate_identifier};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectDocument {
    pub project: GrapesProject,
    #[serde(skip)]
    pub(crate) component_index: std::sync::OnceLock<ComponentIndex>,
    #[serde(skip)]
    pub(crate) cached_hash: std::sync::OnceLock<ProjectHash>,
}

impl Clone for ProjectDocument {
    fn clone(&self) -> Self {
        Self {
            project: self.project.clone(),
            component_index: std::sync::OnceLock::new(),
            cached_hash: std::sync::OnceLock::new(),
        }
    }
}

impl PartialEq for ProjectDocument {
    fn eq(&self, other: &Self) -> bool {
        self.project == other.project
    }
}

impl Eq for ProjectDocument {}

impl ProjectDocument {
    pub fn new(project: GrapesProject) -> Self {
        Self {
            project,
            component_index: std::sync::OnceLock::new(),
            cached_hash: std::sync::OnceLock::new(),
        }
    }

    pub fn component_index(&self) -> &ComponentIndex {
        self.component_index
            .get_or_init(|| ComponentIndex::build(self))
    }

    pub fn hash(&self) -> ProjectHash {
        *self
            .cached_hash
            .get_or_init(|| ProjectHash::from_document(self))
    }

    pub fn component(&self, id: &str) -> Option<&ComponentObject> {
        self.component_index().get_component(self, id)
    }

    pub fn invalidate_caches(&mut self) {
        self.cached_hash.take();
        self.component_index.take();
    }

    pub fn component_mut(&mut self, id: &str) -> Option<&mut ComponentObject> {
        self.invalidate_caches();
        self.project.component_mut(id)
    }

    pub fn contains_component(&self, id: &str) -> bool {
        self.component(id).is_some()
    }

    /// Read the declared document kind (`page`, `layout_template`, `section_symbol`, `starter_preset`).
    pub fn document_kind(&self) -> DocumentKind {
        self.project
            .extensions
            .get(FLY_DOCUMENT_KIND_FIELD)
            .and_then(|val| serde_json::from_value(val.clone()).ok())
            .unwrap_or_default()
    }

    /// Update the declared document kind.
    pub fn set_document_kind(&mut self, kind: DocumentKind) {
        self.invalidate_caches();
        self.project.extensions.insert(
            FLY_DOCUMENT_KIND_FIELD.to_string(),
            serde_json::to_value(kind).unwrap_or(Value::String("page".to_string())),
        );
    }

    /// Clone a template or preset into an independent Page, reissuing component/HTML IDs and
    /// their owned references. Provider form and asset identities remain owned by their providers.
    /// Ambiguous source identities are rejected before the generator is consumed.
    pub fn clone_as_page(&self, generator: &mut impl IdGenerator) -> FlyResult<Self> {
        let mut source_ids = Vec::new();
        let mut dom_ids = Vec::new();
        let mut form_ids = Vec::new();
        for page in &self.project.pages {
            if let Some(component) = &page.component {
                if let Some(reason) =
                    crate::id_reference::ambiguous_tree_identity(std::iter::once(component))
                {
                    return Err(FlyError::Decode(format!(
                        "cannot clone an ambiguous document: {reason}"
                    )));
                }
                component.collect_ids(&mut source_ids);
                component.collect_dom_ids(&mut dom_ids);
                component.collect_form_ids(&mut form_ids);
            }
        }
        // Component and provider-form identities are document-wide, but authored HTML ids live
        // in separate rendered pages and may legitimately repeat between them.
        for (kind, ids) in [("component", &source_ids), ("form", &form_ids)] {
            let mut seen = BTreeSet::new();
            if let Some(id) = ids.iter().find(|id| !seen.insert(*id)) {
                return Err(FlyError::Decode(format!(
                    "cannot clone an ambiguous document: duplicate {kind} id `{id}`"
                )));
            }
        }
        let mut cloned = self.clone();
        let mut reserved = source_ids
            .iter()
            .chain(&dom_ids)
            .chain(&form_ids)
            .cloned()
            .collect();
        let mapping = crate::id_reference::fresh_component_id_mapping(
            source_ids,
            "comp",
            &mut reserved,
            generator,
        );
        for page in &mut cloned.project.pages {
            if let Some(component) = &mut page.component {
                let mut page_dom_ids = Vec::new();
                let mut page_form_ids = Vec::new();
                let mut matching_ids = BTreeSet::new();
                component.collect_dom_ids(&mut page_dom_ids);
                component.collect_form_ids(&mut page_form_ids);
                component.collect_matching_dom_ids(&mut matching_ids);
                let dom_mapping = crate::id_reference::fresh_dom_id_mapping(
                    page_dom_ids,
                    &matching_ids,
                    &page_form_ids.into_iter().collect(),
                    &mapping,
                    &mut reserved,
                    generator,
                );
                component.remap_ids_with_dom(&mapping, &dom_mapping);
            }
        }
        for style in &mut cloned.project.styles {
            crate::style_rule::remap_component_style_rule(style, &mapping);
        }
        remap_runtime_catalogs(&mut cloned.project.extensions, &mapping);
        cloned.set_document_kind(DocumentKind::Page);
        cloned.invalidate_caches();
        Ok(cloned)
    }

    /// Give every component a unique, syntactically valid id, minting new ones where needed.
    ///
    /// Ids that fail [`validate_identifier`] are replaced rather than preserved. An id is
    /// interpolated into HTML attributes and into CSS selectors inside a raw `<style>` element, so
    /// a malformed one is a liability; keeping it would also deadlock the editor, because
    /// validation rejects it and every command runs validation. Self-healing here mirrors how
    /// duplicate ids have always been handled.
    pub fn ensure_stable_ids(&mut self, generator: &mut impl IdGenerator) {
        // Fast path: a document whose ids are all present, valid, and unique needs no
        // repair. This runs on every editor command, so it borrows ids instead of
        // cloning them into sets, and skips the mutating walk entirely — the common
        // case for patch/style commands on an already-stable document.
        if self.all_component_ids_stable() {
            return;
        }
        self.invalidate_caches();
        // Opaque component subtrees also participate in validation, so generated ids must not
        // collide with them (or with authored/provider DOM identities).
        let mut reserved = crate::id_reference::project_identity_ids(self);
        let mut seen = BTreeSet::new();
        self.project
            .ensure_stable_ids(generator, &mut reserved, &mut seen);
    }
}

impl ProjectDocument {
    fn all_component_ids_stable(&self) -> bool {
        let mut seen = std::collections::HashSet::new();
        for page in &self.project.pages {
            let Some(root) = page.component.as_ref() else {
                return false;
            };
            if !check_component_ids_stable(root, &mut seen) {
                return false;
            }
        }
        true
    }
}

fn check_component_ids_stable<'a>(
    node: &'a ComponentNode,
    seen: &mut std::collections::HashSet<&'a str>,
) -> bool {
    let Some(object) = node.as_object() else {
        return false;
    };
    let Some(id) = object.id() else {
        return false;
    };
    if validate_identifier(id).is_err() || !seen.insert(id) {
        return false;
    }
    for child in object.children() {
        if !check_component_ids_stable(child, seen) {
            return false;
        }
    }
    true
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

// Opaque component objects can still contain the well-defined `id`, `attributes`, `flyForm`,
// and `components` slots. Visit only that structural path, not arbitrary provider payloads.
fn visit_opaque_node(value: &Value, visitor: &mut impl FnMut(&Map<String, Value>)) {
    let Some(object) = value.as_object() else {
        return;
    };
    visitor(object);
    if let Some(children) = object.get("components").and_then(Value::as_array) {
        for child in children {
            visit_opaque_node(child, visitor);
        }
    }
}

fn visit_opaque_children(value: &Value, visitor: &mut impl FnMut(&Map<String, Value>)) {
    if let Some(children) = value.as_array() {
        for child in children {
            visit_opaque_node(child, visitor);
        }
    } else {
        visit_opaque_node(value, visitor);
    }
}

fn remap_opaque_node(
    value: &mut Value,
    components: &BTreeMap<String, String>,
    dom: &BTreeMap<String, String>,
    attribute_ids: &BTreeMap<String, String>,
    action_ids: &BTreeMap<String, String>,
) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    for (key, value) in object {
        match key.as_str() {
            "id" => {
                if let Value::String(id) = value
                    && let Some(replacement) = components.get(id)
                {
                    *id = replacement.clone();
                }
            }
            "attributes" => {
                if let Value::Object(attributes) = value {
                    for (name, value) in attributes.iter_mut() {
                        if name.eq_ignore_ascii_case("id")
                            && let Value::String(id) = value
                            && let Some(replacement) = dom.get(id)
                        {
                            *id = replacement.clone();
                        }
                    }
                    remap_attribute_ids(attributes, attribute_ids);
                }
            }
            "components" => {
                if let Value::Array(children) = value {
                    for child in children {
                        remap_opaque_node(child, components, dom, attribute_ids, action_ids);
                    }
                }
            }
            crate::FLY_ACTION_FIELD => remap_action_ids(value, action_ids),
            _ => {}
        }
    }
}

fn remap_opaque_children(
    value: &mut Value,
    components: &BTreeMap<String, String>,
    dom: &BTreeMap<String, String>,
    attribute_ids: &BTreeMap<String, String>,
    action_ids: &BTreeMap<String, String>,
) {
    if let Value::Array(children) = value {
        for child in children {
            remap_opaque_node(child, components, dom, attribute_ids, action_ids);
        }
    } else {
        remap_opaque_node(value, components, dom, attribute_ids, action_ids);
    }
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

    pub(crate) fn ensure_stable_ids(
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
            let id = crate::id_reference::allocate_generated_id(generator, hint, reserved);
            seen.insert(id.clone());
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
        match self {
            Self::Object(object) => {
                if let Some(id) = &object.id {
                    ids.push(id.clone());
                }
                match &object.components {
                    ComponentChildren::Nodes(children) => {
                        for child in children {
                            child.collect_ids(ids);
                        }
                    }
                    ComponentChildren::Opaque(value) => {
                        visit_opaque_children(value, &mut |child| {
                            if let Some(id) = child.get("id").and_then(Value::as_str) {
                                ids.push(id.to_string());
                            }
                        })
                    }
                }
            }
            Self::Opaque(value) => visit_opaque_node(value, &mut |object| {
                if let Some(id) = object.get("id").and_then(Value::as_str) {
                    ids.push(id.to_string());
                }
            }),
        }
    }

    /// Collect authored HTML ids separately from Fly component ids. They are distinct namespaces:
    /// an action may reference either, and an author may supply a DOM id different from its node.
    pub(crate) fn collect_dom_ids(&self, ids: &mut Vec<String>) {
        fn collect(attributes: &Map<String, Value>, ids: &mut Vec<String>) {
            for (name, value) in attributes {
                if name.eq_ignore_ascii_case("id")
                    && let Some(id) = value.as_str().filter(|id| !id.is_empty())
                {
                    ids.push(id.to_string());
                }
            }
        }
        match self {
            Self::Object(object) => {
                collect(&object.attributes, ids);
                match &object.components {
                    ComponentChildren::Nodes(children) => {
                        for child in children {
                            child.collect_dom_ids(ids);
                        }
                    }
                    ComponentChildren::Opaque(value) => {
                        visit_opaque_children(value, &mut |child| {
                            if let Some(attributes) =
                                child.get("attributes").and_then(Value::as_object)
                            {
                                collect(attributes, ids);
                            }
                        })
                    }
                }
            }
            Self::Opaque(value) => visit_opaque_node(value, &mut |object| {
                if let Some(attributes) = object.get("attributes").and_then(Value::as_object) {
                    collect(attributes, ids);
                }
            }),
        }
    }

    /// HTML ids that belong to the very component with the same Fly id. Merely sharing the
    /// string with a different component does not make them the same identity.
    pub(crate) fn collect_matching_dom_ids(&self, ids: &mut BTreeSet<String>) {
        fn collect(
            id: Option<&str>,
            attributes: Option<&Map<String, Value>>,
            ids: &mut BTreeSet<String>,
        ) {
            if let (Some(id), Some(attributes)) = (id, attributes)
                && attributes.iter().any(|(name, value)| {
                    name.eq_ignore_ascii_case("id") && value.as_str() == Some(id)
                })
            {
                ids.insert(id.to_string());
            }
        }
        match self {
            Self::Object(object) => {
                collect(object.id.as_deref(), Some(&object.attributes), ids);
                match &object.components {
                    ComponentChildren::Nodes(children) => {
                        for child in children {
                            child.collect_matching_dom_ids(ids);
                        }
                    }
                    ComponentChildren::Opaque(value) => {
                        visit_opaque_children(value, &mut |child| {
                            collect(
                                child.get("id").and_then(Value::as_str),
                                child.get("attributes").and_then(Value::as_object),
                                ids,
                            );
                        })
                    }
                }
            }
            Self::Opaque(value) => visit_opaque_node(value, &mut |object| {
                collect(
                    object.get("id").and_then(Value::as_str),
                    object.get("attributes").and_then(Value::as_object),
                    ids,
                );
            }),
        }
    }

    /// Collect authored DOM and form identities by node. Rendering an unmaterialized draft
    /// conservatively reserves both; opaque children use only structural paths.
    pub(crate) fn collect_form_dom_owners(&self, owners: &mut Vec<(Option<String>, Vec<String>)>) {
        self.collect_form_dom_owners_inner(owners, false);
    }

    /// Collect the identities that a materialized page will emit. A valid, non-conflicting
    /// `flyForm` replaces its authored HTML id with its own form id; invalid or opaque nodes
    /// remain conservative because Fly cannot lower their form contracts.
    pub(crate) fn collect_materialized_form_dom_owners(
        &self,
        owners: &mut Vec<(Option<String>, Vec<String>)>,
    ) {
        self.collect_form_dom_owners_inner(owners, true);
    }

    fn collect_form_dom_owners_inner(
        &self,
        owners: &mut Vec<(Option<String>, Vec<String>)>,
        materialized: bool,
    ) {
        fn record(
            form: Option<&Value>,
            attributes: Option<&Map<String, Value>>,
            replaces_dom_id: bool,
            owners: &mut Vec<(Option<String>, Vec<String>)>,
        ) {
            let form = form
                .and_then(|form| form.get("id"))
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
                .map(ToString::to_string);
            let dom = attributes
                .filter(|_| !replaces_dom_id)
                .into_iter()
                .flat_map(|attributes| attributes.iter())
                .filter(|(name, _)| name.eq_ignore_ascii_case("id"))
                .filter_map(|(_, value)| value.as_str().filter(|id| !id.is_empty()))
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            if form.is_some() || !dom.is_empty() {
                owners.push((form, dom));
            }
        }
        match self {
            Self::Object(object) => {
                let form = object.extensions.get(crate::FLY_FORM_FIELD);
                let replaces_dom_id =
                    materialized && crate::action::materializable_form(object).is_some();
                record(form, Some(&object.attributes), replaces_dom_id, owners);
                match &object.components {
                    ComponentChildren::Nodes(children) => {
                        for child in children {
                            child.collect_form_dom_owners_inner(owners, materialized);
                        }
                    }
                    ComponentChildren::Opaque(value) => {
                        visit_opaque_children(value, &mut |child| {
                            record(
                                child.get(crate::FLY_FORM_FIELD),
                                child.get("attributes").and_then(Value::as_object),
                                false,
                                owners,
                            );
                        })
                    }
                }
            }
            Self::Opaque(value) => visit_opaque_node(value, &mut |object| {
                record(
                    object.get(crate::FLY_FORM_FIELD),
                    object.get("attributes").and_then(Value::as_object),
                    false,
                    owners,
                );
            }),
        }
    }

    /// Form IDs also name the provider endpoint and cannot be silently minted on copy.
    pub(crate) fn collect_form_ids(&self, ids: &mut Vec<String>) {
        fn collect(form: Option<&Value>, ids: &mut Vec<String>) {
            if let Some(id) = form
                .and_then(|form| form.get("id"))
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
            {
                ids.push(id.to_string());
            }
        }
        match self {
            Self::Object(object) => {
                collect(object.extensions.get(crate::FLY_FORM_FIELD), ids);
                match &object.components {
                    ComponentChildren::Nodes(children) => {
                        for child in children {
                            child.collect_form_ids(ids);
                        }
                    }
                    ComponentChildren::Opaque(value) => {
                        visit_opaque_children(value, &mut |child| {
                            collect(child.get(crate::FLY_FORM_FIELD), ids);
                        })
                    }
                }
            }
            Self::Opaque(value) => visit_opaque_node(value, &mut |object| {
                collect(object.get(crate::FLY_FORM_FIELD), ids);
            }),
        }
    }

    pub(crate) fn remap_ids_with_dom(
        &mut self,
        components: &BTreeMap<String, String>,
        dom: &BTreeMap<String, String>,
    ) {
        // HTML relationships point to HTML ids, with component ids as a legacy fallback.
        let mut attribute_ids = components.clone();
        attribute_ids.extend(dom.clone());
        // Typed actions address component ids, but built-in blocks historically used HTML ids.
        let mut action_ids = dom.clone();
        action_ids.extend(components.clone());
        self.remap_ids_inner(components, dom, &attribute_ids, &action_ids);
    }

    fn remap_ids_inner(
        &mut self,
        components: &BTreeMap<String, String>,
        dom: &BTreeMap<String, String>,
        attribute_ids: &BTreeMap<String, String>,
        action_ids: &BTreeMap<String, String>,
    ) {
        match self {
            Self::Opaque(value) => {
                remap_opaque_node(value, components, dom, attribute_ids, action_ids)
            }
            Self::Object(object) => {
                if let Some(id) = object.id.as_mut()
                    && let Some(replacement) = components.get(id)
                {
                    *id = replacement.clone();
                }
                for (name, value) in &mut object.attributes {
                    if name.eq_ignore_ascii_case("id")
                        && let Value::String(id) = value
                        && let Some(replacement) = dom.get(id)
                    {
                        *id = replacement.clone();
                    }
                }
                remap_attribute_ids(&mut object.attributes, attribute_ids);
                if let Some(action) = object.extensions.get_mut(crate::FLY_ACTION_FIELD) {
                    remap_action_ids(action, action_ids);
                }
                match &mut object.components {
                    ComponentChildren::Nodes(children) => {
                        for child in children {
                            child.remap_ids_inner(components, dom, attribute_ids, action_ids);
                        }
                    }
                    ComponentChildren::Opaque(value) => {
                        remap_opaque_children(value, components, dom, attribute_ids, action_ids);
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
    #[serde(rename = "symbolId", default, skip_serializing_if = "Option::is_none")]
    pub symbol_id: Option<String>,
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
            symbol_id: None,
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

pub const FLY_DOCUMENT_KIND_FIELD: &str = "flyDocumentKind";

/// Declared intent of an authored Fly document.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum DocumentKind {
    /// An independent route document owning its unique content body.
    #[default]
    Page,
    /// A site-wide page frame containing headers, footers, and exactly one PageContentSlot.
    LayoutTemplate,
    /// A standalone reusable section (e.g. promo bar, subscription form, global navigation).
    SectionSymbol,
    /// A pre-built page blueprint cloned deeply into a new page draft at creation.
    StarterPreset,
}

impl DocumentKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Page => "page",
            Self::LayoutTemplate => "layout_template",
            Self::SectionSymbol => "section_symbol",
            Self::StarterPreset => "starter_preset",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GrapesJsCodec;
    use serde_json::json;

    #[test]
    fn cloning_keeps_component_html_and_form_identities_distinct() {
        let doc = GrapesJsCodec::decode_value(json!({
            "pages": [{"component": {"id": "root", "type": "wrapper", "components": [
                {"id": "trigger", "type": "button", "attributes": {"aria-controls": "panel", "form": "contact"},
                 "flyAction": {"kind": "scroll_to", "target_id": "panel"}},
                {"id": "panel", "type": "section"},
                {"id": "other", "type": "section", "attributes": {"id": "panel"}},
                {"id": "contact", "type": "form", "flyForm": {"id": "contact", "method": "post", "provider": "forms", "action": "submit"}}
            ]}}]
        })).expect("document");
        let cloned = doc
            .clone_as_page(&mut crate::SequentialIdGenerator::new("copy"))
            .expect("clone");
        let root = cloned.project.pages[0]
            .component
            .as_ref()
            .unwrap()
            .as_object()
            .unwrap();
        let trigger = root.children()[0].as_object().unwrap();
        let component_panel = root.children()[1].as_object().unwrap();
        let html_panel = root.children()[2].as_object().unwrap();
        let form = root.children()[3].as_object().unwrap();
        assert_eq!(
            trigger.extensions["flyAction"]["target_id"],
            component_panel.id.as_deref().unwrap()
        );
        assert_eq!(
            trigger.attributes["aria-controls"],
            html_panel.attributes["id"]
        );
        assert_ne!(
            component_panel.id.as_deref().unwrap(),
            html_panel.attributes["id"].as_str().unwrap()
        );
        assert_eq!(form.extensions["flyForm"]["id"], "contact");
        assert_eq!(trigger.attributes["form"], "contact");
    }

    #[test]
    fn clone_remaps_only_owned_reference_slots_and_preserves_provider_payloads() {
        let doc = GrapesJsCodec::decode_value(json!({
            "pages": [{"component": {"id": "root", "type": "wrapper", "components": [
                {"id": "hero", "type": "section", "attributes": {
                    "data-config": {"component_id": "hero"}
                }, "providerData": {"component_id": "hero", "flyComponentId": "hero"},
                "flyForm": {"id": "form", "input": {"component_id": "hero"}}}
            ]}}],
            "styles": [{"selectors": [{"name": "hero", "type": 2}], "style": {"color": "red"},
                "providerData": {"component_id": "hero"}}],
            "assets": [{"id": "asset", "src": "/hero.png", "providerData": {"component_id": "hero"}}],
            "flyRuntimeBindings": [{"id": "b", "component_id": "hero", "path": "title", "target": "field", "name": "content", "fallback": {"component_id": "hero"}},
                {"component_id": "hero", "unknownProvider": true}],
            "flyRuntimeConditions": [{"id": "c", "component_id": "hero", "path": "value", "operator": "equals", "expected": {"component_id": "hero"}},
                {"component_id": "hero", "operator": "unrecognized"}],
            "flyRuntimeRepeaters": [{"id": "r", "component_id": "hero", "path": "items", "providerData": {"component_id": "hero"}},
                {"component_id": "hero", "unknownProvider": true}],
            "ownerPayload": {"component_id": "hero"},
            "flySymbols": [{"id": "sym", "components": [{"id": "hero", "type": "text"}],
                "styles": [{"flyComponentId": "hero", "style": {"color": "blue"}}]}]
        })).expect("document");
        let cloned = doc
            .clone_as_page(&mut crate::SequentialIdGenerator::new("copy"))
            .expect("clone");
        let hero = cloned.project.pages[0]
            .component
            .as_ref()
            .unwrap()
            .as_object()
            .unwrap()
            .children()[0]
            .as_object()
            .unwrap();
        let new_id = hero.id.as_deref().unwrap();
        assert_ne!(new_id, "hero");
        assert_eq!(hero.attributes["data-config"]["component_id"], "hero");
        assert_eq!(hero.extensions["providerData"]["component_id"], "hero");
        assert_eq!(hero.extensions["providerData"]["flyComponentId"], "hero");
        assert_eq!(hero.extensions["flyForm"]["input"]["component_id"], "hero");
        assert_eq!(cloned.project.styles[0]["flyComponentId"], new_id);
        assert_eq!(cloned.project.styles[0]["selectors"][0]["name"], new_id);
        assert_eq!(
            cloned.project.styles[0]["providerData"]["component_id"],
            "hero"
        );
        assert_eq!(
            cloned.project.assets[0]["providerData"]["component_id"],
            "hero"
        );
        assert_eq!(
            cloned.project.extensions["flyRuntimeBindings"][0]["component_id"],
            new_id
        );
        assert_eq!(
            cloned.project.extensions["flyRuntimeBindings"][0]["fallback"]["component_id"],
            "hero"
        );
        assert_eq!(
            cloned.project.extensions["flyRuntimeBindings"][1]["component_id"],
            "hero"
        );
        assert_eq!(
            cloned.project.extensions["flyRuntimeConditions"][0]["component_id"],
            new_id
        );
        assert_eq!(
            cloned.project.extensions["flyRuntimeConditions"][0]["expected"]["component_id"],
            "hero"
        );
        assert_eq!(
            cloned.project.extensions["flyRuntimeConditions"][1]["component_id"],
            "hero"
        );
        assert_eq!(
            cloned.project.extensions["flyRuntimeRepeaters"][0]["component_id"],
            new_id
        );
        assert_eq!(
            cloned.project.extensions["flyRuntimeRepeaters"][0]["providerData"]["component_id"],
            "hero"
        );
        assert_eq!(
            cloned.project.extensions["flyRuntimeRepeaters"][1]["component_id"],
            "hero"
        );
        assert_eq!(
            cloned.project.extensions["ownerPayload"]["component_id"],
            "hero"
        );
        assert_eq!(
            cloned.project.extensions["flySymbols"],
            doc.project.extensions["flySymbols"]
        );
    }

    #[test]
    fn cloning_multi_page_preset_allows_separate_pages_to_reuse_html_ids() {
        let doc = GrapesJsCodec::decode_value(json!({
            "pages": [
                {"id": "first", "component": {"id": "first-root", "type": "wrapper", "components": [
                    {"id": "first-trigger", "type": "button", "attributes": {"aria-controls": "details"}},
                    {"id": "first-panel", "type": "section", "attributes": {"id": "details"}}
                ]}},
                {"id": "second", "component": {"id": "second-root", "type": "wrapper", "components": [
                    {"id": "second-trigger", "type": "button", "attributes": {"aria-controls": "details"}},
                    {"id": "second-panel", "type": "section", "attributes": {"id": "details"}}
                ]}}
            ]
        })).unwrap();
        let cloned = doc
            .clone_as_page(&mut crate::SequentialIdGenerator::new("copy"))
            .expect("independent pages");
        let mut ids = Vec::new();
        for page in &cloned.project.pages {
            let root = page.component.as_ref().unwrap().as_object().unwrap();
            let target = root.children()[1].as_object().unwrap().attributes["id"]
                .as_str()
                .unwrap();
            assert_eq!(
                root.children()[0].as_object().unwrap().attributes["aria-controls"],
                target
            );
            assert_ne!(target, "details");
            ids.push(target.to_string());
        }
        assert_ne!(ids[0], ids[1]);
    }

    #[test]
    fn cloning_ambiguous_preset_fails_before_consuming_ids() {
        for components in [
            json!([{"id": "same", "type": "text"}, {"id": "same", "type": "text"}]),
            json!([{"id": "form-node", "type": "form", "flyForm": {"id": "contact"}},
                   {"id": "other", "type": "text", "attributes": {"id": "contact"}}]),
        ] {
            let doc = GrapesJsCodec::decode_value(json!({"pages": [{"component": {"id": "root", "type": "wrapper", "components": components}}]})).unwrap();
            let mut generator = crate::SequentialIdGenerator::new("copy");
            let before = generator.clone();
            let error = doc
                .clone_as_page(&mut generator)
                .expect_err("ambiguous preset");
            assert!(error.to_string().contains("id"), "{error}");
            assert_eq!(generator, before);
        }
    }

    #[test]
    fn clone_as_page_remaps_all_ids_and_sets_kind() {
        let doc = GrapesJsCodec::decode_value(json!({
            "flyDocumentKind": "starter_preset",
            "pages": [{
                "id": "starter-page",
                "component": {
                    "id": "orig-root",
                    "type": "wrapper",
                    "components": [
                        { "id": "orig-h1", "type": "heading", "content": "Title" },
                        { "id": "orig-p", "type": "text", "content": "Desc" }
                    ]
                }
            }],
            "styles": [
                {
                    "selectors": [{ "name": "orig-h1", "type": 2 }],
                    "flyComponentId": "orig-h1",
                    "style": { "color": "red" }
                }
            ]
        }))
        .expect("valid document");

        assert_eq!(doc.document_kind(), DocumentKind::StarterPreset);

        let mut generator = crate::SequentialIdGenerator::default();
        let cloned = doc.clone_as_page(&mut generator).expect("clone");

        assert_eq!(cloned.document_kind(), DocumentKind::Page);
        let orig_root = cloned.component("orig-root");
        assert!(orig_root.is_none(), "Original IDs must be remapped");
        let orig_h1 = cloned.component("orig-h1");
        assert!(orig_h1.is_none(), "Original child IDs must be remapped");

        // Verify style rule identity remapped
        let style_val = &cloned.project.styles[0];
        assert_ne!(
            style_val.get("flyComponentId").and_then(Value::as_str),
            Some("orig-h1")
        );
    }
}
