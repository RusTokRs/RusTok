use crate::{
    apply_binding_command, apply_context_command, apply_dynamic_command, apply_page_command,
    apply_style_rule_command, extend_with_runtime_validation, validate_project, AssetDescriptor,
    BindingCommand, ComponentNode, ComponentObject, ContextCommand, DynamicCommand, FlyError,
    FlyResult, GrapesJsV1Codec, PageCommand, ProjectDocument, RegistrySet, SequentialIdGenerator,
    StyleRuleCommand, ValidationLimits, ValidationReport,
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
    pub replace_style: bool,
    #[serde(default)]
    pub clear_style: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remove_style_properties: Vec<String>,
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
        } else {
            if !self.remove_style_properties.is_empty() {
                if let Some(Value::Object(style)) = component.style.as_mut() {
                    for property in self.remove_style_properties {
                        style.remove(&property);
                    }
                }
            }
            if let Some(style) = self.style {
                if self.replace_style {
                    component.style = Some(style);
                } else {
                    merge_style(&mut component.style, style);
                }
            }
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

fn merge_style(current: &mut Option<Value>, patch: Value) {
    match (current.as_mut(), patch) {
        (Some(Value::Object(current)), Value::Object(patch)) => current.extend(patch),
        (_, patch) => *current = Some(patch),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum AssetCommand {
    Upsert { asset: Value },
    Remove { asset_id: String },
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
    Asset {
        command: AssetCommand,
    },
    StyleRule {
        command: StyleRuleCommand,
    },
    Page {
        command: PageCommand,
    },
    Dynamic {
        command: DynamicCommand,
    },
    Binding {
        command: BindingCommand,
    },
    Context {
        command: ContextCommand,
    },
    Batch {
        commands: Vec<EditorCommand>,
    },
}

impl EditorCommand {
    pub fn batch(commands: impl IntoIterator<Item = EditorCommand>) -> Self {
        Self::Batch {
            commands: commands.into_iter().collect(),
        }
    }
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
        extend_with_runtime_validation(
            &self.document,
            validate_project(&self.document, &self.registries, self.validation_limits),
        )
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BindingCatalog, BindingTarget, BindingTransform, ConditionOperator, ContextFieldDefinition,
        ContextSchemaCatalog, ContextValueKind, DynamicCatalog, GrapesJsV1Codec, RegistrySet,
        RuntimeBinding, RuntimeCondition, FLY_RUNTIME_CONDITIONS_FIELD,
    };
    use serde_json::json;

    fn editor() -> FlyEditor {
        let document = GrapesJsV1Codec::decode_value(json!({
            "assets": [],
            "styles": [],
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "hero",
                        "type": "section",
                        "style": { "color": "red", "padding": "24px" }
                    }]
                }
            }]
        }))
        .expect("document");
        FlyEditor::new(document, RegistrySet::with_builtins())
    }

    #[test]
    fn style_patch_merges_and_can_remove_individual_properties() {
        let mut editor = editor();
        editor
            .apply(EditorCommand::Patch {
                component_id: "hero".to_string(),
                patch: ComponentPatch {
                    style: Some(json!({ "width": "320px" })),
                    remove_style_properties: vec!["color".to_string()],
                    ..ComponentPatch::default()
                },
            })
            .expect("patch");
        let style = editor
            .document()
            .component("hero")
            .and_then(|component| component.style.as_ref())
            .expect("style");
        assert_eq!(style["padding"], "24px");
        assert_eq!(style["width"], "320px");
        assert!(style.get("color").is_none());
    }

    #[test]
    fn dynamic_commands_participate_in_history() {
        let mut editor = editor();
        editor
            .apply(EditorCommand::Dynamic {
                command: DynamicCommand::UpsertCondition {
                    condition: RuntimeCondition {
                        id: "show-hero".to_string(),
                        component_id: "hero".to_string(),
                        path: "flags.hero".to_string(),
                        operator: ConditionOperator::Truthy,
                        expected: None,
                        invert: false,
                        extensions: Map::new(),
                    },
                },
            })
            .expect("dynamic command");
        assert_eq!(
            DynamicCatalog::from_document(editor.document())
                .conditions
                .len(),
            1
        );
        assert!(editor
            .document()
            .project
            .extensions
            .contains_key(FLY_RUNTIME_CONDITIONS_FIELD));
        editor.undo().expect("undo dynamic command");
        assert!(DynamicCatalog::from_document(editor.document())
            .conditions
            .is_empty());
    }

    #[test]
    fn binding_commands_participate_in_history() {
        let mut editor = editor();
        editor
            .apply(EditorCommand::Binding {
                command: BindingCommand::Upsert {
                    binding: RuntimeBinding {
                        id: "hero-content".to_string(),
                        component_id: "hero".to_string(),
                        path: "page.hero".to_string(),
                        target: BindingTarget::Field {
                            name: "content".to_string(),
                        },
                        fallback: None,
                        transform: BindingTransform::Identity,
                        extensions: Map::new(),
                    },
                },
            })
            .expect("binding command");
        assert_eq!(
            BindingCatalog::from_document(editor.document())
                .bindings
                .len(),
            1
        );
        editor.undo().expect("undo binding command");
        assert!(BindingCatalog::from_document(editor.document())
            .bindings
            .is_empty());
    }

    #[test]
    fn context_commands_participate_in_history() {
        let mut editor = editor();
        editor
            .apply(EditorCommand::Context {
                command: ContextCommand::UpsertField {
                    field: ContextFieldDefinition {
                        id: "title".to_string(),
                        path: "page.title".to_string(),
                        kind: ContextValueKind::String,
                        required: true,
                        default: Some(json!("Untitled")),
                        item_kind: None,
                        extensions: Map::new(),
                    },
                },
            })
            .expect("context command");
        assert_eq!(
            ContextSchemaCatalog::from_document(editor.document())
                .fields
                .len(),
            1
        );
        editor.undo().expect("undo context command");
        assert!(ContextSchemaCatalog::from_document(editor.document())
            .fields
            .is_empty());
    }

    #[test]
    fn invalid_runtime_definitions_block_transaction() {
        let mut editor = editor();
        let error = editor
            .apply(EditorCommand::Dynamic {
                command: DynamicCommand::UpsertRepeater {
                    repeater: crate::RuntimeRepeater {
                        id: "root-repeat".to_string(),
                        component_id: "root".to_string(),
                        path: "items".to_string(),
                        item_alias: "item".to_string(),
                        index_alias: "index".to_string(),
                        limit: None,
                        empty_behavior: crate::EmptyRepeaterBehavior::Hide,
                        extensions: Map::new(),
                    },
                },
            })
            .expect_err("root repeater should fail validation");
        assert!(matches!(error, FlyError::Validation(_)));
        assert!(DynamicCatalog::from_document(editor.document())
            .repeaters
            .is_empty());
    }

    #[test]
    fn batch_is_atomic_and_creates_one_history_entry() {
        let mut editor = editor();
        editor
            .apply(EditorCommand::batch([
                EditorCommand::Patch {
                    component_id: "hero".to_string(),
                    patch: ComponentPatch {
                        fields: Map::from_iter([(
                            "content".to_string(),
                            Value::String("Updated".to_string()),
                        )]),
                        ..ComponentPatch::default()
                    },
                },
                EditorCommand::Asset {
                    command: AssetCommand::Upsert {
                        asset: json!({ "id": "hero-image", "src": "/hero.webp" }),
                    },
                },
            ]))
            .expect("batch");
        assert_eq!(editor.history().undo_len(), 1);
        assert_eq!(editor.document().project.assets.len(), 1);
        editor.undo().expect("undo batch");
        assert!(editor.document().project.assets.is_empty());
        assert!(editor
            .document()
            .component("hero")
            .and_then(|component| component.extensions.get("content"))
            .is_none());
    }

    #[test]
    fn failed_batch_does_not_change_document_or_history() {
        let mut editor = editor();
        let before = editor.document().hash();
        let error = editor
            .apply(EditorCommand::batch([
                EditorCommand::Asset {
                    command: AssetCommand::Upsert {
                        asset: json!({ "id": "hero-image", "src": "/hero.webp" }),
                    },
                },
                EditorCommand::Remove {
                    component_id: "missing".to_string(),
                },
            ]))
            .expect_err("batch should fail");
        assert!(matches!(error, FlyError::ComponentNotFound(_)));
        assert_eq!(editor.document().hash(), before);
        assert_eq!(editor.history().undo_len(), 0);
    }
}
