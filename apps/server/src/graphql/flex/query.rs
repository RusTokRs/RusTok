//! GraphQL queries for Flex field definitions.

use async_graphql::{Context, FieldError, Object, Result};
use uuid::Uuid;

use crate::context::{AuthContext, TenantContext};
use crate::graphql::common::PaginationInput;
use crate::graphql::errors::GraphQLError;
use crate::services::field_definition_cache::FieldDefinitionCache;
use crate::services::field_definition_registry::FieldDefRegistry;

use super::{map_flex_error, resolve_entity_type, types::FieldDefinitionObject};

/// Queries for field definitions.
///
/// Routed by `entity_type` through `FieldDefRegistry`.
/// For backward-compatibility, omitted `entity_type` defaults to `"user"`.
#[derive(Default)]
pub struct FlexQuery;

#[Object]
impl FlexQuery {
    /// List all field definitions for the authenticated tenant.
    ///
    /// `entity_type` routes the query to a module-specific service.
    /// When omitted, defaults to `"user"` for backward-compatibility.
    async fn field_definitions(
        &self,
        ctx: &Context<'_>,
        entity_type: Option<String>,
        #[graphql(default)] pagination: PaginationInput,
    ) -> Result<Vec<FieldDefinitionObject>> {
        ctx.data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;
        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;
        let entity_type = resolve_entity_type(entity_type)?;

        let cache = ctx.data::<FieldDefinitionCache>()?;
        if let Some(rows) = cache.get(tenant.id, &entity_type).await {
            return paginate_rows(rows, &pagination);
        }

        let registry = ctx.data::<FieldDefRegistry>()?;
        let service = registry.get(&entity_type).map_err(map_flex_error)?;

        let rows = service
            .list_all(&app_ctx.db, tenant.id)
            .await
            .map_err(map_flex_error)?;

        cache.set(tenant.id, &entity_type, rows.clone()).await;

        paginate_rows(rows, &pagination)
    }

    /// Find a single field definition by id for the requested entity type.
    async fn field_definition(
        &self,
        ctx: &Context<'_>,
        entity_type: Option<String>,
        id: Uuid,
    ) -> Result<Option<FieldDefinitionObject>> {
        ctx.data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;
        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;
        let entity_type = resolve_entity_type(entity_type)?;

        let registry = ctx.data::<FieldDefRegistry>()?;
        let service = registry.get(&entity_type).map_err(map_flex_error)?;

        service
            .find_by_id(&app_ctx.db, tenant.id, id)
            .await
            .map(|row| row.map(FieldDefinitionObject::from))
            .map_err(map_flex_error)
    }
}

fn paginate_rows(
    rows: Vec<crate::services::field_definition_registry::FieldDefinitionView>,
    pagination: &PaginationInput,
) -> Result<Vec<FieldDefinitionObject>> {
    let (offset, limit) = pagination.normalize()?;
    let start = offset as usize;
    if start >= rows.len() {
        return Ok(Vec::new());
    }

    let take_n = limit.max(0) as usize;
    Ok(rows
        .into_iter()
        .skip(start)
        .take(take_n)
        .map(FieldDefinitionObject::from)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::paginate_rows;
    use crate::graphql::common::PaginationInput;
    use crate::services::field_definition_registry::FieldDefinitionView;
    use serde_json::json;
    use uuid::Uuid;

    fn row(idx: usize) -> FieldDefinitionView {
        FieldDefinitionView {
            id: Uuid::new_v4(),
            field_key: format!("k{idx}"),
            field_type: "text".to_string(),
            label: json!({"en": format!("k{idx}")}),
            description: None,
            is_required: false,
            default_value: None,
            validation: None,
            position: idx as i32,
            is_active: true,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    #[test]
    fn paginate_rows_respects_offset_and_limit() {
        let rows = (0..5).map(row).collect();
        let pagination = PaginationInput {
            offset: 1,
            limit: 2,
            ..Default::default()
        };

        let paged = paginate_rows(rows, &pagination).expect("pagination should succeed");
        assert_eq!(paged.len(), 2);
        assert_eq!(paged[0].position, 1);
        assert_eq!(paged[1].position, 2);
    }

    #[test]
    fn paginate_rows_returns_empty_when_offset_out_of_range() {
        let rows = (0..3).map(row).collect();
        let pagination = PaginationInput {
            offset: 100,
            limit: 10,
            ..Default::default()
        };

        let paged = paginate_rows(rows, &pagination).expect("pagination should succeed");
        assert!(paged.is_empty());
    }
}
