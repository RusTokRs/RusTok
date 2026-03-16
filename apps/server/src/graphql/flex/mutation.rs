//! GraphQL mutations for Flex field definitions.
//!
//! RBAC: Admin / SuperAdmin only for all mutations (§11 of the Flex spec).

use async_graphql::{Context, FieldError, Object, Result};
use uuid::Uuid;

use rustok_core::{field_schema::FieldType, UserRole};
use rustok_events::types::EventEnvelope;

use crate::context::{infer_user_role_from_permissions, AuthContext, TenantContext};
use crate::graphql::errors::GraphQLError;
use crate::models::user_field_definitions::{
    CreateFieldDefinitionInput as ServiceCreate, UpdateFieldDefinitionInput as ServiceUpdate,
};
use crate::services::event_bus::event_bus_from_context;
use crate::services::rbac_service::RbacService;
use crate::services::user_field_service::UserFieldService;

use super::types::{
    CreateFieldDefinitionInput, DeleteFieldDefinitionPayload, FieldDefinitionObject,
    UpdateFieldDefinitionInput,
};

#[derive(Default)]
pub struct FlexMutation;

#[Object]
impl FlexMutation {
    /// Create a new custom field definition.
    ///
    /// Requires Admin or SuperAdmin role.
    async fn create_field_definition(
        &self,
        ctx: &Context<'_>,
        input: CreateFieldDefinitionInput,
    ) -> Result<FieldDefinitionObject> {
        let auth = require_admin(ctx)?;
        let tenant = ctx.data::<TenantContext>()?;
        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;

        let field_type: FieldType = serde_json::from_value(serde_json::Value::String(input.field_type))
            .map_err(|_| FieldError::new("Unknown field_type value"))?;

        let label = serde_json::from_value(input.label)
            .map_err(|_| FieldError::new("label must be a JSON object {\"en\": \"…\"}"))?;

        let description = input
            .description
            .map(|v| {
                serde_json::from_value(v)
                    .map_err(|_| FieldError::new("description must be a JSON object"))
            })
            .transpose()?;

        let validation = input
            .validation
            .map(|v| {
                serde_json::from_value(v)
                    .map_err(|_| FieldError::new("validation must be a valid ValidationRule JSON"))
            })
            .transpose()?;

        let service_input = ServiceCreate {
            field_key: input.field_key,
            field_type,
            label,
            description,
            is_required: input.is_required,
            default_value: input.default_value,
            validation,
            position: input.position,
        };

        let (model, event) =
            UserFieldService::create(&app_ctx.db, tenant.id, Some(auth.user_id), service_input)
                .await
                .map_err(|e| FieldError::new(e.to_string()))?;

        publish_event(ctx, event);

        Ok(FieldDefinitionObject::from(model))
    }

    /// Update an existing field definition.
    ///
    /// Requires Admin or SuperAdmin role.
    async fn update_field_definition(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        input: UpdateFieldDefinitionInput,
    ) -> Result<FieldDefinitionObject> {
        let auth = require_admin(ctx)?;
        let tenant = ctx.data::<TenantContext>()?;
        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;

        let label = input
            .label
            .map(|v| {
                serde_json::from_value(v)
                    .map_err(|_| FieldError::new("label must be a JSON object"))
            })
            .transpose()?;

        let description = input
            .description
            .map(|v| {
                serde_json::from_value(v)
                    .map_err(|_| FieldError::new("description must be a JSON object"))
            })
            .transpose()?;

        let validation = input
            .validation
            .map(|v| {
                serde_json::from_value(v)
                    .map_err(|_| FieldError::new("validation must be a valid ValidationRule JSON"))
            })
            .transpose()?;

        let service_input = ServiceUpdate {
            label,
            description,
            is_required: input.is_required,
            default_value: input.default_value,
            validation,
            position: input.position,
            is_active: input.is_active,
        };

        let (model, event) =
            UserFieldService::update(&app_ctx.db, tenant.id, Some(auth.user_id), id, service_input)
                .await
                .map_err(|e| FieldError::new(e.to_string()))?;

        publish_event(ctx, event);

        Ok(FieldDefinitionObject::from(model))
    }

    /// Soft-delete a field definition (`is_active = false`).
    ///
    /// Requires SuperAdmin role. Data in `users.metadata` is preserved.
    async fn delete_field_definition(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
    ) -> Result<DeleteFieldDefinitionPayload> {
        let auth = require_super_admin(ctx)?;
        let tenant = ctx.data::<TenantContext>()?;
        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;

        let event =
            UserFieldService::deactivate(&app_ctx.db, tenant.id, Some(auth.user_id), id)
                .await
                .map_err(|e| FieldError::new(e.to_string()))?;

        publish_event(ctx, event);

        Ok(DeleteFieldDefinitionPayload { success: true })
    }

    /// Reorder field definitions by supplying an ordered list of ids.
    ///
    /// Requires Admin or SuperAdmin role.
    async fn reorder_field_definitions(
        &self,
        ctx: &Context<'_>,
        entity_type: String,
        ids: Vec<Uuid>,
    ) -> Result<Vec<FieldDefinitionObject>> {
        require_admin(ctx)?;
        let tenant = ctx.data::<TenantContext>()?;
        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;

        if entity_type != "user" {
            return Err(FieldError::new(format!(
                "Unknown entity type: {}",
                entity_type
            )));
        }

        let rows = UserFieldService::reorder(&app_ctx.db, tenant.id, &ids)
            .await
            .map_err(|e| FieldError::new(e.to_string()))?;

        Ok(rows.into_iter().map(FieldDefinitionObject::from).collect())
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn require_admin(ctx: &Context<'_>) -> Result<&AuthContext> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
    let role = infer_user_role_from_permissions(&auth.permissions);
    if !matches!(role, UserRole::Admin | UserRole::SuperAdmin) {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Admin or SuperAdmin required to manage field definitions",
        ));
    }
    Ok(auth)
}

fn require_super_admin(ctx: &Context<'_>) -> Result<&AuthContext> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
    let role = infer_user_role_from_permissions(&auth.permissions);
    if role != UserRole::SuperAdmin {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "SuperAdmin required to delete field definitions",
        ));
    }
    Ok(auth)
}

/// Fire-and-forget event publishing — errors are logged but not propagated.
fn publish_event(ctx: &Context<'_>, event: EventEnvelope) {
    if let Ok(app_ctx) = ctx.data::<loco_rs::prelude::AppContext>() {
        let bus = event_bus_from_context(app_ctx);
        if let Err(e) = bus.publish_envelope(event) {
            tracing::warn!(error = %e, "Failed to publish flex event");
        }
    }
}
