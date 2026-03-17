//! SeaORM-backed adapter implementation of `flex::FlexStandaloneService`.

use async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder,
};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use rustok_core::field_schema::FlexError;

use crate::models::{flex_entries, flex_schemas};
use crate::services::flex_standalone_validation_service::FlexStandaloneValidationService;

pub struct FlexStandaloneSeaOrmService {
    db: DatabaseConnection,
}

impl FlexStandaloneSeaOrmService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    fn schema_to_view(model: flex_schemas::Model) -> flex::FlexSchemaView {
        let fields_config = model.parse_field_definitions().unwrap_or_default();

        flex::FlexSchemaView {
            id: model.id,
            slug: model.slug,
            name: model.name,
            description: model.description,
            fields_config,
            settings: model.settings,
            is_active: model.is_active,
            created_at: model.created_at.to_rfc3339(),
            updated_at: model.updated_at.to_rfc3339(),
        }
    }

    fn entry_to_view(model: flex_entries::Model) -> flex::FlexEntryView {
        flex::FlexEntryView {
            id: model.id,
            schema_id: model.schema_id,
            entity_type: model.entity_type,
            entity_id: model.entity_id,
            data: model.data,
            status: model.status,
            created_at: model.created_at.to_rfc3339(),
            updated_at: model.updated_at.to_rfc3339(),
        }
    }

    async fn get_schema_or_not_found(
        &self,
        tenant_id: Uuid,
        schema_id: Uuid,
    ) -> Result<flex_schemas::Model, FlexError> {
        flex_schemas::Entity::find_by_id(schema_id)
            .filter(flex_schemas::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?
            .ok_or(FlexError::NotFound(schema_id))
    }

    async fn normalize_payload(
        &self,
        schema: &flex_schemas::Model,
        data: JsonValue,
    ) -> Result<JsonValue, FlexError> {
        FlexStandaloneValidationService::validate_entry_against_schema(schema, data)
    }
}

#[async_trait]
impl flex::FlexStandaloneService for FlexStandaloneSeaOrmService {
    async fn list_schemas(&self, tenant_id: Uuid) -> Result<Vec<flex::FlexSchemaView>, FlexError> {
        let rows = flex_schemas::Entity::find()
            .filter(flex_schemas::Column::TenantId.eq(tenant_id))
            .order_by_asc(flex_schemas::Column::Slug)
            .all(&self.db)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?;

        Ok(rows.into_iter().map(Self::schema_to_view).collect())
    }

