//! Transport-agnostic contracts for Flex standalone mode (Phase 5).

use async_trait::async_trait;

use crate::events::{
    flex_entry_created_event, flex_entry_deleted_event, flex_entry_updated_event,
    flex_schema_created_event, flex_schema_deleted_event, flex_schema_updated_event,
};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use rustok_core::field_schema::{is_valid_field_key, FieldDefinition, FlexError};
use rustok_events::EventEnvelope;

/// Standalone Flex schema view used by transport adapters.
#[derive(Debug, Clone)]
pub struct FlexSchemaView {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub fields_config: Vec<FieldDefinition>,
    pub settings: JsonValue,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// Standalone Flex entry view used by transport adapters.
#[derive(Debug, Clone)]
pub struct FlexEntryView {
    pub id: Uuid,
    pub schema_id: Uuid,
    pub entity_type: Option<String>,
    pub entity_id: Option<Uuid>,
    pub data: JsonValue,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Transport-agnostic command for creating a standalone schema.
#[derive(Debug, Clone)]
pub struct CreateFlexSchemaCommand {
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub fields_config: Vec<FieldDefinition>,
    pub settings: Option<JsonValue>,
    pub is_active: Option<bool>,
}

/// Transport-agnostic command for updating a standalone schema.
#[derive(Debug, Clone, Default)]
pub struct UpdateFlexSchemaCommand {
    pub name: Option<String>,
    pub description: Option<String>,
    pub fields_config: Option<Vec<FieldDefinition>>,
    pub settings: Option<JsonValue>,
    pub is_active: Option<bool>,
}

/// Transport-agnostic command for creating a standalone entry.
#[derive(Debug, Clone)]
pub struct CreateFlexEntryCommand {
    pub schema_id: Uuid,
    pub entity_type: Option<String>,
    pub entity_id: Option<Uuid>,
    pub data: JsonValue,
    pub status: Option<String>,
}

/// Transport-agnostic command for updating a standalone entry.
#[derive(Debug, Clone, Default)]
pub struct UpdateFlexEntryCommand {
    pub data: Option<JsonValue>,
    pub status: Option<String>,
}

/// Validate standalone schema command before handing it to adapter/service layer.
pub fn validate_create_schema_command(input: &CreateFlexSchemaCommand) -> Result<(), FlexError> {
    if !is_valid_field_key(input.slug.trim()) {
        return Err(FlexError::InvalidFieldKey(
            "schema slug must match ^[a-z][a-z0-9_]{0,127}$".to_string(),
        ));
    }

    if input.name.trim().is_empty() {
        return Err(FlexError::InvalidFieldKey(
            "schema name must not be empty".to_string(),
        ));
    }

    validate_definition_keys(&input.fields_config)
}

/// Validate standalone entry command before handing it to adapter/service layer.
pub fn validate_create_entry_command(input: &CreateFlexEntryCommand) -> Result<(), FlexError> {
    let relation_shape_ok = (input.entity_type.is_some() && input.entity_id.is_some())
        || (input.entity_type.is_none() && input.entity_id.is_none());

    if !relation_shape_ok {
        return Err(FlexError::InvalidFieldKey(
            "entity_type and entity_id must be set together or both be null".to_string(),
        ));
    }

    if let Some(entity_type) = &input.entity_type {
        if !is_valid_field_key(entity_type.trim()) {
            return Err(FlexError::InvalidFieldKey(
                "entity_type must match ^[a-z][a-z0-9_]{0,127}$".to_string(),
            ));
        }
    }

    if let Some(status) = &input.status {
        if status.trim().is_empty() {
            return Err(FlexError::InvalidFieldKey(
                "status must not be empty".to_string(),
            ));
        }
    }

    Ok(())
}

/// Validate standalone schema patch command before handing it to adapter/service layer.
pub fn validate_update_schema_command(input: &UpdateFlexSchemaCommand) -> Result<(), FlexError> {
    if let Some(name) = &input.name {
        if name.trim().is_empty() {
            return Err(FlexError::InvalidFieldKey(
                "schema name must not be empty".to_string(),
            ));
        }
    }

    if let Some(fields_config) = &input.fields_config {
        validate_definition_keys(fields_config)?;
    }

    Ok(())
}

/// Validate standalone entry patch command before handing it to adapter/service layer.
pub fn validate_update_entry_command(input: &UpdateFlexEntryCommand) -> Result<(), FlexError> {
    if let Some(status) = &input.status {
        if status.trim().is_empty() {
            return Err(FlexError::InvalidFieldKey(
                "status must not be empty".to_string(),
            ));
        }
    }

    Ok(())
}

/// Orchestrates schema listing through standalone service.
pub async fn list_schemas(
    service: &dyn FlexStandaloneService,
    tenant_id: Uuid,
) -> Result<Vec<FlexSchemaView>, FlexError> {
    service.list_schemas(tenant_id).await
}

/// Orchestrates schema lookup through standalone service.
pub async fn find_schema(
    service: &dyn FlexStandaloneService,
    tenant_id: Uuid,
    schema_id: Uuid,
) -> Result<Option<FlexSchemaView>, FlexError> {
    service.find_schema(tenant_id, schema_id).await
}

/// Orchestrates schema deletion through standalone service.
pub async fn delete_schema(
    service: &dyn FlexStandaloneService,
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    schema_id: Uuid,
) -> Result<(), FlexError> {
    service.delete_schema(tenant_id, actor_id, schema_id).await
}

/// Orchestrates entries listing through standalone service.
pub async fn list_entries(
    service: &dyn FlexStandaloneService,
    tenant_id: Uuid,
    schema_id: Uuid,
) -> Result<Vec<FlexEntryView>, FlexError> {
    service.list_entries(tenant_id, schema_id).await
}

/// Orchestrates entry lookup through standalone service.
pub async fn find_entry(
    service: &dyn FlexStandaloneService,
    tenant_id: Uuid,
    schema_id: Uuid,
    entry_id: Uuid,
) -> Result<Option<FlexEntryView>, FlexError> {
    service.find_entry(tenant_id, schema_id, entry_id).await
}

/// Orchestrates entry deletion through standalone service.
pub async fn delete_entry(
    service: &dyn FlexStandaloneService,
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    schema_id: Uuid,
    entry_id: Uuid,
) -> Result<(), FlexError> {
    service
        .delete_entry(tenant_id, actor_id, schema_id, entry_id)
        .await
}

/// Orchestrates `create_schema` with contract-level validation.
pub async fn create_schema(
    service: &dyn FlexStandaloneService,
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    input: CreateFlexSchemaCommand,
) -> Result<FlexSchemaView, FlexError> {
    validate_create_schema_command(&input)?;
    service.create_schema(tenant_id, actor_id, input).await
}

/// Orchestrates `update_schema` with contract-level validation.
pub async fn update_schema(
    service: &dyn FlexStandaloneService,
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    schema_id: Uuid,
    input: UpdateFlexSchemaCommand,
) -> Result<FlexSchemaView, FlexError> {
    validate_update_schema_command(&input)?;
    service
        .update_schema(tenant_id, actor_id, schema_id, input)
        .await
}

/// Orchestrates `create_entry` with contract-level validation.
pub async fn create_entry(
    service: &dyn FlexStandaloneService,
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    input: CreateFlexEntryCommand,
) -> Result<FlexEntryView, FlexError> {
    validate_create_entry_command(&input)?;
    service.create_entry(tenant_id, actor_id, input).await
}

/// Orchestrates `update_entry` with contract-level validation.
pub async fn update_entry(
    service: &dyn FlexStandaloneService,
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    schema_id: Uuid,
    entry_id: Uuid,
    input: UpdateFlexEntryCommand,
) -> Result<FlexEntryView, FlexError> {
    validate_update_entry_command(&input)?;
    service
        .update_entry(tenant_id, actor_id, schema_id, entry_id, input)
        .await
}

/// Orchestrates `create_schema` and builds `flex.schema.created` event envelope.
pub async fn create_schema_with_event(
    service: &dyn FlexStandaloneService,
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    input: CreateFlexSchemaCommand,
) -> Result<(FlexSchemaView, EventEnvelope), FlexError> {
    let view = create_schema(service, tenant_id, actor_id, input).await?;
    let event = flex_schema_created_event(tenant_id, actor_id, view.id, view.slug.clone());
    Ok((view, event))
}

/// Orchestrates `update_schema` and builds `flex.schema.updated` event envelope.
pub async fn update_schema_with_event(
    service: &dyn FlexStandaloneService,
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    schema_id: Uuid,
    input: UpdateFlexSchemaCommand,
) -> Result<(FlexSchemaView, EventEnvelope), FlexError> {
    let view = update_schema(service, tenant_id, actor_id, schema_id, input).await?;
    let event = flex_schema_updated_event(tenant_id, actor_id, view.id, view.slug.clone());
    Ok((view, event))
}

/// Orchestrates `delete_schema` and builds `flex.schema.deleted` event envelope.
pub async fn delete_schema_with_event(
    service: &dyn FlexStandaloneService,
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    schema_id: Uuid,
) -> Result<EventEnvelope, FlexError> {
    delete_schema(service, tenant_id, actor_id, schema_id).await?;
    Ok(flex_schema_deleted_event(tenant_id, actor_id, schema_id))
}

/// Orchestrates `create_entry` and builds `flex.entry.created` event envelope.
pub async fn create_entry_with_event(
    service: &dyn FlexStandaloneService,
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    input: CreateFlexEntryCommand,
) -> Result<(FlexEntryView, EventEnvelope), FlexError> {
    let view = create_entry(service, tenant_id, actor_id, input).await?;
    let event = flex_entry_created_event(
        tenant_id,
        actor_id,
        view.schema_id,
        view.id,
        view.entity_type.clone(),
        view.entity_id,
    );
    Ok((view, event))
}

/// Orchestrates `update_entry` and builds `flex.entry.updated` event envelope.
pub async fn update_entry_with_event(
    service: &dyn FlexStandaloneService,
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    schema_id: Uuid,
    entry_id: Uuid,
    input: UpdateFlexEntryCommand,
) -> Result<(FlexEntryView, EventEnvelope), FlexError> {
    let view = update_entry(service, tenant_id, actor_id, schema_id, entry_id, input).await?;
    let event = flex_entry_updated_event(tenant_id, actor_id, view.schema_id, view.id);
    Ok((view, event))
}

/// Orchestrates `delete_entry` and builds `flex.entry.deleted` event envelope.
pub async fn delete_entry_with_event(
    service: &dyn FlexStandaloneService,
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    schema_id: Uuid,
    entry_id: Uuid,
) -> Result<EventEnvelope, FlexError> {
    delete_entry(service, tenant_id, actor_id, schema_id, entry_id).await?;
    Ok(flex_entry_deleted_event(
        tenant_id, actor_id, schema_id, entry_id,
    ))
}

fn validate_definition_keys(definitions: &[FieldDefinition]) -> Result<(), FlexError> {
    let mut unique = std::collections::HashSet::new();
    for def in definitions {
        if !is_valid_field_key(def.field_key.trim()) {
            return Err(FlexError::InvalidFieldKey(format!(
                "invalid field key in fields_config: {}",
                def.field_key
            )));
        }

        if !unique.insert(def.field_key.as_str()) {
            return Err(FlexError::DuplicateFieldKey(def.field_key.clone()));
        }
    }
    Ok(())
}

/// Service contract for standalone Flex mode.
#[async_trait]
pub trait FlexStandaloneService: Send + Sync {
    async fn list_schemas(&self, tenant_id: Uuid) -> Result<Vec<FlexSchemaView>, FlexError>;

