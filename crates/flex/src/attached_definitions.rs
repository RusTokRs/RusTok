//! Generic attached field-definition persistence for reusable Flex donors.
//!
//! Historical donors keep their existing definition tables while they migrate.
//! New donors can use this Flex-owned store instead of introducing another
//! `<module>_field_definitions` service stack.

use std::collections::HashSet;

use async_trait::async_trait;
use chrono::Utc;
use sea_orm::entity::prelude::*;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, TransactionTrait,
};
use uuid::Uuid;

use rustok_core::field_schema::{CustomFieldsSchema, FlexError};
use rustok_events::EventEnvelope;

use crate::registry::{
    CreateFieldDefinitionCommand, FieldDefinitionService, FieldDefinitionSource,
    FieldDefinitionView, FieldDefinitionViewSource, UpdateFieldDefinitionCommand,
    field_definition_created_event, field_definition_deleted_event,
    field_definition_description_json, field_definition_from_source,
    field_definition_label_json, field_definition_position_or_next,
    field_definition_type_name, field_definition_updated_event, field_definition_validation_json,
    is_valid_flex_entity_type, validate_field_definition_create,
};

pub const GENERIC_ATTACHED_FIELD_DEFINITIONS_TABLE: &str = "flex_attached_field_definitions";
pub const MAX_GENERIC_ATTACHED_FIELDS_PER_TENANT: usize = 50;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "flex_attached_field_definitions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub entity_type: String,
    pub field_key: String,
    pub field_type: String,
    pub label: Json,
    pub description: Option<Json>,
    pub is_localized: bool,
    pub is_required: bool,
    pub default_value: Option<Json>,
    pub validation: Option<Json>,
    pub position: i32,
    pub is_active: bool,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl FieldDefinitionSource for Model {
    fn field_key(&self) -> &str {
        &self.field_key
    }

    fn field_type(&self) -> &str {
        &self.field_type
    }

    fn label(&self) -> &serde_json::Value {
        &self.label
    }

    fn description(&self) -> Option<&serde_json::Value> {
        self.description.as_ref()
    }

    fn is_localized(&self) -> bool {
        self.is_localized
    }

    fn is_required(&self) -> bool {
        self.is_required
    }

    fn default_value(&self) -> Option<&serde_json::Value> {
        self.default_value.as_ref()
    }

    fn validation(&self) -> Option<&serde_json::Value> {
        self.validation.as_ref()
    }

    fn position(&self) -> i32 {
        self.position
    }

    fn is_active(&self) -> bool {
        self.is_active
    }
}

impl FieldDefinitionViewSource for Model {
    fn id(&self) -> Uuid {
        self.id
    }

    fn field_key(&self) -> &str {
        &self.field_key
    }

    fn field_type(&self) -> &str {
        &self.field_type
    }

    fn label(&self) -> &serde_json::Value {
        &self.label
    }

    fn description(&self) -> Option<&serde_json::Value> {
        self.description.as_ref()
    }

    fn is_localized(&self) -> bool {
        self.is_localized
    }

    fn is_required(&self) -> bool {
        self.is_required
    }

    fn default_value(&self) -> Option<&serde_json::Value> {
        self.default_value.as_ref()
    }

    fn validation(&self) -> Option<&serde_json::Value> {
        self.validation.as_ref()
    }

    fn position(&self) -> i32 {
        self.position
    }

    fn is_active(&self) -> bool {
        self.is_active
    }

    fn created_at(&self) -> String {
        self.created_at.to_rfc3339()
    }

    fn updated_at(&self) -> String {
        self.updated_at.to_rfc3339()
    }
}

/// Reusable registry adapter for a namespaced attached donor.
#[derive(Debug, Clone, Copy)]
pub struct GenericAttachedFieldDefinitionService {
    entity_type: &'static str,
}