    async fn find_schema(
        &self,
        tenant_id: Uuid,
        schema_id: Uuid,
    ) -> Result<Option<flex::FlexSchemaView>, FlexError> {
        let row = flex_schemas::Entity::find_by_id(schema_id)
            .filter(flex_schemas::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?;

        Ok(row.map(Self::schema_to_view))
    }

    async fn create_schema(
        &self,
        tenant_id: Uuid,
        _actor_id: Option<Uuid>,
        input: flex::CreateFlexSchemaCommand,
    ) -> Result<flex::FlexSchemaView, FlexError> {
        let row = flex_schemas::ActiveModel {
            id: Set(rustok_core::generate_id()),
            tenant_id: Set(tenant_id),
            slug: Set(input.slug),
            name: Set(input.name),
            description: Set(input.description),
            fields_config: Set(serde_json::to_value(input.fields_config).unwrap_or_default()),
            settings: Set(input.settings.unwrap_or_else(|| serde_json::json!({}))),
            is_active: Set(input.is_active.unwrap_or(true)),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(&self.db)
        .await
        .map_err(|e| FlexError::Database(e.to_string()))?;

        Ok(Self::schema_to_view(row))
    }

    async fn update_schema(
        &self,
        tenant_id: Uuid,
        _actor_id: Option<Uuid>,
        schema_id: Uuid,
        input: flex::UpdateFlexSchemaCommand,
    ) -> Result<flex::FlexSchemaView, FlexError> {
        let row = self.get_schema_or_not_found(tenant_id, schema_id).await?;
        let mut model: flex_schemas::ActiveModel = row.into();

        if let Some(name) = input.name {
            model.name = Set(name);
        }
        if let Some(description) = input.description {
            model.description = Set(Some(description));
        }
        if let Some(fields_config) = input.fields_config {
            model.fields_config = Set(serde_json::to_value(fields_config).unwrap_or_default());
        }
        if let Some(settings) = input.settings {
            model.settings = Set(settings);
        }
        if let Some(is_active) = input.is_active {
            model.is_active = Set(is_active);
        }

        let updated = model
            .update(&self.db)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?;

        Ok(Self::schema_to_view(updated))
    }

    async fn delete_schema(
        &self,
        tenant_id: Uuid,
        _actor_id: Option<Uuid>,
        schema_id: Uuid,
    ) -> Result<(), FlexError> {
        let row = self.get_schema_or_not_found(tenant_id, schema_id).await?;

        flex_schemas::Entity::delete_by_id(row.id)
            .exec(&self.db)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?;

        Ok(())
    }

    async fn list_entries(
        &self,
        tenant_id: Uuid,
        schema_id: Uuid,
    ) -> Result<Vec<flex::FlexEntryView>, FlexError> {
        self.get_schema_or_not_found(tenant_id, schema_id).await?;

        let rows = flex_entries::Entity::find()
            .filter(flex_entries::Column::TenantId.eq(tenant_id))
            .filter(flex_entries::Column::SchemaId.eq(schema_id))
            .order_by_asc(flex_entries::Column::CreatedAt)
            .all(&self.db)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?;

        Ok(rows.into_iter().map(Self::entry_to_view).collect())
    }

    async fn find_entry(
        &self,
        tenant_id: Uuid,
        schema_id: Uuid,
        entry_id: Uuid,
    ) -> Result<Option<flex::FlexEntryView>, FlexError> {
        let row = flex_entries::Entity::find_by_id(entry_id)
            .filter(flex_entries::Column::TenantId.eq(tenant_id))
            .filter(flex_entries::Column::SchemaId.eq(schema_id))
            .one(&self.db)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?;

        Ok(row.map(Self::entry_to_view))
    }

    async fn create_entry(
        &self,
        tenant_id: Uuid,
        _actor_id: Option<Uuid>,
        input: flex::CreateFlexEntryCommand,
    ) -> Result<flex::FlexEntryView, FlexError> {
        let schema = self
            .get_schema_or_not_found(tenant_id, input.schema_id)
            .await?;
        let normalized = self.normalize_payload(&schema, input.data).await?;

        let row = flex_entries::ActiveModel {
            id: Set(rustok_core::generate_id()),
            tenant_id: Set(tenant_id),
            schema_id: Set(input.schema_id),
            entity_type: Set(input.entity_type),
            entity_id: Set(input.entity_id),
            data: Set(normalized),
            status: Set(input.status.unwrap_or_else(|| "draft".to_string())),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(&self.db)
        .await
        .map_err(|e| FlexError::Database(e.to_string()))?;

        Ok(Self::entry_to_view(row))
    }

    async fn update_entry(
        &self,
        tenant_id: Uuid,
        _actor_id: Option<Uuid>,
        schema_id: Uuid,
        entry_id: Uuid,
        input: flex::UpdateFlexEntryCommand,
    ) -> Result<flex::FlexEntryView, FlexError> {
        let schema = self.get_schema_or_not_found(tenant_id, schema_id).await?;
        let row = flex_entries::Entity::find_by_id(entry_id)
            .filter(flex_entries::Column::TenantId.eq(tenant_id))
            .filter(flex_entries::Column::SchemaId.eq(schema_id))
            .one(&self.db)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?
            .ok_or(FlexError::NotFound(entry_id))?;

        let mut model: flex_entries::ActiveModel = row.into();

        if let Some(data) = input.data {
            let normalized = self.normalize_payload(&schema, data).await?;
            model.data = Set(normalized);
        }

        if let Some(status) = input.status {
            model.status = Set(status);
        }

        let updated = model
            .update(&self.db)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?;

        Ok(Self::entry_to_view(updated))
    }

    async fn delete_entry(
        &self,
        tenant_id: Uuid,
        _actor_id: Option<Uuid>,
        schema_id: Uuid,
        entry_id: Uuid,
    ) -> Result<(), FlexError> {
        let row = flex_entries::Entity::find_by_id(entry_id)
            .filter(flex_entries::Column::TenantId.eq(tenant_id))
            .filter(flex_entries::Column::SchemaId.eq(schema_id))
            .one(&self.db)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?
            .ok_or(FlexError::NotFound(entry_id))?;

        flex_entries::Entity::delete_by_id(row.id)
            .exec(&self.db)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?;

        Ok(())
    }
}