    async fn find_schema(
        &self,
        tenant_id: Uuid,
        schema_id: Uuid,
    ) -> Result<Option<FlexSchemaView>, FlexError>;

    async fn create_schema(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        input: CreateFlexSchemaCommand,
    ) -> Result<FlexSchemaView, FlexError>;

    async fn update_schema(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        schema_id: Uuid,
        input: UpdateFlexSchemaCommand,
    ) -> Result<FlexSchemaView, FlexError>;

    async fn delete_schema(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        schema_id: Uuid,
    ) -> Result<(), FlexError>;

    async fn list_entries(
        &self,
        tenant_id: Uuid,
        schema_id: Uuid,
    ) -> Result<Vec<FlexEntryView>, FlexError>;

    async fn find_entry(
        &self,
        tenant_id: Uuid,
        schema_id: Uuid,
        entry_id: Uuid,
    ) -> Result<Option<FlexEntryView>, FlexError>;

    async fn create_entry(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        input: CreateFlexEntryCommand,
    ) -> Result<FlexEntryView, FlexError>;

    async fn update_entry(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        schema_id: Uuid,
        entry_id: Uuid,
        input: UpdateFlexEntryCommand,
    ) -> Result<FlexEntryView, FlexError>;

    async fn delete_entry(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        schema_id: Uuid,
        entry_id: Uuid,
    ) -> Result<(), FlexError>;
}

#[cfg(test)]
mod tests {
    use super::{
        create_entry, create_entry_with_event, create_schema, create_schema_with_event,
        delete_entry, delete_entry_with_event, delete_schema, delete_schema_with_event, find_entry,
        find_schema, list_entries, list_schemas, update_entry_with_event, update_schema_with_event,
        validate_create_entry_command, validate_create_schema_command,
        validate_update_entry_command, validate_update_schema_command, CreateFlexEntryCommand,
        CreateFlexSchemaCommand, FlexEntryView, FlexSchemaView, FlexStandaloneService,
        UpdateFlexEntryCommand, UpdateFlexSchemaCommand,
    };
    use async_trait::async_trait;

