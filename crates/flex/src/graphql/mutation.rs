use async_graphql::{Context, Object, Result};
use rustok_api::Permission;
use rustok_core::EventBus;
use rustok_core::field_schema::FieldType;
use rustok_events::EventEnvelope;
use uuid::Uuid;

use super::{
    CreateFieldDefinitionInput, CreateFlexEntryInput, CreateFlexSchemaInput,
    DeleteFieldDefinitionPayload, DeleteFlexPayload, FieldDefinitionObject, FlexEntryObject,
    FlexSchemaObject, UpdateFieldDefinitionInput, UpdateFlexEntryInput, UpdateFlexSchemaInput,
    bad_user_input, map_flex_error, require_access, resolve_entity_type, runtime::runtime,
};
use crate::{
    CreateFieldDefinitionCommand, CreateFlexEntryCommand, CreateFlexSchemaCommand,
    UpdateFieldDefinitionCommand, UpdateFlexEntryCommand, UpdateFlexSchemaCommand,
};

#[derive(Default)]
pub struct FlexMutation;

#[Object]
impl FlexMutation {
    /// Create a new attached custom field definition.
    async fn create_field_definition(
        &self,
        ctx: &Context<'_>,
        input: CreateFieldDefinitionInput,
    ) -> Result<FieldDefinitionObject> {
        let (tenant, auth) = require_access(ctx, Permission::FLEX_SCHEMAS_CREATE)?;
        let runtime = runtime(ctx)?;

        let field_type: FieldType =
            serde_json::from_value(serde_json::Value::String(input.field_type.clone()))
                .map_err(|_| bad_user_input("Unknown field_type value"))?;

        let label = serde_json::from_value(input.label)
            .map_err(|_| bad_user_input("label must be a JSON object {\"en\": \"...\"}"))?;

        let description = input
            .description
            .map(|value| {
                serde_json::from_value(value)
                    .map_err(|_| bad_user_input("description must be a JSON object"))
            })
            .transpose()?;

        let validation = input
            .validation
            .map(|value| {
                serde_json::from_value(value)
                    .map_err(|_| bad_user_input("validation must be a valid ValidationRule JSON"))
            })
            .transpose()?;

        let entity_type = resolve_entity_type(input.entity_type)?;
        let (view, event) = crate::create_field_definition(
            runtime.field_registry(),
            runtime.db(),
            tenant.id,
            &entity_type,
            Some(auth.user_id),
            CreateFieldDefinitionCommand {
                field_key: input.field_key,
                field_type,
                label,
                description,
                is_localized: input.is_localized,
                is_required: input.is_required,
                default_value: input.default_value,
                validation,
                position: input.position,
            },
        )
        .await
        .map_err(map_flex_error)?;

        publish_event(ctx, event);
        invalidate_field_def_cache(runtime, tenant.id, &entity_type).await;

        Ok(FieldDefinitionObject::from(view))
    }

    /// Update an attached custom field definition.
    async fn update_field_definition(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        input: UpdateFieldDefinitionInput,
    ) -> Result<FieldDefinitionObject> {
        let (tenant, auth) = require_access(ctx, Permission::FLEX_SCHEMAS_UPDATE)?;
        let runtime = runtime(ctx)?;

        let label = input
            .label
            .map(|value| {
                serde_json::from_value(value)
                    .map_err(|_| bad_user_input("label must be a JSON object"))
            })
            .transpose()?;

        let description = input
            .description
            .map(|value| {
                serde_json::from_value(value)
                    .map_err(|_| bad_user_input("description must be a JSON object"))
            })
            .transpose()?;

        let validation = input
            .validation
            .map(|value| {
                serde_json::from_value(value)
                    .map_err(|_| bad_user_input("validation must be a valid ValidationRule JSON"))
            })
            .transpose()?;

        let entity_type = resolve_entity_type(input.entity_type)?;
        let (view, event) = crate::update_field_definition(
            runtime.field_registry(),
            runtime.db(),
            tenant.id,
            &entity_type,
            Some(auth.user_id),
            id,
            UpdateFieldDefinitionCommand {
                label,
                description,
                is_localized: input.is_localized,
                is_required: input.is_required,
                default_value: input.default_value,
                validation,
                position: input.position,
                is_active: input.is_active,
            },
        )
        .await
        .map_err(map_flex_error)?;

        publish_event(ctx, event);
        invalidate_field_def_cache(runtime, tenant.id, &entity_type).await;

        Ok(FieldDefinitionObject::from(view))
    }

