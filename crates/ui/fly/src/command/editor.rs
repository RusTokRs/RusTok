use super::{AssetCommand, EditorCommand, History, HistoryEntry, RevisionState};
use crate::{
    AssetDescriptor, FlyError, FlyResult, ProjectDocument, ProjectSnapshot, RegistrySet,
    UniqueIdGenerator, ValidationLimits, ValidationReport, apply_binding_command,
    apply_context_command, apply_dynamic_command, apply_page_command, apply_style_rule_command,
    apply_symbol_command, apply_translation_command, extend_with_runtime_validation,
    validate_project,
};
use serde::{Deserialize, Serialize};
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

    pub fn apply(&mut self, command: EditorCommand) -> FlyResult<ValidationReport> {
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

        let inverse = invert_command(&self.document, &command);

        // Fast path: incremental validation for non-structural patch commands when last_report is valid.
        if let EditorCommand::Patch { component_id, .. } = &command
            && let Some(prev_report) = &self.last_report
            && prev_report.is_valid()
        {
            let mut after = (*self.document).clone();
            self.apply_to_document(&mut after, &command)?;
            if let Some(inc_report) = crate::validate_patch_incremental(
                &after,
                component_id,
                prev_report,
                &self.registries,
                self.validation_limits,
            ) {
                let errors = inc_report.errors().cloned().collect::<Vec<_>>();
                if !errors.is_empty() {
                    return Err(FlyError::Validation(errors));
                }
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
        }

        // One speculative copy to mutate. A command must not leave a partially applied document
        // behind when validation rejects it, so the working copy is unavoidable — but it is now
        // the only deep copy on this path.
        let mut after = (*self.document).clone();
        self.apply_to_document(&mut after, &command)?;
        after.ensure_stable_ids(&mut self.id_generator);
        let report = extend_with_runtime_validation(
            &after,
            validate_project(&after, &self.registries, self.validation_limits),
        );
        let errors = report.errors().cloned().collect::<Vec<_>>();
        if !errors.is_empty() {
            return Err(FlyError::Validation(errors));
        }

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
        let entry = self.history.pop_undo()?;
        self.document = Arc::clone(&entry.before);
        self.history.push_redo(entry);
        self.selection = self
            .selection
            .take()
            .filter(|id| self.document.contains_component(id));
        self.revision.mark_changed(&self.document);
        self.last_report = None;
        Ok(&self.document)
    }

    pub fn redo(&mut self) -> FlyResult<&ProjectDocument> {
        let entry = self.history.pop_redo()?;
        self.document = Arc::clone(&entry.after);
        self.history.push_undo(entry);
        self.selection = self
            .selection
            .take()
            .filter(|id| self.document.contains_component(id));
        self.revision.mark_changed(&self.document);
        self.last_report = None;
        Ok(&self.document)
    }

    fn apply_to_document(
        &self,
        document: &mut ProjectDocument,
        command: &EditorCommand,
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
            } => document
                .project
                .insert_component(parent_id.as_deref(), *index, component.clone()),
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
                    self.apply_to_document(document, command)?;
                }
                Ok(())
            }
        };
        document.invalidate_caches();
        result
    }
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