    use rustok_core::field_schema::{FieldDefinition, FieldType, FlexError};
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use uuid::Uuid;

    fn sample_definition(key: &str) -> FieldDefinition {
        FieldDefinition {
            field_key: key.to_string(),
            field_type: FieldType::Text,
            label: HashMap::from([("en".to_string(), "Label".to_string())]),
            description: None,
            is_required: false,
            default_value: None,
            validation: None,
            position: 0,
            is_active: true,
        }
    }

    #[test]
    fn update_command_default_is_empty_patch() {
        let patch = UpdateFlexSchemaCommand::default();
        assert!(patch.name.is_none());
        assert!(patch.description.is_none());
        assert!(patch.fields_config.is_none());
        assert!(patch.settings.is_none());
        assert!(patch.is_active.is_none());
    }

    #[test]
    fn create_schema_command_keeps_optional_flags() {
        let cmd = CreateFlexSchemaCommand {
            slug: "landing".to_string(),
            name: "Landing".to_string(),
            description: None,
            fields_config: Vec::new(),
            settings: None,
            is_active: None,
        };

        assert!(cmd.is_active.is_none());
        assert!(cmd.settings.is_none());
    }

    #[test]
    fn validate_schema_command_rejects_invalid_slug_and_duplicate_keys() {
        let invalid_slug = CreateFlexSchemaCommand {
            slug: "Landing Page".to_string(),
            name: "Landing".to_string(),
            description: None,
            fields_config: vec![],
            settings: None,
            is_active: None,
        };

        assert!(validate_create_schema_command(&invalid_slug).is_err());

        let duplicate_keys = CreateFlexSchemaCommand {
            slug: "landing_page".to_string(),
            name: "Landing".to_string(),
            description: None,
            fields_config: vec![sample_definition("title"), sample_definition("title")],
            settings: None,
            is_active: None,
        };

        assert!(validate_create_schema_command(&duplicate_keys).is_err());
    }

