use axum::{
    Json,
    extract::{Path, State},
    routing::get,
};
use rustok_events::EventEnvelope;
use uuid::Uuid;

use crate::error::Result;
use crate::extractors::{
    rbac::{
        RequireFlexEntriesCreate, RequireFlexEntriesDelete, RequireFlexEntriesList,
        RequireFlexEntriesRead, RequireFlexEntriesUpdate, RequireFlexSchemasCreate,
        RequireFlexSchemasDelete, RequireFlexSchemasList, RequireFlexSchemasRead,
        RequireFlexSchemasUpdate,
    },
    tenant::CurrentTenant,
};
use crate::services::event_bus::event_bus_from_context;
use crate::services::flex_standalone_service::FlexStandaloneSeaOrmService;
use crate::services::server_runtime_context::ServerRuntimeContext;
use flex::rest::{
    CreateFlexEntryRequest, CreateFlexSchemaRequest, DeleteFlexResponse, FlexEntryResponse,
    FlexSchemaResponse, UpdateFlexEntryRequest, UpdateFlexSchemaRequest,
};

#[utoipa::path(
    get,
    path = "/api/v1/flex/schemas",
    tag = "flex",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "List standalone Flex schemas", body = [FlexSchemaResponse]),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
async fn list_schemas(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireFlexSchemasList(_user): RequireFlexSchemasList,
) -> Result<Json<Vec<FlexSchemaResponse>>> {
    let service = FlexStandaloneSeaOrmService::new(ctx.db_clone());
    let rows = flex::list_schemas(&service, tenant.id)
        .await
        .map_err(map_flex_rest_error)?;
    Ok(Json(
        rows.into_iter().map(FlexSchemaResponse::from).collect(),
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/flex/schemas/{schema_id}",
    tag = "flex",
    security(("bearer_auth" = [])),
    params(("schema_id" = Uuid, Path, description = "Standalone Flex schema ID")),
    responses(
        (status = 200, description = "Standalone Flex schema", body = FlexSchemaResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found")
    )
)]
async fn get_schema(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireFlexSchemasRead(_user): RequireFlexSchemasRead,
    Path(schema_id): Path<Uuid>,
) -> Result<Json<FlexSchemaResponse>> {
    let service = FlexStandaloneSeaOrmService::new(ctx.db_clone());
    let row = flex::find_schema(&service, tenant.id, schema_id)
        .await
        .map_err(map_flex_rest_error)?
        .ok_or(crate::error::Error::NotFound)?;
    Ok(Json(FlexSchemaResponse::from(row)))
}

#[utoipa::path(
    post,
    path = "/api/v1/flex/schemas",
    tag = "flex",
    security(("bearer_auth" = [])),
    request_body = CreateFlexSchemaRequest,
    responses(
        (status = 200, description = "Created standalone Flex schema", body = FlexSchemaResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
async fn create_schema(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireFlexSchemasCreate(user): RequireFlexSchemasCreate,
    Json(input): Json<CreateFlexSchemaRequest>,
) -> Result<Json<FlexSchemaResponse>> {
    let service = FlexStandaloneSeaOrmService::new(ctx.db_clone());
    let (row, event) = flex::create_schema_with_event(
        &service,
        tenant.id,
        Some(user.user.id),
        input.into_command().map_err(map_flex_config_parse_error)?,
    )
    .await
    .map_err(map_flex_rest_error)?;

    publish_event(&ctx, event);
    Ok(Json(FlexSchemaResponse::from(row)))
}

#[utoipa::path(
    patch,
    path = "/api/v1/flex/schemas/{schema_id}",
    tag = "flex",
    security(("bearer_auth" = [])),
    params(("schema_id" = Uuid, Path, description = "Standalone Flex schema ID")),
    request_body = UpdateFlexSchemaRequest,
    responses(
        (status = 200, description = "Updated standalone Flex schema", body = FlexSchemaResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found")
    )
)]
async fn update_schema(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireFlexSchemasUpdate(user): RequireFlexSchemasUpdate,
    Path(schema_id): Path<Uuid>,
    Json(input): Json<UpdateFlexSchemaRequest>,
) -> Result<Json<FlexSchemaResponse>> {
    let service = FlexStandaloneSeaOrmService::new(ctx.db_clone());
    let (row, event) = flex::update_schema_with_event(
        &service,
        tenant.id,
        Some(user.user.id),
        schema_id,
        input.into_command().map_err(map_flex_config_parse_error)?,
    )
    .await
    .map_err(map_flex_rest_error)?;

    publish_event(&ctx, event);
    Ok(Json(FlexSchemaResponse::from(row)))
}

#[utoipa::path(
    delete,
    path = "/api/v1/flex/schemas/{schema_id}",
    tag = "flex",
    security(("bearer_auth" = [])),
    params(("schema_id" = Uuid, Path, description = "Standalone Flex schema ID")),
    responses(
        (status = 200, description = "Deleted standalone Flex schema", body = DeleteFlexResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found")
    )
)]
async fn delete_schema(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireFlexSchemasDelete(user): RequireFlexSchemasDelete,
    Path(schema_id): Path<Uuid>,
) -> Result<Json<DeleteFlexResponse>> {
    let service = FlexStandaloneSeaOrmService::new(ctx.db_clone());
    let event = flex::delete_schema_with_event(&service, tenant.id, Some(user.user.id), schema_id)
        .await
        .map_err(map_flex_rest_error)?;

    publish_event(&ctx, event);
    Ok(Json(DeleteFlexResponse::success()))
}

#[utoipa::path(
    get,
    path = "/api/v1/flex/schemas/{schema_id}/entries",
    tag = "flex",
    security(("bearer_auth" = [])),
    params(("schema_id" = Uuid, Path, description = "Standalone Flex schema ID")),
    responses(
        (status = 200, description = "List standalone Flex entries", body = [FlexEntryResponse]),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
async fn list_entries(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireFlexEntriesList(_user): RequireFlexEntriesList,
    Path(schema_id): Path<Uuid>,
) -> Result<Json<Vec<FlexEntryResponse>>> {
    let service = FlexStandaloneSeaOrmService::new(ctx.db_clone());
    let rows = flex::list_entries(&service, tenant.id, schema_id)
        .await
        .map_err(map_flex_rest_error)?;
    Ok(Json(
        rows.into_iter().map(FlexEntryResponse::from).collect(),
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/flex/schemas/{schema_id}/entries/{entry_id}",
    tag = "flex",
    security(("bearer_auth" = [])),
    params(
        ("schema_id" = Uuid, Path, description = "Standalone Flex schema ID"),
        ("entry_id" = Uuid, Path, description = "Standalone Flex entry ID")
    ),
    responses(
        (status = 200, description = "Standalone Flex entry", body = FlexEntryResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found")
    )
)]
async fn get_entry(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireFlexEntriesRead(_user): RequireFlexEntriesRead,
    Path((schema_id, entry_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<FlexEntryResponse>> {
    let service = FlexStandaloneSeaOrmService::new(ctx.db_clone());
    let row = flex::find_entry(&service, tenant.id, schema_id, entry_id)
        .await
        .map_err(map_flex_rest_error)?
        .ok_or(crate::error::Error::NotFound)?;
    Ok(Json(FlexEntryResponse::from(row)))
}

#[utoipa::path(
    post,
    path = "/api/v1/flex/schemas/{schema_id}/entries",
    tag = "flex",
    security(("bearer_auth" = [])),
    params(("schema_id" = Uuid, Path, description = "Standalone Flex schema ID")),
    request_body = CreateFlexEntryRequest,
    responses(
        (status = 200, description = "Created standalone Flex entry", body = FlexEntryResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
async fn create_entry(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireFlexEntriesCreate(user): RequireFlexEntriesCreate,
    Path(schema_id): Path<Uuid>,
    Json(input): Json<CreateFlexEntryRequest>,
) -> Result<Json<FlexEntryResponse>> {
    let service = FlexStandaloneSeaOrmService::new(ctx.db_clone());
    let (row, event) = flex::create_entry_with_event(
        &service,
        tenant.id,
        Some(user.user.id),
        input.into_command(schema_id),
    )
    .await
    .map_err(map_flex_rest_error)?;

    publish_event(&ctx, event);
    Ok(Json(FlexEntryResponse::from(row)))
}

#[utoipa::path(
    patch,
    path = "/api/v1/flex/schemas/{schema_id}/entries/{entry_id}",
    tag = "flex",
    security(("bearer_auth" = [])),
    params(
        ("schema_id" = Uuid, Path, description = "Standalone Flex schema ID"),
        ("entry_id" = Uuid, Path, description = "Standalone Flex entry ID")
    ),
    request_body = UpdateFlexEntryRequest,
    responses(
        (status = 200, description = "Updated standalone Flex entry", body = FlexEntryResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found")
    )
)]
async fn update_entry(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireFlexEntriesUpdate(user): RequireFlexEntriesUpdate,
    Path((schema_id, entry_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<UpdateFlexEntryRequest>,
) -> Result<Json<FlexEntryResponse>> {
    let service = FlexStandaloneSeaOrmService::new(ctx.db_clone());
    let (row, event) = flex::update_entry_with_event(
        &service,
        tenant.id,
        Some(user.user.id),
        schema_id,
        entry_id,
        input.into_command(),
    )
    .await
    .map_err(map_flex_rest_error)?;

    publish_event(&ctx, event);
    Ok(Json(FlexEntryResponse::from(row)))
}

#[utoipa::path(
    delete,
    path = "/api/v1/flex/schemas/{schema_id}/entries/{entry_id}",
    tag = "flex",
    security(("bearer_auth" = [])),
    params(
        ("schema_id" = Uuid, Path, description = "Standalone Flex schema ID"),
        ("entry_id" = Uuid, Path, description = "Standalone Flex entry ID")
    ),
    responses(
        (status = 200, description = "Deleted standalone Flex entry", body = DeleteFlexResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found")
    )
)]
async fn delete_entry(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    RequireFlexEntriesDelete(user): RequireFlexEntriesDelete,
    Path((schema_id, entry_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<DeleteFlexResponse>> {
    let service = FlexStandaloneSeaOrmService::new(ctx.db_clone());
    let event =
        flex::delete_entry_with_event(&service, tenant.id, Some(user.user.id), schema_id, entry_id)
            .await
            .map_err(map_flex_rest_error)?;

    publish_event(&ctx, event);
    Ok(Json(DeleteFlexResponse::success()))
}

pub fn router() -> crate::routes::ServerRouter {
    axum::Router::new()
        .route(
            "/api/v1/flex/schemas/",
            get(list_schemas).post(create_schema),
        )
        .route(
            "/api/v1/flex/schemas/{schema_id}",
            get(get_schema).patch(update_schema).delete(delete_schema),
        )
        .route(
            "/api/v1/flex/schemas/{schema_id}/entries",
            get(list_entries).post(create_entry),
        )
        .route(
            "/api/v1/flex/schemas/{schema_id}/entries/{entry_id}",
            get(get_entry).patch(update_entry).delete(delete_entry),
        )
}

fn publish_event(ctx: &ServerRuntimeContext, event: EventEnvelope) {
    let bus = event_bus_from_context(ctx);
    if let Err(error) = bus.publish_envelope(event) {
        tracing::warn!(error = %error, "Failed to publish flex standalone REST event");
    }
}

fn map_flex_config_parse_error(
    error: flex::FieldDefinitionsConfigParseError,
) -> crate::error::Error {
    crate::error::Error::BadRequest(error.message().to_string())
}

fn map_flex_rest_error(error: rustok_core::field_schema::FlexError) -> crate::error::Error {
    let mapped = flex::map_flex_error(error);
    match mapped.kind {
        flex::FlexMappedErrorKind::Internal => crate::error::Error::InternalServerError,
        flex::FlexMappedErrorKind::NotFound => crate::error::Error::NotFound,
        flex::FlexMappedErrorKind::BadUserInput => crate::error::Error::BadRequest(mapped.message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::TenantContext;
    use crate::extractors::auth::CurrentUser;
    use crate::models::{flex_entries, flex_schemas, tenants, users};
    use rustok_api::Permission;
    use rustok_core::{
        UserRole, UserStatus,
        field_schema::{FieldDefinition, FieldType},
    };
    use rustok_migrations::Migrator;
    use rustok_test_utils::db::setup_test_db_with_migrations;
    use sea_orm::{ActiveModelTrait, EntityTrait, Set};
    use serde_json::json;
    use std::collections::HashMap;

    fn test_runtime_context(db: sea_orm::DatabaseConnection) -> ServerRuntimeContext {
        ServerRuntimeContext::new(db, crate::common::settings::RustokSettings::default())
    }

    fn tenant_context(model: &tenants::Model) -> TenantContext {
        TenantContext {
            id: model.id,
            name: model.name.clone(),
            slug: model.slug.clone(),
            domain: model.domain.clone(),
            settings: model.settings.clone(),
            default_locale: model.default_locale.clone(),
            is_active: model.is_active,
        }
    }

    fn current_user(tenant_id: Uuid) -> CurrentUser {
        CurrentUser {
            user: users::Model {
                id: Uuid::new_v4(),
                tenant_id,
                email: "flex-admin@example.com".to_string(),
                password_hash: "hash".to_string(),
                name: Some("Flex Admin".to_string()),
                status: UserStatus::Active,
                email_verified_at: None,
                last_login_at: None,
                metadata: json!({}),
                created_at: chrono::Utc::now().into(),
                updated_at: chrono::Utc::now().into(),
            },
            session_id: Uuid::new_v4(),
            permissions: vec![
                Permission::FLEX_SCHEMAS_CREATE,
                Permission::FLEX_SCHEMAS_READ,
                Permission::FLEX_SCHEMAS_UPDATE,
                Permission::FLEX_SCHEMAS_DELETE,
                Permission::FLEX_SCHEMAS_LIST,
                Permission::FLEX_ENTRIES_CREATE,
                Permission::FLEX_ENTRIES_READ,
                Permission::FLEX_ENTRIES_UPDATE,
                Permission::FLEX_ENTRIES_DELETE,
                Permission::FLEX_ENTRIES_LIST,
            ],
            inferred_role: UserRole::Admin,
            actor_kind: rustok_core::SecurityActorKind::User,
            client_id: None,
            scopes: Vec::new(),
            grant_type: "direct".to_string(),
        }
    }

    fn field_definition(field_key: &str, is_localized: bool) -> FieldDefinition {
        FieldDefinition {
            field_key: field_key.to_string(),
            field_type: FieldType::Text,
            label: HashMap::from([("en".to_string(), field_key.to_string())]),
            description: None,
            is_localized,
            is_required: false,
            default_value: None,
            validation: None,
            position: 0,
            is_active: true,
        }
    }

    #[tokio::test]
    async fn rest_handlers_roundtrip_standalone_schema_and_entry() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let ctx = test_runtime_context(db.clone());

        let mut tenant = tenants::ActiveModel::new("Flex Tenant", "flex-rest");
        tenant.default_locale = Set("ru".to_string());
        let tenant = tenant
            .insert(&db)
            .await
            .expect("tenant should insert for flex rest tests");
        let tenant_ctx = tenant_context(&tenant);
        let user = current_user(tenant.id);

        let Json(created_schema) = create_schema(
            State(ctx.clone()),
            CurrentTenant(tenant_ctx.clone()),
            RequireFlexSchemasCreate(current_user(tenant.id)),
            Json(CreateFlexSchemaRequest {
                slug: "landing_page".to_string(),
                name: "Лендинг".to_string(),
                description: Some("Описание".to_string()),
                fields_config: serde_json::to_value(vec![
                    field_definition("slug", false),
                    field_definition("title", true),
                ])
                .expect("fields config json"),
                settings: Some(json!({"layout": "hero"})),
                is_active: Some(true),
            }),
        )
        .await
        .expect("schema should create");

        assert_eq!(created_schema.slug, "landing_page");
        assert_eq!(created_schema.name, "Лендинг");

        let Json(listed_schemas) = list_schemas(
            State(ctx.clone()),
            CurrentTenant(tenant_ctx.clone()),
            RequireFlexSchemasList(current_user(tenant.id)),
        )
        .await
        .expect("schemas should list");
        assert_eq!(listed_schemas.len(), 1);
        assert_eq!(listed_schemas[0].id, created_schema.id);

        let Json(updated_schema) = update_schema(
            State(ctx.clone()),
            CurrentTenant(tenant_ctx.clone()),
            RequireFlexSchemasUpdate(current_user(tenant.id)),
            Path(created_schema.id),
            Json(UpdateFlexSchemaRequest {
                name: Some("Лендинг 2".to_string()),
                description: Some("Новое описание".to_string()),
                fields_config: None,
                settings: Some(json!({"layout": "feature-grid"})),
                is_active: Some(true),
            }),
        )
        .await
        .expect("schema should update");
        assert_eq!(updated_schema.name, "Лендинг 2");
        assert_eq!(updated_schema.settings, json!({"layout": "feature-grid"}));

        let Json(created_entry) = create_entry(
            State(ctx.clone()),
            CurrentTenant(tenant_ctx.clone()),
            RequireFlexEntriesCreate(current_user(tenant.id)),
            Path(created_schema.id),
            Json(CreateFlexEntryRequest {
                entity_type: None,
                entity_id: None,
                data: json!({"slug": "landing", "title": "Привет"}),
                status: Some("draft".to_string()),
            }),
        )
        .await
        .expect("entry should create");

        assert_eq!(created_entry.schema_id, created_schema.id);
        assert_eq!(
            created_entry.data,
            json!({"slug": "landing", "title": "Привет"})
        );

        let Json(updated_entry) = update_entry(
            State(ctx.clone()),
            CurrentTenant(tenant_ctx.clone()),
            RequireFlexEntriesUpdate(current_user(tenant.id)),
            Path((created_schema.id, created_entry.id)),
            Json(UpdateFlexEntryRequest {
                data: Some(json!({"slug": "landing", "title": "Здравствуйте"})),
                status: Some("published".to_string()),
            }),
        )
        .await
        .expect("entry should update");

        assert_eq!(updated_entry.status, "published");
        assert_eq!(
            updated_entry.data,
            json!({"slug": "landing", "title": "Здравствуйте"})
        );

        let Json(listed_entries) = list_entries(
            State(ctx.clone()),
            CurrentTenant(tenant_ctx.clone()),
            RequireFlexEntriesList(current_user(tenant.id)),
            Path(created_schema.id),
        )
        .await
        .expect("entries should list");
        assert_eq!(listed_entries.len(), 1);
        assert_eq!(listed_entries[0].id, created_entry.id);

        let Json(fetched_entry) = get_entry(
            State(ctx.clone()),
            CurrentTenant(tenant_ctx.clone()),
            RequireFlexEntriesRead(current_user(tenant.id)),
            Path((created_schema.id, created_entry.id)),
        )
        .await
        .expect("entry should resolve");
        assert_eq!(fetched_entry.status, "published");
        assert_eq!(
            fetched_entry.data,
            json!({"slug": "landing", "title": "Здравствуйте"})
        );

        let Json(delete_entry_response) = delete_entry(
            State(ctx.clone()),
            CurrentTenant(tenant_ctx.clone()),
            RequireFlexEntriesDelete(current_user(tenant.id)),
            Path((created_schema.id, created_entry.id)),
        )
        .await
        .expect("entry should delete");
        assert!(delete_entry_response.success);
        assert!(
            flex_entries::Entity::find_by_id(created_entry.id)
                .one(&db)
                .await
                .expect("entry lookup should succeed")
                .is_none()
        );

        let Json(delete_schema_response) = delete_schema(
            State(ctx),
            CurrentTenant(tenant_ctx),
            RequireFlexSchemasDelete(user),
            Path(created_schema.id),
        )
        .await
        .expect("schema should delete");
        assert!(delete_schema_response.success);
        assert!(
            flex_schemas::Entity::find_by_id(created_schema.id)
                .one(&db)
                .await
                .expect("schema lookup should succeed")
                .is_none()
        );
    }

    #[tokio::test]
    async fn create_schema_rejects_invalid_fields_config_payload() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let ctx = test_runtime_context(db.clone());
        let tenant = tenants::ActiveModel::new("Flex Tenant", "flex-rest-invalid")
            .insert(&db)
            .await
            .expect("tenant should insert for invalid payload test");

        let error = create_schema(
            State(ctx),
            CurrentTenant(tenant_context(&tenant)),
            RequireFlexSchemasCreate(current_user(tenant.id)),
            Json(CreateFlexSchemaRequest {
                slug: "broken".to_string(),
                name: "Broken".to_string(),
                description: None,
                fields_config: json!({"not": "an array"}),
                settings: None,
                is_active: None,
            }),
        )
        .await
        .expect_err("invalid fields config must be rejected");

        match error {
            crate::error::Error::BadRequest(message) => {
                assert!(message.contains("fields_config must be a valid JSON array"));
            }
            other => panic!("expected bad request, got {other:?}"),
        }
    }
}
