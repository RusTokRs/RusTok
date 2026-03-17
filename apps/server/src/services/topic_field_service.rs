//! Service for managing `topic_field_definitions` — Flex Phase 4.
//!
//! Provides schema loading (with validation-ready [`CustomFieldsSchema`])
//! and full CRUD for field definitions with:
//!
//! - Guardrail: max [`MAX_FIELDS_PER_TENANT`] definitions per tenant
//! - field_key format validation (`^[a-z][a-z0-9_]{0,127}$`)
//! - Duplicate key detection
//! - Event emission: `FieldDefinitionCreated/Updated/Deleted`

use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder,
};
use uuid::Uuid;

use rustok_core::field_schema::{is_valid_field_key, CustomFieldsSchema, FieldType, FlexError};
use rustok_events::types::{DomainEvent, EventEnvelope};

use crate::models::topic_field_definitions::{
    ActiveModel, Column, CreateFieldDefinitionInput, Entity, Model, UpdateFieldDefinitionInput,
    MAX_FIELDS_PER_TENANT,
};

/// Service for topic custom field definitions.
pub struct TopicFieldService;

impl TopicFieldService {
    // ── Schema loading ────────────────────────────────────────────────────

    /// Load the active schema for a tenant from the database.
    ///
    /// The returned [`CustomFieldsSchema`] can be used directly with
    /// `validate()`, `apply_defaults()`, and `strip_unknown()`.
    pub async fn get_schema(
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> Result<CustomFieldsSchema, FlexError> {
        let rows = Entity::find_active_by_tenant(db, tenant_id)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?;

        let definitions = rows
            .into_iter()
            .filter_map(|r| r.into_field_definition())
            .collect();

        Ok(CustomFieldsSchema::new(definitions))
    }

    // ── CRUD ─────────────────────────────────────────────────────────────

    /// Create a new field definition.
    ///
    /// Enforces:
    /// - field_key format (`^[a-z][a-z0-9_]{0,127}$`)
    /// - no duplicate key for this tenant
    /// - max [`MAX_FIELDS_PER_TENANT`] active definitions
    ///
    /// Returns the created row and an event envelope ready for publishing.
    pub async fn create(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        input: CreateFieldDefinitionInput,
    ) -> Result<(Model, EventEnvelope), FlexError> {
        // Guardrail: field_key format
        if !is_valid_field_key(&input.field_key) {
            return Err(FlexError::InvalidFieldKey(input.field_key));
        }

        // Guardrail: duplicate key
        let existing = Entity::find()
            .filter(Column::TenantId.eq(tenant_id))
            .filter(Column::FieldKey.eq(&input.field_key))
            .one(db)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?;

        if existing.is_some() {
            return Err(FlexError::DuplicateFieldKey(input.field_key));
        }

        // Guardrail: max fields per tenant
        let count = Entity::find()
            .filter(Column::TenantId.eq(tenant_id))
            .filter(Column::IsActive.eq(true))
            .count(db)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?;

        if count >= MAX_FIELDS_PER_TENANT as u64 {
            return Err(FlexError::TooManyFields {
                entity_type: "topic".to_string(),
                max: MAX_FIELDS_PER_TENANT,
            });
        }

        let next_position = input.position.unwrap_or_else(|| count as i32);

        let field_type_str = serde_json::to_value(input.field_type)
            .ok()
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_default();

        let model = ActiveModel {
            id: Set(rustok_core::generate_id()),
            tenant_id: Set(tenant_id),
            field_key: Set(input.field_key.clone()),
            field_type: Set(field_type_str.clone()),
            label: Set(serde_json::to_value(&input.label).unwrap_or_default()),
            description: Set(input
                .description
                .as_ref()
                .map(|d| serde_json::to_value(d).unwrap_or_default())),
            is_required: Set(input.is_required),
            default_value: Set(input.default_value.clone()),
            validation: Set(input
                .validation
                .as_ref()
                .map(|v| serde_json::to_value(v).unwrap_or_default())),
            position: Set(next_position),
            is_active: Set(true),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(db)
        .await
        .map_err(|e| FlexError::Database(e.to_string()))?;

        let event = EventEnvelope::new(
            tenant_id,
            actor_id,
            DomainEvent::FieldDefinitionCreated {
                tenant_id,
                entity_type: "topic".to_string(),
                field_key: input.field_key,
                field_type: field_type_str,
            },
        );

        Ok((model, event))
    }

    /// Update an existing field definition.
    ///
    /// Returns the updated row and an event envelope.
    pub async fn update(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        id: Uuid,
        input: UpdateFieldDefinitionInput,
    ) -> Result<(Model, EventEnvelope), FlexError> {
        let row = Entity::find_by_id(id)
            .filter(Column::TenantId.eq(tenant_id))
            .one(db)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?
            .ok_or(FlexError::NotFound(id))?;

        let field_key = row.field_key.clone();
        let mut model: ActiveModel = row.into();

        if let Some(label) = input.label {
            model.label = Set(serde_json::to_value(label).unwrap_or_default());
        }
        if let Some(desc) = input.description {
            model.description = Set(Some(serde_json::to_value(desc).unwrap_or_default()));
        }
        if let Some(req) = input.is_required {
            model.is_required = Set(req);
        }
        if let Some(dv) = input.default_value {
            model.default_value = Set(Some(dv));
        }
        if let Some(val) = input.validation {
            model.validation = Set(Some(serde_json::to_value(val).unwrap_or_default()));
        }
        if let Some(pos) = input.position {
            model.position = Set(pos);
        }
        if let Some(active) = input.is_active {
            model.is_active = Set(active);
        }

        let updated = model
            .update(db)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?;

        let event = EventEnvelope::new(
            tenant_id,
            actor_id,
            DomainEvent::FieldDefinitionUpdated {
                tenant_id,
                entity_type: "topic".to_string(),
                field_key: field_key.clone(),
            },
        );

        Ok((updated, event))
    }

    /// Soft-delete a field definition (sets `is_active = false`).
    ///
    /// Data already stored in topic metadata (`nodes.metadata` for `kind=topic`) is preserved.
    pub async fn deactivate(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        id: Uuid,
    ) -> Result<EventEnvelope, FlexError> {
        let row = Entity::find_by_id(id)
            .filter(Column::TenantId.eq(tenant_id))
            .one(db)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?
            .ok_or(FlexError::NotFound(id))?;

        let field_key = row.field_key.clone();
        let mut model: ActiveModel = row.into();
        model.is_active = Set(false);
        model
            .update(db)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?;

        let event = EventEnvelope::new(
            tenant_id,
            actor_id,
            DomainEvent::FieldDefinitionDeleted {
                tenant_id,
                entity_type: "topic".to_string(),
                field_key,
            },
        );

        Ok(event)
    }

    /// Reorder definitions by setting their `position` according to the supplied
    /// `ids` slice (first id → position 0, second → 1, …).
    pub async fn reorder(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        ids: &[Uuid],
    ) -> Result<Vec<Model>, FlexError> {
        let mut result = Vec::with_capacity(ids.len());

        for (pos, &id) in ids.iter().enumerate() {
            let row = Entity::find_by_id(id)
                .filter(Column::TenantId.eq(tenant_id))
                .one(db)
                .await
                .map_err(|e| FlexError::Database(e.to_string()))?
                .ok_or(FlexError::NotFound(id))?;

            let mut model: ActiveModel = row.into();
            model.position = Set(pos as i32);
            let updated = model
                .update(db)
                .await
                .map_err(|e| FlexError::Database(e.to_string()))?;

            result.push(updated);
        }

        Ok(result)
    }

    /// List all definitions (including inactive) for a tenant, ordered by position.
    pub async fn list_all(
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> Result<Vec<Model>, FlexError> {
        Entity::find()
            .filter(Column::TenantId.eq(tenant_id))
            .order_by_asc(Column::Position)
            .all(db)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))
    }

    /// Find a single definition by id (tenant-scoped).
    pub async fn find_by_id(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<Model>, FlexError> {
        Entity::find_by_id(id)
            .filter(Column::TenantId.eq(tenant_id))
            .one(db)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::TopicFieldService;
    use crate::models::topic_field_definitions::{Model, MAX_FIELDS_PER_TENANT};
    use chrono::Utc;
    use rustok_core::field_schema::{FieldType, FlexError};
    use rustok_events::types::DomainEvent;
    use sea_orm::{DatabaseBackend, MockDatabase};
    use serde_json::json;
    use std::collections::HashMap;
    use uuid::Uuid;

    fn create_input(
        field_key: &str,
    ) -> crate::models::topic_field_definitions::CreateFieldDefinitionInput {
        crate::models::topic_field_definitions::CreateFieldDefinitionInput {
            field_key: field_key.to_string(),
            field_type: FieldType::Text,
            label: HashMap::from([("en".to_string(), "Label".to_string())]),
            description: None,
            is_required: false,
            default_value: None,
            validation: None,
            position: None,
        }
    }

    fn row(tenant_id: Uuid, field_key: &str) -> Model {
        let now = Utc::now().into();
        Model {
            id: Uuid::new_v4(),
            tenant_id,
            field_key: field_key.to_string(),
            field_type: "text".to_string(),
            label: json!({"en": "Label"}),
            description: None,
            is_required: false,
            default_value: None,
            validation: None,
            position: 0,
            is_active: true,
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn create_rejects_invalid_field_key_without_hitting_database() {
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let tenant_id = Uuid::new_v4();

        let err = TopicFieldService::create(
            &db,
            tenant_id,
            Some(Uuid::new_v4()),
            create_input("invalid-key"),
        )
        .await
        .expect_err("invalid field key should fail before db access");

        match err {
            FlexError::InvalidFieldKey(key) => assert_eq!(key, "invalid-key"),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[tokio::test]
    async fn create_rejects_duplicate_field_key() {
        let tenant_id = Uuid::new_v4();
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([vec![row(tenant_id, "phone")]])
            .into_connection();

        let err =
            TopicFieldService::create(&db, tenant_id, Some(Uuid::new_v4()), create_input("phone"))
                .await
                .expect_err("duplicate key should fail");

        match err {
            FlexError::DuplicateFieldKey(key) => assert_eq!(key, "phone"),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[tokio::test]
    async fn create_enforces_max_active_fields_guardrail() {
        let tenant_id = Uuid::new_v4();
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([Vec::<Model>::new()])
            .append_query_results([vec![(MAX_FIELDS_PER_TENANT as i64)]])
            .into_connection();

        let err =
            TopicFieldService::create(&db, tenant_id, Some(Uuid::new_v4()), create_input("phone"))
                .await
                .expect_err("max fields guardrail should fail");

        match err {
            FlexError::TooManyFields { entity_type, max } => {
                assert_eq!(entity_type, "topic");
                assert_eq!(max, MAX_FIELDS_PER_TENANT);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[tokio::test]
    async fn create_emits_field_definition_created_event() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let created = row(tenant_id, "phone");

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([Vec::<Model>::new()])
            .append_query_results([vec![(0_i64)]])
            .append_query_results([vec![created.clone()]])
            .into_connection();

        let (model, envelope) =
            TopicFieldService::create(&db, tenant_id, Some(actor_id), create_input("phone"))
                .await
                .expect("create should succeed");

        assert_eq!(model.field_key, "phone");
        assert_eq!(envelope.tenant_id, tenant_id);
        assert_eq!(envelope.actor_id, Some(actor_id));
        match envelope.event {
            DomainEvent::FieldDefinitionCreated {
                tenant_id: event_tenant,
                entity_type,
                field_key,
                field_type,
            } => {
                assert_eq!(event_tenant, tenant_id);
                assert_eq!(entity_type, "topic");
                assert_eq!(field_key, "phone");
                assert_eq!(field_type, "text");
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[tokio::test]
    async fn update_emits_field_definition_updated_event() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let existing = row(tenant_id, "phone");
        let id = existing.id;

        let mut updated = existing.clone();
        updated.is_required = true;

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([vec![existing]])
            .append_query_results([vec![updated.clone()]])
            .into_connection();

        let input = crate::models::topic_field_definitions::UpdateFieldDefinitionInput {
            is_required: Some(true),
            ..Default::default()
        };

        let (model, envelope) =
            TopicFieldService::update(&db, tenant_id, Some(actor_id), id, input)
                .await
                .expect("update should succeed");

        assert!(model.is_required);
        assert_eq!(envelope.tenant_id, tenant_id);
        assert_eq!(envelope.actor_id, Some(actor_id));
        match envelope.event {
            DomainEvent::FieldDefinitionUpdated {
                tenant_id: event_tenant,
                entity_type,
                field_key,
            } => {
                assert_eq!(event_tenant, tenant_id);
                assert_eq!(entity_type, "topic");
                assert_eq!(field_key, "phone");
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[tokio::test]
    async fn get_schema_skips_unknown_field_type_rows() {
        let tenant_id = Uuid::new_v4();
        let mut invalid = row(tenant_id, "legacy");
        invalid.field_type = "legacy_custom".to_string();

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([vec![row(tenant_id, "phone"), invalid]])
            .into_connection();

        let schema = TopicFieldService::get_schema(&db, tenant_id)
            .await
            .expect("schema should load");

        let errors = schema.validate(&json!({"phone": "ok", "legacy": 123}));
        assert!(
            errors.is_empty(),
            "unknown field type rows should be skipped"
        );

        let only_legacy = schema.validate(&json!({"legacy": 123}));
        assert!(only_legacy.is_empty());
    }

    #[tokio::test]
    async fn update_returns_not_found_for_missing_definition() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let id = Uuid::new_v4();
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([Vec::<Model>::new()])
            .into_connection();

        let err = TopicFieldService::update(
            &db,
            tenant_id,
            Some(actor_id),
            id,
            crate::models::topic_field_definitions::UpdateFieldDefinitionInput::default(),
        )
        .await
        .expect_err("missing row should return not found");

        match err {
            FlexError::NotFound(missing_id) => assert_eq!(missing_id, id),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[tokio::test]
    async fn deactivate_returns_not_found_for_missing_definition() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let id = Uuid::new_v4();
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([Vec::<Model>::new()])
            .into_connection();

        let err = TopicFieldService::deactivate(&db, tenant_id, Some(actor_id), id)
            .await
            .expect_err("missing row should return not found");

        match err {
            FlexError::NotFound(missing_id) => assert_eq!(missing_id, id),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[tokio::test]
    async fn deactivate_emits_field_definition_deleted_event() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let model = row(tenant_id, "phone");
        let id = model.id;

        let mut deactivated = model.clone();
        deactivated.is_active = false;

        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([vec![model]])
            .append_query_results([vec![deactivated]])
            .into_connection();

        let envelope = TopicFieldService::deactivate(&db, tenant_id, Some(actor_id), id)
            .await
            .expect("deactivate should succeed");

        assert_eq!(envelope.tenant_id, tenant_id);
        assert_eq!(envelope.actor_id, Some(actor_id));
        match envelope.event {
            DomainEvent::FieldDefinitionDeleted {
                tenant_id: e_tenant,
                entity_type,
                field_key,
            } => {
                assert_eq!(e_tenant, tenant_id);
                assert_eq!(entity_type, "topic");
                assert_eq!(field_key, "phone");
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }
}
