use super::{AssetCommand, EditorCommand, History, HistoryEntry, RevisionState};
use crate::{
    AssetDescriptor, FlyError, FlyResult, ProjectDocument, ProjectSnapshot, RegistrySet,
    UniqueIdGenerator, ValidationLimits, ValidationReport, apply_binding_command,
    apply_context_command, apply_dynamic_command, apply_page_command, apply_style_rule_command,
    apply_symbol_command, apply_translation_command, extend_with_runtime_validation,
    validate_project,
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
        extend_with_runtime_validation(
            &self.document,
            validate_project(&self.document, &self.registries, self.validation_limits),
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

        // Keep even ID allocation transactional: a refused edit must not advance editor state.
        let mut generator = self.id_generator.clone();

        // Fast path: incremental validation for non-structural patch commands when last_report is valid.
        // When incremental validation declines, the already-applied working copy is handed to the
        // full path below instead of being cloned and applied a second time.
        let mut speculative = None;
        if let EditorCommand::Patch { component_id, .. } = &command
            && let Some(prev_report) = &self.last_report
            && prev_report.is_valid()
        {
            let component_id = component_id.clone();
            let mut after = (*self.document).clone();
            Self::apply_to_document(&mut after, &mut command, &mut generator, false)?;
            if let Some(inc_report) = crate::validate_patch_incremental(
                &after,
                &component_id,
                prev_report,
                &self.registries,
                self.validation_limits,
            ) {
                let errors = inc_report.errors().cloned().collect::<Vec<_>>();
                if !errors.is_empty() {
                    return Err(FlyError::Validation(errors));
                }
                let inverse = invert_command(&self.document, &command);
                self.id_generator = generator;
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
                self.last_report = Some(inc_report.clone());
                return Ok(inc_report);
            }
            speculative = Some(after);
        }

        // One speculative copy to mutate. A command must not leave a partially applied document
        // behind when validation rejects it, so the working copy is unavoidable — but it is now
        // the only deep copy on this path.
        let mut after = match speculative {
            Some(after) => after,
            None => {
                let mut after = (*self.document).clone();
                Self::apply_to_document(&mut after, &mut command, &mut generator, false)?;
                after
            }
        };
        after.ensure_stable_ids(&mut generator);
        let report = extend_with_runtime_validation(
            &after,
            validate_project(&after, &self.registries, self.validation_limits),
        );
        let errors = report.errors().cloned().collect::<Vec<_>>();
        if !errors.is_empty() {
            return Err(FlyError::Validation(errors));
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
        let (entry, size) = self.history.pop_undo()?;
        self.document = Arc::clone(&entry.before);
        self.history.push_redo(entry, size);
        self.selection = self
            .selection
            .take()
            .filter(|id| self.document.contains_component(id));
        self.revision.mark_changed(&self.document);
        self.last_report = None;
        Ok(&self.document)
    }

    pub fn redo(&mut self) -> FlyResult<&ProjectDocument> {
        let (entry, size) = self.history.pop_redo()?;
        self.document = Arc::clone(&entry.after);
        self.history.push_undo(entry, size);
        self.selection = self
            .selection
            .take()
            .filter(|id| self.document.contains_component(id));
        self.revision.mark_changed(&self.document);
        self.last_report = None;
        Ok(&self.document)
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
                    .project
                    .insert_component(parent_id.as_deref(), *index, component.clone())
            }
            EditorCommand::Remove { component_id } => {
                document.project.remove_component(component_id)?;
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
                    location.parent_component_id.as_deref() == new_parent_id.as_deref()
                        && location.index < insertion_index
                }) {
                    insertion_index = insertion_index.saturating_sub(1);
                }

                let component = document.project.remove_component(component_id)?;
                document.project.insert_component(
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
                document.project.assets[index] = asset.clone();
            } else {
                document.project.assets.push(asset.clone());
            }
            Ok(())
        }
        AssetCommand::Remove { asset_id } => {
            let before = document.project.assets.len();
            document.project.assets.retain(|candidate| {
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

/// Compute the exact inverse of an editor command against the current document state.
///
/// Enables lightweight delta-based history tracking and inverse-command undo operations.
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
            let component = document.component(component_id)?;
            Some(EditorCommand::Insert {
                parent_id: location.parent_component_id,
                index: location.index,
                component: crate::ComponentNode::Object(Box::new(component.clone())),
            })
        }
        EditorCommand::Move { component_id, .. } => {
            let location = document.component_location(component_id)?;
            Some(EditorCommand::Move {
                component_id: component_id.clone(),
                new_parent_id: location.parent_component_id,
                index: location.index,
            })
        }
        EditorCommand::Patch {
            component_id,
            patch,
        } => {
            let component = document.component(component_id)?;
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

            if patch.clear_style || patch.replace_style {
                inverse_patch.replace_style = true;
                inverse_patch.style = component.style.clone();
            } else if patch.style.is_some() || !patch.remove_style_properties.is_empty() {
                if let Some(serde_json::Value::Object(orig_style)) = &component.style {
                    let mut restore_style = serde_json::Map::new();
                    if let Some(serde_json::Value::Object(patch_style)) = &patch.style {
                        for prop in patch_style.keys() {
                            if let Some(old_val) = orig_style.get(prop) {
                                restore_style.insert(prop.clone(), old_val.clone());
                            } else {
                                inverse_patch.remove_style_properties.push(prop.clone());
                            }
                        }
                    }
                    for prop in &patch.remove_style_properties {
                        if let Some(old_val) = orig_style.get(prop) {
                            restore_style.insert(prop.clone(), old_val.clone());
                        }
                    }
                    if !restore_style.is_empty() {
                        inverse_patch.style = Some(serde_json::Value::Object(restore_style));
                    }
                } else {
                    inverse_patch.replace_style = true;
                    inverse_patch.style = component.style.clone();
                }
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
        EditorCommand::Batch { commands } => {
            let mut inverted = Vec::with_capacity(commands.len());
            for cmd in commands.iter().rev() {
                inverted.push(invert_command(document, cmd)?);
            }
            Some(EditorCommand::Batch { commands: inverted })
        }
        _ => None,
    }
}
