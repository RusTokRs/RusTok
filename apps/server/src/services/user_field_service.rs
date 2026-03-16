//! Service for managing `user_field_definitions` — Flex Phase 2.
//!
//! Provides schema loading (with validation-ready [`CustomFieldsSchema`])
//! and full CRUD for field definitions with:
//!
//! - Guardrail: max [`MAX_FIELDS_PER_TENANT`] definitions per tenant
//! - field_key format validation (`^[a-z][a-z0-9_]{0,127}$`)
//! - Duplicate key detection
//! - Event emission: `FieldDefinitionCreated/Updated/Deleted`

use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait,
    QueryFilter, QueryOrder,
};
use uuid::Uuid;

use rustok_core::field_schema::{
    CustomFieldsSchema, FieldType, FlexError,
    is_valid_field_key,
};
use rustok_events::types::{DomainEvent, EventEnvelope};

use crate::models::user_field_definitions::{
    ActiveModel, Column, CreateFieldDefinitionInput, Entity, Model, UpdateFieldDefinitionInput,
    MAX_FIELDS_PER_TENANT,
};

/// Service for user custom field definitions.
pub struct UserFieldService;

impl UserFieldService {
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
                entity_type: "user".to_string(),
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
                entity_type: "user".to_string(),
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
                entity_type: "user".to_string(),
                field_key: field_key.clone(),
            },
        );

        Ok((updated, event))
    }

    /// Soft-delete a field definition (sets `is_active = false`).
    ///
    /// Data already stored in `users.metadata` is preserved.
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
                entity_type: "user".to_string(),
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
