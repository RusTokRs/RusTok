use async_graphql::{Context, Object, Result};
use rustok_api::{Permission, graphql::PaginationInput};
use uuid::Uuid;

use super::{
    FieldDefinitionObject, FlexEntryObject, FlexSchemaObject, map_flex_error, require_access,
    resolve_entity_type, runtime::runtime,
};
use crate::FieldDefinitionView;

#[derive(Default)]
pub struct FlexQuery;

#[Object]
impl FlexQuery {
    /// List attached field definitions for the authenticated tenant.
    async fn field_definitions(
        &self,
        ctx: &Context<'_>,
        entity_type: Option<String>,
        #[graphql(default)] pagination: PaginationInput,
    ) -> Result<Vec<FieldDefinitionObject>> {
        let (tenant, _) = require_access(ctx, Permission::FLEX_SCHEMAS_LIST)?;
        let runtime = runtime(ctx)?;
        let entity_type = resolve_entity_type(entity_type)?;

        let rows = crate::list_field_definitions_with_cache(
            runtime.field_registry(),
            runtime.db(),
            runtime.field_definition_cache(),
            tenant.id,
            &entity_type,
        )
        .await
        .map_err(map_flex_error)?;

        paginate_rows(rows, &pagination)
    }

    /// Find one attached field definition by id for the requested entity type.
    async fn field_definition(
        &self,
        ctx: &Context<'_>,
        entity_type: Option<String>,
        id: Uuid,
    ) -> Result<Option<FieldDefinitionObject>> {
        let (tenant, _) = require_access(ctx, Permission::FLEX_SCHEMAS_READ)?;
        let runtime = runtime(ctx)?;
        let entity_type = resolve_entity_type(entity_type)?;

        crate::find_field_definition(
            runtime.field_registry(),
            runtime.db(),
            tenant.id,
            &entity_type,
            id,
        )
        .await
        .map(|row| row.map(FieldDefinitionObject::from))
        .map_err(map_flex_error)
    }

    /// List standalone Flex schemas for the authenticated tenant.
    async fn flex_schemas(&self, ctx: &Context<'_>) -> Result<Vec<FlexSchemaObject>> {
        let (tenant, _) = require_access(ctx, Permission::FLEX_SCHEMAS_LIST)?;
        let service = runtime(ctx)?.standalone_service();

        crate::list_schemas(service.as_ref(), tenant.id)
            .await
            .map(|rows| rows.into_iter().map(FlexSchemaObject::from).collect())
            .map_err(map_flex_error)
    }

    /// Find a single standalone Flex schema by id.
    async fn flex_schema(&self, ctx: &Context<'_>, id: Uuid) -> Result<Option<FlexSchemaObject>> {
        let (tenant, _) = require_access(ctx, Permission::FLEX_SCHEMAS_READ)?;
        let service = runtime(ctx)?.standalone_service();

        crate::find_schema(service.as_ref(), tenant.id, id)
            .await
            .map(|row| row.map(FlexSchemaObject::from))
            .map_err(map_flex_error)
    }

    /// List entries for a standalone Flex schema.
    async fn flex_entries(
        &self,
        ctx: &Context<'_>,
        schema_id: Uuid,
    ) -> Result<Vec<FlexEntryObject>> {
        let (tenant, _) = require_access(ctx, Permission::FLEX_ENTRIES_LIST)?;
        let service = runtime(ctx)?.standalone_service();

        crate::list_entries(service.as_ref(), tenant.id, schema_id)
            .await
            .map(|rows| rows.into_iter().map(FlexEntryObject::from).collect())
            .map_err(map_flex_error)
    }

    /// Find a single standalone Flex entry by id.
    async fn flex_entry(
        &self,
        ctx: &Context<'_>,
        schema_id: Uuid,
        id: Uuid,
    ) -> Result<Option<FlexEntryObject>> {
        let (tenant, _) = require_access(ctx, Permission::FLEX_ENTRIES_READ)?;
        let service = runtime(ctx)?.standalone_service();

        crate::find_entry(service.as_ref(), tenant.id, schema_id, id)
            .await
            .map(|row| row.map(FlexEntryObject::from))
            .map_err(map_flex_error)
    }
}

fn paginate_rows(
    rows: Vec<FieldDefinitionView>,
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
    use crate::FieldDefinitionView;
    use rustok_api::graphql::PaginationInput;
    use serde_json::json;
    use uuid::Uuid;

    fn row(idx: usize) -> FieldDefinitionView {
        FieldDefinitionView {
            id: Uuid::new_v4(),
            field_key: format!("k{idx}"),
            field_type: "text".to_string(),
            label: json!({"en": format!("k{idx}")}),
            description: None,
            is_localized: false,
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