    #[test]
    fn validate_update_schema_command_rejects_empty_name_and_duplicate_keys() {
        let empty_name = UpdateFlexSchemaCommand {
            name: Some("   ".to_string()),
            ..Default::default()
        };
        assert!(validate_update_schema_command(&empty_name).is_err());

        let duplicate_keys = UpdateFlexSchemaCommand {
            fields_config: Some(vec![sample_definition("title"), sample_definition("title")]),
            ..Default::default()
        };
        assert!(validate_update_schema_command(&duplicate_keys).is_err());
    }

    #[test]
    fn validate_update_entry_command_rejects_empty_status() {
        let invalid = UpdateFlexEntryCommand {
            data: None,
            status: Some("   ".to_string()),
        };

        assert!(validate_update_entry_command(&invalid).is_err());

        let valid = UpdateFlexEntryCommand {
            data: None,
            status: Some("published".to_string()),
        };

        assert!(validate_update_entry_command(&valid).is_ok());
    }

    #[test]
    fn validate_entry_command_enforces_entity_binding_pair() {
        let invalid = CreateFlexEntryCommand {
            schema_id: Uuid::new_v4(),
            entity_type: Some("product".to_string()),
            entity_id: None,
            data: json!({"title": "Hello"}),
            status: Some("draft".to_string()),
        };

        assert!(validate_create_entry_command(&invalid).is_err());

        let valid = CreateFlexEntryCommand {
            schema_id: Uuid::new_v4(),
            entity_type: Some("product".to_string()),
            entity_id: Some(Uuid::new_v4()),
            data: json!({"title": "Hello"}),
            status: Some("draft".to_string()),
        };

        assert!(validate_create_entry_command(&valid).is_ok());
    }

    struct MockStandaloneService {
        create_schema_calls: Arc<AtomicUsize>,
        update_schema_calls: Arc<AtomicUsize>,
        delete_schema_calls: Arc<AtomicUsize>,
        list_schema_calls: Arc<AtomicUsize>,
        find_schema_calls: Arc<AtomicUsize>,

