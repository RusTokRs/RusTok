use fly_ui::ContributionAssemblyResult;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub const PAGE_BUILDER_CONSUMER_PROPERTIES_FORMAT: &str = "page_builder_consumer_properties_v1";
pub const CONSUMER_PROPERTY_CONTRACT_INVALID: &str =
    "PAGE_BUILDER_CONSUMER_PROPERTY_CONTRACT_INVALID";
pub const CONSUMER_PROPERTY_EDITOR_UNAVAILABLE: &str =
    "PAGE_BUILDER_CONSUMER_PROPERTY_EDITOR_UNAVAILABLE";
pub const CONSUMER_PROPERTY_SAVE_FAILED: &str = "PAGE_BUILDER_CONSUMER_PROPERTY_SAVE_FAILED";

const MAX_PROPERTY_FIELDS: usize = 32;
const MAX_PROPERTY_VALUE_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConsumerPropertyFieldKind {
    Text,
    TextArea,
    StringList,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConsumerPropertyFieldDescriptor {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub help: Option<String>,
    pub kind: ConsumerPropertyFieldKind,
    #[serde(default)]
    pub required: bool,
    pub max_bytes: usize,
    #[serde(default)]
    pub placeholder: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConsumerPropertyEditorSchema {
    pub format: String,
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    pub fields: Vec<ConsumerPropertyFieldDescriptor>,
}

impl ConsumerPropertyEditorSchema {
    pub fn validate(&self) -> Result<(), ConsumerPropertyEditorError> {
        require_exact_format(&self.format)?;
        require_identifier(&self.id, "consumer property schema id")?;
        require_text(&self.title, "consumer property schema title")?;
        if self.fields.is_empty() || self.fields.len() > MAX_PROPERTY_FIELDS {
            return Err(ConsumerPropertyEditorError::contract(format!(
                "consumer property schema must contain between 1 and {MAX_PROPERTY_FIELDS} fields"
            )));
        }

        let mut field_ids = BTreeSet::new();
        for field in &self.fields {
            require_identifier(&field.id, "consumer property field id")?;
            require_text(&field.label, "consumer property field label")?;
            if field.max_bytes == 0 || field.max_bytes > MAX_PROPERTY_VALUE_BYTES {
                return Err(ConsumerPropertyEditorError::contract(format!(
                    "consumer property field `{}` has an invalid byte limit",
                    field.id
                )));
            }
            if !field_ids.insert(field.id.as_str()) {
                return Err(ConsumerPropertyEditorError::contract(format!(
                    "consumer property field `{}` is duplicated",
                    field.id
                )));
            }
        }
        Ok(())
    }

    pub fn validate_values(
        &self,
        values: &BTreeMap<String, String>,
    ) -> Result<(), ConsumerPropertyEditorError> {
        self.validate()?;
        let expected = self
            .fields
            .iter()
            .map(|field| field.id.as_str())
            .collect::<BTreeSet<_>>();
        let actual = values.keys().map(String::as_str).collect::<BTreeSet<_>>();
        if expected != actual {
            return Err(ConsumerPropertyEditorError::contract(
                "consumer property values must contain the exact registered field set",
            ));
        }

        for field in &self.fields {
            let value = values
                .get(&field.id)
                .expect("exact consumer property field set was validated");
            if field.required && value.trim().is_empty() {
                return Err(ConsumerPropertyEditorError::contract(format!(
                    "consumer property field `{}` is required",
                    field.id
                )));
            }
            if value.len() > field.max_bytes {
                return Err(ConsumerPropertyEditorError::contract(format!(
                    "consumer property field `{}` exceeds {} bytes",
                    field.id, field.max_bytes
                )));
            }
            if value
                .chars()
                .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
            {
                return Err(ConsumerPropertyEditorError::contract(format!(
                    "consumer property field `{}` contains a disallowed control character",
                    field.id
                )));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConsumerPropertyEditorSnapshot {
    pub revision: String,
    pub scope_label: String,
    pub values: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SaveConsumerPropertiesInput {
    pub contribution_id: String,
    pub property_editor_id: String,
    pub expected_revision: String,
    pub values: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConsumerPropertySaveReceipt {
    pub contribution_id: String,
    pub property_editor_id: String,
    pub revision: String,
    pub values: BTreeMap<String, String>,
}

#[cfg(not(target_arch = "wasm32"))]
pub type ConsumerPropertyLoadFuture = Pin<
    Box<
        dyn Future<Output = Result<ConsumerPropertyEditorSnapshot, ConsumerPropertyEditorError>>
            + Send
            + 'static,
    >,
>;

#[cfg(target_arch = "wasm32")]
pub type ConsumerPropertyLoadFuture = Pin<
    Box<
        dyn Future<Output = Result<ConsumerPropertyEditorSnapshot, ConsumerPropertyEditorError>>
            + 'static,
    >,
>;

#[cfg(not(target_arch = "wasm32"))]
pub type ConsumerPropertySaveFuture = Pin<
    Box<
        dyn Future<Output = Result<ConsumerPropertySaveReceipt, ConsumerPropertyEditorError>>
            + Send
            + 'static,
    >,
>;

#[cfg(target_arch = "wasm32")]
pub type ConsumerPropertySaveFuture = Pin<
    Box<
        dyn Future<Output = Result<ConsumerPropertySaveReceipt, ConsumerPropertyEditorError>>
            + 'static,
    >,
>;

pub trait ConsumerPropertyEditorPort: Send + Sync {
    fn load(&self) -> ConsumerPropertyLoadFuture;

    fn save(&self, input: SaveConsumerPropertiesInput) -> ConsumerPropertySaveFuture;
}

#[derive(Clone)]
pub struct ConsumerPropertyEditorRuntime {
    pub contribution_id: String,
    pub property_editor_id: String,
    pub provider: String,
    pub component_type: String,
    pub schema: ConsumerPropertyEditorSchema,
    port: Arc<dyn ConsumerPropertyEditorPort>,
}

impl ConsumerPropertyEditorRuntime {
    pub fn new(
        contribution_id: impl Into<String>,
        property_editor_id: impl Into<String>,
        provider: impl Into<String>,
        component_type: impl Into<String>,
        schema: ConsumerPropertyEditorSchema,
        port: Arc<dyn ConsumerPropertyEditorPort>,
    ) -> Self {
        Self {
            contribution_id: contribution_id.into(),
            property_editor_id: property_editor_id.into(),
            provider: provider.into(),
            component_type: component_type.into(),
            schema,
            port,
        }
    }

    pub fn verify_contribution(
        &self,
        assembly: &ContributionAssemblyResult,
    ) -> Result<(), ConsumerPropertyEditorError> {
        self.schema.validate()?;
        require_identifier(&self.contribution_id, "consumer contribution id")?;
        require_identifier(&self.property_editor_id, "consumer property editor id")?;
        require_identifier(&self.provider, "consumer property provider")?;
        require_identifier(&self.component_type, "consumer property component type")?;
        if !assembly.is_valid() {
            return Err(ConsumerPropertyEditorError::unavailable(
                "consumer contribution assembly contains errors",
            ));
        }
        let contribution = assembly
            .registry
            .get(&self.contribution_id)
            .ok_or_else(|| {
                ConsumerPropertyEditorError::unavailable(format!(
                    "consumer contribution `{}` is not registered",
                    self.contribution_id
                ))
            })?;
        if contribution.provider != self.provider {
            return Err(ConsumerPropertyEditorError::contract(format!(
                "consumer contribution `{}` belongs to provider `{}`, expected `{}`",
                self.contribution_id, contribution.provider, self.provider
            )));
        }
        let property_editor = contribution
            .property_editors
            .iter()
            .find(|editor| editor.id == self.property_editor_id)
            .ok_or_else(|| {
                ConsumerPropertyEditorError::unavailable(format!(
                    "consumer property editor `{}` is not registered",
                    self.property_editor_id
                ))
            })?;
        if property_editor.provider != self.provider
            || property_editor.component_type != self.component_type
        {
            return Err(ConsumerPropertyEditorError::contract(
                "consumer property runtime provider or component type does not match the registered editor",
            ));
        }
        let registered_schema = serde_json::from_value::<ConsumerPropertyEditorSchema>(
            property_editor.property_schema.clone(),
        )
        .map_err(|error| {
            ConsumerPropertyEditorError::contract(format!(
                "registered consumer property schema is invalid: {error}"
            ))
        })?;
        registered_schema.validate()?;
        if registered_schema != self.schema {
            return Err(ConsumerPropertyEditorError::contract(
                "consumer property runtime schema does not match the registered contribution",
            ));
        }
        Ok(())
    }

    pub fn load(&self) -> ConsumerPropertyLoadFuture {
        self.port.load()
    }

    pub fn prepare_save_input(
        &self,
        snapshot: &ConsumerPropertyEditorSnapshot,
        values: BTreeMap<String, String>,
    ) -> Result<SaveConsumerPropertiesInput, ConsumerPropertyEditorError> {
        require_text(&snapshot.revision, "consumer property revision")?;
        require_text(&snapshot.scope_label, "consumer property scope label")?;
        self.schema.validate_values(&snapshot.values)?;
        self.schema.validate_values(&values)?;
        Ok(SaveConsumerPropertiesInput {
            contribution_id: self.contribution_id.clone(),
            property_editor_id: self.property_editor_id.clone(),
            expected_revision: snapshot.revision.clone(),
            values,
        })
    }

    pub fn save(&self, input: SaveConsumerPropertiesInput) -> ConsumerPropertySaveFuture {
        self.port.save(input)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, thiserror::Error)]
#[error("{message} ({stable_code})")]
pub struct ConsumerPropertyEditorError {
    pub message: String,
    pub stable_code: String,
}

impl ConsumerPropertyEditorError {
    pub fn contract(message: impl Into<String>) -> Self {
        Self::new(message, CONSUMER_PROPERTY_CONTRACT_INVALID)
    }

    pub fn unavailable(message: impl Into<String>) -> Self {
        Self::new(message, CONSUMER_PROPERTY_EDITOR_UNAVAILABLE)
    }

    pub fn save(message: impl Into<String>) -> Self {
        Self::new(message, CONSUMER_PROPERTY_SAVE_FAILED)
    }

    pub fn with_stable_code(message: impl Into<String>, stable_code: impl Into<String>) -> Self {
        Self::new(message, stable_code)
    }

    fn new(message: impl Into<String>, stable_code: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            stable_code: stable_code.into(),
        }
    }
}

fn require_exact_format(format: &str) -> Result<(), ConsumerPropertyEditorError> {
    if format == PAGE_BUILDER_CONSUMER_PROPERTIES_FORMAT {
        Ok(())
    } else {
        Err(ConsumerPropertyEditorError::contract(
            "unsupported consumer property schema format",
        ))
    }
}

fn require_identifier(value: &str, label: &str) -> Result<(), ConsumerPropertyEditorError> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 128
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
        })
    {
        Err(ConsumerPropertyEditorError::contract(format!(
            "{label} is invalid"
        )))
    } else {
        Ok(())
    }
}

fn require_text(value: &str, label: &str) -> Result<(), ConsumerPropertyEditorError> {
    if value.trim().is_empty() {
        Err(ConsumerPropertyEditorError::contract(format!(
            "{label} must not be empty"
        )))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fly_ui::{
        AccessibilityMetadata, ContributionDescriptor, ContributionRegistry,
        PropertyEditorDescriptor,
    };
    use serde_json::{Map, json};

    struct NoopPort;

    impl ConsumerPropertyEditorPort for NoopPort {
        fn load(&self) -> ConsumerPropertyLoadFuture {
            Box::pin(async { Err(ConsumerPropertyEditorError::unavailable("not loaded")) })
        }

        fn save(&self, _input: SaveConsumerPropertiesInput) -> ConsumerPropertySaveFuture {
            Box::pin(async { Err(ConsumerPropertyEditorError::save("not saved")) })
        }
    }

    fn schema() -> ConsumerPropertyEditorSchema {
        ConsumerPropertyEditorSchema {
            format: PAGE_BUILDER_CONSUMER_PROPERTIES_FORMAT.to_string(),
            id: "pages.metadata.schema".to_string(),
            title: "Page metadata".to_string(),
            description: None,
            fields: vec![ConsumerPropertyFieldDescriptor {
                id: "title".to_string(),
                label: "Title".to_string(),
                help: None,
                kind: ConsumerPropertyFieldKind::Text,
                required: true,
                max_bytes: 512,
                placeholder: None,
            }],
        }
    }

    fn assembly(schema: &ConsumerPropertyEditorSchema) -> ContributionAssemblyResult {
        let contribution = ContributionDescriptor {
            id: "pages.metadata".to_string(),
            provider: "rustok.pages".to_string(),
            required_capabilities: BTreeSet::new(),
            blocks: Vec::new(),
            renderers: Vec::new(),
            property_editors: vec![PropertyEditorDescriptor {
                id: "pages.metadata.editor".to_string(),
                component_type: "rustok-pages-metadata".to_string(),
                provider: "rustok.pages".to_string(),
                property_schema: serde_json::to_value(schema).expect("schema"),
                accessibility: AccessibilityMetadata {
                    label_message_id: "pages.metadata.label".to_string(),
                    description_message_id: None,
                    keyboard_hint_message_id: None,
                },
            }],
            messages: BTreeMap::from([(
                "pages.metadata.label".to_string(),
                "Page metadata".to_string(),
            )]),
            metadata: Map::new(),
        };
        let mut registry = ContributionRegistry::default();
        registry.register(contribution).expect("contribution");
        ContributionAssemblyResult {
            registry,
            registered_contributions: 1,
            ..ContributionAssemblyResult::default()
        }
    }

    fn runtime(schema: ConsumerPropertyEditorSchema) -> ConsumerPropertyEditorRuntime {
        ConsumerPropertyEditorRuntime::new(
            "pages.metadata",
            "pages.metadata.editor",
            "rustok.pages",
            "rustok-pages-metadata",
            schema,
            Arc::new(NoopPort),
        )
    }

    #[test]
    fn runtime_requires_exact_registered_identity_schema_and_values() {
        let schema = schema();
        let assembly = assembly(&schema);
        let runtime = runtime(schema);
        runtime
            .verify_contribution(&assembly)
            .expect("registered runtime");
        let snapshot = ConsumerPropertyEditorSnapshot {
            revision: "pages:page-1:metadata:v1".to_string(),
            scope_label: "Page 1".to_string(),
            values: BTreeMap::from([("title".to_string(), "Home".to_string())]),
        };
        let input = runtime
            .prepare_save_input(&snapshot, snapshot.values.clone())
            .expect("input");
        assert_eq!(input.expected_revision, snapshot.revision);
    }

    #[test]
    fn runtime_rejects_provider_or_component_type_mismatch() {
        let schema = schema();
        let assembly = assembly(&schema);
        let runtime = ConsumerPropertyEditorRuntime::new(
            "pages.metadata",
            "pages.metadata.editor",
            "other.provider",
            "other-component",
            schema,
            Arc::new(NoopPort),
        );
        assert!(runtime.verify_contribution(&assembly).is_err());
    }

    #[test]
    fn schema_rejects_unknown_or_missing_values() {
        let schema = schema();
        assert!(schema.validate_values(&BTreeMap::new()).is_err());
        assert!(
            schema
                .validate_values(&BTreeMap::from([
                    ("title".to_string(), "Home".to_string()),
                    ("unknown".to_string(), json!(true).to_string()),
                ]))
                .is_err()
        );
    }
}
