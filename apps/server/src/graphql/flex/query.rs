//! GraphQL queries for Flex field definitions.

use async_graphql::{Context, FieldError, Object, Result};
use uuid::Uuid;

use crate::context::{AuthContext, TenantContext};
use crate::graphql::errors::GraphQLError;
use crate::services::user_field_service::UserFieldService;

use super::types::FieldDefinitionObject;

/// Queries for field definitions.
///
/// Currently routes `entity_type = "user"` only.  Further entity types will be
/// added in Phase 4 via a registry (§12 of the Flex spec).
#[derive(Default)]
pub struct FlexQuery;

#[Object]
impl FlexQuery {
    /// List all field definitions for the authenticated tenant.
    ///
    /// `entity_type` is accepted for forward-compatibility but only `"user"` is
    /// supported at this phase.
    async fn field_definitions(
        &self,
        ctx: &Context<'_>,
        entity_type: String,
    ) -> Result<Vec<FieldDefinitionObject>> {
        ctx.data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;
        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;

        if entity_type != "user" {
            return Err(FieldError::new(format!(
                "Unknown entity type: {}",
                entity_type
            )));
        }

        let rows = UserFieldService::list_all(&app_ctx.db, tenant.id)
            .await
            .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        Ok(rows.into_iter().map(FieldDefinitionObject::from).collect())
    }

    /// Find a single field definition by id.
    async fn field_definition(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
    ) -> Result<Option<FieldDefinitionObject>> {
        ctx.data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;
        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;

        let row = UserFieldService::find_by_id(&app_ctx.db, tenant.id, id)
            .await
            .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        Ok(row.map(FieldDefinitionObject::from))
    }
}
