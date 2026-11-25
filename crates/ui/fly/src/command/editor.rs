use super::{AssetCommand, EditorCommand, History, HistoryEntry, RevisionState};
use crate::{
    AssetDescriptor, FlyError, FlyResult, ProjectDocument, ProjectSnapshot, RegistrySet,
    UniqueIdGenerator, ValidationLimits, ValidationReport, apply_binding_command,
    apply_context_command, apply_dynamic_command, apply_page_command, apply_style_rule_command,
    apply_symbol_command, apply_translation_command,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FlyEditor {
    /// Shared with history entries rather than copied into them.
    ///
    /// Every applied command produced three deep copies of the whole project; the document that
    /// becomes `self.document` is byte-identical to the entry's `after`, and the one it displaces
    /// is byte-identical to the next entry's `before`. Behind an `Arc` those are the same
    /// allocation, which leaves exactly one deep copy per command — the speculative one a command
    /// has to mutate — and none at all for undo and redo.
    document: Arc<ProjectDocument>,
    registries: RegistrySet,
    // Host policy is runtime configuration, not part of the persisted project/editor snapshot.
    #[serde(skip, default = "crate::EmbedProviderRegistry::with_builtins")]
    embed_registry: crate::EmbedProviderRegistry,
    selection: Option<String>,
    history: History,
    revision: RevisionState,
    validation_limits: ValidationLimits,
    pub(crate) id_generator: UniqueIdGenerator,
    /// Last full validation report, so selection-only commands can return it instead of
    /// re-validating the whole document per click. Selection cannot change validity;
    /// anything that changes the document or the registries clears or refreshes this.
    #[serde(skip)]
    last_report: Option<ValidationReport>,
}

impl FlyEditor {
    pub fn new(mut document: ProjectDocument, registries: RegistrySet) -> Self {
        let mut id_generator = UniqueIdGenerator::default();
        document.ensure_stable_ids(&mut id_generator);
        let revision = RevisionState::new(&document);
        Self {
            document: Arc::new(document),
            registries,
            embed_registry: crate::EmbedProviderRegistry::with_builtins(),
            selection: None,
            history: History::new(100),
            revision,
            validation_limits: ValidationLimits::default(),
            id_generator,
            last_report: None,
        }
    }

    pub fn with_history_limit(mut self, limit: usize) -> Self {
        self.history = History::new(limit);
        self
    }

    /// Cap how many bytes of retained document state undo history may hold.
    ///
    /// An entry count alone does not bound memory: each entry keeps a full before/after document
    /// pair, so a large project multiplies the limit by its own size.
    pub fn with_history_memory_budget(mut self, memory_budget_bytes: usize) -> Self {
        self.history = std::mem::replace(&mut self.history, History::new(1))
            .with_memory_budget(memory_budget_bytes);
        self
    }

    pub fn with_validation_limits(mut self, limits: ValidationLimits) -> Self {
        self.validation_limits = limits;
        self.last_report = None;
        self
    }

    /// Use a host-provided embed catalog for command and history validation.
    ///
    /// Pass the same registry to the corresponding render function so validation and rendering
    /// enforce one provider/host/path policy.
    pub fn with_embed_registry(mut self, registry: crate::EmbedProviderRegistry) -> Self {
        self.embed_registry = registry;
        self.last_report = None;
        self
    }

    pub fn embed_registry(&self) -> &crate::EmbedProviderRegistry {
        &self.embed_registry
    }

    pub fn document(&self) -> &ProjectDocument {
        &self.document
    }

    pub fn registries(&self) -> &RegistrySet {
        &self.registries
    }

    pub fn registries_mut(&mut self) -> &mut RegistrySet {
        self.last_report = None;
        &mut self.registries
    }

    pub fn selection(&self) -> Option<&str> {
        self.selection.as_deref()
    }

    pub fn history(&self) -> &History {
        &self.history
    }

    pub fn revision(&self) -> &RevisionState {
        &self.revision
    }

    pub fn revision_mut(&mut self) -> &mut RevisionState {
        &mut self.revision
    }

    pub fn validate(&self) -> ValidationReport {
        self.validate_document(&self.document)
    }

    fn validate_document(&self, document: &ProjectDocument) -> ValidationReport {
        crate::validate_project_with_embed_registry(
            document,
            &self.registries,
            self.validation_limits,
            &self.embed_registry,
        )
    }

    pub fn restore_snapshot(&mut self, snapshot: &ProjectSnapshot) -> FlyResult<ValidationReport> {
        self.apply(EditorCommand::restore_snapshot(snapshot.clone()))
    }

    pub fn apply(&mut self, mut command: EditorCommand) -> FlyResult<ValidationReport> {
        if let EditorCommand::Select { component_id } = &command {
            if let Some(id) = component_id.as_deref()
                && !self.document.contains_component(id)
            {
                return Err(FlyError::ComponentNotFound(id.to_string()));
            }
            self.selection = component_id.clone();
            if let Some(report) = &self.last_report {
                return Ok(report.clone());
            }
            let report = self.validate();
            self.last_report = Some(report.clone());
            return Ok(report);
        }

        // A verified no-op must not consume a history slot or advance the
        // revision. Still validate: an empty command cannot legalize a document
        // that was already invalid when the editor was constructed.
        if matches!(&command, EditorCommand::Batch { commands } if commands.is_empty()) {
            let report = self.last_report.clone().unwrap_or_else(|| self.validate());
            let errors = report.errors().cloned().collect::<Vec<_>>();
            if !errors.is_empty() {
                return Err(FlyError::Validation(errors));
            }
            self.last_report = Some(report.clone());
            return Ok(report);
        }

        // Keep even ID allocation transactional: a refused edit must not advance editor state.
        let mut generator = self.id_generator.clone();

        // A patch can change project-wide invariants (form identity, action targets,
        // parent/child contracts, layout slots and runtime bindings). The prior report
        // cannot prove those invariants after a mutation. Use one speculative document
        // and one canonical validation path for every mutating command.
        let mut after = (*self.document).clone();
        Self::apply_to_document(&mut after, &mut command, &mut generator, false)?;
        after.ensure_stable_ids(&mut generator);
        let report = self.validate_document(&after);
        let errors = report.errors().cloned().collect::<Vec<_>>();
        if !errors.is_empty() {
            return Err(FlyError::Validation(errors));
        }

        // An idempotent edit may pass validation without changing any project
        // data. Preserve the existing revision and redo history in that case.
        if after == *self.document {
            self.last_report = Some(report.clone());
            return Ok(report);
        }

        // The command now carries any IDs reissued during insertion, so its inverse names the
        // inserted component rather than a pre-existing component with the same original id.
        let inverse = invert_command(&self.document, &command);
        self.id_generator = generator;

        // Install the new document and reuse the same allocation for the history entry. The
        // displaced document becomes the entry's `before`, which the previous entry already
        // holds as its `after` — so consecutive entries share, and history memory roughly halves.
        let after = Arc::new(after);
        let history_after = Arc::clone(&after);
        let before = std::mem::replace(&mut self.document, after);
        self.selection = self
            .selection
            .take()
            .filter(|id| self.document.contains_component(id));
        self.history.push(HistoryEntry {
            command,
            before,
            after: history_after,
            inverse,
        });
        self.revision.mark_changed(&self.document);
        self.last_report = Some(report.clone());
        Ok(report)
    }

    pub fn undo(&mut self) -> FlyResult<&ProjectDocument> {
        let entry = self.history.peek_undo().ok_or(FlyError::UndoHistoryEmpty)?;
        let report = self.validate_history_transition(&entry.after, &entry.before, "undo")?;
        let (entry, size) = self.history.pop_undo()?;
        self.document = Arc::clone(&entry.before);
        self.history.push_redo(entry, size);
        self.selection = self
            .selection
            .take()
            .filter(|id| self.document.contains_component(id));
        self.revision.mark_changed(&self.document);
        self.last_report = Some(report);
        Ok(&self.document)
    }

    pub fn redo(&mut self) -> FlyResult<&ProjectDocument> {
        let entry = self.history.peek_redo().ok_or(FlyError::RedoHistoryEmpty)?;
        let report = self.validate_history_transition(&entry.before, &entry.after, "redo")?;
        let (entry, size) = self.history.pop_redo()?;
        self.document = Arc::clone(&entry.after);
        self.history.push_undo(entry, size);
        self.selection = self
            .selection
            .take()
            .filter(|id| self.document.contains_component(id));
        self.revision.mark_changed(&self.document);
        self.last_report = Some(report);
        Ok(&self.document)
    }

    /// History is serialized independently of the live document. Verify its
    /// endpoint and the current host policy before moving either stack.
    fn validate_history_transition(
        &self,
        expected: &ProjectDocument,
        candidate: &ProjectDocument,
        operation: &str,
    ) -> FlyResult<ValidationReport> {
        if expected != self.document.as_ref() {
            return Err(FlyError::Validation(vec![crate::ValidationDiagnostic {
                severity: crate::ValidationSeverity::Error,
                code: "history_base_mismatch".to_string(),
                path: format!("history.{operation}"),
                message: "history entry does not belong to the current document state".to_string(),
            }]));
        }
        let report = self.validate_document(candidate);
        let errors = report.errors().cloned().collect::<Vec<_>>();
        if !errors.is_empty() {
            return Err(FlyError::Validation(errors));
        }
        Ok(report)
    }

    fn apply_to_document(
        document: &mut ProjectDocument,
        command: &mut EditorCommand,
        generator: &mut UniqueIdGenerator,
        in_batch: bool,
    ) -> FlyResult<()> {
        document.invalidate_caches();
        let result = match command {
            EditorCommand::Select { .. } => Err(FlyError::Decode(
                "selection commands cannot be nested in a project transaction".to_string(),
            )),
            EditorCommand::Insert {
                parent_id,
                index,
                component,
            } => {
                prepare_insert_ids(
                    document,
                    parent_id.as_deref(),
                    component,
                    generator,
                    in_batch,
                )?;
                document
                    .project_mut()
                    .insert_component(parent_id.as_deref(), *index, component.clone())
            }
            EditorCommand::Remove { component_id } => {
                document.project_mut().remove_component(component_id)?;
                Ok(())
            }
            EditorCommand::Move {
                component_id,
                new_parent_id,
                index,
            } => {
                if let Some(parent_id) = new_parent_id.as_deref()
                    && (parent_id == component_id
                        || document.is_component_descendant_of(parent_id, component_id))
                {
                    return Err(FlyError::RecursiveMove {
                        component: component_id.clone(),
                        parent: parent_id.to_string(),
                    });
                }

                let previous_location = document.component_location(component_id);
                let mut insertion_index = *index;
                if previous_location.as_ref().is_some_and(|location| {
                    location.index < insertion_index
                        && document.component_has_parent(component_id, new_parent_id.as_deref())
                }) {
                    insertion_index = insertion_index.saturating_sub(1);
                }

                let component = document.project_mut().remove_component(component_id)?;
                document.project_mut().insert_component(
                    new_parent_id.as_deref(),
                    insertion_index,
                    component,
                )
            }
            EditorCommand::Patch {
                component_id,
                patch,
            } => {
                let component = document
                    .component_mut(component_id)
                    .ok_or_else(|| FlyError::ComponentNotFound(component_id.clone()))?;
                patch.clone().apply(component)
            }
            EditorCommand::Asset { command } => apply_asset_command(document, command),
            EditorCommand::Symbol { command } => apply_symbol_command(document, command),
            EditorCommand::StyleRule { command } => apply_style_rule_command(document, command),
            EditorCommand::Theme { command } => crate::apply_theme_command(document, command),
            EditorCommand::Page { command } => apply_page_command(document, command),
            EditorCommand::Dynamic { command } => apply_dynamic_command(document, command),
            EditorCommand::Binding { command } => apply_binding_command(document, command),
            EditorCommand::Context { command } => apply_context_command(document, command),
            EditorCommand::Translation { command } => apply_translation_command(document, command),
            EditorCommand::RestoreSnapshot { snapshot } => {
                // `restore` verifies the digest only when one is present, so a snapshot that
                // simply omits the field falls back to `project_hash` — a cheap FNV-1a
                // change-detection token that is trivial to forge. Nothing untrusted reaches
                // this path today, but `EditorCommand` is `Deserialize`, so the first API that
                // accepts a command would make the downgrade reachable. Everything `capture`
                // produces carries a digest, so requiring one costs nothing and fails closed.
                *document = snapshot.restore_verified()?;
                Ok(())
            }
            EditorCommand::Batch { commands } => {
                for command in commands {
                    Self::apply_to_document(document, command, generator, true)?;
                }
                Ok(())
            }
        };
        document.invalidate_caches();
        result
    }
}

/// Prepare a newly inserted subtree before the document-wide stable-id pass. That later pass can
/// repair duplicate component ids but cannot know which of two identical ids an action refers to;
/// rewrite the subtree and its internal references while the insertion boundary is still known.
fn prepare_insert_ids(
    document: &ProjectDocument,
    parent_id: Option<&str>,
    component: &mut crate::ComponentNode,
    generator: &mut UniqueIdGenerator,
    in_batch: bool,
) -> FlyResult<()> {
    let existing = crate::id_reference::project_identity_ids(document);
    let mut component_ids = Vec::new();
    let mut dom_ids = Vec::new();
    let mut form_ids = Vec::new();
    let mut matching_ids = BTreeSet::new();
    component.collect_ids(&mut component_ids);
    component.collect_dom_ids(&mut dom_ids);
    component.collect_form_ids(&mut form_ids);
    component.collect_matching_dom_ids(&mut matching_ids);
    let component_set = component_ids.iter().cloned().collect::<BTreeSet<_>>();
    let dom_set = dom_ids.iter().cloned().collect::<BTreeSet<_>>();
    let form_set = form_ids.iter().cloned().collect::<BTreeSet<_>>();
    if component_set.len() != component_ids.len()
        || dom_set.len() != dom_ids.len()
        || form_set.len() != form_ids.len()
    {
        return Err(FlyError::Decode(
            "inserted subtree contains duplicate component, HTML or form IDs; its references are ambiguous".to_string(),
        ));
    }
    if let Some(id) = crate::id_reference::conflicting_form_dom_id(std::iter::once(&*component)) {
        return Err(FlyError::Decode(format!(
            "form id `{id}` belongs to a different HTML element in the inserted subtree"
        )));
    }
    if !form_ids.is_empty() {
        // Form IDs are provider resources across the document. HTML IDs only collide on the
        // destination page, and an unrelated Fly component ID is not an HTML/form owner.
        let existing_forms = crate::id_reference::project_form_ids(document);
        let page_root = match parent_id {
            Some(parent_id) => document
                .component_location(parent_id)
                .and_then(|location| document.project.pages.get(location.page_index))
                .and_then(|page| page.component.as_ref()),
            None => document
                .project
                .pages
                .iter()
                .find_map(|page| page.component.as_ref()),
        };
        let page_dom = crate::id_reference::materialized_dom_ids(page_root);
        if let Some(id) = form_ids
            .iter()
            .find(|id| existing_forms.contains(*id) || page_dom.contains(*id))
        {
            return Err(FlyError::Decode(format!(
                "form id `{id}` already exists; choose another form identity before inserting it"
            )));
        }
    }
    let mut reserved = existing.clone();
    reserved.extend(component_set.iter().cloned());
    reserved.extend(dom_set.iter().cloned());
    reserved.extend(form_set.iter().cloned());
    let mut component_mapping = BTreeMap::new();
    for id in &component_ids {
        if existing.contains(id) || crate::validate_identifier(id).is_err() {
            if in_batch {
                // Subsequent commands could refer either to the existing node or to the newly
                // inserted one. Refuse that ambiguous transaction rather than rewrite the wrong
                // style/binding/patch; callers can use ProjectFragment to mint ids first.
                return Err(FlyError::Decode(format!(
                    "batch insert component id `{id}` collides or is invalid; remap the fragment before batching"
                )));
            }
            let replacement =
                crate::id_reference::allocate_generated_id(generator, "insert", &mut reserved);
            component_mapping.insert(id.clone(), replacement);
        }
    }
    let mut dom_mapping = form_set
        .iter()
        .map(|id| (id.clone(), id.clone()))
        .collect::<BTreeMap<_, _>>();
    for id in &dom_ids {
        if form_set.contains(id) {
            continue;
        }
        if existing.contains(id) || component_mapping.contains_key(id) {
            if in_batch {
                return Err(FlyError::Decode(format!(
                    "batch insert HTML id `{id}` collides; remap the fragment before batching"
                )));
            }
            let replacement = if matching_ids.contains(id) {
                component_mapping.get(id).cloned()
            } else {
                None
            }
            .unwrap_or_else(|| {
                crate::id_reference::allocate_generated_id(generator, "dom", &mut reserved)
            });
            dom_mapping.insert(id.clone(), replacement);
        }
    }
    if !component_mapping.is_empty() || !dom_mapping.is_empty() {
        component.remap_ids_with_dom(&component_mapping, &dom_mapping);
    }
    // Assign missing ids before recording the command so its inverse can name the inserted root.
    let mut seen = existing;
    component.ensure_stable_ids(generator, &mut reserved, &mut seen);
    Ok(())
}

fn apply_asset_command(document: &mut ProjectDocument, command: &AssetCommand) -> FlyResult<()> {
    match command {
        AssetCommand::Upsert { asset } => {
            let descriptor = AssetDescriptor::from_value(asset.clone()).ok_or_else(|| {
                FlyError::InvalidAssetReference(
                    "asset must be an object with src, source, or url".to_string(),
                )
            })?;
            if let Some(index) = document.project.assets.iter().position(|candidate| {
                AssetDescriptor::from_value(candidate.clone())
                    .is_some_and(|candidate| candidate.id == descriptor.id)
            }) {
                document.project_mut().assets[index] = asset.clone();
            } else {
                document.project_mut().assets.push(asset.clone());
            }
            Ok(())
        }
        AssetCommand::Remove { asset_id } => {
            let before = document.project.assets.len();
            document.project_mut().assets.retain(|candidate| {
                AssetDescriptor::from_value(candidate.clone())
                    .is_none_or(|candidate| candidate.id != *asset_id)
            });
            if document.project.assets.len() == before {
                return Err(FlyError::AssetNotFound(asset_id.clone()));
            }
            Ok(())
        }
    }
}

/// Compute a local inverse against the pre-command document when representable.
///
/// Snapshot history is authoritative for undo and redo. Compound commands and
/// edits whose side effects cannot be represented by a local inverse return None.
pub fn invert_command(
    document: &ProjectDocument,
    command: &EditorCommand,
) -> Option<EditorCommand> {
    match command {
        EditorCommand::Select { .. } => None,
        EditorCommand::Insert { component, .. } => {
            let id = component.id()?.to_string();
            Some(EditorCommand::Remove { component_id: id })
        }
        EditorCommand::Remove { component_id } => {
            let location = document.component_location(component_id)?;
            if !document.component_has_parent(component_id, location.parent_component_id.as_deref()) {
                return None;
            }
            let component = document.component(component_id)?;
            Some(EditorCommand::Insert {
                parent_id: location.parent_component_id,
                index: location.index,
                component: crate::ComponentNode::Object(Box::new(component.clone())),
            })
        }
        EditorCommand::Move {
            component_id,
            new_parent_id,
            index,
        } => {
            let location = document.component_location(component_id)?;
            let original_parent = location.parent_component_id;
            if !document.component_has_parent(component_id, original_parent.as_deref()) {
                // An unaddressable parent (or a page root) cannot be restored
                // by a Move command. Snapshot history remains authoritative.
                return None;
            }
            let inverse_index = if document.component_has_parent(component_id, new_parent_id.as_deref())
                && *index < location.index
            {
                // Move indices describe insertion slots before removal.
                location.index.checked_add(1)?
            } else {
                location.index
            };
            Some(EditorCommand::Move {
                component_id: component_id.clone(),
                new_parent_id: original_parent,
                index: inverse_index,
            })
        }
        EditorCommand::Patch {
            component_id,
            patch,
        } => {
            let component = document.component(component_id)?;
            // Setting or removing content also removes scalar text children.
            // A field patch cannot recreate those children.
            if (patch.fields.contains_key("content")
                || patch.remove_fields.iter().any(|field| field == "content"))
                && component.children().iter().any(|child| {
                    matches!(
                        child,
                        crate::ComponentNode::Opaque(
                            serde_json::Value::String(_)
                            | serde_json::Value::Number(_)
                            | serde_json::Value::Bool(_)
                        )
                    )
                })
            {
                return None;
            }
            let mut inverse_patch = crate::ComponentPatch::default();

            for (key, _) in &patch.attributes {
                if let Some(old_val) = component.attributes.get(key) {
                    inverse_patch
                        .attributes
                        .insert(key.clone(), old_val.clone());
                } else {
                    inverse_patch.remove_attributes.push(key.clone());
                }
            }
            for key in &patch.remove_attributes {
                if let Some(old_val) = component.attributes.get(key) {
                    inverse_patch
                        .attributes
                        .insert(key.clone(), old_val.clone());
                }
            }

            if patch.clear_style
                || patch.replace_style
                || patch.style.is_some()
                || !patch.remove_style_properties.is_empty()
            {
                // A merge can replace a scalar style, and absence is distinct
                // from an empty object. Restore the complete original slot.
                inverse_patch = match &component.style {
                    Some(style) => inverse_patch.replace_style(style.clone()),
                    None => inverse_patch.clear_style(),
                };
            }

            for (key, _) in &patch.fields {
                match key.as_str() {
                    "type" => {
                        if let Some(old_type) = &component.component_type {
                            inverse_patch.fields.insert(
                                "type".to_string(),
                                serde_json::Value::String(old_type.clone()),
                            );
                        } else {
                            inverse_patch.remove_fields.push("type".to_string());
                        }
                    }
                    "tagName" => {
                        if let Some(old_tag) = &component.tag_name {
                            inverse_patch.fields.insert(
                                "tagName".to_string(),
                                serde_json::Value::String(old_tag.clone()),
                            );
                        } else {
                            inverse_patch.remove_fields.push("tagName".to_string());
                        }
                    }
                    "provider" => {
                        if let Some(old_provider) = &component.provider {
                            inverse_patch.fields.insert(
                                "provider".to_string(),
                                serde_json::Value::String(old_provider.clone()),
                            );
                        } else {
                            inverse_patch.remove_fields.push("provider".to_string());
                        }
                    }
                    _ => {
                        if let Some(old_val) = component.extensions.get(key) {
                            inverse_patch.fields.insert(key.clone(), old_val.clone());
                        } else {
                            inverse_patch.remove_fields.push(key.clone());
                        }
                    }
                }
            }
            for key in &patch.remove_fields {
                match key.as_str() {
                    "type" => {
                        if let Some(old_type) = &component.component_type {
                            inverse_patch.fields.insert(
                                "type".to_string(),
                                serde_json::Value::String(old_type.clone()),
                            );
                        }
                    }
                    "tagName" => {
                        if let Some(old_tag) = &component.tag_name {
                            inverse_patch.fields.insert(
                                "tagName".to_string(),
                                serde_json::Value::String(old_tag.clone()),
                            );
                        }
                    }
                    "provider" => {
                        if let Some(old_provider) = &component.provider {
                            inverse_patch.fields.insert(
                                "provider".to_string(),
                                serde_json::Value::String(old_provider.clone()),
                            );
                        }
                    }
                    _ => {
                        if let Some(old_val) = component.extensions.get(key) {
                            inverse_patch.fields.insert(key.clone(), old_val.clone());
                        }
                    }
                }
            }

            Some(EditorCommand::Patch {
                component_id: component_id.clone(),
                patch: inverse_patch,
            })
        }
        EditorCommand::Asset { command } => match command {
            AssetCommand::Upsert { asset } => {
                let descriptor = AssetDescriptor::from_value(asset.clone())?;
                if let Some(existing) = document.project.assets.iter().find(|candidate| {
                    AssetDescriptor::from_value((*candidate).clone())
                        .is_some_and(|candidate| candidate.id == descriptor.id)
                }) {
                    Some(EditorCommand::Asset {
                        command: AssetCommand::Upsert {
                            asset: existing.clone(),
                        },
                    })
                } else {
                    Some(EditorCommand::Asset {
                        command: AssetCommand::Remove {
                            asset_id: descriptor.id,
                        },
                    })
                }
            }
            AssetCommand::Remove { asset_id } => {
                let existing = document.project.assets.iter().find(|candidate| {
                    AssetDescriptor::from_value((*candidate).clone())
                        .is_some_and(|candidate| candidate.id == *asset_id)
                })?;
                Some(EditorCommand::Asset {
                    command: AssetCommand::Upsert {
                        asset: existing.clone(),
                    },
                })
            }
        },
        EditorCommand::Theme { command } => match command {
            crate::ThemeCommand::Upsert { token } => {
                let current_tokens = crate::decode_theme_tokens(document).ok()?;
                if let Some(existing) = current_tokens.iter().find(|t| t.name == token.name) {
                    Some(EditorCommand::Theme {
                        command: crate::ThemeCommand::Upsert {
                            token: existing.clone(),
                        },
                    })
                } else {
                    Some(EditorCommand::Theme {
                        command: crate::ThemeCommand::Remove {
                            name: token.name.clone(),
                        },
                    })
                }
            }
            crate::ThemeCommand::Remove { name } => {
                let current_tokens = crate::decode_theme_tokens(document).ok()?;
                let existing = current_tokens.iter().find(|t| t.name == *name)?;
                Some(EditorCommand::Theme {
                    command: crate::ThemeCommand::Upsert {
                        token: existing.clone(),
                    },
                })
            }
            crate::ThemeCommand::Rename { old_name, new_name } => Some(EditorCommand::Theme {
                command: crate::ThemeCommand::Rename {
                    old_name: new_name.clone(),
                    new_name: old_name.clone(),
                },
            }),
            crate::ThemeCommand::Replace { .. } => {
                let current_tokens = crate::decode_theme_tokens(document).ok()?;
                Some(EditorCommand::Theme {
                    command: crate::ThemeCommand::Replace {
                        tokens: current_tokens,
                    },
                })
            }
        },
        // Inverting every step against the initial document is incorrect:
        // previous steps may change indices, assets or the existence of targets.
        // Use the already retained exact before/after snapshots for transactions.
        EditorCommand::Batch { .. } => None,
        _ => None,
    }
}
