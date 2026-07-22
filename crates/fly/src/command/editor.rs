use super::{AssetCommand, EditorCommand, History, HistoryEntry, RevisionState};
use crate::{
    AssetDescriptor, FlyError, FlyResult, ProjectDocument, ProjectSnapshot, RegistrySet,
    SequentialIdGenerator, ValidationLimits, ValidationReport, apply_binding_command,
    apply_context_command, apply_dynamic_command, apply_page_command, apply_style_rule_command,
    apply_translation_command, extend_with_runtime_validation, validate_project,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FlyEditor {
    document: ProjectDocument,
    registries: RegistrySet,
    selection: Option<String>,
    history: History,
    revision: RevisionState,
    validation_limits: ValidationLimits,
    pub(crate) id_generator: SequentialIdGenerator,
}

impl FlyEditor {
    pub fn new(mut document: ProjectDocument, registries: RegistrySet) -> Self {
        let mut id_generator = SequentialIdGenerator::default();
        document.ensure_stable_ids(&mut id_generator);
        let revision = RevisionState::new(&document);
        Self {
            document,
            registries,
            selection: None,
            history: History::new(100),
            revision,
            validation_limits: ValidationLimits::default(),
            id_generator,
        }
    }

    pub fn with_history_limit(mut self, limit: usize) -> Self {
        self.history = History::new(limit);
        self
    }

    pub fn with_validation_limits(mut self, limits: ValidationLimits) -> Self {
        self.validation_limits = limits;
        self
    }

    pub fn document(&self) -> &ProjectDocument {
        &self.document
    }

    pub fn registries(&self) -> &RegistrySet {
        &self.registries
    }

    pub fn registries_mut(&mut self) -> &mut RegistrySet {
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
            if let Some(id) = component_id.as_deref() {
                if !self.document.contains_component(id) {
                    return Err(FlyError::ComponentNotFound(id.to_string()));
                }
            }
            self.selection = component_id.clone();
            return Ok(self.validate());
        }

        let before = self.document.clone();
        let mut after = before.clone();
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

        self.document = after.clone();
        self.selection = self
            .selection
            .take()
            .filter(|id| self.document.contains_component(id));
        self.history.push(HistoryEntry {
            command,
            before,
            after,
        });
        self.revision.mark_changed(&self.document);
        Ok(report)
    }

    pub fn undo(&mut self) -> FlyResult<&ProjectDocument> {
        let entry = self.history.pop_undo()?;
        self.document = entry.before.clone();
        self.history.redo.push(entry);
        self.selection = self
            .selection
            .take()
            .filter(|id| self.document.contains_component(id));
        self.revision.mark_changed(&self.document);
        Ok(&self.document)
    }

    pub fn redo(&mut self) -> FlyResult<&ProjectDocument> {
        let entry = self.history.pop_redo()?;
        self.document = entry.after.clone();
        self.history.undo.push(entry);
        self.selection = self
            .selection
            .take()
            .filter(|id| self.document.contains_component(id));
        self.revision.mark_changed(&self.document);
        Ok(&self.document)
    }

    fn apply_to_document(
        &self,
        document: &mut ProjectDocument,
        command: &EditorCommand,
    ) -> FlyResult<()> {
        match command {
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
                if let Some(parent_id) = new_parent_id.as_deref() {
                    if parent_id == component_id
                        || document.is_component_descendant_of(parent_id, component_id)
                    {
                        return Err(FlyError::RecursiveMove {
                            component: component_id.clone(),
                            parent: parent_id.to_string(),
                        });
                    }
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
                patch.clone().apply(component);
                Ok(())
            }
            EditorCommand::Asset { command } => apply_asset_command(document, command),
            EditorCommand::StyleRule { command } => apply_style_rule_command(document, command),
            EditorCommand::Page { command } => apply_page_command(document, command),
            EditorCommand::Dynamic { command } => apply_dynamic_command(document, command),
            EditorCommand::Binding { command } => apply_binding_command(document, command),
            EditorCommand::Context { command } => apply_context_command(document, command),
            EditorCommand::Translation { command } => apply_translation_command(document, command),
            EditorCommand::RestoreSnapshot { snapshot } => {
                *document = snapshot.restore()?;
                Ok(())
            }
            EditorCommand::Batch { commands } => {
                for command in commands {
                    self.apply_to_document(document, command)?;
                }
                Ok(())
            }
        }
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