    /// Soft-delete an attached field definition.
    async fn delete_field_definition(
        &self,
        ctx: &Context<'_>,
        entity_type: Option<String>,
        id: Uuid,
    ) -> Result<DeleteFieldDefinitionPayload> {
        let (tenant, auth) = require_access(ctx, Permission::FLEX_SCHEMAS_DELETE)?;
        let runtime = runtime(ctx)?;
        let entity_type = resolve_entity_type(entity_type)?;

        let event = crate::deactivate_field_definition(
            runtime.field_registry(),
            runtime.db(),
            tenant.id,
            &entity_type,
            Some(auth.user_id),
            id,
        )
        .await
        .map_err(map_flex_error)?;

        publish_event(ctx, event);
        invalidate_field_def_cache(runtime, tenant.id, &entity_type).await;

        Ok(DeleteFieldDefinitionPayload { success: true })
    }

    /// Reorder attached field definitions by supplying an ordered list of ids.
    async fn reorder_field_definitions(
        &self,
        ctx: &Context<'_>,
        entity_type: Option<String>,
        ids: Vec<Uuid>,
    ) -> Result<Vec<FieldDefinitionObject>> {
        let (tenant, _) = require_access(ctx, Permission::FLEX_SCHEMAS_UPDATE)?;
        let runtime = runtime(ctx)?;
        let entity_type = resolve_entity_type(entity_type)?;

        let rows = crate::reorder_field_definitions(
            runtime.field_registry(),
            runtime.db(),
            tenant.id,
            &entity_type,
            &ids,
        )
        .await
        .map_err(map_flex_error)?;

        invalidate_field_def_cache(runtime, tenant.id, &entity_type).await;

        Ok(rows.into_iter().map(FieldDefinitionObject::from).collect())
    }

    /// Create a standalone Flex schema.
    async fn create_flex_schema(
        &self,
        ctx: &Context<'_>,
        input: CreateFlexSchemaInput,
    ) -> Result<FlexSchemaObject> {
        let (tenant, auth) = require_access(ctx, Permission::FLEX_SCHEMAS_CREATE)?;
        let service = runtime(ctx)?.standalone_service();

        let (view, event) = crate::create_schema_with_event(
            service.as_ref(),
            tenant.id,
            Some(auth.user_id),
            CreateFlexSchemaCommand {
                slug: input.slug,
                name: input.name,
                description: input.description,
                fields_config: parse_fields_config(input.fields_config)?,
                settings: input.settings,
                is_active: input.is_active,
            },
        )
        .await
        .map_err(map_flex_error)?;

        publish_event(ctx, event);
        Ok(FlexSchemaObject::from(view))
    }

    /// Update a standalone Flex schema.
    async fn update_flex_schema(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        input: UpdateFlexSchemaInput,
    ) -> Result<FlexSchemaObject> {
        let (tenant, auth) = require_access(ctx, Permission::FLEX_SCHEMAS_UPDATE)?;
        let service = runtime(ctx)?.standalone_service();

        let (view, event) = crate::update_schema_with_event(
            service.as_ref(),
            tenant.id,
            Some(auth.user_id),
            id,
            UpdateFlexSchemaCommand {
                name: input.name,
                description: input.description,
                fields_config: input.fields_config.map(parse_fields_config).transpose()?,
                settings: input.settings,
                is_active: input.is_active,
            },
        )
        .await
        .map_err(map_flex_error)?;

        publish_event(ctx, event);
        Ok(FlexSchemaObject::from(view))
    }

