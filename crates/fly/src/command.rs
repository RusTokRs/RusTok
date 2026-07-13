use crate::{
    validate_project, ComponentNode, ComponentObject, FlyError, FlyResult, GrapesJsV1Codec,
    ProjectDocument, RegistrySet, SequentialIdGenerator, ValidationDiagnostic, ValidationLimits,
    ValidationReport,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ComponentPatch {
    #[serde(default)]
    pub attributes: Map<String, Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remove_attributes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<Value>,
    #[serde(default)]
    pub clear_style: bool,
    #[serde(default)]
    pub fields: Map<String, Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remove_fields: Vec<String>,
}

impl ComponentPatch {
    fn apply(self, component: &mut ComponentObject) {
        for attribute in self.remove_attributes {
            component.attributes.remove(&attribute);
        }
        component.attributes.extend(self.attributes);

        if self.clear_style {
            component.style = None;
        } else if let Some(style) = self.style {
            component.style = Some(style);
        }

        for field in self.remove_fields {
            match field.as_str() {
                "tagName" => component.tag_name = None,
                "provider" => component.provider = None,
                "schemaVersion" => component.schema_version = None,
                _ => {
                    component.extensions.remove(&field);
                }
            }
        }
        for (key, value) in self.fields {
            match key.as_str() {
                "type" => component.component_type = value.as_str().map(ToString::to_string),
                "tagName" => component.tag_name = value.as_str().map(ToString::to_string),
                "provider" => component.provider = value.as_str().map(ToString::to_string),
                "schemaVersion" => {
                    component.schema_version = value.as_str().map(ToString::to_string)
                }
                _ => {
                    component.extensions.insert(key, value);
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum EditorCommand {
    Select {
        component_id: Option<String>,
    },
    Insert {
        parent_id: Option<String>,
        index: usize,
        component: ComponentNode,
    },
    Remove {
        component_id: String,
    },
    Move {
        component_id: String,
        new_parent_id: Option<String>,
        index: usize,
    },
    Patch {
        component_id: String,
        patch: ComponentPatch,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HistoryEntry {
    pub command: EditorCommand,
    pub before: ProjectDocument,
    pub after: ProjectDocument,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct History {
    limit: usize,
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
}

impl History {
    pub fn new(limit: usize) -> Self {
        Self {
            limit: limit.max(1),
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }

    fn push(&mut self, entry: HistoryEntry) {
        self.undo.push(entry);
        self.redo.clear();
        if self.undo.len() > self.limit {
            self.undo.remove(0);
        }
    }

    fn pop_undo(&mut self) -> FlyResult<HistoryEntry> {
        self.undo.pop().ok_or(FlyError::UndoHistoryEmpty)
    }

    fn pop_redo(&mut self) -> FlyResult<HistoryEntry> {
        self.redo.pop().ok_or(FlyError::RedoHistoryEmpty)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectHash(pub u64);

impl ProjectHash {
    pub fn from_document(document: &ProjectDocument) -> Self {
        let bytes = GrapesJsV1Codec::encode_vec(document)
            .unwrap_or_else(|_| serde_json::to_vec(&document.project).unwrap_or_default());
        Self::from_bytes(&bytes)
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut hash = 0xcbf29ce484222325_u64;
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        Self(hash)
    }

    pub fn hex(self) -> String {
        format!("{:016x}", self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RevisionState {
    pub dirty: bool,
    pub command_sequence: u64,
    pub last_acknowledged_revision: Option<String>,
    pub project_hash: ProjectHash,
    pub save_in_progress: bool,
    pub save_failed: bool,
}

impl RevisionState {
    pub fn new(document: &ProjectDocument) -> Self {
        Self {
            dirty: false,
            command_sequence: 0,
            last_acknowledged_revision: None,
            project_hash: document.hash(),
            save_in_progress: false,
            save_failed: false,
        }
    }

    fn mark_changed(&mut self, document: &ProjectDocument) {
        self.dirty = true;
        self.command_sequence = self.command_sequence.saturating_add(1);
        self.project_hash = document.hash();
        self.save_failed = false;
    }

    pub fn begin_save(&mut self) {
        self.save_in_progress = true;
        self.save_failed = false;
    }

    pub fn fail_save(&mut self) {
        self.save_in_progress = false;
        self.save_failed = true;
    }

    pub fn acknowledge(
        &mut self,
        expected_hash: ProjectHash,
        revision: impl Into<String>,
    ) -> FlyResult<()> {
        if self.project_hash != expected_hash {
            return Err(FlyError::RevisionConflict {
                expected: expected_hash.hex(),
                actual: self.project_hash.hex(),
            });
        }
        self.last_acknowledged_revision = Some(revision.into());
        self.dirty = false;
        self.save_in_progress = false;
        self.save_failed = false;
        Ok(())
    }
}

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
        validate_project(&self.document, &self.registries, self.validation_limits)
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
        let report = validate_project(&after, &self.registries, self.validation_limits);
        let errors = report.errors().cloned().collect::<Vec<_>>();
        if !errors.is_empty() {
            return Err(FlyError::Validation(errors));
        }

        if let EditorCommand::Remove { component_id } = &command {
            if self.selection.as_deref() == Some(component_id) {
                self.selection = None;
            }
        }

        self.document = after.clone();
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
            EditorCommand::Select { .. } => Ok(()),
            EditorCommand::Insert {
                parent_id,
                index,
                component,
            } => document.project.insert_component(
                parent_id.as_deref(),
                *index,
                component.clone(),
            ),
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
        }
    }
}
