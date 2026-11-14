//! Service for managing `order_field_definitions` — Flex Phase 4.
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
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, TransactionTrait,
};
use uuid::Uuid;

use rustok_core::field_schema::{CustomFieldsSchema, FlexError};
use rustok_events::EventEnvelope;

use crate::models::order_field_definitions::{
    ActiveModel, Column, CreateFieldDefinitionInput, Entity, MAX_FIELDS_PER_TENANT, Model,
    UpdateFieldDefinitionInput,
};

/// Service for order custom field definitions.
pub struct OrderFieldService;

impl OrderFieldService {
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
            .map(|row| row.into_field_definition())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| {
                FlexError::Database(format!("invalid persisted order field definition: {error}"))
            })?;

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
        // The field-key format rule belongs to Flex; delegating it here rejects a
        // malformed key before this adapter opens a transaction or reads state.
        flex::validate_field_definition_key(&input.field_key)?;
        let txn = db
            .begin()
            .await
            .map_err(|error| FlexError::Database(error.to_string()))?;
        // Serialize the count-and-insert guard on databases with row-level locks.
        // SQLite has no SELECT FOR UPDATE; its write transaction remains the fallback.
        let tenant_query = crate::models::tenants::Entity::find_by_id(tenant_id);
        match db.get_database_backend() {
            sea_orm::DatabaseBackend::Postgres | sea_orm::DatabaseBackend::MySql => {
                tenant_query.lock_exclusive().one(&txn).await
            }
            _ => tenant_query.one(&txn).await,
        }
        .map_err(|error| FlexError::Database(error.to_string()))?
        .ok_or(FlexError::NotFound(tenant_id))?;
        let existing = Entity::find()
            .filter(Column::TenantId.eq(tenant_id))
            .filter(Column::FieldKey.eq(&input.field_key))
            .one(&txn)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?;

        let count = Entity::find()
            .filter(Column::TenantId.eq(tenant_id))
            .filter(Column::IsActive.eq(true))
            .count(&txn)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?;

        flex::validate_field_definition_create(
            "order",
            &input.field_key,
            existing.is_some(),
            count,
            MAX_FIELDS_PER_TENANT,
        )?;

        let next_position = flex::field_definition_position_or_next(input.position, count);
        let field_type_str = flex::field_definition_type_name(input.field_type);

        let model = ActiveModel {
            id: Set(rustok_core::generate_id()),
            tenant_id: Set(tenant_id),
            field_key: Set(input.field_key.clone()),
            field_type: Set(field_type_str.clone()),
            label: Set(flex::field_definition_label_json(&input.label)),
            description: Set(input
                .description
                .as_ref()
                .map(flex::field_definition_description_json)),
            is_localized: Set(input.is_localized),
            is_required: Set(input.is_required),
            default_value: Set(input.default_value.clone()),
            validation: Set(input
                .validation
                .as_ref()
                .map(flex::field_definition_validation_json)),
            position: Set(next_position),
            is_active: Set(true),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(&txn)
        .await
        .map_err(|e| FlexError::Database(e.to_string()))?;

        let event = flex::field_definition_created_event(
            tenant_id,
            actor_id,
            "order",
            input.field_key,
            field_type_str,
        );

        crate::services::field_definition_event::persist(&txn, event.clone()).await?;
        txn.commit()
            .await
            .map_err(|error| FlexError::Database(error.to_string()))?;
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
        let txn = db
            .begin()
            .await
            .map_err(|error| FlexError::Database(error.to_string()))?;
        let query = Entity::find_by_id(id).filter(Column::TenantId.eq(tenant_id));
        let row = match db.get_database_backend() {
            sea_orm::DatabaseBackend::Postgres | sea_orm::DatabaseBackend::MySql => {
                query.lock_exclusive().one(&txn).await
            }
            _ => query.one(&txn).await,
        }
        .map_err(|error| FlexError::Database(error.to_string()))?
        .ok_or(FlexError::NotFound(id))?;

        let field_key = row.field_key.clone();
        let mut model: ActiveModel = row.into();

        if let Some(label) = input.label {
            model.label = Set(flex::field_definition_label_json(&label));
        }
        if let Some(desc) = input.description {
            model.description = Set(Some(flex::field_definition_description_json(&desc)));
        }
        if let Some(is_localized) = input.is_localized {
            model.is_localized = Set(is_localized);
        }
        if let Some(req) = input.is_required {
            model.is_required = Set(req);
        }
        if let Some(dv) = input.default_value {
            model.default_value = Set(Some(dv));
        }
        if let Some(val) = input.validation {
            model.validation = Set(Some(flex::field_definition_validation_json(&val)));
        }
        if let Some(pos) = input.position {
            model.position = Set(pos);
        }
        if let Some(active) = input.is_active {
            model.is_active = Set(active);
        }

        let updated = model
            .update(&txn)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?;

        let event =
            flex::field_definition_updated_event(tenant_id, actor_id, "order", field_key.clone());

        crate::services::field_definition_event::persist(&txn, event.clone()).await?;
        txn.commit()
            .await
            .map_err(|error| FlexError::Database(error.to_string()))?;
        Ok((updated, event))
    }

    /// Soft-delete a field definition (sets `is_active = false`).
    ///
    /// Data already stored in `orders.metadata` is preserved.
    pub async fn deactivate(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        id: Uuid,
    ) -> Result<EventEnvelope, FlexError> {
        let txn = db
            .begin()
            .await
            .map_err(|error| FlexError::Database(error.to_string()))?;
        let query = Entity::find_by_id(id).filter(Column::TenantId.eq(tenant_id));
        let row = match db.get_database_backend() {
            sea_orm::DatabaseBackend::Postgres | sea_orm::DatabaseBackend::MySql => {
                query.lock_exclusive().one(&txn).await
            }
            _ => query.one(&txn).await,
        }
        .map_err(|error| FlexError::Database(error.to_string()))?
        .ok_or(FlexError::NotFound(id))?;

        let field_key = row.field_key.clone();
        let mut model: ActiveModel = row.into();
        model.is_active = Set(false);
        model
            .update(&txn)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?;

        let event = flex::field_definition_deleted_event(tenant_id, actor_id, "order", field_key);

        crate::services::field_definition_event::persist(&txn, event.clone()).await?;
        txn.commit()
            .await
            .map_err(|error| FlexError::Database(error.to_string()))?;
        Ok(event)
    }

    /// Reorder definitions by setting their `position` according to the supplied
    /// `ids` slice (first id → position 0, second → 1, …).
    pub async fn reorder(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        ids: &[Uuid],
    ) -> Result<Vec<Model>, FlexError> {
        flex::validate_field_definition_reorder_request(ids)?;

        let txn = db
            .begin()
            .await
            .map_err(|error| FlexError::Database(error.to_string()))?;
        let query = Entity::find()
            .filter(Column::TenantId.eq(tenant_id))
            .order_by_asc(Column::Id);
        let rows = match db.get_database_backend() {
            sea_orm::DatabaseBackend::Postgres | sea_orm::DatabaseBackend::MySql => {
                query.lock_exclusive().all(&txn).await
            }
            _ => query.all(&txn).await,
        }
        .map_err(|error| FlexError::Database(error.to_string()))?;
        let existing_ids = rows
            .iter()
            .map(|row| row.id)
            .collect::<std::collections::HashSet<_>>();
        flex::validate_field_definition_reorder_coverage(ids, &existing_ids)?;

        let mut rows_by_id = rows
            .into_iter()
            .map(|row| (row.id, row))
            .collect::<std::collections::HashMap<_, _>>();
        let mut result = Vec::with_capacity(ids.len());
        for (position, id) in ids.iter().copied().enumerate() {
            let Some(row) = rows_by_id.remove(&id) else {
                // Coverage validation above proved the id is in the locked set.
                return Err(FlexError::NotFound(id));
            };
            let mut model: ActiveModel = row.into();
            model.position = Set(position as i32);
            let updated = model
                .update(&txn)
                .await
                .map_err(|error| FlexError::Database(error.to_string()))?;
            result.push(updated);
        }

        txn.commit()
            .await
            .map_err(|error| FlexError::Database(error.to_string()))?;
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
    use super::OrderFieldService;
    use crate::models::order_field_definitions::{MAX_FIELDS_PER_TENANT, Model};
    use chrono::Utc;
    use rustok_core::field_schema::{FieldType, FlexError};
    use rustok_events::DomainEvent;
    use rustok_migrations::SqliteTestMigrator as Migrator;
    use rustok_test_utils::db::setup_test_db_with_migrations;
    use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
    use serde_json::json;
    use std::collections::HashMap;
    use uuid::Uuid;

    fn create_input(
        field_key: &str,
    ) -> crate::models::order_field_definitions::CreateFieldDefinitionInput {
        crate::models::order_field_definitions::CreateFieldDefinitionInput {
            field_key: field_key.to_string(),
            field_type: FieldType::Text,
            label: HashMap::from([("en".to_string(), "Label".to_string())]),
            description: None,
            is_localized: false,
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
            is_localized: false,
            is_required: false,
            default_value: None,
            validation: None,
            position: 0,
            is_active: true,
            created_at: now,
            updated_at: now,
        }
    }

    async fn test_db() -> DatabaseConnection {
        setup_test_db_with_migrations::<Migrator>().await
    }

    async fn insert_tenant(db: &DatabaseConnection, tenant_id: Uuid) {
        let mut tenant = crate::models::tenants::ActiveModel::new(
            "Field definitions test tenant",
            &format!("field-definitions-{tenant_id}"),
        );
        tenant.id = Set(tenant_id);
        tenant
            .insert(db)
            .await
            .expect("insert field definition test tenant");
    }

    async fn insert_row(db: &DatabaseConnection, model: Model) {
        crate::models::order_field_definitions::ActiveModel {
            id: Set(model.id),
            tenant_id: Set(model.tenant_id),
            field_key: Set(model.field_key),
            field_type: Set(model.field_type),
            label: Set(model.label),
            description: Set(model.description),
            is_localized: Set(model.is_localized),
            is_required: Set(model.is_required),
            default_value: Set(model.default_value),
            validation: Set(model.validation),
            position: Set(model.position),
            is_active: Set(model.is_active),
            created_at: Set(model.created_at),
            updated_at: Set(model.updated_at),
        }
        .insert(db)
        .await
        .expect("failed to insert order field definition");
    }

    #[tokio::test]
    async fn reorder_requires_complete_unique_list_and_writes_the_requested_order() {
        let tenant_id = Uuid::new_v4();
        let db = test_db().await;
        let mut first = row(tenant_id, "first");
        first.position = 10;
        let mut second = row(tenant_id, "second");
        second.position = 20;
        let mut third = row(tenant_id, "third");
        third.position = 30;
        let ids = [first.id, second.id, third.id];
        insert_row(&db, first).await;
        insert_row(&db, second).await;
        insert_row(&db, third).await;

        let partial = OrderFieldService::reorder(&db, tenant_id, &[ids[0]])
            .await
            .expect_err("partial reorder must not leave position collisions");
        assert!(
            matches!(partial, FlexError::InvalidFieldKey(_)),
            "partial reorder must be reported as a malformed request: {partial:?}"
        );

        let duplicate = OrderFieldService::reorder(&db, tenant_id, &[ids[0], ids[0], ids[2]])
            .await
            .expect_err("duplicate reorder ids must be rejected");
        assert!(
            matches!(duplicate, FlexError::InvalidFieldKey(_)),
            "duplicate reorder ids must be reported as a malformed request: {duplicate:?}"
        );

        let unknown = OrderFieldService::reorder(&db, tenant_id, &[ids[0], ids[1], Uuid::new_v4()])
            .await
            .expect_err("a reorder naming an unknown definition must be rejected");
        assert!(
            matches!(unknown, FlexError::NotFound(_)),
            "an unknown identifier is a missing definition, not a malformed request: {unknown:?}"
        );

        let unchanged = OrderFieldService::list_all(&db, tenant_id)
            .await
            .expect("load definitions after rejected requests");
        assert_eq!(
            unchanged
                .iter()
                .map(|definition| (definition.id, definition.position))
                .collect::<std::collections::HashMap<_, _>>(),
            std::collections::HashMap::from([(ids[0], 10), (ids[1], 20), (ids[2], 30)])
        );

        let reordered = OrderFieldService::reorder(&db, tenant_id, &[ids[2], ids[0], ids[1]])
            .await
            .expect("reorder every field");
        assert_eq!(
            reordered
                .iter()
                .map(|definition| (definition.id, definition.position))
                .collect::<Vec<_>>(),
            vec![(ids[2], 0), (ids[0], 1), (ids[1], 2)]
        );
    }

    #[tokio::test]
    async fn create_rejects_invalid_field_key_without_hitting_database() {
        let db = test_db().await;
        let tenant_id = Uuid::new_v4();

        // No tenant row is inserted: reaching the database lookup would surface
        // `NotFound(tenant_id)` instead of the owner-owned key validation error.
        let too_long = "a".repeat(129);
        for invalid_key in ["", "Invalid", "1leading", "has-dash", too_long.as_str()] {
            let err = OrderFieldService::create(
                &db,
                tenant_id,
                Some(Uuid::new_v4()),
                create_input(invalid_key),
            )
            .await
            .expect_err("invalid field key should fail before db access");

            match err {
                FlexError::InvalidFieldKey(key) => assert_eq!(key, invalid_key),
                other => panic!("unexpected error for {invalid_key:?}: {other:?}"),
            }
        }
    }

    #[tokio::test]
    async fn create_rejects_duplicate_field_key() {
        let tenant_id = Uuid::new_v4();
        let db = test_db().await;
        insert_tenant(&db, tenant_id).await;
        insert_row(&db, row(tenant_id, "phone")).await;

        let err =
            OrderFieldService::create(&db, tenant_id, Some(Uuid::new_v4()), create_input("phone"))
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
        let db = test_db().await;
        insert_tenant(&db, tenant_id).await;
        for idx in 0..MAX_FIELDS_PER_TENANT {
            insert_row(&db, row(tenant_id, &format!("field_{idx}"))).await;
        }

        let err =
            OrderFieldService::create(&db, tenant_id, Some(Uuid::new_v4()), create_input("phone"))
                .await
                .expect_err("max fields guardrail should fail");

        match err {
            FlexError::TooManyFields { entity_type, max } => {
                assert_eq!(entity_type, "order");
                assert_eq!(max, MAX_FIELDS_PER_TENANT);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[tokio::test]
    async fn create_emits_field_definition_created_event() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let db = test_db().await;
        insert_tenant(&db, tenant_id).await;

        let (model, envelope) =
            OrderFieldService::create(&db, tenant_id, Some(actor_id), create_input("phone"))
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
                assert_eq!(entity_type, "order");
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
        let db = test_db().await;
        insert_row(&db, existing).await;

        let input = crate::models::order_field_definitions::UpdateFieldDefinitionInput {
            is_localized: Some(true),
            is_required: Some(true),
            ..Default::default()
        };

        let (model, envelope) =
            OrderFieldService::update(&db, tenant_id, Some(actor_id), id, input)
                .await
                .expect("update should succeed");

        assert!(model.is_required);
        assert!(model.is_localized);
        assert_eq!(envelope.tenant_id, tenant_id);
        assert_eq!(envelope.actor_id, Some(actor_id));
        match envelope.event {
            DomainEvent::FieldDefinitionUpdated {
                tenant_id: event_tenant,
                entity_type,
                field_key,
            } => {
                assert_eq!(event_tenant, tenant_id);
                assert_eq!(entity_type, "order");
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
        let db = test_db().await;
        insert_row(&db, row(tenant_id, "phone")).await;
        insert_row(&db, invalid).await;

        let schema = OrderFieldService::get_schema(&db, tenant_id)
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
        let db = test_db().await;

        let err = OrderFieldService::update(
            &db,
            tenant_id,
            Some(actor_id),
            id,
            crate::models::order_field_definitions::UpdateFieldDefinitionInput::default(),
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
        let db = test_db().await;

        let err = OrderFieldService::deactivate(&db, tenant_id, Some(actor_id), id)
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
        let db = test_db().await;
        insert_row(&db, model).await;

        let envelope = OrderFieldService::deactivate(&db, tenant_id, Some(actor_id), id)
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
                assert_eq!(entity_type, "order");
                assert_eq!(field_key, "phone");
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }
}