impl GenericAttachedFieldDefinitionService {
    pub fn new(entity_type: &'static str) -> Self {
        assert!(
            is_valid_flex_entity_type(entity_type),
            "static Flex entity type must satisfy the namespaced entity-type contract"
        );
        Self { entity_type }
    }

    pub fn entity_type_name(&self) -> &'static str {
        self.entity_type
    }

    pub async fn get_schema(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> Result<CustomFieldsSchema, FlexError> {
        let rows = Entity::find()
            .filter(Column::TenantId.eq(tenant_id))
            .filter(Column::EntityType.eq(self.entity_type))
            .filter(Column::IsActive.eq(true))
            .order_by_asc(Column::Position)
            .all(db)
            .await
            .map_err(database_error)?;
        let definitions = rows
            .iter()
            .filter_map(field_definition_from_source)
            .collect();
        Ok(CustomFieldsSchema::new(definitions))
    }

    async fn find_scoped(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<Model>, FlexError> {
        Entity::find_by_id(id)
            .filter(Column::TenantId.eq(tenant_id))
            .filter(Column::EntityType.eq(self.entity_type))
            .one(db)
            .await
            .map_err(database_error)
    }
}

#[async_trait]
impl FieldDefinitionService for GenericAttachedFieldDefinitionService {
    fn entity_type(&self) -> &'static str {
        self.entity_type
    }

    async fn list_all(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> Result<Vec<FieldDefinitionView>, FlexError> {
        Entity::find()
            .filter(Column::TenantId.eq(tenant_id))
            .filter(Column::EntityType.eq(self.entity_type))
            .order_by_asc(Column::Position)
            .order_by_asc(Column::FieldKey)
            .all(db)
            .await
            .map_err(database_error)
            .map(|rows| {
                rows.iter()
                    .map(FieldDefinitionView::from_source)
                    .collect()
            })
    }

    async fn find_by_id(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<FieldDefinitionView>, FlexError> {
        self.find_scoped(db, tenant_id, id)
            .await
            .map(|row| row.as_ref().map(FieldDefinitionView::from_source))
    }

    async fn reorder(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
        ids: &[Uuid],
    ) -> Result<Vec<FieldDefinitionView>, FlexError> {
        let unique = ids.iter().copied().collect::<HashSet<_>>();
        if unique.len() != ids.len() {
            return Err(FlexError::Database(
                "attached field-definition reorder contains duplicate ids".to_string(),
            ));
        }

        let txn = db.begin().await.map_err(database_error)?;
        let mut rows = Vec::with_capacity(ids.len());
        for (position, id) in ids.iter().copied().enumerate() {
            let row = Entity::find_by_id(id)
                .filter(Column::TenantId.eq(tenant_id))
                .filter(Column::EntityType.eq(self.entity_type))
                .one(&txn)
                .await
                .map_err(database_error)?
                .ok_or(FlexError::NotFound(id))?;
            let mut active: ActiveModel = row.into();
            active.position = Set(position as i32);
            active.updated_at = Set(Utc::now().fixed_offset());
            let row = active.update(&txn).await.map_err(database_error)?;
            rows.push(FieldDefinitionView::from_source(&row));
        }
        txn.commit().await.map_err(database_error)?;
        Ok(rows)
    }

    async fn create(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        input: CreateFieldDefinitionCommand,
    ) -> Result<(FieldDefinitionView, EventEnvelope), FlexError> {
        let existing = Entity::find()
            .filter(Column::TenantId.eq(tenant_id))
            .filter(Column::EntityType.eq(self.entity_type))
            .filter(Column::FieldKey.eq(&input.field_key))
            .one(db)
            .await
            .map_err(database_error)?;
        let active_count = Entity::find()
            .filter(Column::TenantId.eq(tenant_id))
            .filter(Column::EntityType.eq(self.entity_type))
            .filter(Column::IsActive.eq(true))
            .count(db)
            .await
            .map_err(database_error)?;

        validate_field_definition_create(
            self.entity_type,
            &input.field_key,
            existing.is_some(),
            active_count,
            MAX_GENERIC_ATTACHED_FIELDS_PER_TENANT,
        )?;

        let field_type = field_definition_type_name(input.field_type);
        let now = Utc::now().fixed_offset();
        let field_key = input.field_key.clone();
        let insert = ActiveModel {
            id: Set(rustok_core::generate_id()),
            tenant_id: Set(tenant_id),
            entity_type: Set(self.entity_type.to_string()),
            field_key: Set(field_key.clone()),
            field_type: Set(field_type.clone()),
            label: Set(field_definition_label_json(&input.label)),
            description: Set(input
                .description
                .as_ref()
                .map(field_definition_description_json)),
            is_localized: Set(input.is_localized),
            is_required: Set(input.is_required),
            default_value: Set(input.default_value),
            validation: Set(input
                .validation
                .as_ref()
                .map(field_definition_validation_json)),
            position: Set(field_definition_position_or_next(input.position, active_count)),
            is_active: Set(true),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(db)
        .await;

        let row = match insert {
            Ok(row) => row,
            Err(error) if is_unique_constraint(&error) => {
                return Err(FlexError::DuplicateFieldKey(field_key));
            }
            Err(error) => return Err(database_error(error)),
        };
        let event = field_definition_created_event(
            tenant_id,
            actor_id,
            self.entity_type,
            field_key,
            field_type,
        );
        Ok((FieldDefinitionView::from_source(&row), event))
    }

    async fn update(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        id: Uuid,
        input: UpdateFieldDefinitionCommand,
    ) -> Result<(FieldDefinitionView, EventEnvelope), FlexError> {
        let row = self
            .find_scoped(db, tenant_id, id)
            .await?
            .ok_or(FlexError::NotFound(id))?;
        let field_key = row.field_key.clone();
        let mut active: ActiveModel = row.into();
        if let Some(label) = input.label {
            active.label = Set(field_definition_label_json(&label));
        }
        if let Some(description) = input.description {
            active.description = Set(Some(field_definition_description_json(&description)));
        }
        if let Some(is_localized) = input.is_localized {
            active.is_localized = Set(is_localized);
        }
        if let Some(is_required) = input.is_required {
            active.is_required = Set(is_required);
        }
        if let Some(default_value) = input.default_value {
            active.default_value = Set(Some(default_value));
        }
        if let Some(validation) = input.validation {
            active.validation = Set(Some(field_definition_validation_json(&validation)));
        }
        if let Some(position) = input.position {
            active.position = Set(position);
        }
        if let Some(is_active) = input.is_active {
            active.is_active = Set(is_active);
        }
        active.updated_at = Set(Utc::now().fixed_offset());
        let row = active.update(db).await.map_err(database_error)?;
        let event = field_definition_updated_event(
            tenant_id,
            actor_id,
            self.entity_type,
            field_key,
        );
        Ok((FieldDefinitionView::from_source(&row), event))
    }

    async fn deactivate(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        id: Uuid,
    ) -> Result<EventEnvelope, FlexError> {
        let row = self
            .find_scoped(db, tenant_id, id)
            .await?
            .ok_or(FlexError::NotFound(id))?;
        let field_key = row.field_key.clone();
        let mut active: ActiveModel = row.into();
        active.is_active = Set(false);
        active.updated_at = Set(Utc::now().fixed_offset());
        active.update(db).await.map_err(database_error)?;
        Ok(field_definition_deleted_event(
            tenant_id,
            actor_id,
            self.entity_type,
            field_key,
        ))
    }
}

fn database_error(error: sea_orm::DbErr) -> FlexError {
    FlexError::Database(error.to_string())
}

fn is_unique_constraint(error: &sea_orm::DbErr) -> bool {
    matches!(
        error.sql_err(),
        Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
    )
}
