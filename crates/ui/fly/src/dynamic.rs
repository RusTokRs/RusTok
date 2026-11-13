use crate::style_rule::remap_component_style_rule;
use crate::{
    ComponentChildren, ComponentNode, ComponentObject, FLY_COMPONENT_RULE_FIELD, FLY_RULE_ID_FIELD,
    FlyError, FlyResult, ProjectDocument, StyleRuleDescriptor, StyleRuleIdentity,
    ValidationDiagnostic, ValidationSeverity, is_valid_runtime_context_path, resolve_context_path,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const FLY_RUNTIME_CONDITIONS_FIELD: &str = "flyRuntimeConditions";
pub const FLY_RUNTIME_REPEATERS_FIELD: &str = "flyRuntimeRepeaters";
pub const DEFAULT_REPEATER_LIMIT: usize = 100;
pub const MAX_REPEATER_LIMIT: usize = 1_000;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConditionOperator {
    Exists,
    Equals,
    NotEquals,
    Truthy,
    Falsy,
    Contains,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeCondition {
    pub id: String,
    pub component_id: String,
    pub path: String,
    pub operator: ConditionOperator,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected: Option<Value>,
    #[serde(default)]
    pub invert: bool,
    #[serde(flatten)]
    pub extensions: Map<String, Value>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum EmptyRepeaterBehavior {
    #[default]
    Hide,
    KeepTemplate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeRepeater {
    pub id: String,
    pub component_id: String,
    pub path: String,
    #[serde(default = "default_item_alias")]
    pub item_alias: String,
    #[serde(default = "default_index_alias")]
    pub index_alias: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
    #[serde(default)]
    pub empty_behavior: EmptyRepeaterBehavior,
    #[serde(flatten)]
    pub extensions: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum DynamicCommand {
    UpsertCondition { condition: RuntimeCondition },
    RemoveCondition { condition_id: String },
    UpsertRepeater { repeater: RuntimeRepeater },
    RemoveRepeater { repeater_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct DynamicCatalog {
    pub conditions: Vec<RuntimeCondition>,
    pub repeaters: Vec<RuntimeRepeater>,
    pub unknown_condition_entries: Vec<Value>,
    pub unknown_repeater_entries: Vec<Value>,
}

impl DynamicCatalog {
    pub fn from_document(document: &ProjectDocument) -> Self {
        let (conditions, unknown_condition_entries) = decode_entries(
            document
                .project
                .extensions
                .get(FLY_RUNTIME_CONDITIONS_FIELD),
        );
        let (repeaters, unknown_repeater_entries) =
            decode_entries(document.project.extensions.get(FLY_RUNTIME_REPEATERS_FIELD));
        Self {
            conditions,
            repeaters,
            unknown_condition_entries,
            unknown_repeater_entries,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeMaterialization {
    pub document: ProjectDocument,
    pub diagnostics: Vec<ValidationDiagnostic>,
    pub evaluated_conditions: usize,
    pub hidden_components: usize,
    pub repeated_nodes: usize,
}

pub fn apply_dynamic_command(
    document: &mut ProjectDocument,
    command: &DynamicCommand,
) -> FlyResult<()> {
    let mut catalog = DynamicCatalog::from_document(document);
    match command {
        DynamicCommand::UpsertCondition { condition } => {
            validate_definition_identity(
                document,
                &condition.id,
                &condition.component_id,
                &condition.path,
            )?;
            upsert_by_id(&mut catalog.conditions, condition.clone(), |value| {
                &value.id
            });
        }
        DynamicCommand::RemoveCondition { condition_id } => {
            let before = catalog.conditions.len();
            catalog
                .conditions
                .retain(|condition| condition.id != *condition_id);
            if catalog.conditions.len() == before {
                return Err(FlyError::Decode(format!(
                    "runtime condition `{condition_id}` was not found"
                )));
            }
        }
        DynamicCommand::UpsertRepeater { repeater } => {
            validate_definition_identity(
                document,
                &repeater.id,
                &repeater.component_id,
                &repeater.path,
            )?;
            if repeater
                .limit
                .is_some_and(|limit| limit > MAX_REPEATER_LIMIT)
            {
                return Err(FlyError::Decode(format!(
                    "runtime repeater limit must not exceed {MAX_REPEATER_LIMIT}"
                )));
            }
            upsert_by_id(&mut catalog.repeaters, repeater.clone(), |value| &value.id);
        }
        DynamicCommand::RemoveRepeater { repeater_id } => {
            let before = catalog.repeaters.len();
            catalog
                .repeaters
                .retain(|repeater| repeater.id != *repeater_id);
            if catalog.repeaters.len() == before {
                return Err(FlyError::Decode(format!(
                    "runtime repeater `{repeater_id}` was not found"
                )));
            }
        }
    }
    write_catalog(document, catalog)
}

/// Maximum repeater nesting Fly will expand.
///
/// Nested repeaters multiply: three levels at the default limit is a million nodes. The depth cap
/// plus [`MAX_TOTAL_REPEATED_NODES`] bound the blast radius of a hostile or careless document.
pub const MAX_REPEATER_DEPTH: usize = 8;

/// Maximum total nodes all repeaters combined may generate in one materialization.
pub const MAX_TOTAL_REPEATED_NODES: usize = 50_000;

pub fn materialize_runtime(document: &ProjectDocument, context: &Value) -> RuntimeMaterialization {
    let catalog = DynamicCatalog::from_document(document);
    let mut materialized = document.clone();
    let mut diagnostics = Vec::new();
    let mut hidden_components = 0usize;
    let original_styles = materialized.project.styles.clone();

    // Components that live *inside* a repeater template. Their conditions must be evaluated once
    // per generated item, against that item's local context — evaluating them here, against the
    // global context, is what previously made a condition on `item.*` delete the template outright
    // (the path does not resolve globally, so the component was judged falsy and removed).
    let repeater_scoped = repeater_scoped_component_ids(&materialized, &catalog);

    let mut deferred_conditions: BTreeMap<String, Vec<RuntimeCondition>> = BTreeMap::new();

    for condition in &catalog.conditions {
        if !is_valid_runtime_context_path(&condition.path) {
            diagnostics.push(runtime_diagnostic(
                ValidationSeverity::Warning,
                "runtime_condition_path_invalid",
                Some(condition.component_id.clone()),
                format!(
                    "condition `{}` has an invalid context path `{}`",
                    condition.id, condition.path
                ),
            ));
            continue;
        }

        if repeater_scoped.contains(&condition.component_id) {
            deferred_conditions
                .entry(condition.component_id.clone())
                .or_default()
                .push(condition.clone());
            continue;
        }

        let matched = evaluate_condition(condition, context);
        if !matched {
            match hide_runtime_component(&mut materialized, &condition.component_id) {
                Ok(()) => hidden_components = hidden_components.saturating_add(1),
                Err(error) => diagnostics.push(runtime_diagnostic(
                    ValidationSeverity::Warning,
                    "runtime_condition_target_missing",
                    Some(condition.component_id.clone()),
                    error.to_string(),
                )),
            }
        }
    }

    // Pre-flight each repeater definition once. Definition-level problems are reported whether or
    // not the target survives condition evaluation.
    let mut expandable: BTreeMap<String, RuntimeRepeater> = BTreeMap::new();
    for repeater in &catalog.repeaters {
        if !is_valid_runtime_context_path(&repeater.path) {
            diagnostics.push(runtime_diagnostic(
                ValidationSeverity::Warning,
                "runtime_repeater_path_invalid",
                Some(repeater.component_id.clone()),
                format!(
                    "repeater `{}` has an invalid context path `{}`",
                    repeater.id, repeater.path
                ),
            ));
            continue;
        }
        if repeater.item_alias.trim().is_empty() || repeater.index_alias.trim().is_empty() {
            diagnostics.push(runtime_diagnostic(
                ValidationSeverity::Warning,
                "runtime_repeater_alias_empty",
                Some(repeater.component_id.clone()),
                format!("repeater `{}` aliases must not be empty", repeater.id),
            ));
            continue;
        }
        if repeater
            .limit
            .is_some_and(|limit| limit > MAX_REPEATER_LIMIT)
        {
            diagnostics.push(runtime_diagnostic(
                ValidationSeverity::Warning,
                "runtime_repeater_limit_exceeded",
                Some(repeater.component_id.clone()),
                format!(
                    "repeater `{}` exceeds maximum limit {MAX_REPEATER_LIMIT}; output was clamped",
                    repeater.id
                ),
            ));
        }
        if !materialized.contains_component(&repeater.component_id) {
            diagnostics.push(runtime_diagnostic(
                ValidationSeverity::Info,
                "runtime_repeater_target_hidden",
                Some(repeater.component_id.clone()),
                format!(
                    "repeater `{}` target is not present after condition evaluation",
                    repeater.id
                ),
            ));
            continue;
        }
        expandable.insert(repeater.component_id.clone(), repeater.clone());
    }

    let used_ids = crate::id_reference::project_identity_ids(&materialized);
    let used_style_ids = original_styles
        .iter()
        .filter_map(StyleRuleIdentity::from_value)
        .map(|rule| rule.id)
        .collect();
    let mut expander = RuntimeExpander {
        used_ids,
        used_style_ids,
        repeaters: expandable,
        deferred_conditions,
        original_styles: &original_styles,
        generated_styles: Vec::new(),
        retired_style_ids: BTreeSet::new(),
        diagnostics: Vec::new(),
        repeated_nodes: 0,
        hidden_components: 0,
        budget_reported: false,
    };

    // One traversal of each page expands every repeater at every depth, instead of re-walking the
    // whole document once per repeater definition.
    let mut pages = std::mem::take(&mut materialized.project.pages);
    for page in &mut pages {
        if let Some(root) = page.component.as_mut() {
            let origin = BTreeMap::new();
            expander.expand_node(root, context, 0, "", &origin);
        }
    }
    materialized.project.pages = pages;

    let RuntimeExpander {
        generated_styles,
        retired_style_ids,
        diagnostics: expansion_diagnostics,
        repeated_nodes,
        hidden_components: hidden_in_repeaters,
        ..
    } = expander;

    diagnostics.extend(expansion_diagnostics);
    hidden_components = hidden_components.saturating_add(hidden_in_repeaters);
    remove_style_rules_for_component_ids(&mut materialized, &retired_style_ids);
    materialized.project.styles.extend(generated_styles);
    // The preflight lookups populate the document's index before expansion. Its locations (and
    // any cached hash) must describe the returned tree, not the original repeater templates.
    materialized.invalidate_caches();

    RuntimeMaterialization {
        document: materialized,
        diagnostics,
        evaluated_conditions: catalog.conditions.len(),
        hidden_components,
        repeated_nodes,
    }
}

/// Ids of components that are strict descendants of some repeater target.
fn repeater_scoped_component_ids(
    document: &ProjectDocument,
    catalog: &DynamicCatalog,
) -> BTreeSet<String> {
    let mut scoped = BTreeSet::new();
    for repeater in &catalog.repeaters {
        let mut subtree = component_subtree_ids(document, &repeater.component_id);
        subtree.remove(&repeater.component_id);
        scoped.extend(subtree);
    }
    scoped
}

/// Expands repeaters in a single depth-first traversal.
///
/// # Why this replaced the per-repeater loop
///
/// The previous implementation iterated repeater definitions and, for each, located the target in
/// the whole document, removed it, and re-inserted N remapped clones. Nested repeaters could not
/// work under that scheme in either processing order:
///
/// * outer first — the inner repeater's `component_id` no longer exists (it became
///   `row--outer-0`, `row--outer-1`, …), so the inner repeater silently never expanded;
/// * inner first — the inner expansion used the *global* context rather than the outer item's,
///   so every outer copy received identical inner data.
///
/// Expanding during a single traversal fixes both: a clone's nested repeaters are expanded with
/// that clone's own local context, before the clone's ids are rewritten.
struct RuntimeExpander<'a> {
    used_ids: BTreeSet<String>,
    used_style_ids: BTreeSet<String>,
    repeaters: BTreeMap<String, RuntimeRepeater>,
    deferred_conditions: BTreeMap<String, Vec<RuntimeCondition>>,
    original_styles: &'a [Value],
    generated_styles: Vec<Value>,
    retired_style_ids: BTreeSet<String>,
    diagnostics: Vec<ValidationDiagnostic>,
    repeated_nodes: usize,
    hidden_components: usize,
    budget_reported: bool,
}

impl RuntimeExpander<'_> {
    /// Translate a possibly-suffixed id back to the id the definitions are keyed by.
    fn origin_id<'b>(id: &'b str, origin: &'b BTreeMap<String, String>) -> &'b str {
        origin.get(id).map(String::as_str).unwrap_or(id)
    }

    fn retire_styles_for(&mut self, node: &ComponentNode, origin: &BTreeMap<String, String>) {
        let mut ids = Vec::new();
        node.collect_ids(&mut ids);
        for id in ids {
            self.retired_style_ids
                .insert(Self::origin_id(&id, origin).to_string());
        }
    }

    /// Prove how many copies of each form survive scoped conditions and nested repeaters.
    /// Follow the same alias, empty-template and depth rules as expansion, but stop as soon as
    /// a form would repeat. Opaque children remain conservative and the proof is budgeted.
    #[allow(clippy::too_many_arguments)]
    fn visible_form_counts(
        &self,
        node: &ComponentNode,
        context: &Value,
        origin: &BTreeMap<String, String>,
        scope_root: bool,
        depth: usize,
        budget: &mut usize,
        counts: &mut BTreeMap<String, usize>,
    ) -> bool {
        if !scope_root && let Some(source) = node.id().map(|id| Self::origin_id(id, origin)) {
            if self
                .deferred_conditions
                .get(source)
                .is_some_and(|conditions| {
                    conditions
                        .iter()
                        .any(|condition| !evaluate_condition(condition, context))
                })
            {
                return true;
            }
            if let Some(repeater) = self.repeaters.get(source) {
                let mut forms = Vec::new();
                node.collect_form_ids(&mut forms);
                if forms.is_empty() {
                    return true;
                }
                if depth >= MAX_REPEATER_DEPTH {
                    return false;
                }
                let values = resolve_path(context, &repeater.path)
                    .and_then(Value::as_array)
                    .map(Vec::as_slice)
                    .unwrap_or_default();
                let limit = repeater
                    .limit
                    .unwrap_or(DEFAULT_REPEATER_LIMIT)
                    .min(MAX_REPEATER_LIMIT);
                if values.is_empty() || limit == 0 {
                    return repeater.empty_behavior == EmptyRepeaterBehavior::Hide
                        || forms.into_iter().all(|id| record_visible_form(counts, &id));
                }
                return values.iter().take(limit).enumerate().all(|(index, item)| {
                    *budget = budget.saturating_add(1);
                    if *budget > MAX_TOTAL_REPEATED_NODES {
                        return false;
                    }
                    let local_context = build_local_context(
                        context,
                        &repeater.item_alias,
                        item.clone(),
                        &repeater.index_alias,
                        index,
                    );
                    self.visible_form_counts(
                        node,
                        &local_context,
                        origin,
                        true,
                        depth + 1,
                        budget,
                        counts,
                    )
                });
            }
        }
        let Some(object) = node.as_object() else {
            let mut forms = Vec::new();
            node.collect_form_ids(&mut forms);
            return forms.is_empty();
        };
        let own_form_id = object
            .extensions
            .get(crate::FLY_FORM_FIELD)
            .and_then(|form| form.get("id"))
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty());
        if own_form_id.is_some_and(|id| !record_visible_form(counts, id)) {
            return false;
        }
        match &object.components {
            ComponentChildren::Nodes(children) => children.iter().all(|child| {
                self.visible_form_counts(child, context, origin, false, depth, budget, counts)
            }),
            ComponentChildren::Opaque(_) => {
                let mut forms = Vec::new();
                node.collect_form_ids(&mut forms);
                forms.len() <= usize::from(own_form_id.is_some())
            }
        }
    }

    /// Existing literal ids are remapped throughout the cloned subtree. A nested repeater may
    /// subsequently replace its own literals, but those intermediate ids can also collide with
    /// an interpolated id in this scope; reserve only values whose owners can survive conditions.
    fn collect_live_literal_dom_ids(
        &self,
        node: &ComponentNode,
        context: &Value,
        origin: &BTreeMap<String, String>,
        scope_root: bool,
        ids: &mut BTreeSet<String>,
    ) {
        if !scope_root
            && let Some(source) = node.id().map(|id| Self::origin_id(id, origin))
            && !self.repeaters.contains_key(source)
            && self
                .deferred_conditions
                .get(source)
                .is_some_and(|conditions| {
                    conditions
                        .iter()
                        .any(|condition| !evaluate_condition(condition, context))
                })
        {
            return;
        }
        let Some(object) = node.as_object() else {
            let mut dom = Vec::new();
            node.collect_dom_ids(&mut dom);
            ids.extend(dom.into_iter().filter(|id| !id.contains("{{")));
            return;
        };
        if let Some(form) = crate::action::materializable_form(object) {
            ids.insert(form.id);
        } else {
            ids.extend(object.attributes.iter().filter_map(|(name, value)| {
                name.eq_ignore_ascii_case("id")
                    .then(|| value.as_str())
                    .flatten()
                    .filter(|id| !id.is_empty() && !id.contains("{{"))
                    .map(ToString::to_string)
            }));
        }
        match &object.components {
            ComponentChildren::Nodes(children) => {
                for child in children {
                    self.collect_live_literal_dom_ids(child, context, origin, false, ids);
                }
            }
            ComponentChildren::Opaque(_) => {
                let mut dom = Vec::new();
                node.collect_dom_ids(&mut dom);
                ids.extend(dom.into_iter().filter(|id| !id.contains("{{")));
            }
        }
    }

    /// Collect templated HTML identities owned by this repeater's scope. A nested repeater
    /// has its own item/index aliases and must resolve its ids only after its context exists.
    fn collect_scoped_template_ids(
        &self,
        node: &ComponentNode,
        context: &Value,
        origin: &BTreeMap<String, String>,
        scope_root: bool,
        ids: &mut Vec<String>,
    ) {
        if !scope_root && let Some(source) = node.id().map(|id| Self::origin_id(id, origin)) {
            if self.repeaters.contains_key(source) {
                return;
            }
            if self
                .deferred_conditions
                .get(source)
                .is_some_and(|conditions| {
                    conditions
                        .iter()
                        .any(|condition| !evaluate_condition(condition, context))
                })
            {
                return;
            }
        }
        let Some(object) = node.as_object() else {
            return;
        };
        // A valid form's authored `id` is discarded during form materialization.
        if crate::action::materializable_form(object).is_none() {
            ids.extend(object.attributes.iter().filter_map(|(name, value)| {
                (name.eq_ignore_ascii_case("id"))
                    .then(|| value.as_str())
                    .flatten()
                    .filter(|id| id.contains("{{"))
                    .map(ToString::to_string)
            }));
        }
        for child in object.children() {
            self.collect_scoped_template_ids(child, context, origin, false, ids);
        }
    }

    /// Rewrite just the recognized DOM reference slots. Outer identities may be referenced
    /// from nested repeaters, except where the nested item or index alias shadows a path in the
    /// template; there the nested scope must perform its own interpolation instead.
    fn remap_scoped_template_ids(
        &self,
        node: &mut ComponentNode,
        mapping: &BTreeMap<String, String>,
        origin: &BTreeMap<String, String>,
        context: Option<&Value>,
        scope_root: bool,
    ) {
        let nested = (!scope_root)
            .then(|| node.id().map(|id| Self::origin_id(id, origin)))
            .flatten()
            .and_then(|id| self.repeaters.get(id));
        let filtered = nested.map(|repeater| {
            mapping
                .iter()
                .filter(|(source, _)| {
                    !template_uses_alias(source, &repeater.item_alias)
                        && !template_uses_alias(source, &repeater.index_alias)
                })
                .map(|(source, target)| (source.clone(), target.clone()))
                .collect::<BTreeMap<_, _>>()
        });
        let mapping = filtered.as_ref().unwrap_or(mapping);
        // A nested item may rebind the same alias; never evaluate its templates with the
        // enclosing context. Exact outer references can still be rewritten where unshadowed.
        let context = if nested.is_some() { None } else { context };
        if mapping.is_empty() {
            return;
        }
        // Resolved strings are lookup keys for equivalent templates, not ownership claims over
        // literal references. A literal `#item-apple` or action target can name an existing node
        // outside this repeater even when a local template also resolves to `item-apple`.
        let mut references = mapping
            .iter()
            .filter(|(source, _)| source.contains("{{"))
            .map(|(source, target)| (source.clone(), target.clone()))
            .collect::<BTreeMap<_, _>>();
        let ComponentNode::Object(object) = node else {
            node.remap_ids_with_dom(&BTreeMap::new(), &references);
            return;
        };
        for (name, value) in &mut object.attributes {
            if name.eq_ignore_ascii_case("id")
                && let Value::String(id) = value
                && id.contains("{{")
                && let Some(replacement) = mapping.get(id)
            {
                *id = replacement.clone();
            }
        }
        if let Some(context) = context {
            collect_interpolated_reference_aliases(object, context, mapping, &mut references);
        }
        crate::id_reference::remap_attribute_ids(&mut object.attributes, &references);
        if let Some(action) = object.extensions.get_mut(crate::FLY_ACTION_FIELD) {
            crate::id_reference::remap_action_ids(action, &references);
        }
        if let Some(children) = object.children_mut() {
            for child in children {
                self.remap_scoped_template_ids(child, mapping, origin, context, false);
            }
        } else {
            // Opaque children cannot host a Fly repeater, but recognized reference fields in
            // their preserved structure still participate in the ordinary remapping contract.
            node.remap_ids_with_dom(&BTreeMap::new(), &references);
        }
    }

    fn expand_node(
        &mut self,
        node: &mut ComponentNode,
        context: &Value,
        depth: usize,
        suffix: &str,
        origin: &BTreeMap<String, String>,
    ) {
        let Some(object) = node.as_object_mut() else {
            return;
        };
        let Some(children) = object.children_mut() else {
            return;
        };
        let taken = std::mem::take(children);
        let mut output = Vec::with_capacity(taken.len());
        for child in taken {
            self.expand_child(child, context, depth, suffix, origin, &mut output);
        }
        if let Some(children) = node.as_object_mut().and_then(ComponentObject::children_mut) {
            *children = output;
        }
    }

    fn expand_child(
        &mut self,
        mut child: ComponentNode,
        context: &Value,
        depth: usize,
        suffix: &str,
        origin: &BTreeMap<String, String>,
        output: &mut Vec<ComponentNode>,
    ) {
        let current_id = child.id().map(ToString::to_string);
        let source_id = current_id
            .as_deref()
            .map(|id| Self::origin_id(id, origin).to_string());

        // Conditions deferred out of the global pass, now evaluated against the local context.
        //
        // Resolved into a `bool` before the branch on purpose: in a let-chain the immutable
        // borrow of `self.deferred_conditions` would stay live for the whole body, which calls
        // `&mut self` methods.
        let hidden_by_condition = source_id
            .as_deref()
            .and_then(|id| self.deferred_conditions.get(id))
            .is_some_and(|conditions| {
                conditions
                    .iter()
                    .any(|condition| !evaluate_condition(condition, context))
            });
        if hidden_by_condition {
            self.retire_styles_for(&child, origin);
            self.hidden_components = self.hidden_components.saturating_add(1);
            return;
        }

        let repeater = source_id
            .as_deref()
            .and_then(|id| self.repeaters.get(id))
            .cloned();

        let Some(repeater) = repeater else {
            self.expand_node(&mut child, context, depth, suffix, origin);
            output.push(child);
            return;
        };

        if depth >= MAX_REPEATER_DEPTH {
            self.diagnostics.push(runtime_diagnostic(
                ValidationSeverity::Error,
                "runtime_repeater_depth_exceeded",
                Some(repeater.component_id.clone()),
                format!(
                    "repeater `{}` is nested deeper than the maximum {MAX_REPEATER_DEPTH}; it was left unexpanded",
                    repeater.id
                ),
            ));
            output.push(child);
            return;
        }

        self.expand_repeater_node(child, &repeater, context, depth, suffix, origin, output);
    }

    #[allow(clippy::too_many_arguments)]
    fn expand_repeater_node(
        &mut self,
        template: ComponentNode,
        repeater: &RuntimeRepeater,
        context: &Value,
        depth: usize,
        suffix: &str,
        origin: &BTreeMap<String, String>,
        output: &mut Vec<ComponentNode>,
    ) {
        let values = resolve_path(context, &repeater.path)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let limit = repeater
            .limit
            .unwrap_or(DEFAULT_REPEATER_LIMIT)
            .min(MAX_REPEATER_LIMIT);
        let values = values.into_iter().take(limit).collect::<Vec<_>>();

        if values.is_empty() {
            if repeater.empty_behavior == EmptyRepeaterBehavior::Hide {
                self.retire_styles_for(&template, origin);
                return;
            }
            output.push(template);
            return;
        }

        // A repeated subtree with duplicate source identities cannot be remapped without guessing
        // which element a reference intended. Preserve the template and report the ambiguity.
        let mut component_ids = Vec::new();
        let mut dom_ids = Vec::new();
        let mut form_ids = Vec::new();
        template.collect_ids(&mut component_ids);
        template.collect_dom_ids(&mut dom_ids);
        template.collect_form_ids(&mut form_ids);
        if [&component_ids, &dom_ids, &form_ids]
            .into_iter()
            .any(|ids| ids.iter().collect::<BTreeSet<_>>().len() != ids.len())
            || crate::id_reference::conflicting_form_dom_id(std::iter::once(&template)).is_some()
        {
            self.diagnostics.push(runtime_diagnostic(
                ValidationSeverity::Error,
                "runtime_repeater_identity_ambiguous",
                Some(repeater.component_id.clone()),
                format!("repeater `{}` contains ambiguous component, HTML or form IDs; it was left unexpanded", repeater.id),
            ));
            output.push(template);
            return;
        }
        if self.repeated_nodes.saturating_add(values.len()) > MAX_TOTAL_REPEATED_NODES {
            if !self.budget_reported {
                self.budget_reported = true;
                self.diagnostics.push(runtime_diagnostic(
                    ValidationSeverity::Error,
                    "runtime_repeater_budget_exceeded",
                    Some(repeater.component_id.clone()),
                    format!(
                        "expanding repeater `{}` would exceed the total budget of {MAX_TOTAL_REPEATED_NODES} generated nodes; it was left unexpanded",
                        repeater.id
                    ),
                ));
            }
            output.push(template);
            return;
        }

        // `flyForm.id` also names the provider resource; never silently mint a different one.
        // Conditions may remove the form from all but one item, which is safe to expand. When
        // that cannot be proven, fail closed before creating any copies or losing style rules.
        if values.len() > 1 && !form_ids.is_empty() {
            let mut counts = BTreeMap::new();
            let mut budget = values.len();
            let safe = values.iter().enumerate().all(|(index, item)| {
                let local_context = build_local_context(
                    context,
                    &repeater.item_alias,
                    item.clone(),
                    &repeater.index_alias,
                    index,
                );
                self.visible_form_counts(
                    &template,
                    &local_context,
                    origin,
                    true,
                    depth + 1,
                    &mut budget,
                    &mut counts,
                )
            });
            if !safe {
                self.diagnostics.push(runtime_diagnostic(
                    ValidationSeverity::Error,
                    "runtime_repeater_form_identity_ambiguous",
                    Some(repeater.component_id.clone()),
                    format!(
                        "repeater `{}` may duplicate a provider form identity; it was left unexpanded",
                        repeater.id
                    ),
                ));
                output.push(template);
                return;
            }
        }

        self.retire_styles_for(&template, origin);

        for (index, item) in values.iter().enumerate() {
            let local_context = build_local_context(
                context,
                &repeater.item_alias,
                item.clone(),
                &repeater.index_alias,
                index,
            );
            let child_suffix = format!("{suffix}--{}-{index}", sanitize_identifier(&repeater.id));

            let mut clone = template.clone();

            // Rewrite ids first so that nested expansion sees unique ids, and record where each
            // one came from so nested definitions can still be looked up by their original id.
            let mut current_ids = Vec::new();
            clone.collect_ids(&mut current_ids);
            let mut id_mapping = BTreeMap::new();
            let mut style_mapping = BTreeMap::new();
            let mut child_origin = BTreeMap::new();
            for current in current_ids {
                let source = Self::origin_id(&current, origin).to_string();
                let preferred = format!("{source}{child_suffix}");
                let target = crate::id_reference::allocate_derived_id(
                    &preferred,
                    &source,
                    &mut self.used_ids,
                );
                style_mapping.insert(source.clone(), target.clone());
                child_origin.insert(target.clone(), source);
                id_mapping.insert(current, target);
            }
            // Runtime copies need unique HTML ids as well as component ids. Otherwise every
            // repeated FAQ/tab references the first item's panel and ARIA anchors are duplicated.
            let mut dom_ids = Vec::new();
            let mut matching_ids = BTreeSet::new();
            clone.collect_dom_ids(&mut dom_ids);
            clone.collect_matching_dom_ids(&mut matching_ids);
            let mut dom_mapping = form_ids
                .iter()
                .map(|id| (id.clone(), id.clone()))
                .collect::<BTreeMap<_, _>>();
            for source in dom_ids {
                // A template needs the item's context before it can become an HTML identity.
                // Treating its literal braces as an identifier destroys interpolation and
                // produces a different, data-independent ID for every copy.
                if source.contains("{{") || dom_mapping.contains_key(&source) {
                    continue;
                }
                let target = if matching_ids.contains(&source) {
                    id_mapping.get(&source).cloned()
                } else {
                    None
                }
                .unwrap_or_else(|| {
                    let preferred = format!("{source}{child_suffix}");
                    crate::id_reference::allocate_derived_id(
                        &preferred,
                        &source,
                        &mut self.used_ids,
                    )
                });
                dom_mapping.insert(source, target);
            }
            clone.remap_ids_with_dom(&id_mapping, &dom_mapping);

            // Resolve only this scope's dynamic DOM IDs. Keep the author's interpolated value
            // when it is free, but mint a deterministic alternative on a real collision and
            // update the owned references before entering nested scopes.
            let mut template_ids = Vec::new();
            self.collect_scoped_template_ids(
                &clone,
                &local_context,
                &child_origin,
                true,
                &mut template_ids,
            );
            let mut template_mapping = BTreeMap::new();
            let mut live_dom = BTreeSet::new();
            self.collect_live_literal_dom_ids(
                &clone,
                &local_context,
                &child_origin,
                true,
                &mut live_dom,
            );
            for source in template_ids {
                let Some(interpolated) = interpolate_html_id(&source, &local_context) else {
                    self.diagnostics.push(runtime_diagnostic(
                        ValidationSeverity::Error,
                        "runtime_repeater_html_id_invalid",
                        Some(repeater.component_id.clone()),
                        format!("repeater `{}` has an unresolved or invalid HTML id template `{source}`", repeater.id),
                    ));
                    continue;
                };
                let target = if self.used_ids.insert(interpolated.clone()) {
                    interpolated.clone()
                } else {
                    let preferred = format!("{interpolated}{child_suffix}");
                    crate::id_reference::allocate_derived_id(
                        &preferred,
                        &interpolated,
                        &mut self.used_ids,
                    )
                };
                // A literal reference in this clone may already have been remapped to the
                // interpolated spelling. The two owners are indistinguishable to a value-only
                // rewrite, so fail closed instead of silently rewiring the unrelated literal.
                if target != interpolated && live_dom.contains(&interpolated) {
                    self.diagnostics.push(runtime_diagnostic(
                        ValidationSeverity::Error,
                        "runtime_repeater_html_id_ambiguous",
                        Some(repeater.component_id.clone()),
                        format!("repeater `{}` resolves `{source}` to an HTML id also owned by another node", repeater.id),
                    ));
                }
                if target != interpolated {
                    template_mapping.insert(interpolated, target.clone());
                }
                live_dom.insert(target.clone());
                template_mapping.insert(source, target);
            }
            self.remap_scoped_template_ids(
                &mut clone,
                &template_mapping,
                &child_origin,
                Some(&local_context),
                true,
            );

            // Nested repeaters expand against this item's context, not the outer one.
            self.expand_node(
                &mut clone,
                &local_context,
                depth + 1,
                &child_suffix,
                &child_origin,
            );

            interpolate_node(&mut clone, &local_context);
            // Nested repeaters and deferred conditions may have replaced or removed nodes whose
            // ids were in the outer mapping. Do not emit styles for those dead intermediate ids.
            let mut live_ids = Vec::new();
            clone.collect_ids(&mut live_ids);
            let live_ids = live_ids.into_iter().collect::<BTreeSet<_>>();
            style_mapping.retain(|_, target| live_ids.contains(target));
            self.generated_styles.extend(remap_styles(
                self.original_styles,
                &style_mapping,
                &child_suffix,
                &mut self.used_style_ids,
            ));
            self.repeated_nodes = self.repeated_nodes.saturating_add(1);
            output.push(clone);
        }
    }
}

pub fn validate_dynamic_definitions(document: &ProjectDocument) -> Vec<ValidationDiagnostic> {
    let catalog = DynamicCatalog::from_document(document);
    let mut diagnostics = Vec::new();
    let mut ids = BTreeSet::new();

    for condition in &catalog.conditions {
        validate_common_definition(
            document,
            "condition",
            &condition.id,
            &condition.component_id,
            &condition.path,
            &mut ids,
            &mut diagnostics,
        );
        if matches!(
            condition.operator,
            ConditionOperator::Equals | ConditionOperator::NotEquals | ConditionOperator::Contains
        ) && condition.expected.is_none()
        {
            diagnostics.push(runtime_diagnostic(
                ValidationSeverity::Warning,
                "runtime_condition_expected_missing",
                Some(condition.component_id.clone()),
                format!(
                    "condition `{}` uses an operator that normally requires `expected`",
                    condition.id
                ),
            ));
        }
    }

    for repeater in &catalog.repeaters {
        validate_common_definition(
            document,
            "repeater",
            &repeater.id,
            &repeater.component_id,
            &repeater.path,
            &mut ids,
            &mut diagnostics,
        );
        if document
            .component_location(&repeater.component_id)
            .is_some_and(|location| location.parent_component_id.is_none())
        {
            diagnostics.push(runtime_diagnostic(
                ValidationSeverity::Error,
                "runtime_repeater_targets_page_root",
                Some(repeater.component_id.clone()),
                format!("repeater `{}` cannot target a page root", repeater.id),
            ));
        }
        if repeater.item_alias.trim().is_empty() || repeater.index_alias.trim().is_empty() {
            diagnostics.push(runtime_diagnostic(
                ValidationSeverity::Error,
                "runtime_repeater_alias_empty",
                Some(repeater.component_id.clone()),
                format!("repeater `{}` aliases must not be empty", repeater.id),
            ));
        }
        if repeater.limit == Some(0) {
            diagnostics.push(runtime_diagnostic(
                ValidationSeverity::Warning,
                "runtime_repeater_zero_limit",
                Some(repeater.component_id.clone()),
                format!("repeater `{}` has a zero item limit", repeater.id),
            ));
        }
        if repeater
            .limit
            .is_some_and(|limit| limit > MAX_REPEATER_LIMIT)
        {
            diagnostics.push(runtime_diagnostic(
                ValidationSeverity::Error,
                "runtime_repeater_limit_exceeded",
                Some(repeater.component_id.clone()),
                format!(
                    "repeater `{}` exceeds maximum limit {MAX_REPEATER_LIMIT}",
                    repeater.id
                ),
            ));
        }
    }

    if !catalog.unknown_condition_entries.is_empty() {
        diagnostics.push(runtime_diagnostic(
            ValidationSeverity::Info,
            "opaque_runtime_conditions",
            None,
            format!(
                "{} runtime condition entries are opaque and preserved",
                catalog.unknown_condition_entries.len()
            ),
        ));
    }
    if !catalog.unknown_repeater_entries.is_empty() {
        diagnostics.push(runtime_diagnostic(
            ValidationSeverity::Info,
            "opaque_runtime_repeaters",
            None,
            format!(
                "{} runtime repeater entries are opaque and preserved",
                catalog.unknown_repeater_entries.len()
            ),
        ));
    }

    diagnostics
}

fn default_item_alias() -> String {
    "item".to_string()
}

fn default_index_alias() -> String {
    "index".to_string()
}

fn decode_entries<T>(value: Option<&Value>) -> (Vec<T>, Vec<Value>)
where
    T: for<'de> Deserialize<'de>,
{
    let mut known = Vec::new();
    let mut unknown = Vec::new();
    let Some(Value::Array(entries)) = value else {
        return (known, unknown);
    };
    for entry in entries {
        match serde_json::from_value::<T>(entry.clone()) {
            Ok(value) => known.push(value),
            Err(_) => unknown.push(entry.clone()),
        }
    }
    (known, unknown)
}

fn write_catalog(document: &mut ProjectDocument, catalog: DynamicCatalog) -> FlyResult<()> {
    write_entries(
        document,
        FLY_RUNTIME_CONDITIONS_FIELD,
        catalog.conditions,
        catalog.unknown_condition_entries,
    )?;
    write_entries(
        document,
        FLY_RUNTIME_REPEATERS_FIELD,
        catalog.repeaters,
        catalog.unknown_repeater_entries,
    )
}

fn write_entries<T>(
    document: &mut ProjectDocument,
    field: &str,
    known: Vec<T>,
    unknown: Vec<Value>,
) -> FlyResult<()>
where
    T: Serialize,
{
    let mut entries = known
        .into_iter()
        .map(|entry| {
            serde_json::to_value(entry).map_err(|error| FlyError::Encode(error.to_string()))
        })
        .collect::<FlyResult<Vec<_>>>()?;
    entries.extend(unknown);
    if entries.is_empty() {
        document.project.extensions.remove(field);
    } else {
        document
            .project
            .extensions
            .insert(field.to_string(), Value::Array(entries));
    }
    Ok(())
}

fn validate_definition_identity(
    document: &ProjectDocument,
    id: &str,
    component_id: &str,
    path: &str,
) -> FlyResult<()> {
    if id.trim().is_empty() {
        return Err(FlyError::Decode(
            "runtime definition id must not be empty".to_string(),
        ));
    }
    if !is_valid_runtime_context_path(path) {
        return Err(FlyError::Decode(format!(
            "runtime definition path `{path}` is invalid"
        )));
    }
    if !document.contains_component(component_id) {
        return Err(FlyError::ComponentNotFound(component_id.to_string()));
    }
    Ok(())
}

fn upsert_by_id<T>(values: &mut Vec<T>, value: T, id: impl Fn(&T) -> &str) {
    let target = id(&value).to_string();
    if let Some(index) = values.iter().position(|candidate| id(candidate) == target) {
        values[index] = value;
    } else {
        values.push(value);
    }
}

fn validate_common_definition(
    document: &ProjectDocument,
    kind: &str,
    id: &str,
    component_id: &str,
    path: &str,
    ids: &mut BTreeSet<String>,
    diagnostics: &mut Vec<ValidationDiagnostic>,
) {
    if id.trim().is_empty() {
        diagnostics.push(runtime_diagnostic(
            ValidationSeverity::Error,
            "runtime_definition_id_empty",
            Some(component_id.to_string()),
            format!("runtime {kind} id must not be empty"),
        ));
    } else if !ids.insert(id.to_string()) {
        diagnostics.push(runtime_diagnostic(
            ValidationSeverity::Error,
            "duplicate_runtime_definition_id",
            Some(component_id.to_string()),
            format!("runtime definition id `{id}` is duplicated"),
        ));
    }
    if !is_valid_runtime_context_path(path) {
        diagnostics.push(runtime_diagnostic(
            ValidationSeverity::Error,
            "runtime_definition_path_invalid",
            Some(component_id.to_string()),
            format!("runtime {kind} `{id}` path `{path}` is invalid"),
        ));
    }
    if !document.contains_component(component_id) {
        diagnostics.push(runtime_diagnostic(
            ValidationSeverity::Error,
            "runtime_definition_target_missing",
            Some(component_id.to_string()),
            format!("runtime {kind} `{id}` targets missing component `{component_id}`"),
        ));
    }
}

fn evaluate_condition(condition: &RuntimeCondition, context: &Value) -> bool {
    let resolved = resolve_path(context, &condition.path);
    let matched = match condition.operator {
        ConditionOperator::Exists => resolved.is_some_and(|value| !value.is_null()),
        ConditionOperator::Equals => resolved == condition.expected.as_ref(),
        ConditionOperator::NotEquals => resolved != condition.expected.as_ref(),
        ConditionOperator::Truthy => resolved.is_some_and(is_truthy),
        ConditionOperator::Falsy => resolved.is_none_or(|value| !is_truthy(value)),
        ConditionOperator::Contains => {
            resolved.is_some_and(|value| contains(value, condition.expected.as_ref()))
        }
    };
    if condition.invert { !matched } else { matched }
}

fn contains(value: &Value, expected: Option<&Value>) -> bool {
    let Some(expected) = expected else {
        return false;
    };
    match value {
        Value::Array(values) => values.iter().any(|value| value == expected),
        Value::String(value) => expected
            .as_str()
            .is_some_and(|expected| value.contains(expected)),
        Value::Object(values) => expected
            .as_str()
            .is_some_and(|expected| values.contains_key(expected)),
        _ => false,
    }
}

fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.0),
        Value::String(value) => !value.trim().is_empty(),
        Value::Array(value) => !value.is_empty(),
        Value::Object(value) => !value.is_empty(),
    }
}

fn hide_runtime_component(document: &mut ProjectDocument, component_id: &str) -> FlyResult<()> {
    let location = document
        .component_location(component_id)
        .ok_or_else(|| FlyError::ComponentNotFound(component_id.to_string()))?;
    let style_targets = component_subtree_ids(document, component_id);
    if location.parent_component_id.is_some() {
        remove_style_rules_for_component_ids(document, &style_targets);
        document.project.remove_component(component_id)?;
        document.invalidate_caches();
        return Ok(());
    }
    remove_style_rules_for_component_ids(document, &style_targets);
    let page = document
        .project
        .pages
        .get_mut(location.page_index)
        .ok_or_else(|| FlyError::PageNotFound(location.page_index.to_string()))?;
    page.component = Some(ComponentNode::Object(Box::new(ComponentObject {
        id: Some(component_id.to_string()),
        component_type: Some("wrapper".to_string()),
        style: Some(Value::Object(Map::from_iter([(
            "display".to_string(),
            Value::String("none".to_string()),
        )]))),
        components: ComponentChildren::Nodes(Vec::new()),
        ..ComponentObject::default()
    })));
    document.invalidate_caches();
    Ok(())
}

fn component_subtree_ids(document: &ProjectDocument, component_id: &str) -> BTreeSet<String> {
    let mut ids = Vec::new();
    if let Some(component) = document.component(component_id) {
        ComponentNode::Object(Box::new(component.clone())).collect_ids(&mut ids);
    }
    ids.into_iter().collect()
}

fn remove_style_rules_for_component_ids(
    document: &mut ProjectDocument,
    component_ids: &BTreeSet<String>,
) {
    if component_ids.is_empty() {
        return;
    }
    document.project.styles.retain(|raw| {
        StyleRuleIdentity::from_value(raw).is_none_or(|rule| {
            rule.component_id
                .as_ref()
                .is_none_or(|component_id| !component_ids.contains(component_id))
        })
    });
}

/// Clone the style rules of a repeater template for one generated item.
///
/// `mapping` is keyed by *original* component id, because style rules always reference the ids as
/// they appear in the authored document, even when the node being generated is itself nested
/// inside another repeater's clone. `suffix` is the accumulated `--<repeater>-<index>` chain, so
/// rule ids stay unique across nesting levels rather than colliding on the innermost index.
fn remap_styles(
    styles: &[Value],
    mapping: &BTreeMap<String, String>,
    suffix: &str,
    used_ids: &mut BTreeSet<String>,
) -> Vec<Value> {
    styles
        .iter()
        .filter_map(|raw| {
            let descriptor = StyleRuleDescriptor::from_value(raw.clone())?;
            let source_id = descriptor.component_id.as_ref()?;
            let target_id = mapping.get(source_id)?;
            let mut value = raw.clone();
            remap_component_style_rule(&mut value, mapping);
            if let Some(object) = value.as_object_mut() {
                object.insert(
                    FLY_COMPONENT_RULE_FIELD.to_string(),
                    Value::String(target_id.clone()),
                );
                let preferred = format!("{}{suffix}", descriptor.id);
                let id =
                    crate::id_reference::allocate_derived_id(&preferred, &descriptor.id, used_ids);
                object.insert(FLY_RULE_ID_FIELD.to_string(), Value::String(id));
            }
            Some(value)
        })
        .collect()
}

fn record_visible_form(counts: &mut BTreeMap<String, usize>, id: &str) -> bool {
    let count = counts.entry(id.to_string()).or_default();
    *count = count.saturating_add(1);
    *count <= 1
}

fn build_local_context(
    root: &Value,
    item_alias: &str,
    item: Value,
    index_alias: &str,
    index: usize,
) -> Value {
    let mut object = root.as_object().cloned().unwrap_or_default();
    object.insert(item_alias.to_string(), item);
    object.insert(
        index_alias.to_string(),
        Value::Number(Number::from(index as u64)),
    );
    Value::Object(object)
}

/// Resolve a templated DOM id without erasing missing paths or manufacturing an invalid id.
/// Static publication treats failure as blocking; preview may still display the draft diagnostic.
fn interpolate_html_id(value: &str, context: &Value) -> Option<String> {
    let mut output = String::with_capacity(value.len());
    let mut remaining = value;
    while let Some(start) = remaining.find("{{") {
        output.push_str(&remaining[..start]);
        let after = &remaining[start + 2..];
        let end = after.find("}}")?;
        let path = after[..end].trim();
        if path.is_empty() {
            return None;
        }
        let resolved = resolve_path(context, path)?;
        if !matches!(
            resolved,
            Value::String(_) | Value::Number(_) | Value::Bool(_)
        ) {
            return None;
        }
        output.push_str(&scalar_text(resolved));
        remaining = &after[end + 2..];
    }
    output.push_str(remaining);
    // HTML ids are not component ids: Unicode is valid and must survive interpolation.
    // Whitespace/control characters cannot participate safely in IDREF lists or URLs, and
    // leaving template delimiters here could cause a later interpolation pass to run them again.
    if output.is_empty()
        || output
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
        || output.contains("{{")
        || output.contains("}}")
    {
        return None;
    }
    Some(output)
}

fn collect_interpolated_reference_aliases(
    object: &ComponentObject,
    context: &Value,
    mapping: &BTreeMap<String, String>,
    references: &mut BTreeMap<String, String>,
) {
    fn add_alias(
        value: &str,
        context: &Value,
        mapping: &BTreeMap<String, String>,
        references: &mut BTreeMap<String, String>,
    ) {
        if value.contains("{{")
            && let Some(resolved) = interpolate_html_id(value, context)
            && let Some(target) = mapping.get(&resolved)
        {
            references.insert(value.to_string(), target.clone());
        }
    }
    for (name, value) in &object.attributes {
        let Some(value) = value.as_str() else {
            continue;
        };
        let name = name.to_ascii_lowercase();
        if name == "href" {
            if let Some(id) = value.strip_prefix('#') {
                add_alias(id, context, mapping, references);
            }
        } else if crate::id_reference::ID_REFERENCE_LIST_ATTRIBUTES.contains(&name.as_str()) {
            for id in crate::id_reference::id_list_tokens(value) {
                add_alias(id, context, mapping, references);
            }
        } else if crate::id_reference::ID_REFERENCE_ATTRIBUTES.contains(&name.as_str()) {
            add_alias(value, context, mapping, references);
        }
    }
    let Some(action) = object
        .extensions
        .get(crate::FLY_ACTION_FIELD)
        .and_then(Value::as_object)
    else {
        return;
    };
    let fields: &[&str] = match action.get("kind").and_then(Value::as_str) {
        Some("toggle_visibility" | "scroll_to" | "animate" | "toggle_accordion") => &["target_id"],
        Some("switch_tab") => &["panel_id"],
        Some("switch_slide") => &["slider_id", "target_slide"],
        _ => &[],
    };
    for field in fields {
        if let Some(id) = action.get(*field).and_then(Value::as_str) {
            add_alias(id, context, mapping, references);
        }
    }
}

fn template_uses_alias(value: &str, alias: &str) -> bool {
    let mut remaining = value;
    while let Some(start) = remaining.find("{{") {
        let after = &remaining[start + 2..];
        let Some(end) = after.find("}}") else {
            break;
        };
        let path = after[..end].trim();
        let path = path.strip_prefix('$').unwrap_or(path);
        let path = path.strip_prefix('.').unwrap_or(path);
        if path == alias
            || path
                .strip_prefix(alias)
                .is_some_and(|rest| rest.starts_with('.') || rest.starts_with('['))
        {
            return true;
        }
        remaining = &after[end + 2..];
    }
    false
}

fn interpolate_node(node: &mut ComponentNode, context: &Value) {
    match node {
        ComponentNode::Opaque(value) => interpolate_value(value, context),
        ComponentNode::Object(component) => {
            for value in component.attributes.values_mut() {
                interpolate_value(value, context);
            }
            if let Some(style) = component.style.as_mut() {
                interpolate_value(style, context);
            }
            for value in &mut component.traits {
                interpolate_value(value, context);
            }
            for value in component.extensions.values_mut() {
                interpolate_value(value, context);
            }
            if let Some(children) = component.children_mut() {
                for child in children {
                    interpolate_node(child, context);
                }
            }
        }
    }
}

fn interpolate_value(value: &mut Value, context: &Value) {
    match value {
        Value::String(text) => {
            if let Some(path) = exact_template_path(text) {
                if let Some(resolved) = resolve_path(context, path) {
                    *value = resolved.clone();
                }
                return;
            }
            *text = interpolate_text(text, context);
        }
        Value::Array(values) => {
            for value in values {
                interpolate_value(value, context);
            }
        }
        Value::Object(values) => {
            for value in values.values_mut() {
                interpolate_value(value, context);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn exact_template_path(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    let inner = trimmed
        .strip_prefix("{{")
        .and_then(|value| value.strip_suffix("}}"))
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    if inner.contains("{{") || inner.contains("}}") {
        return None;
    }
    Some(inner)
}

fn interpolate_text(value: &str, context: &Value) -> String {
    let mut output = String::with_capacity(value.len());
    let mut remaining = value;
    while let Some(start) = remaining.find("{{") {
        output.push_str(&remaining[..start]);
        let after = &remaining[start + 2..];
        let Some(end) = after.find("}}") else {
            output.push_str(&remaining[start..]);
            return output;
        };
        let path = after[..end].trim();
        if let Some(resolved) = resolve_path(context, path) {
            output.push_str(&scalar_text(resolved));
        }
        remaining = &after[end + 2..];
    }
    output.push_str(remaining);
    output
}

fn scalar_text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.clone(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Array(_) | Value::Object(_) => serde_json::to_string(value).unwrap_or_default(),
    }
}

fn resolve_path<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    resolve_context_path(root, path)
}

fn sanitize_identifier(value: &str) -> String {
    let value = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    let value = value.trim_matches('-');
    if value.is_empty() {
        "repeat".to_string()
    } else {
        value.to_string()
    }
}

fn runtime_diagnostic(
    severity: ValidationSeverity,
    code: impl Into<String>,
    component_id: Option<String>,
    message: impl Into<String>,
) -> ValidationDiagnostic {
    ValidationDiagnostic {
        severity,
        code: code.into(),
        path: component_id
            .as_deref()
            .map(|component_id| format!("component:{component_id}"))
            .unwrap_or_else(|| "project.runtime".to_string()),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GrapesJsCodec, StyleRuleCatalog};
    use serde_json::json;

    fn document() -> ProjectDocument {
        GrapesJsCodec::decode_value(json!({
            "styles": [{
                "selectors": [{ "name": "card", "type": 2 }],
                "style": { "padding": "12px" },
                "flyComponentId": "card"
            }],
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "banner",
                        "type": "section",
                        "content": "Visible"
                    }, {
                        "id": "card",
                        "type": "section",
                        "components": [{
                            "id": "card-title",
                            "type": "heading",
                            "content": "{{item.title}} #{{index}}"
                        }]
                    }]
                }
            }],
            "flyRuntimeConditions": [{
                "id": "show-banner",
                "component_id": "banner",
                "path": "flags.banner",
                "operator": "truthy"
            }],
            "flyRuntimeRepeaters": [{
                "id": "cards",
                "component_id": "card",
                "path": "items",
                "item_alias": "item",
                "index_alias": "index"
            }]
        }))
        .expect("document")
    }

    #[test]
    fn conditions_hide_components_without_mutating_source() {
        let source = document();
        let materialized = materialize_runtime(
            &source,
            &json!({ "flags": { "banner": false }, "items": [] }),
        );
        assert!(source.contains_component("banner"));
        assert!(!materialized.document.contains_component("banner"));
        assert_eq!(materialized.hidden_components, 1);
    }

    #[test]
    fn repeaters_clone_interpolate_and_remap_style_rules() {
        let source = document();
        let materialized = materialize_runtime(
            &source,
            &json!({
                "flags": { "banner": true },
                "items": [{ "title": "One" }, { "title": "Two" }]
            }),
        );
        assert_eq!(materialized.repeated_nodes, 2);
        assert!(materialized.document.contains_component("card--cards-0"));
        assert!(
            materialized
                .document
                .contains_component("card-title--cards-1")
        );
        assert_eq!(
            materialized
                .document
                .component("card-title--cards-1")
                .and_then(|component| component.extensions.get("content"))
                .and_then(Value::as_str),
            Some("Two #1")
        );
        assert!(
            StyleRuleCatalog::from_document(&materialized.document)
                .component_rules("card--cards-0")
                .next()
                .is_some()
        );
        assert!(source.contains_component("card"));
    }

    #[test]
    fn materialized_locations_and_index_reflect_removed_and_repeated_nodes() {
        let source = document();
        let materialized = materialize_runtime(
            &source,
            &json!({
                "flags": { "banner": false },
                "items": [{ "title": "One" }, { "title": "Two" }]
            }),
        );
        let result = &materialized.document;
        assert!(result.component_location("banner").is_none());
        assert!(result.component_location("card").is_none());
        assert!(result.component_location("card--cards-0").is_some());
        assert!(result.component_location("card-title--cards-1").is_some());
        assert!(!result.component_index().contains("card"));
        assert!(result.component_index().contains("card--cards-0"));
        assert!(source.component_location("banner").is_some());
        assert!(source.component_location("card").is_some());
    }

    #[test]
    fn repeater_retargets_selector_only_style_and_preserves_provider_payload() {
        let source = GrapesJsCodec::decode_value(json!({
            "pages": [{"component": {"id": "root", "type": "wrapper", "components": [{"id": "item", "type": "section"}]}}],
            "styles": [{"id": "item-style", "selectors": "#item", "style": {"color": "red"}, "providerData": {"component_id": "item"}}],
            "flyRuntimeRepeaters": [{"id": "items", "component_id": "item", "path": "items"}]
        })).unwrap();
        let result = materialize_runtime(&source, &json!({"items": [1, 2]}));
        assert_eq!(result.repeated_nodes, 2);
        let catalog = StyleRuleCatalog::from_document(&result.document);
        let first = catalog
            .component_rule("item--items-0", &crate::StyleRuleScope::Base)
            .unwrap();
        let second = catalog
            .component_rule("item--items-1", &crate::StyleRuleScope::Base)
            .unwrap();
        assert_ne!(first.id, second.id);
        assert_eq!(first.raw["selectors"], "#item--items-0");
        assert_eq!(second.raw["selectors"], "#item--items-1");
        assert_eq!(first.raw["providerData"]["component_id"], "item");
        let css = crate::render_page(
            &result.document,
            &crate::PageSelection::First,
            &crate::RenderPolicy::default(),
        )
        .unwrap()
        .css;
        assert!(
            css.contains("[data-fly-style-id=\"item--items-0\"]{color:red}"),
            "{css}"
        );
        assert!(
            css.contains("[data-fly-style-id=\"item--items-1\"]{color:red}"),
            "{css}"
        );
    }

    #[test]
    fn repeated_components_keep_distinct_html_ids_and_local_actions() {
        let source = GrapesJsCodec::decode_value(json!({
            "pages": [{"component": {"id": "root", "type": "wrapper", "components": [{
                "id": "item", "type": "container", "components": [
                    {"id": "trigger", "type": "button", "attributes": {"aria-controls": "answer"},
                     "flyAction": {"kind": "toggle_accordion", "target_id": "answer"}},
                    {"id": "panel", "type": "container", "attributes": {"id": "answer"}}
                ]
            }]}}],
            "flyRuntimeRepeaters": [{"id": "items", "component_id": "item", "path": "items", "item_alias": "item", "index_alias": "index"}]
        })).expect("source");
        let result = materialize_runtime(&source, &json!({"items": [1, 2]}));
        let mut ids = Vec::new();
        for index in 0..2 {
            let group = result
                .document
                .component(&format!("item--items-{index}"))
                .unwrap();
            let trigger = group.children()[0].as_object().unwrap();
            let panel = group.children()[1].as_object().unwrap();
            let id = panel.attributes["id"].as_str().unwrap();
            assert_eq!(trigger.attributes["aria-controls"], id);
            assert_eq!(trigger.extensions["flyAction"]["target_id"], id);
            ids.push(id.to_string());
        }
        assert_ne!(ids[0], ids[1]);
        assert_eq!(
            materialize_runtime(&source, &json!({"items": [1, 2]})).document,
            result.document
        );
    }

    #[test]
    fn repeaters_preserve_interpolated_html_ids_and_references() {
        let source = GrapesJsCodec::decode_value(json!({
            "pages": [{"component": {"id": "root", "type": "wrapper", "components": [{
                "id": "item", "type": "container", "components": [
                    {"id": "link", "type": "link", "attributes": {"href": "#item-{{item.slug}}", "aria-controls": "item-{{item.slug}}"}},
                    {"id": "target", "type": "section", "attributes": {"id": "item-{{item.slug}}"}}
                ]
            }]}}],
            "flyRuntimeRepeaters": [{"id": "items", "component_id": "item", "path": "items"}]
        })).expect("source");
        let result = materialize_runtime(
            &source,
            &json!({"items": [{"slug": "apple"}, {"slug": "pear"}]}),
        );
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        for (index, expected) in ["item-apple", "item-pear"].into_iter().enumerate() {
            let item = result
                .document
                .component(&format!("item--items-{index}"))
                .unwrap();
            let link = item.children()[0].as_object().unwrap();
            let target = item.children()[1].as_object().unwrap();
            assert_eq!(target.attributes["id"], expected);
            assert_eq!(link.attributes["href"], format!("#{expected}"));
            assert_eq!(link.attributes["aria-controls"], expected);
        }
    }

    #[test]
    fn repeated_interpolated_html_ids_remain_unique_when_values_collide() {
        let source = GrapesJsCodec::decode_value(json!({
            "pages": [{"component": {"id": "root", "type": "wrapper", "components": [
                {"id": "outside", "type": "section", "attributes": {"id": "item-apple"}},
                {"id": "item", "type": "container", "components": [
                    {"id": "link", "type": "link", "attributes": {"href": "#item-{{item.slug}}", "aria-controls": "item-{{item.slug}}"}},
                    {"id": "target", "type": "section", "attributes": {"id": "item-{{item.slug}}"}}
                ]}
            ]}}],
            "flyRuntimeRepeaters": [{"id": "items", "component_id": "item", "path": "items"}]
        })).expect("source");
        let result = materialize_runtime(
            &source,
            &json!({"items": [{"slug": "apple"}, {"slug": "apple"}]}),
        );
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let mut ids = BTreeSet::from(["item-apple".to_string()]);
        for index in 0..2 {
            let item = result
                .document
                .component(&format!("item--items-{index}"))
                .unwrap();
            let link = item.children()[0].as_object().unwrap();
            let target = item.children()[1].as_object().unwrap();
            let id = target.attributes["id"].as_str().unwrap();
            assert!(ids.insert(id.to_string()), "duplicate {id}");
            assert_eq!(link.attributes["href"], format!("#{id}"));
            assert_eq!(link.attributes["aria-controls"], id);
        }
        assert_eq!(
            materialize_runtime(
                &source,
                &json!({"items": [{"slug": "apple"}, {"slug": "apple"}]})
            )
            .document,
            result.document
        );
    }

    #[test]
    fn dynamic_id_collision_does_not_retarget_literal_external_references() {
        let source = GrapesJsCodec::decode_value(json!({
            "pages": [{"component": {"id": "root", "type": "wrapper", "components": [
                {"id": "item-apple", "type": "section", "attributes": {"id": "item-apple"}},
                {"id": "item", "type": "container", "components": [
                    {"id": "external-link", "type": "link", "attributes": {"href": "#item-apple", "aria-controls": "item-apple"}},
                    {"id": "external-action", "type": "button", "flyAction": {"kind": "scroll_to", "target_id": "item-apple"}},
                    {"id": "local-link", "type": "link", "attributes": {"href": "#item-{{ item.slug }}", "aria-controls": "item-{{ item.slug }}"}},
                    {"id": "target", "type": "section", "attributes": {"id": "item-{{item.slug}}"}}
                ]}
            ]}}],
            "flyRuntimeRepeaters": [{"id": "items", "component_id": "item", "path": "items"}]
        })).unwrap();
        let result = materialize_runtime(&source, &json!({"items": [{"slug": "apple"}]}));
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let item = result.document.component("item--items-0").unwrap();
        let external = item.children()[0].as_object().unwrap();
        assert_eq!(external.attributes["href"], "#item-apple");
        assert_eq!(external.attributes["aria-controls"], "item-apple");
        assert_eq!(
            item.children()[1].as_object().unwrap().extensions["flyAction"]["target_id"],
            "item-apple"
        );
        let local = item.children()[2].as_object().unwrap();
        let id = item.children()[3].as_object().unwrap().attributes["id"]
            .as_str()
            .unwrap();
        assert_ne!(id, "item-apple");
        assert_eq!(local.attributes["href"], format!("#{id}"));
        assert_eq!(local.attributes["aria-controls"], id);
    }

    #[test]
    fn interpolated_html_ids_support_unicode_and_collision_references() {
        let source = GrapesJsCodec::decode_value(json!({
            "pages": [{"component": {"id": "root", "type": "wrapper", "components": [{
                "id": "item", "type": "container", "components": [
                    {"id": "link", "type": "link", "attributes": {"href": "#item-{{ item.slug }}", "aria-controls": "item-{{ item.slug }}"}},
                    {"id": "target", "type": "section", "attributes": {"id": "item-{{item.slug}}"}}
                ]
            }]}}],
            "flyRuntimeRepeaters": [{"id": "items", "component_id": "item", "path": "items"}]
        })).unwrap();
        let result = materialize_runtime(
            &source,
            &json!({"items": [{"slug": "яблоко"}, {"slug": "яблоко"}]}),
        );
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let mut ids = BTreeSet::new();
        for (index, node) in result
            .document
            .component("root")
            .unwrap()
            .children()
            .iter()
            .enumerate()
        {
            let item = node.as_object().unwrap();
            let id = item.children()[1].as_object().unwrap().attributes["id"]
                .as_str()
                .unwrap();
            if index == 0 {
                assert_eq!(id, "item-яблоко");
            }
            assert!(ids.insert(id.to_string()));
            assert_eq!(
                item.children()[0].as_object().unwrap().attributes["href"],
                format!("#{id}")
            );
            assert_eq!(
                item.children()[0].as_object().unwrap().attributes["aria-controls"],
                id
            );
        }
    }

    #[test]
    fn collision_rewrites_references_with_equivalent_template_spellings() {
        let source = GrapesJsCodec::decode_value(json!({
            "pages": [{"component": {"id": "root", "type": "wrapper", "components": [
                {"id": "item", "type": "container", "components": [
                    {"id": "link", "type": "link", "attributes": {
                        "href": "#item-{{ item.slug }}", "aria-controls": "item-{{ item.slug }}"
                    }},
                    {"id": "target", "type": "section", "attributes": {"id": "item-{{item.slug}}"}}
                ]}
            ]}}],
            "flyRuntimeRepeaters": [{"id": "items", "component_id": "item", "path": "items"}]
        }))
        .unwrap();
        let result = materialize_runtime(
            &source,
            &json!({"items": [{"slug": "same"}, {"slug": "same"}]}),
        );
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        for item in result.document.component("root").unwrap().children() {
            let item = item.as_object().unwrap();
            let link = item.children()[0].as_object().unwrap();
            let id = item.children()[1].as_object().unwrap().attributes["id"]
                .as_str()
                .unwrap();
            assert_eq!(link.attributes["href"], format!("#{id}"));
            assert_eq!(link.attributes["aria-controls"], id);
        }
    }

    #[test]
    fn interpolated_collision_with_another_live_node_is_blocking() {
        let source = GrapesJsCodec::decode_value(json!({
            "pages": [{"component": {"id": "root", "type": "wrapper", "components": [{
                "id": "item", "type": "container", "components": [
                    {"id": "static", "type": "section", "attributes": {"id": "fixed"}},
                    {"id": "dynamic", "type": "section", "attributes": {"id": "{{item.id}}"}}
                ]
            }]}}],
            "flyRuntimeRepeaters": [{"id": "items", "component_id": "item", "path": "items"}]
        }))
        .unwrap();
        let result = materialize_runtime(&source, &json!({"items": [{"id": "fixed--items-0"}]}));
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| d.code == "runtime_repeater_html_id_ambiguous"
                    && d.severity == ValidationSeverity::Error)
        );
    }

    #[test]
    fn dynamic_id_references_support_spaces_inside_template_expressions() {
        let source = GrapesJsCodec::decode_value(json!({
            "pages": [{"component": {"id": "root", "type": "wrapper", "components": [
                {"id": "outside", "type": "section", "attributes": {"id": "item-apple"}},
                {"id": "item", "type": "container", "components": [
                    {"id": "link", "type": "link", "attributes": {
                        "href": "#item-{{ item.slug }}", "aria-controls": "helper item-{{ item.slug }}"
                    }},
                    {"id": "target", "type": "section", "attributes": {"id": "item-{{ item.slug }}"}}
                ]}
            ]}}],
            "flyRuntimeRepeaters": [{"id": "items", "component_id": "item", "path": "items"}]
        })).expect("source");
        let result = materialize_runtime(&source, &json!({"items": [{"slug": "apple"}]}));
        let item = result.document.component("item--items-0").unwrap();
        let link = item.children()[0].as_object().unwrap();
        let id = item.children()[1].as_object().unwrap().attributes["id"]
            .as_str()
            .unwrap();
        assert_ne!(id, "item-apple");
        assert_eq!(link.attributes["href"], format!("#{id}"));
        assert_eq!(link.attributes["aria-controls"], format!("helper {id}"));
    }

    #[test]
    fn unresolved_visible_dynamic_html_ids_are_blocking_but_hidden_nodes_are_ignored() {
        let source = GrapesJsCodec::decode_value(json!({
            "pages": [{"component": {"id": "root", "type": "wrapper", "components": [{
                "id": "item", "type": "container", "components": [
                    {"id": "target", "type": "section", "attributes": {"id": "item-{{item.slug}}"}}
                ]
            }]}}],
            "flyRuntimeConditions": [{"id": "show", "component_id": "target", "path": "item.show", "operator": "truthy"}],
            "flyRuntimeRepeaters": [{"id": "items", "component_id": "item", "path": "items"}]
        })).unwrap();
        let valid = materialize_runtime(
            &source,
            &json!({"items": [
                {"show": false}, {"show": true, "slug": "apple"}
            ]}),
        );
        assert!(valid.diagnostics.is_empty(), "{:?}", valid.diagnostics);
        assert_eq!(
            valid
                .document
                .component("target--items-1")
                .unwrap()
                .attributes["id"],
            "item-apple"
        );
        let missing = materialize_runtime(&source, &json!({"items": [{"show": true}]}));
        assert!(missing.diagnostics.iter().any(|diagnostic| diagnostic.code
            == "runtime_repeater_html_id_invalid"
            && diagnostic.severity == ValidationSeverity::Error));
    }

    #[test]
    fn nested_repeaters_resolve_dynamic_ids_in_their_own_scope() {
        let source = GrapesJsCodec::decode_value(json!({
            "pages": [{"component": {"id": "root", "type": "wrapper", "components": [
                {"id": "outside", "type": "section", "attributes": {"id": "row-parent"}},
                {"id": "row", "type": "container", "attributes": {"id": "row-{{row.slug}}"}, "components": [
                    {"id": "entry", "type": "container", "attributes": {"id": "entry-{{item.slug}}"}, "components": [
                        {"id": "entry-link", "type": "link", "attributes": {"href": "#entry-{{item.slug}}"}},
                        {"id": "row-link", "type": "link", "attributes": {"href": "#row-{{row.slug}}"}}
                    ]}
                ]}
            ]}}],
            "flyRuntimeRepeaters": [
                {"id": "rows", "component_id": "row", "path": "rows", "item_alias": "row"},
                {"id": "entries", "component_id": "entry", "path": "row.entries", "item_alias": "item"}
            ]
        })).expect("source");
        let result = materialize_runtime(
            &source,
            &json!({
                "item": {"slug": "global"},
                "rows": [{"slug": "parent", "entries": [{"slug": "apple"}, {"slug": "pear"}]}]
            }),
        );
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let row = result.document.component("row--rows-0").unwrap();
        let row_id = row.attributes["id"].as_str().unwrap();
        assert_ne!(row_id, "row-parent"); // already owned by the outside node
        for (entry, expected) in row.children().iter().zip(["entry-apple", "entry-pear"]) {
            let entry = entry.as_object().unwrap();
            assert_eq!(entry.attributes["id"], expected);
            assert_eq!(
                entry.children()[0].as_object().unwrap().attributes["href"],
                format!("#{expected}")
            );
            assert_eq!(
                entry.children()[1].as_object().unwrap().attributes["href"],
                format!("#{row_id}")
            );
        }
    }

    #[test]
    fn nested_repeater_alias_shadowing_is_preserved_for_dynamic_references() {
        let source = GrapesJsCodec::decode_value(json!({
            "pages": [{"component": {"id": "root", "type": "wrapper", "components": [{
                "id": "outer", "type": "container", "attributes": {"id": "ref-{{item.slug}}"}, "components": [{
                    "id": "inner", "type": "container", "components": [
                        {"id": "link", "type": "link", "attributes": {"href": "#ref-{{item.slug}}"}}
                    ]
                }]
            }]}}],
            "flyRuntimeRepeaters": [
                {"id": "outer-repeat", "component_id": "outer", "path": "rows", "item_alias": "item"},
                {"id": "inner-repeat", "component_id": "inner", "path": "item.entries", "item_alias": "item"}
            ]
        })).expect("source");
        let result = materialize_runtime(
            &source,
            &json!({"rows": [{"slug": "parent", "entries": [{"slug": "child"}]}]}),
        );
        let outer = result.document.component("outer--outer-repeat-0").unwrap();
        assert_eq!(outer.attributes["id"], "ref-parent");
        let inner = outer.children()[0].as_object().unwrap();
        assert_eq!(
            inner.children()[0].as_object().unwrap().attributes["href"],
            "#ref-child"
        );
    }

    #[test]
    fn repeaters_avoid_existing_component_and_html_names() {
        let source = GrapesJsCodec::decode_value(json!({
            "styles": [{"flyComponentId": "panel", "flyRuleId": "rule", "style": {"color": "red"}}],
            "pages": [{"component": {"id": "root", "type": "wrapper", "components": [
                {"id": "item--items-0", "type": "text", "attributes": {"id": "panel--items-1"}},
                {"id": "item", "type": "container", "components": [
                    {"id": "trigger", "type": "button", "flyAction": {"kind": "scroll_to", "target_id": "panel"}},
                    {"id": "panel", "type": "section", "attributes": {"id": "panel"}}
                ]}
            ]}}],
            "flyRuntimeRepeaters": [{"id": "items", "component_id": "item", "path": "items"}]
        })).expect("source");
        let result = materialize_runtime(&source, &json!({"items": [1, 2]}));
        let root = result.document.component("root").unwrap();
        let mut seen = BTreeSet::from(["item--items-0".to_string(), "panel--items-1".to_string()]);
        for node in &root.children()[1..] {
            let item = node.as_object().unwrap();
            assert!(seen.insert(item.id.as_ref().unwrap().clone()));
            let trigger = item.children()[0].as_object().unwrap();
            let panel = item.children()[1].as_object().unwrap();
            let id = panel.id.as_ref().unwrap();
            assert!(seen.insert(id.clone()));
            assert_eq!(trigger.extensions["flyAction"]["target_id"], id.as_str());
            assert_eq!(panel.attributes["id"], id.as_str());
            assert!(
                StyleRuleCatalog::from_document(&result.document)
                    .component_rules(id)
                    .next()
                    .is_some()
            );
        }
    }

    #[test]
    fn nested_repeaters_preserve_local_dom_references() {
        let source = GrapesJsCodec::decode_value(json!({
            "pages": [{"component": {"id": "root", "type": "wrapper", "components": [{
                "id": "outer", "type": "container", "components": [{
                    "id": "inner", "type": "container", "components": [
                        {"id": "trigger", "type": "button", "attributes": {"aria-controls": "answer"},
                         "flyAction": {"kind": "toggle_accordion", "target_id": "answer"}},
                        {"id": "panel", "type": "section", "attributes": {"id": "answer"}}
                    ]
                }]
            }]}}],
            "flyRuntimeRepeaters": [
                {"id": "outer-repeat", "component_id": "outer", "path": "items", "item_alias": "item"},
                {"id": "inner-repeat", "component_id": "inner", "path": "item.children", "item_alias": "child"}
            ]
        })).expect("source");
        let context = json!({"items": [{"children": [1, 2]}, {"children": [3]}]});
        let result = materialize_runtime(&source, &context);
        let root = result.document.component("root").unwrap();
        let mut seen = BTreeSet::new();
        for outer in root.children() {
            for inner in outer.as_object().unwrap().children() {
                let inner = inner.as_object().unwrap();
                let trigger = inner.children()[0].as_object().unwrap();
                let panel = inner.children()[1].as_object().unwrap();
                let id = panel.attributes["id"].as_str().unwrap();
                assert!(seen.insert(id.to_string()));
                assert_eq!(trigger.attributes["aria-controls"], id);
                assert_eq!(trigger.extensions["flyAction"]["target_id"], id);
            }
        }
        assert_eq!(seen.len(), 3);
        assert_eq!(
            materialize_runtime(&source, &context).document,
            result.document
        );
    }

    #[test]
    fn nested_repeater_styles_are_unique_bounded_and_only_target_survivors() {
        let mut source = nested_document();
        let long_rule = "r".repeat(crate::MAXIMUM_IDENTIFIER_LENGTH);
        source.project.styles = vec![
            json!({"flyComponentId": "tag", "flyRuleId": long_rule, "style": {"color": "red"}}),
            json!({"flyComponentId": "card-title", "flyRuleId": "title-rule", "style": {"color": "blue"}}),
        ];
        source.project.extensions.insert(FLY_RUNTIME_CONDITIONS_FIELD.to_string(), json!([{
            "id": "show-title", "component_id": "card-title", "path": "item.featured", "operator": "truthy"
        }]));
        let result = materialize_runtime(
            &source,
            &json!({"items": [
                {"title": "One", "featured": true, "tags": ["a", "b"]},
                {"title": "Two", "featured": false, "tags": ["c"]}
            ]}),
        );
        let catalog = StyleRuleCatalog::from_document(&result.document);
        let mut ids = BTreeSet::new();
        for rule in &catalog.rules {
            let target = rule.component_id.as_deref().unwrap();
            assert!(
                result.document.contains_component(target),
                "orphan style for {target}"
            );
            assert!(
                crate::validate_identifier(&rule.id).is_ok(),
                "invalid rule id {}",
                rule.id
            );
            assert!(ids.insert(rule.id.clone()), "duplicate rule id {}", rule.id);
        }
        assert_eq!(catalog.rules.len(), 4); // three tags, one surviving title
        assert_eq!(
            materialize_runtime(
                &source,
                &json!({"items": [
                    {"title": "One", "featured": true, "tags": ["a", "b"]},
                    {"title": "Two", "featured": false, "tags": ["c"]}
                ]})
            )
            .document,
            result.document
        );
    }

    #[test]
    fn repeater_rejects_duplicate_component_html_and_form_identities_without_losing_styles() {
        let variants = [
            json!([
                {"id": "duplicate", "type": "text"},
                {"id": "duplicate", "type": "text"}
            ]),
            json!([
                {"id": "first", "type": "text", "attributes": {"id": "duplicate"}},
                {"id": "second", "type": "text", "attributes": {"id": "duplicate"}}
            ]),
            json!([
                {"id": "first", "type": "form", "flyForm": {"id": "duplicate", "provider": "forms", "action": "submit"}},
                {"id": "second", "type": "form", "flyForm": {"id": "duplicate", "provider": "forms", "action": "submit"}}
            ]),
        ];
        for children in variants {
            let source = GrapesJsCodec::decode_value(json!({
                "styles": [{"flyComponentId": "item", "flyRuleId": "item-rule", "style": {"color": "red"}}],
                "pages": [{"component": {"id": "root", "type": "wrapper", "components": [{
                    "id": "item", "type": "container", "components": children
                }]}}],
                "flyRuntimeRepeaters": [{"id": "items", "component_id": "item", "path": "items"}]
            })).expect("source");
            let result = materialize_runtime(&source, &json!({"items": [1, 2]}));
            assert_eq!(result.repeated_nodes, 0);
            assert_eq!(result.document, source);
            assert!(result.diagnostics.iter().any(|diagnostic| diagnostic.code
                == "runtime_repeater_identity_ambiguous"
                && diagnostic.severity == ValidationSeverity::Error));
        }
    }

    #[test]
    fn repeater_reports_cross_node_form_html_collision_even_for_one_item() {
        let source = GrapesJsCodec::decode_value(json!({
            "pages": [{"component": {"id": "root", "type": "wrapper", "components": [{
                "id": "item", "type": "container", "components": [
                    {"id": "form-node", "type": "form", "flyForm": {"id": "contact", "provider": "forms", "action": "submit"}},
                    {"id": "other", "type": "section", "attributes": {"id": "contact"}}
                ]
            }]}}],
            "flyRuntimeRepeaters": [{"id": "items", "component_id": "item", "path": "items"}]
        })).unwrap();
        let result = materialize_runtime(&source, &json!({"items": [1]}));
        assert_eq!(result.repeated_nodes, 0);
        assert_eq!(result.document, source);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "runtime_repeater_identity_ambiguous")
        );
    }

    #[test]
    fn repeater_expands_when_item_conditions_keep_at_most_one_form() {
        let source = GrapesJsCodec::decode_value(json!({
            "pages": [{"component": {"id": "root", "type": "wrapper", "components": [{
                "id": "card", "type": "section", "components": [
                    {"id": "name", "type": "text", "content": "{{item.name}}"},
                    {"id": "form", "type": "form", "flyForm": {"id": "contact", "action_url": "/contact"}}
                ]
            }]}}],
            "flyRuntimeConditions": [{"id": "show-form", "component_id": "form", "path": "item.showForm", "operator": "truthy"}],
            "flyRuntimeRepeaters": [{"id": "items", "component_id": "card", "path": "items"}]
        })).unwrap();
        for (flags, expected_forms) in [(vec![true, false], 1), (vec![false, false], 0)] {
            let context = json!({"items": [
                {"name": "Apple", "showForm": flags[0]}, {"name": "Pear", "showForm": flags[1]}
            ]});
            let result = crate::materialize_project_with_runtime_context(&source, &context);
            assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
            assert_eq!(result.repeated_nodes, 2);
            let html = crate::render_page(
                &result.document,
                &crate::PageSelection::First,
                &crate::RenderPolicy::default(),
            )
            .unwrap()
            .html;
            assert!(html.contains("Apple") && html.contains("Pear"), "{html}");
            assert_eq!(
                html.matches(" id=\"contact\"").count(),
                expected_forms,
                "{html}"
            );
            assert!(!html.contains("{{item.name}}"), "{html}");
        }
        let duplicate = materialize_runtime(
            &source,
            &json!({"items": [
                {"showForm": true}, {"showForm": true}
            ]}),
        );
        assert!(
            duplicate
                .diagnostics
                .iter()
                .any(|d| d.code == "runtime_repeater_form_identity_ambiguous"
                    && d.severity == ValidationSeverity::Error)
        );
    }

    #[test]
    fn nested_repeater_form_count_uses_each_inner_context() {
        let source = GrapesJsCodec::decode_value(json!({
            "pages": [{"component": {"id": "root", "type": "wrapper", "components": [{
                "id": "row", "type": "section", "components": [{
                    "id": "entry", "type": "container", "components": [
                        {"id": "label", "type": "text", "content": "{{child.name}}"},
                        {"id": "form", "type": "form", "flyForm": {"id": "contact", "action_url": "/contact"}}
                    ]
                }]
            }]}}],
            "flyRuntimeConditions": [{"id": "show-form", "component_id": "form", "path": "child.show", "operator": "truthy"}],
            "flyRuntimeRepeaters": [
                {"id": "rows", "component_id": "row", "path": "rows", "item_alias": "item"},
                {"id": "entries", "component_id": "entry", "path": "item.entries", "item_alias": "child"}
            ]
        })).unwrap();
        let context = json!({"rows": [
            {"entries": [{"name": "Apple", "show": true}]},
            {"entries": [{"name": "Pear", "show": false}]}
        ]});
        let result = crate::materialize_project_with_runtime_context(&source, &context);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(result.repeated_nodes, 4);
        let html = crate::render_page(
            &result.document,
            &crate::PageSelection::First,
            &crate::RenderPolicy::default(),
        )
        .unwrap()
        .html;
        assert!(html.contains("Apple") && html.contains("Pear"), "{html}");
        assert_eq!(html.matches(" id=\"contact\"").count(), 1, "{html}");
        let duplicate = materialize_runtime(
            &source,
            &json!({"rows": [
                {"entries": [{"show": true}]}, {"entries": [{"show": true}]}
            ]}),
        );
        assert!(
            duplicate
                .diagnostics
                .iter()
                .any(|d| d.code == "runtime_repeater_form_identity_ambiguous"
                    && d.severity == ValidationSeverity::Error)
        );
    }

    #[test]
    fn repeater_refuses_to_duplicate_provider_form_identity() {
        let source = GrapesJsCodec::decode_value(json!({
            "pages": [{"component": {"id": "root", "type": "wrapper", "components": [{
                "id": "item", "type": "form", "flyForm": {"id": "contact", "provider": "forms", "action": "submit"}
            }]}}],
            "flyRuntimeRepeaters": [{"id": "items", "component_id": "item", "path": "items"}]
        })).expect("source");
        let result = materialize_runtime(&source, &json!({"items": [1, 2]}));
        assert_eq!(result.repeated_nodes, 0);
        assert!(result.document.contains_component("item"));
        assert!(result.diagnostics.iter().any(|diagnostic| diagnostic.code
            == "runtime_repeater_form_identity_ambiguous"
            && diagnostic.severity == ValidationSeverity::Error));
    }

    #[test]
    fn commands_preserve_unknown_entries() {
        let mut document = document();
        document.project.extensions.insert(
            FLY_RUNTIME_CONDITIONS_FIELD.to_string(),
            json!([{ "providerCondition": true }]),
        );
        apply_dynamic_command(
            &mut document,
            &DynamicCommand::UpsertCondition {
                condition: RuntimeCondition {
                    id: "show-card".to_string(),
                    component_id: "card".to_string(),
                    path: "flags.card".to_string(),
                    operator: ConditionOperator::Truthy,
                    expected: None,
                    invert: false,
                    extensions: Map::new(),
                },
            },
        )
        .expect("upsert condition");
        let entries = document.project.extensions[FLY_RUNTIME_CONDITIONS_FIELD]
            .as_array()
            .expect("condition array");
        assert_eq!(entries.len(), 2);
        assert!(
            entries
                .iter()
                .any(|entry| entry.get("providerCondition").is_some())
        );
    }

    #[test]
    fn runtime_definition_paths_use_strict_context_path_contract() {
        let mut document = document();
        let error = apply_dynamic_command(
            &mut document,
            &DynamicCommand::UpsertCondition {
                condition: RuntimeCondition {
                    id: "bad-path".to_string(),
                    component_id: "card".to_string(),
                    path: "items[0".to_string(),
                    operator: ConditionOperator::Truthy,
                    expected: None,
                    invert: false,
                    extensions: Map::new(),
                },
            },
        )
        .expect_err("invalid path");
        assert!(matches!(error, FlyError::Decode(_)));
    }

    #[test]
    fn materialization_skips_repeaters_with_empty_aliases() {
        let mut source = document();
        source.project.extensions.insert(
            FLY_RUNTIME_REPEATERS_FIELD.to_string(),
            json!([{ "id": "bad", "component_id": "card", "path": "items", "item_alias": "" }]),
        );
        let materialized = materialize_runtime(&source, &json!({ "items": [{ "title": "One" }] }));
        assert_eq!(materialized.repeated_nodes, 0);
        assert!(materialized.document.contains_component("card"));
        assert!(
            materialized
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.code == "runtime_repeater_alias_empty" })
        );
    }

    fn nested_document() -> ProjectDocument {
        GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "card",
                        "type": "section",
                        "components": [
                            { "id": "card-title", "type": "heading", "content": "{{item.title}}" },
                            { "id": "tag", "type": "span", "content": "{{tag}}" }
                        ]
                    }]
                }
            }],
            "flyRuntimeRepeaters": [{
                "id": "cards",
                "component_id": "card",
                "path": "items",
                "item_alias": "item",
                "index_alias": "index"
            }, {
                "id": "tags",
                "component_id": "tag",
                "path": "item.tags",
                "item_alias": "tag",
                "index_alias": "tagIndex"
            }]
        }))
        .expect("document")
    }

    #[test]
    fn nested_repeaters_expand_against_their_own_item_context() {
        // Previously impossible in either processing order: expanding the outer repeater first
        // renamed `tag` out of existence, and expanding the inner one first gave every outer copy
        // the same inner data.
        let materialized = materialize_runtime(
            &nested_document(),
            &json!({
                "items": [
                    { "title": "One", "tags": ["a", "b"] },
                    { "title": "Two", "tags": ["c"] }
                ]
            }),
        );

        let document = &materialized.document;
        for id in [
            "card--cards-0",
            "card--cards-1",
            "tag--cards-0--tags-0",
            "tag--cards-0--tags-1",
            "tag--cards-1--tags-0",
        ] {
            assert!(document.contains_component(id), "missing {id}");
        }

        // The second outer item has one tag, so a third inner clone must not exist.
        assert!(!document.contains_component("tag--cards-1--tags-1"));

        let content = |id: &str| {
            document
                .component(id)
                .and_then(|component| component.extensions.get("content"))
                .and_then(Value::as_str)
                .map(ToString::to_string)
        };
        assert_eq!(content("tag--cards-0--tags-0").as_deref(), Some("a"));
        assert_eq!(content("tag--cards-0--tags-1").as_deref(), Some("b"));
        assert_eq!(content("tag--cards-1--tags-0").as_deref(), Some("c"));
        assert_eq!(content("card-title--cards-1").as_deref(), Some("Two"));

        // 2 outer clones + 3 inner clones.
        assert_eq!(materialized.repeated_nodes, 5);
    }

    #[test]
    fn conditions_inside_a_repeater_are_evaluated_per_item() {
        // A condition on `item.*` cannot be evaluated against the global context: the path does
        // not resolve there, so the old global pre-pass judged it falsy and deleted the template
        // for every item.
        let mut source = nested_document();
        source.project.extensions.insert(
            FLY_RUNTIME_CONDITIONS_FIELD.to_string(),
            json!([{
                "id": "show-title",
                "component_id": "card-title",
                "path": "item.featured",
                "operator": "truthy"
            }]),
        );

        let materialized = materialize_runtime(
            &source,
            &json!({
                "items": [
                    { "title": "One", "featured": true, "tags": [] },
                    { "title": "Two", "featured": false, "tags": [] }
                ]
            }),
        );

        assert!(
            materialized
                .document
                .contains_component("card-title--cards-0"),
            "featured item lost its title"
        );
        assert!(
            !materialized
                .document
                .contains_component("card-title--cards-1"),
            "unfeatured item kept its title"
        );
        assert_eq!(materialized.hidden_components, 1);
    }

    #[test]
    fn nesting_beyond_the_depth_cap_is_reported_rather_than_expanded() {
        // A chain of repeaters deeper than the cap. Every level repeats over the same top-level
        // `items` key, which stays visible in each local context, so each level has exactly one
        // item to expand and the chain is bounded only by MAX_REPEATER_DEPTH.
        let depth = MAX_REPEATER_DEPTH + 2;
        let mut component = json!({ "id": "node-0", "type": "span", "content": "leaf" });
        let mut repeaters = Vec::new();
        for level in 0..depth {
            if level > 0 {
                component = json!({
                    "id": format!("node-{level}"),
                    "type": "section",
                    "components": [component]
                });
            }
            repeaters.push(json!({
                "id": format!("rep-{level}"),
                "component_id": format!("node-{level}"),
                "path": "items",
                "item_alias": "item",
                "index_alias": "index"
            }));
        }

        let source = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": { "id": "root", "type": "wrapper", "components": [component] }
            }],
            "flyRuntimeRepeaters": repeaters
        }))
        .expect("document");

        let materialized = materialize_runtime(&source, &json!({ "items": [{ "n": 1 }] }));

        assert!(
            materialized
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "runtime_repeater_depth_exceeded"),
            "{:?}",
            materialized.diagnostics
        );
        // The cap must bound the work, not merely warn about it.
        assert!(materialized.repeated_nodes <= MAX_REPEATER_DEPTH);
    }
}