    /// Delete a standalone Flex schema.
    async fn delete_flex_schema(&self, ctx: &Context<'_>, id: Uuid) -> Result<DeleteFlexPayload> {
        let (tenant, auth) = require_access(ctx, Permission::FLEX_SCHEMAS_DELETE)?;
        let service = runtime(ctx)?.standalone_service();

        let event =
            crate::delete_schema_with_event(service.as_ref(), tenant.id, Some(auth.user_id), id)
                .await
                .map_err(map_flex_error)?;

        publish_event(ctx, event);
        Ok(DeleteFlexPayload { success: true })
    }

    /// Create a standalone Flex entry.
    async fn create_flex_entry(
        &self,
        ctx: &Context<'_>,
        input: CreateFlexEntryInput,
    ) -> Result<FlexEntryObject> {
        let (tenant, auth) = require_access(ctx, Permission::FLEX_ENTRIES_CREATE)?;
        let service = runtime(ctx)?.standalone_service();

        let (view, event) = crate::create_entry_with_event(
            service.as_ref(),
            tenant.id,
            Some(auth.user_id),
            CreateFlexEntryCommand {
                schema_id: input.schema_id,
                entity_type: input.entity_type,
                entity_id: input.entity_id,
                data: input.data,
                status: input.status,
            },
        )
        .await
        .map_err(map_flex_error)?;

        publish_event(ctx, event);
        Ok(FlexEntryObject::from(view))
    }

    /// Update a standalone Flex entry.
    async fn update_flex_entry(
        &self,
        ctx: &Context<'_>,
        schema_id: Uuid,
        id: Uuid,
        input: UpdateFlexEntryInput,
    ) -> Result<FlexEntryObject> {
        let (tenant, auth) = require_access(ctx, Permission::FLEX_ENTRIES_UPDATE)?;
        let service = runtime(ctx)?.standalone_service();

        let (view, event) = crate::update_entry_with_event(
            service.as_ref(),
            tenant.id,
            Some(auth.user_id),
            schema_id,
            id,
            UpdateFlexEntryCommand {
                data: input.data,
                status: input.status,
            },
        )
        .await
        .map_err(map_flex_error)?;

        publish_event(ctx, event);
        Ok(FlexEntryObject::from(view))
    }

    /// Delete a standalone Flex entry.
    async fn delete_flex_entry(
        &self,
        ctx: &Context<'_>,
        schema_id: Uuid,
        id: Uuid,
    ) -> Result<DeleteFlexPayload> {
        let (tenant, auth) = require_access(ctx, Permission::FLEX_ENTRIES_DELETE)?;
        let service = runtime(ctx)?.standalone_service();

        let event = crate::delete_entry_with_event(
            service.as_ref(),
            tenant.id,
            Some(auth.user_id),
            schema_id,
            id,
        )
        .await
        .map_err(map_flex_error)?;

        publish_event(ctx, event);
        Ok(DeleteFlexPayload { success: true })
    }
}

async fn invalidate_field_def_cache(
    runtime: &super::runtime::FlexGraphqlRuntime,
    tenant_id: Uuid,
    entity_type: &str,
) {
    crate::invalidate_field_definition_cache(
        runtime.field_definition_cache(),
        tenant_id,
        entity_type,
    )
    .await;
}

fn parse_fields_config(
    value: serde_json::Value,
) -> Result<Vec<rustok_core::field_schema::FieldDefinition>> {
    crate::parse_field_definitions_config(value).map_err(|error| bad_user_input(error.message()))
}

fn publish_event(ctx: &Context<'_>, event: EventEnvelope) {
    if let Ok(bus) = ctx.data::<EventBus>() {
        if let Err(error) = bus.publish_envelope(event) {
            tracing::warn!(error = %error, "Failed to publish flex event");
        }
    }
}