        create_entry_calls: Arc<AtomicUsize>,
        update_entry_calls: Arc<AtomicUsize>,
        delete_entry_calls: Arc<AtomicUsize>,
        list_entries_calls: Arc<AtomicUsize>,
        find_entry_calls: Arc<AtomicUsize>,
    }

    fn mock_service() -> MockStandaloneService {
        MockStandaloneService {
            create_schema_calls: Arc::new(AtomicUsize::new(0)),
            update_schema_calls: Arc::new(AtomicUsize::new(0)),
            delete_schema_calls: Arc::new(AtomicUsize::new(0)),
            list_schema_calls: Arc::new(AtomicUsize::new(0)),
            find_schema_calls: Arc::new(AtomicUsize::new(0)),
            create_entry_calls: Arc::new(AtomicUsize::new(0)),
            update_entry_calls: Arc::new(AtomicUsize::new(0)),
            delete_entry_calls: Arc::new(AtomicUsize::new(0)),
            list_entries_calls: Arc::new(AtomicUsize::new(0)),
            find_entry_calls: Arc::new(AtomicUsize::new(0)),
        }
    }

    #[async_trait]
    impl FlexStandaloneService for MockStandaloneService {
        async fn list_schemas(&self, _tenant_id: Uuid) -> Result<Vec<FlexSchemaView>, FlexError> {
            self.list_schema_calls.fetch_add(1, Ordering::SeqCst);
            Ok(Vec::new())
        }

        async fn find_schema(
            &self,
            _tenant_id: Uuid,
            _schema_id: Uuid,
        ) -> Result<Option<FlexSchemaView>, FlexError> {
            self.find_schema_calls.fetch_add(1, Ordering::SeqCst);
            Ok(None)
        }

        async fn create_schema(
            &self,
            _tenant_id: Uuid,
            _actor_id: Option<Uuid>,
            input: CreateFlexSchemaCommand,
        ) -> Result<FlexSchemaView, FlexError> {
            self.create_schema_calls.fetch_add(1, Ordering::SeqCst);
            Ok(FlexSchemaView {
                id: Uuid::new_v4(),
                slug: input.slug,
                name: input.name,
                description: input.description,
                fields_config: input.fields_config,
                settings: input.settings.unwrap_or_else(|| json!({})),
                is_active: input.is_active.unwrap_or(true),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:00:00Z".to_string(),
            })
        }

        async fn update_schema(
            &self,
            _tenant_id: Uuid,
            _actor_id: Option<Uuid>,
            schema_id: Uuid,
            input: UpdateFlexSchemaCommand,
        ) -> Result<FlexSchemaView, FlexError> {
            self.update_schema_calls.fetch_add(1, Ordering::SeqCst);
            Ok(FlexSchemaView {
                id: schema_id,
                slug: "landing_page".to_string(),
                name: input.name.unwrap_or_else(|| "Landing".to_string()),
                description: input.description,
                fields_config: input.fields_config.unwrap_or_default(),
                settings: input.settings.unwrap_or_else(|| json!({})),
                is_active: input.is_active.unwrap_or(true),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-02T00:00:00Z".to_string(),
            })
        }

        async fn delete_schema(
            &self,
            _tenant_id: Uuid,
            _actor_id: Option<Uuid>,
            _schema_id: Uuid,
        ) -> Result<(), FlexError> {
            self.delete_schema_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn list_entries(
            &self,
            _tenant_id: Uuid,
            _schema_id: Uuid,
        ) -> Result<Vec<FlexEntryView>, FlexError> {
            self.list_entries_calls.fetch_add(1, Ordering::SeqCst);
            Ok(Vec::new())
        }

        async fn find_entry(
            &self,
            _tenant_id: Uuid,
            _schema_id: Uuid,
            _entry_id: Uuid,
        ) -> Result<Option<FlexEntryView>, FlexError> {
            self.find_entry_calls.fetch_add(1, Ordering::SeqCst);
            Ok(None)
        }

        async fn create_entry(
            &self,
            _tenant_id: Uuid,
            _actor_id: Option<Uuid>,
            input: CreateFlexEntryCommand,
        ) -> Result<FlexEntryView, FlexError> {
            self.create_entry_calls.fetch_add(1, Ordering::SeqCst);
            Ok(FlexEntryView {
                id: Uuid::new_v4(),
                schema_id: input.schema_id,
                entity_type: input.entity_type,
                entity_id: input.entity_id,
                data: input.data,
                status: input.status.unwrap_or_else(|| "draft".to_string()),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:00:00Z".to_string(),
            })
        }

        async fn update_entry(
            &self,
            _tenant_id: Uuid,
            _actor_id: Option<Uuid>,
            schema_id: Uuid,
            entry_id: Uuid,
            input: UpdateFlexEntryCommand,
        ) -> Result<FlexEntryView, FlexError> {
            self.update_entry_calls.fetch_add(1, Ordering::SeqCst);
            Ok(FlexEntryView {
                id: entry_id,
                schema_id,
                entity_type: None,
                entity_id: None,
                data: input.data.unwrap_or_else(|| json!({"title": "Updated"})),
                status: input.status.unwrap_or_else(|| "published".to_string()),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-02T00:00:00Z".to_string(),
            })
        }

        async fn delete_entry(
            &self,
            _tenant_id: Uuid,
            _actor_id: Option<Uuid>,
            _schema_id: Uuid,
            _entry_id: Uuid,
        ) -> Result<(), FlexError> {
            self.delete_entry_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn create_schema_orchestration_skips_service_on_invalid_input() {
        let calls = Arc::new(AtomicUsize::new(0));
        let service = MockStandaloneService {
            create_schema_calls: calls.clone(),
            update_schema_calls: Arc::new(AtomicUsize::new(0)),
            create_entry_calls: Arc::new(AtomicUsize::new(0)),
            update_entry_calls: Arc::new(AtomicUsize::new(0)),
            list_schema_calls: Arc::new(AtomicUsize::new(0)),
            find_schema_calls: Arc::new(AtomicUsize::new(0)),
            delete_entry_calls: Arc::new(AtomicUsize::new(0)),
            delete_schema_calls: Arc::new(AtomicUsize::new(0)),
            list_entries_calls: Arc::new(AtomicUsize::new(0)),
            find_entry_calls: Arc::new(AtomicUsize::new(0)),
        };

        let res = create_schema(
            &service,
            Uuid::new_v4(),
            Some(Uuid::new_v4()),
            CreateFlexSchemaCommand {
                slug: "Invalid Slug".to_string(),
                name: "Landing".to_string(),
                description: None,
                fields_config: vec![],
                settings: None,
                is_active: None,
            },
        )
        .await;

        assert!(res.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn create_entry_orchestration_calls_service_for_valid_input() {
        let calls = Arc::new(AtomicUsize::new(0));
        let service = MockStandaloneService {
            create_schema_calls: Arc::new(AtomicUsize::new(0)),
            update_schema_calls: Arc::new(AtomicUsize::new(0)),
            create_entry_calls: calls.clone(),
            update_entry_calls: Arc::new(AtomicUsize::new(0)),
            list_schema_calls: Arc::new(AtomicUsize::new(0)),
            find_schema_calls: Arc::new(AtomicUsize::new(0)),
            delete_entry_calls: Arc::new(AtomicUsize::new(0)),
            delete_schema_calls: Arc::new(AtomicUsize::new(0)),
            list_entries_calls: Arc::new(AtomicUsize::new(0)),
            find_entry_calls: Arc::new(AtomicUsize::new(0)),
        };

        let res = create_entry(
            &service,
            Uuid::new_v4(),
            Some(Uuid::new_v4()),
            CreateFlexEntryCommand {
                schema_id: Uuid::new_v4(),
                entity_type: Some("product".to_string()),
                entity_id: Some(Uuid::new_v4()),
                data: json!({"title": "Hello"}),
                status: Some("draft".to_string()),
            },
        )
        .await
        .expect("valid input should pass");

        assert_eq!(res.status, "draft");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn list_and_find_schema_orchestration_delegate_to_service() {
        let list_calls = Arc::new(AtomicUsize::new(0));
        let find_calls = Arc::new(AtomicUsize::new(0));
        let service = MockStandaloneService {
            create_schema_calls: Arc::new(AtomicUsize::new(0)),
            update_schema_calls: Arc::new(AtomicUsize::new(0)),
            create_entry_calls: Arc::new(AtomicUsize::new(0)),
            update_entry_calls: Arc::new(AtomicUsize::new(0)),
            list_schema_calls: list_calls.clone(),
            find_schema_calls: find_calls.clone(),
            delete_entry_calls: Arc::new(AtomicUsize::new(0)),
            delete_schema_calls: Arc::new(AtomicUsize::new(0)),
            list_entries_calls: Arc::new(AtomicUsize::new(0)),
            find_entry_calls: Arc::new(AtomicUsize::new(0)),
        };

        let _ = list_schemas(&service, Uuid::new_v4()).await.expect("list");
        let _ = find_schema(&service, Uuid::new_v4(), Uuid::new_v4())
            .await
            .expect("find");

        assert_eq!(list_calls.load(Ordering::SeqCst), 1);
        assert_eq!(find_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn delete_entry_orchestration_delegates_to_service() {
        let delete_calls = Arc::new(AtomicUsize::new(0));
        let service = MockStandaloneService {
            create_schema_calls: Arc::new(AtomicUsize::new(0)),
            update_schema_calls: Arc::new(AtomicUsize::new(0)),
            create_entry_calls: Arc::new(AtomicUsize::new(0)),
            update_entry_calls: Arc::new(AtomicUsize::new(0)),
            list_schema_calls: Arc::new(AtomicUsize::new(0)),
            find_schema_calls: Arc::new(AtomicUsize::new(0)),
            delete_entry_calls: delete_calls.clone(),
            delete_schema_calls: Arc::new(AtomicUsize::new(0)),
            list_entries_calls: Arc::new(AtomicUsize::new(0)),
            find_entry_calls: Arc::new(AtomicUsize::new(0)),
        };

        delete_entry(
            &service,
            Uuid::new_v4(),
            Some(Uuid::new_v4()),
            Uuid::new_v4(),
            Uuid::new_v4(),
        )
        .await
        .expect("delete entry");

        assert_eq!(delete_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn list_and_find_entry_orchestration_delegate_to_service() {
        let list_calls = Arc::new(AtomicUsize::new(0));
        let find_calls = Arc::new(AtomicUsize::new(0));
        let service = MockStandaloneService {
            create_schema_calls: Arc::new(AtomicUsize::new(0)),
            update_schema_calls: Arc::new(AtomicUsize::new(0)),
            create_entry_calls: Arc::new(AtomicUsize::new(0)),
            update_entry_calls: Arc::new(AtomicUsize::new(0)),
            list_schema_calls: Arc::new(AtomicUsize::new(0)),
            find_schema_calls: Arc::new(AtomicUsize::new(0)),
            delete_entry_calls: Arc::new(AtomicUsize::new(0)),
            delete_schema_calls: Arc::new(AtomicUsize::new(0)),
            list_entries_calls: list_calls.clone(),
            find_entry_calls: find_calls.clone(),
        };

        let _ = list_entries(&service, Uuid::new_v4(), Uuid::new_v4())
            .await
            .expect("list entries");
        let _ = find_entry(&service, Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4())
            .await
            .expect("find entry");

        assert_eq!(list_calls.load(Ordering::SeqCst), 1);
        assert_eq!(find_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn delete_schema_orchestration_delegates_to_service() {
        let delete_calls = Arc::new(AtomicUsize::new(0));
        let service = MockStandaloneService {
            create_schema_calls: Arc::new(AtomicUsize::new(0)),
            update_schema_calls: Arc::new(AtomicUsize::new(0)),
            create_entry_calls: Arc::new(AtomicUsize::new(0)),
            update_entry_calls: Arc::new(AtomicUsize::new(0)),
            list_schema_calls: Arc::new(AtomicUsize::new(0)),
            find_schema_calls: Arc::new(AtomicUsize::new(0)),
            delete_entry_calls: Arc::new(AtomicUsize::new(0)),
            delete_schema_calls: delete_calls.clone(),
            list_entries_calls: Arc::new(AtomicUsize::new(0)),
            find_entry_calls: Arc::new(AtomicUsize::new(0)),
        };

        delete_schema(
            &service,
            Uuid::new_v4(),
            Some(Uuid::new_v4()),
            Uuid::new_v4(),
        )
        .await
        .expect("delete schema");

        assert_eq!(delete_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn create_schema_with_event_returns_event_envelope() {
        let service = mock_service();

        let tenant_id = Uuid::new_v4();
        let actor_id = Some(Uuid::new_v4());
        let (view, event) = create_schema_with_event(
            &service,
            tenant_id,
            actor_id,
            CreateFlexSchemaCommand {
                slug: "landing_page".to_string(),
                name: "Landing".to_string(),
                description: None,
                fields_config: vec![],
                settings: None,
                is_active: None,
            },
        )
        .await
        .expect("create schema with event");

        assert_eq!(event.event_type, "flex.schema.created");
        assert_eq!(event.tenant_id, tenant_id);
        assert_eq!(view.slug, "landing_page");
    }

    #[tokio::test]
    async fn delete_entry_with_event_returns_event_envelope() {
        let service = mock_service();

        let tenant_id = Uuid::new_v4();
        let schema_id = Uuid::new_v4();
        let entry_id = Uuid::new_v4();
        let event = delete_entry_with_event(
            &service,
            tenant_id,
            Some(Uuid::new_v4()),
            schema_id,
            entry_id,
        )
        .await
        .expect("delete entry with event");

        assert_eq!(event.event_type, "flex.entry.deleted");
        assert_eq!(event.tenant_id, tenant_id);
    }

    #[tokio::test]
    async fn update_schema_with_event_returns_event_envelope() {
        let service = mock_service();

        let tenant_id = Uuid::new_v4();
        let schema_id = Uuid::new_v4();
        let (_view, event) = update_schema_with_event(
            &service,
            tenant_id,
            Some(Uuid::new_v4()),
            schema_id,
            UpdateFlexSchemaCommand {
                name: Some("Landing v2".to_string()),
                ..Default::default()
            },
        )
        .await
        .expect("update schema with event");

        assert_eq!(event.event_type, "flex.schema.updated");
        assert_eq!(event.tenant_id, tenant_id);
    }

    #[tokio::test]
    async fn delete_schema_with_event_returns_event_envelope() {
        let service = mock_service();

        let tenant_id = Uuid::new_v4();
        let schema_id = Uuid::new_v4();
        let event = delete_schema_with_event(&service, tenant_id, Some(Uuid::new_v4()), schema_id)
            .await
            .expect("delete schema with event");

        assert_eq!(event.event_type, "flex.schema.deleted");
        assert_eq!(event.tenant_id, tenant_id);
    }

    #[tokio::test]
    async fn create_entry_with_event_returns_event_envelope() {
        let service = mock_service();

        let tenant_id = Uuid::new_v4();
        let (_view, event) = create_entry_with_event(
            &service,
            tenant_id,
            Some(Uuid::new_v4()),
            CreateFlexEntryCommand {
                schema_id: Uuid::new_v4(),
                entity_type: None,
                entity_id: None,
                data: json!({"title": "Hello"}),
                status: Some("draft".to_string()),
            },
        )
        .await
        .expect("create entry with event");

        assert_eq!(event.event_type, "flex.entry.created");
        assert_eq!(event.tenant_id, tenant_id);
    }

    #[tokio::test]
    async fn update_entry_with_event_returns_event_envelope() {
        let service = mock_service();

        let tenant_id = Uuid::new_v4();
        let schema_id = Uuid::new_v4();
        let entry_id = Uuid::new_v4();
        let (_view, event) = update_entry_with_event(
            &service,
            tenant_id,
            Some(Uuid::new_v4()),
            schema_id,
            entry_id,
            UpdateFlexEntryCommand {
                data: Some(json!({"title": "Updated"})),
                status: Some("published".to_string()),
            },
        )
        .await
        .expect("update entry with event");

        assert_eq!(event.event_type, "flex.entry.updated");
        assert_eq!(event.tenant_id, tenant_id);
    }

    #[tokio::test]
    async fn update_schema_with_event_skips_service_on_invalid_input() {
        let update_calls = Arc::new(AtomicUsize::new(0));
        let mut service = mock_service();
        service.update_schema_calls = update_calls.clone();

        let res = update_schema_with_event(
            &service,
            Uuid::new_v4(),
            Some(Uuid::new_v4()),
            Uuid::new_v4(),
            UpdateFlexSchemaCommand {
                name: Some("   ".to_string()),
                ..Default::default()
            },
        )
        .await;

        assert!(res.is_err());
        assert_eq!(update_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn update_entry_with_event_skips_service_on_invalid_input() {
        let update_calls = Arc::new(AtomicUsize::new(0));
        let mut service = mock_service();
        service.update_entry_calls = update_calls.clone();

        let res = update_entry_with_event(
            &service,
            Uuid::new_v4(),
            Some(Uuid::new_v4()),
            Uuid::new_v4(),
            Uuid::new_v4(),
            UpdateFlexEntryCommand {
                data: None,
                status: Some("   ".to_string()),
            },
        )
        .await;

        assert!(res.is_err());
        assert_eq!(update_calls.load(Ordering::SeqCst), 0);
    }
}
