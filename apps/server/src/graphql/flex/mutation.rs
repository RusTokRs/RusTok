//! GraphQL mutations for Flex field definitions.
//!
//! RBAC is enforced with explicit `flex_schemas:*` permissions resolved by the
//! server RBAC runtime.

use async_graphql::{Context, Object, Result};
use uuid::Uuid;

use rustok_core::{field_schema::FieldType, Permission};
use rustok_events::EventEnvelope;

use crate::context::TenantContext;
use crate::services::event_bus::event_bus_from_context;
use crate::services::field_definition_cache::FieldDefinitionCache;
use crate::services::flex_standalone_service::FlexStandaloneSeaOrmService;
use flex::{
    CreateFieldDefinitionCommand, CreateFlexEntryCommand, CreateFlexSchemaCommand,
    FieldDefRegistry, UpdateFieldDefinitionCommand, UpdateFlexEntryCommand,
    UpdateFlexSchemaCommand,
};

use super::{
    bad_user_input, map_flex_error, require_permission, resolve_entity_type,
    types::{
        CreateFieldDefinitionInput, CreateFlexEntryInput, CreateFlexSchemaInput,
        DeleteFieldDefinitionPayload, DeleteFlexPayload, FieldDefinitionObject, FlexEntryObject,
        FlexSchemaObject, UpdateFieldDefinitionInput, UpdateFlexEntryInput, UpdateFlexSchemaInput,
    },
};

#[derive(Default)]
pub struct FlexMutation;

#[Object]
impl FlexMutation {
    /// Create a new custom field definition.
    ///
    /// Requires `flex_schemas:create`.
    async fn create_field_definition(
        &self,
        ctx: &Context<'_>,
        input: CreateFieldDefinitionInput,
    ) -> Result<FieldDefinitionObject> {
        let auth = require_permission(ctx, Permission::FLEX_SCHEMAS_CREATE)?;
        let tenant = ctx.data::<TenantContext>()?;
        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;

        let field_type: FieldType =
            serde_json::from_value(serde_json::Value::String(input.field_type.clone()))
                .map_err(|_| bad_user_input("Unknown field_type value"))?;

        let label = serde_json::from_value(input.label)
            .map_err(|_| bad_user_input("label must be a JSON object {\"en\": \"...\"}"))?;

        let description = input
            .description
            .map(|v| {
                serde_json::from_value(v)
                    .map_err(|_| bad_user_input("description must be a JSON object"))
            })
            .transpose()?;

        let validation = input
            .validation
            .map(|v| {
                serde_json::from_value(v)
                    .map_err(|_| bad_user_input("validation must be a valid ValidationRule JSON"))
            })
            .transpose()?;

        let entity_type = resolve_entity_type(input.entity_type)?;

        let registry = ctx.data::<FieldDefRegistry>()?;

        let service_input = CreateFieldDefinitionCommand {
            field_key: input.field_key,
            field_type,
            label,
            description,
            is_localized: input.is_localized,
            is_required: input.is_required,
            default_value: input.default_value,
            validation,
            position: input.position,
        };

        let (model, event) = flex::create_field_definition(
            registry,
            &app_ctx.db,
            tenant.id,
            &entity_type,
            Some(auth.user_id),
            service_input,
        )
        .await
        .map_err(map_flex_error)?;

        publish_event(ctx, event);
        invalidate_field_def_cache(ctx, tenant.id, &entity_type).await;

        Ok(FieldDefinitionObject::from(model))
    }

    /// Update an existing field definition.
    ///
    /// Requires `flex_schemas:update`.
    async fn update_field_definition(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        input: UpdateFieldDefinitionInput,
    ) -> Result<FieldDefinitionObject> {
        let auth = require_permission(ctx, Permission::FLEX_SCHEMAS_UPDATE)?;
        let tenant = ctx.data::<TenantContext>()?;
        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;

        let label = input
            .label
            .map(|v| {
                serde_json::from_value(v).map_err(|_| bad_user_input("label must be a JSON object"))
            })
            .transpose()?;

        let description = input
            .description
            .map(|v| {
                serde_json::from_value(v)
                    .map_err(|_| bad_user_input("description must be a JSON object"))
            })
            .transpose()?;

        let validation = input
            .validation
            .map(|v| {
                serde_json::from_value(v)
                    .map_err(|_| bad_user_input("validation must be a valid ValidationRule JSON"))
            })
            .transpose()?;

        let entity_type = resolve_entity_type(input.entity_type)?;

        let registry = ctx.data::<FieldDefRegistry>()?;

        let service_input = UpdateFieldDefinitionCommand {
            label,
            description,
            is_localized: input.is_localized,
            is_required: input.is_required,
            default_value: input.default_value,
            validation,
            position: input.position,
            is_active: input.is_active,
        };

        let (model, event) = flex::update_field_definition(
            registry,
            &app_ctx.db,
            tenant.id,
            &entity_type,
            Some(auth.user_id),
            id,
            service_input,
        )
        .await
        .map_err(map_flex_error)?;

        publish_event(ctx, event);
        invalidate_field_def_cache(ctx, tenant.id, &entity_type).await;

        Ok(FieldDefinitionObject::from(model))
    }

    /// Soft-delete a field definition (`is_active = false`).
    ///
    /// Requires `flex_schemas:delete`. Data in `users.metadata` is preserved.
    async fn delete_field_definition(
        &self,
        ctx: &Context<'_>,
        entity_type: Option<String>,
        id: Uuid,
    ) -> Result<DeleteFieldDefinitionPayload> {
        let auth = require_permission(ctx, Permission::FLEX_SCHEMAS_DELETE)?;
        let tenant = ctx.data::<TenantContext>()?;
        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;

        let entity_type = resolve_entity_type(entity_type)?;

        let registry = ctx.data::<FieldDefRegistry>()?;

        let event = flex::deactivate_field_definition(
            registry,
            &app_ctx.db,
            tenant.id,
            &entity_type,
            Some(auth.user_id),
            id,
        )
        .await
        .map_err(map_flex_error)?;

        publish_event(ctx, event);
        invalidate_field_def_cache(ctx, tenant.id, &entity_type).await;

        Ok(DeleteFieldDefinitionPayload { success: true })
    }

    /// Reorder field definitions by supplying an ordered list of ids.
    ///
    /// Requires `flex_schemas:update`.
    async fn reorder_field_definitions(
        &self,
        ctx: &Context<'_>,
        entity_type: Option<String>,
        ids: Vec<Uuid>,
    ) -> Result<Vec<FieldDefinitionObject>> {
        require_permission(ctx, Permission::FLEX_SCHEMAS_UPDATE)?;
        let tenant = ctx.data::<TenantContext>()?;
        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;

        let entity_type = resolve_entity_type(entity_type)?;

        let registry = ctx.data::<FieldDefRegistry>()?;

        let rows =
            flex::reorder_field_definitions(registry, &app_ctx.db, tenant.id, &entity_type, &ids)
                .await
                .map_err(map_flex_error)?;

        invalidate_field_def_cache(ctx, tenant.id, &entity_type).await;

        Ok(rows.into_iter().map(FieldDefinitionObject::from).collect())
    }

    /// Create a standalone Flex schema.
    ///
    /// Requires `flex_schemas:create`.
    async fn create_flex_schema(
        &self,
        ctx: &Context<'_>,
        input: CreateFlexSchemaInput,
    ) -> Result<FlexSchemaObject> {
        let auth = require_permission(ctx, Permission::FLEX_SCHEMAS_CREATE)?;
        let tenant = ctx.data::<TenantContext>()?;
        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;
        let service = FlexStandaloneSeaOrmService::new(app_ctx.db.clone());

        let (view, event) = flex::create_schema_with_event(
            &service,
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
    ///
    /// Requires `flex_schemas:update`.
    async fn update_flex_schema(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        input: UpdateFlexSchemaInput,
    ) -> Result<FlexSchemaObject> {
        let auth = require_permission(ctx, Permission::FLEX_SCHEMAS_UPDATE)?;
        let tenant = ctx.data::<TenantContext>()?;
        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;
        let service = FlexStandaloneSeaOrmService::new(app_ctx.db.clone());

        let (view, event) = flex::update_schema_with_event(
            &service,
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
    ///
    /// Requires `flex_schemas:delete`.
    async fn delete_flex_schema(&self, ctx: &Context<'_>, id: Uuid) -> Result<DeleteFlexPayload> {
        let auth = require_permission(ctx, Permission::FLEX_SCHEMAS_DELETE)?;
        let tenant = ctx.data::<TenantContext>()?;
        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;
        let service = FlexStandaloneSeaOrmService::new(app_ctx.db.clone());

        let event = flex::delete_schema_with_event(&service, tenant.id, Some(auth.user_id), id)
            .await
            .map_err(map_flex_error)?;

        publish_event(ctx, event);
        Ok(DeleteFlexPayload { success: true })
    }

    /// Create a standalone Flex entry.
    ///
    /// Requires `flex_entries:create`.
    async fn create_flex_entry(
        &self,
        ctx: &Context<'_>,
        input: CreateFlexEntryInput,
    ) -> Result<FlexEntryObject> {
        let auth = require_permission(ctx, Permission::FLEX_ENTRIES_CREATE)?;
        let tenant = ctx.data::<TenantContext>()?;
        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;
        let service = FlexStandaloneSeaOrmService::new(app_ctx.db.clone());

        let (view, event) = flex::create_entry_with_event(
            &service,
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
    ///
    /// Requires `flex_entries:update`.
    async fn update_flex_entry(
        &self,
        ctx: &Context<'_>,
        schema_id: Uuid,
        id: Uuid,
        input: UpdateFlexEntryInput,
    ) -> Result<FlexEntryObject> {
        let auth = require_permission(ctx, Permission::FLEX_ENTRIES_UPDATE)?;
        let tenant = ctx.data::<TenantContext>()?;
        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;
        let service = FlexStandaloneSeaOrmService::new(app_ctx.db.clone());

        let (view, event) = flex::update_entry_with_event(
            &service,
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
    ///
    /// Requires `flex_entries:delete`.
    async fn delete_flex_entry(
        &self,
        ctx: &Context<'_>,
        schema_id: Uuid,
        id: Uuid,
    ) -> Result<DeleteFlexPayload> {
        let auth = require_permission(ctx, Permission::FLEX_ENTRIES_DELETE)?;
        let tenant = ctx.data::<TenantContext>()?;
        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;
        let service = FlexStandaloneSeaOrmService::new(app_ctx.db.clone());

        let event =
            flex::delete_entry_with_event(&service, tenant.id, Some(auth.user_id), schema_id, id)
                .await
                .map_err(map_flex_error)?;

        publish_event(ctx, event);
        Ok(DeleteFlexPayload { success: true })
    }
}

fn parse_fields_config(
    value: serde_json::Value,
) -> Result<Vec<rustok_core::field_schema::FieldDefinition>> {
    flex::parse_field_definitions_config(value).map_err(|error| bad_user_input(error.message()))
}

async fn invalidate_field_def_cache(ctx: &Context<'_>, tenant_id: Uuid, entity_type: &str) {
    if let Ok(cache) = ctx.data::<FieldDefinitionCache>() {
        flex::invalidate_field_definition_cache(cache, tenant_id, entity_type).await;
    }
}

/// Fire-and-forget event publishing: errors are logged but not propagated.
fn publish_event(ctx: &Context<'_>, event: EventEnvelope) {
    if let Ok(app_ctx) = ctx.data::<loco_rs::prelude::AppContext>() {
        let bus = event_bus_from_context(app_ctx);
        if let Err(e) = bus.publish_envelope(event) {
            tracing::warn!(error = %e, "Failed to publish flex event");
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::context::{AuthContext, TenantContext};
    use crate::graphql::flex::{auth_has_permission, FlexMutation, FlexQuery};
    use crate::models::tenants;
    use crate::services::field_definition_cache::FieldDefinitionCache;
    use crate::services::field_definition_registry_bootstrap::build_field_def_registry;
    use async_graphql::{EmptySubscription, Request, Schema, Variables};
    use loco_rs::{
        app::{AppContext, SharedStore},
        cache,
        environment::Environment,
        storage::{self, Storage},
        tests_cfg::config::test_config,
    };
    use rustok_core::Permission;
    use rustok_test_utils::db::setup_test_db;
    use sea_orm::{ActiveModelTrait, ConnectionTrait, DatabaseConnection, DbBackend, Set};
    use sea_orm_migration::SchemaManager;
    use serde_json::json;
    use std::sync::Arc;
    use uuid::Uuid;

    type FlexTestSchema = Schema<FlexQuery, FlexMutation, EmptySubscription>;

    fn test_app_context(db: DatabaseConnection) -> AppContext {
        AppContext {
            environment: Environment::Test,
            db,
            queue_provider: None,
            config: test_config(),
            mailer: None,
            storage: Storage::single(storage::drivers::mem::new()).into(),
            cache: Arc::new(cache::Cache::new(cache::drivers::null::new())),
            shared_store: Arc::new(SharedStore::default()),
        }
    }

    fn auth_context(tenant_id: Uuid, permissions: Vec<Permission>) -> AuthContext {
        AuthContext {
            user_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions,
            client_id: None,
            scopes: Vec::new(),
            grant_type: "direct".to_string(),
        }
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

    fn build_test_schema(
        app_ctx: AppContext,
        tenant: TenantContext,
        auth: AuthContext,
    ) -> FlexTestSchema {
        Schema::build(FlexQuery, FlexMutation, EmptySubscription)
            .data(app_ctx)
            .data(tenant)
            .data(auth)
            .data(build_field_def_registry())
            .data(FieldDefinitionCache::new())
            .finish()
    }

    async fn setup_flex_graphql_test_db() -> DatabaseConnection {
        use crate::models::_entities::{
            flex_entries, flex_entry_localized_values, flex_schema_translations, flex_schemas,
        };

        let db = setup_test_db().await;
        let builder = db.get_database_backend();
        assert_eq!(builder, DbBackend::Sqlite, "expected sqlite test backend");
        let schema = sea_orm::Schema::new(builder);
        let manager = SchemaManager::new(&db);

        ensure_entity_table(
            &db,
            &builder,
            schema.create_table_from_entity(tenants::Entity),
            "tenants",
        )
        .await;
        rustok_core::field_schema::create_field_definitions_table(&manager, "user", "users")
            .await
            .expect("create user_field_definitions for flex graphql tests");
        ensure_entity_table(
            &db,
            &builder,
            schema.create_table_from_entity(flex_schemas::Entity),
            "flex_schemas",
        )
        .await;
        ensure_entity_table(
            &db,
            &builder,
            schema.create_table_from_entity(flex_entries::Entity),
            "flex_entries",
        )
        .await;
        ensure_entity_table(
            &db,
            &builder,
            schema.create_table_from_entity(flex_schema_translations::Entity),
            "flex_schema_translations",
        )
        .await;
        ensure_entity_table(
            &db,
            &builder,
            schema.create_table_from_entity(flex_entry_localized_values::Entity),
            "flex_entry_localized_values",
        )
        .await;

        db
    }

    async fn ensure_entity_table(
        db: &DatabaseConnection,
        builder: &DbBackend,
        mut statement: sea_orm::sea_query::TableCreateStatement,
        table_name: &str,
    ) {
        statement.if_not_exists();
        db.execute(builder.build(&statement))
            .await
            .unwrap_or_else(|error| panic!("create {table_name} for flex graphql tests: {error}"));
    }

    #[test]
    fn explicit_flex_permission_is_accepted() {
        let auth = auth_context(Uuid::new_v4(), vec![Permission::FLEX_SCHEMAS_UPDATE]);
        assert!(auth_has_permission(&auth, Permission::FLEX_SCHEMAS_UPDATE));
    }

    #[test]
    fn manage_permission_grants_delete() {
        let auth = auth_context(Uuid::new_v4(), vec![Permission::FLEX_SCHEMAS_MANAGE]);
        assert!(auth_has_permission(&auth, Permission::FLEX_SCHEMAS_DELETE));
    }

    #[test]
    fn missing_permission_is_rejected() {
        let auth = auth_context(Uuid::new_v4(), vec![Permission::FLEX_SCHEMAS_UPDATE]);
        assert!(!auth_has_permission(&auth, Permission::FLEX_SCHEMAS_DELETE));
    }

    #[tokio::test]
    async fn graphql_field_definition_crud_roundtrip_uses_live_registry_routing() {
        let db = setup_flex_graphql_test_db().await;
        let app_ctx = test_app_context(db.clone());
        let mut tenant = tenants::ActiveModel::new("Flex Tenant", "flex-graphql");
        tenant.default_locale = Set("en".to_string());
        let tenant = tenant
            .insert(&db)
            .await
            .expect("tenant should insert for graphql flex tests");
        let tenant_ctx = tenant_context(&tenant);

        let schema = build_test_schema(
            app_ctx,
            tenant_ctx.clone(),
            auth_context(
                tenant.id,
                vec![
                    Permission::FLEX_SCHEMAS_CREATE,
                    Permission::FLEX_SCHEMAS_READ,
                    Permission::FLEX_SCHEMAS_UPDATE,
                    Permission::FLEX_SCHEMAS_DELETE,
                    Permission::FLEX_SCHEMAS_LIST,
                ],
            ),
        );

        let create_first = schema
            .execute(
                Request::new(
                    r#"
                        mutation($input: CreateFieldDefinitionInput!) {
                            createFieldDefinition(input: $input) {
                                id
                                fieldKey
                                position
                                isActive
                                isLocalized
                            }
                        }
                    "#,
                )
                .variables(Variables::from_json(json!({
                    "input": {
                        "entityType": "user",
                        "fieldKey": "nickname",
                        "fieldType": "text",
                        "label": { "en": "Nickname" },
                        "isLocalized": false,
                        "isRequired": false
                    }
                }))),
            )
            .await;
        assert!(
            create_first.errors.is_empty(),
            "create first field failed: {:?}",
            create_first.errors
        );
        let create_first_json = create_first
            .data
            .into_json()
            .expect("create first response should serialize");
        let first_id = create_first_json["createFieldDefinition"]["id"]
            .as_str()
            .expect("first field id should be present")
            .to_string();
        assert_eq!(
            create_first_json["createFieldDefinition"]["fieldKey"],
            "nickname"
        );
        assert_eq!(create_first_json["createFieldDefinition"]["position"], 0);

        let create_second = schema
            .execute(
                Request::new(
                    r#"
                        mutation($input: CreateFieldDefinitionInput!) {
                            createFieldDefinition(input: $input) {
                                id
                                fieldKey
                                position
                                isLocalized
                            }
                        }
                    "#,
                )
                .variables(Variables::from_json(json!({
                    "input": {
                        "entityType": "user",
                        "fieldKey": "bio",
                        "fieldType": "text",
                        "label": { "en": "Bio" },
                        "isLocalized": true,
                        "isRequired": false
                    }
                }))),
            )
            .await;
        assert!(
            create_second.errors.is_empty(),
            "create second field failed: {:?}",
            create_second.errors
        );
        let create_second_json = create_second
            .data
            .into_json()
            .expect("create second response should serialize");
        let second_id = create_second_json["createFieldDefinition"]["id"]
            .as_str()
            .expect("second field id should be present")
            .to_string();
        assert_eq!(
            create_second_json["createFieldDefinition"]["fieldKey"],
            "bio"
        );
        assert_eq!(create_second_json["createFieldDefinition"]["position"], 1);
        assert_eq!(
            create_second_json["createFieldDefinition"]["isLocalized"],
            true
        );

        let list_before_update = schema
            .execute(Request::new(
                r#"
                    query {
                        fieldDefinitions(entityType: "user", pagination: { offset: 0, limit: 10 }) {
                            id
                            fieldKey
                            position
                            isActive
                        }
                    }
                "#,
            ))
            .await;
        assert!(
            list_before_update.errors.is_empty(),
            "list before update failed: {:?}",
            list_before_update.errors
        );
        let list_before_update_json = list_before_update
            .data
            .into_json()
            .expect("list response should serialize");
        assert_eq!(
            list_before_update_json["fieldDefinitions"]
                .as_array()
                .expect("fieldDefinitions should be an array")
                .len(),
            2
        );
        assert_eq!(
            list_before_update_json["fieldDefinitions"][0]["fieldKey"],
            "nickname"
        );
        assert_eq!(
            list_before_update_json["fieldDefinitions"][1]["fieldKey"],
            "bio"
        );

        let find_second = schema
            .execute(
                Request::new(
                    r#"
                        query($id: UUID!) {
                            fieldDefinition(entityType: "user", id: $id) {
                                id
                                fieldKey
                                isLocalized
                            }
                        }
                    "#,
                )
                .variables(Variables::from_json(json!({ "id": second_id }))),
            )
            .await;
        assert!(
            find_second.errors.is_empty(),
            "find second field failed: {:?}",
            find_second.errors
        );
        let find_second_json = find_second
            .data
            .into_json()
            .expect("find response should serialize");
        assert_eq!(find_second_json["fieldDefinition"]["fieldKey"], "bio");
        assert_eq!(find_second_json["fieldDefinition"]["isLocalized"], true);

        let update_first = schema
            .execute(
                Request::new(
                    r#"
                        mutation($id: UUID!, $input: UpdateFieldDefinitionInput!) {
                            updateFieldDefinition(id: $id, input: $input) {
                                id
                                fieldKey
                                isLocalized
                                position
                                label
                            }
                        }
                    "#,
                )
                .variables(Variables::from_json(json!({
                    "id": first_id,
                    "input": {
                        "entityType": "user",
                        "label": { "en": "Display name" },
                        "isLocalized": true,
                        "position": 4
                    }
                }))),
            )
            .await;
        assert!(
            update_first.errors.is_empty(),
            "update first field failed: {:?}",
            update_first.errors
        );
        let update_first_json = update_first
            .data
            .into_json()
            .expect("update response should serialize");
        assert_eq!(
            update_first_json["updateFieldDefinition"]["fieldKey"],
            "nickname"
        );
        assert_eq!(
            update_first_json["updateFieldDefinition"]["isLocalized"],
            true
        );
        assert_eq!(update_first_json["updateFieldDefinition"]["position"], 4);
        assert_eq!(
            update_first_json["updateFieldDefinition"]["label"],
            json!({ "en": "Display name" })
        );

        let reorder = schema
            .execute(
                Request::new(
                    r#"
                        mutation($ids: [UUID!]!) {
                            reorderFieldDefinitions(entityType: "user", ids: $ids) {
                                id
                                fieldKey
                                position
                            }
                        }
                    "#,
                )
                .variables(Variables::from_json(json!({
                    "ids": [second_id, first_id]
                }))),
            )
            .await;
        assert!(
            reorder.errors.is_empty(),
            "reorder fields failed: {:?}",
            reorder.errors
        );
        let reorder_json = reorder
            .data
            .into_json()
            .expect("reorder response should serialize");
        assert_eq!(
            reorder_json["reorderFieldDefinitions"][0]["fieldKey"],
            "bio"
        );
        assert_eq!(reorder_json["reorderFieldDefinitions"][0]["position"], 0);
        assert_eq!(
            reorder_json["reorderFieldDefinitions"][1]["fieldKey"],
            "nickname"
        );
        assert_eq!(reorder_json["reorderFieldDefinitions"][1]["position"], 1);

        let delete_second = schema
            .execute(
                Request::new(
                    r#"
                        mutation($id: UUID!) {
                            deleteFieldDefinition(entityType: "user", id: $id) {
                                success
                            }
                        }
                    "#,
                )
                .variables(Variables::from_json(json!({ "id": second_id }))),
            )
            .await;
        assert!(
            delete_second.errors.is_empty(),
            "delete second field failed: {:?}",
            delete_second.errors
        );
        let delete_second_json = delete_second
            .data
            .into_json()
            .expect("delete response should serialize");
        assert_eq!(delete_second_json["deleteFieldDefinition"]["success"], true);

        let list_after_delete = schema
            .execute(Request::new(
                r#"
                    query {
                        fieldDefinitions(entityType: "user", pagination: { offset: 0, limit: 10 }) {
                            fieldKey
                            position
                            isActive
                        }
                    }
                "#,
            ))
            .await;
        assert!(
            list_after_delete.errors.is_empty(),
            "list after delete failed: {:?}",
            list_after_delete.errors
        );
        let list_after_delete_json = list_after_delete
            .data
            .into_json()
            .expect("list after delete response should serialize");
        assert_eq!(
            list_after_delete_json["fieldDefinitions"][0]["fieldKey"],
            "bio"
        );
        assert_eq!(
            list_after_delete_json["fieldDefinitions"][0]["isActive"],
            false
        );
        assert_eq!(
            list_after_delete_json["fieldDefinitions"][1]["fieldKey"],
            "nickname"
        );
        assert_eq!(
            list_after_delete_json["fieldDefinitions"][1]["isActive"],
            true
        );
    }

    #[tokio::test]
    async fn create_field_definition_requires_explicit_flex_permission() {
        let db = setup_flex_graphql_test_db().await;
        let app_ctx = test_app_context(db.clone());
        let tenant = tenants::ActiveModel::new("Flex Tenant", "flex-graphql-denied")
            .insert(&db)
            .await
            .expect("tenant should insert for denied graphql test");
        let schema = build_test_schema(
            app_ctx,
            tenant_context(&tenant),
            auth_context(tenant.id, vec![Permission::FLEX_SCHEMAS_READ]),
        );

        let response = schema
            .execute(
                Request::new(
                    r#"
                        mutation($input: CreateFieldDefinitionInput!) {
                            createFieldDefinition(input: $input) {
                                id
                            }
                        }
                    "#,
                )
                .variables(Variables::from_json(json!({
                    "input": {
                        "entityType": "user",
                        "fieldKey": "nickname",
                        "fieldType": "text",
                        "label": { "en": "Nickname" }
                    }
                }))),
            )
            .await;

        assert_eq!(response.errors.len(), 1);
        assert!(
            response.errors[0].message.contains("required"),
            "unexpected error message: {}",
            response.errors[0].message
        );
    }

    #[tokio::test]
    async fn field_definitions_query_requires_explicit_flex_list_permission() {
        let db = setup_flex_graphql_test_db().await;
        let app_ctx = test_app_context(db.clone());
        let tenant = tenants::ActiveModel::new("Flex Tenant", "flex-graphql-list-denied")
            .insert(&db)
            .await
            .expect("tenant should insert for denied list test");

        let admin_schema = build_test_schema(
            app_ctx.clone(),
            tenant_context(&tenant),
            auth_context(
                tenant.id,
                vec![
                    Permission::FLEX_SCHEMAS_CREATE,
                    Permission::FLEX_SCHEMAS_LIST,
                ],
            ),
        );
        let create_response = admin_schema
            .execute(
                Request::new(
                    r#"
                        mutation($input: CreateFieldDefinitionInput!) {
                            createFieldDefinition(input: $input) {
                                id
                            }
                        }
                    "#,
                )
                .variables(Variables::from_json(json!({
                    "input": {
                        "entityType": "user",
                        "fieldKey": "nickname",
                        "fieldType": "text",
                        "label": { "en": "Nickname" }
                    }
                }))),
            )
            .await;
        assert!(
            create_response.errors.is_empty(),
            "field setup should succeed: {:?}",
            create_response.errors
        );

        let denied_schema = build_test_schema(
            app_ctx,
            tenant_context(&tenant),
            auth_context(tenant.id, vec![Permission::FLEX_SCHEMAS_READ]),
        );
        let response = denied_schema
            .execute(Request::new(
                r#"
                    query {
                        fieldDefinitions(entityType: "user", pagination: { offset: 0, limit: 10 }) {
                            id
                        }
                    }
                "#,
            ))
            .await;

        assert_eq!(response.errors.len(), 1);
        assert!(
            response.errors[0].message.contains("required"),
            "unexpected error message: {}",
            response.errors[0].message
        );
    }

    #[tokio::test]
    async fn field_definition_query_requires_explicit_flex_read_permission() {
        let db = setup_flex_graphql_test_db().await;
        let app_ctx = test_app_context(db.clone());
        let tenant = tenants::ActiveModel::new("Flex Tenant", "flex-graphql-read-denied")
            .insert(&db)
            .await
            .expect("tenant should insert for denied read test");

        let admin_schema = build_test_schema(
            app_ctx.clone(),
            tenant_context(&tenant),
            auth_context(
                tenant.id,
                vec![
                    Permission::FLEX_SCHEMAS_CREATE,
                    Permission::FLEX_SCHEMAS_READ,
                ],
            ),
        );
        let create_response = admin_schema
            .execute(
                Request::new(
                    r#"
                        mutation($input: CreateFieldDefinitionInput!) {
                            createFieldDefinition(input: $input) {
                                id
                            }
                        }
                    "#,
                )
                .variables(Variables::from_json(json!({
                    "input": {
                        "entityType": "user",
                        "fieldKey": "nickname",
                        "fieldType": "text",
                        "label": { "en": "Nickname" }
                    }
                }))),
            )
            .await;
        assert!(
            create_response.errors.is_empty(),
            "field setup should succeed: {:?}",
            create_response.errors
        );
        let create_json = create_response
            .data
            .into_json()
            .expect("create response should serialize");
        let field_id = create_json["createFieldDefinition"]["id"]
            .as_str()
            .expect("field id should be present")
            .to_string();

        let denied_schema = build_test_schema(
            app_ctx,
            tenant_context(&tenant),
            auth_context(tenant.id, vec![Permission::FLEX_SCHEMAS_LIST]),
        );
        let response = denied_schema
            .execute(
                Request::new(
                    r#"
                        query($id: UUID!) {
                            fieldDefinition(entityType: "user", id: $id) {
                                id
                            }
                        }
                    "#,
                )
                .variables(Variables::from_json(json!({ "id": field_id }))),
            )
            .await;

        assert_eq!(response.errors.len(), 1);
        assert!(
            response.errors[0].message.contains("required"),
            "unexpected error message: {}",
            response.errors[0].message
        );
    }

    #[tokio::test]
    async fn graphql_standalone_flex_schema_and_entry_roundtrip() {
        let db = setup_flex_graphql_test_db().await;
        let app_ctx = test_app_context(db.clone());
        let mut tenant = tenants::ActiveModel::new("Flex Tenant", "flex-standalone-graphql");
        tenant.default_locale = Set("ru".to_string());
        let tenant = tenant
            .insert(&db)
            .await
            .expect("tenant should insert for standalone graphql test");

        let schema = build_test_schema(
            app_ctx,
            tenant_context(&tenant),
            auth_context(
                tenant.id,
                vec![
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
            ),
        );

        let create_schema = schema
            .execute(
                Request::new(
                    r#"
                        mutation($input: CreateFlexSchemaInput!) {
                            createFlexSchema(input: $input) {
                                id
                                slug
                                name
                                description
                                isActive
                            }
                        }
                    "#,
                )
                .variables(Variables::from_json(json!({
                    "input": {
                        "slug": "landing_page",
                        "name": "Лендинг",
                        "description": "Описание",
                        "fieldsConfig": [
                            {
                                "fieldKey": "slug",
                                "fieldType": "text",
                                "label": { "en": "Slug" },
                                "isLocalized": false,
                                "isRequired": true
                            },
                            {
                                "fieldKey": "title",
                                "fieldType": "text",
                                "label": { "en": "Title" },
                                "isLocalized": true,
                                "isRequired": false
                            }
                        ],
                        "settings": { "layout": "hero" },
                        "isActive": true
                    }
                }))),
            )
            .await;
        assert!(
            create_schema.errors.is_empty(),
            "create standalone schema failed: {:?}",
            create_schema.errors
        );
        let create_schema_json = create_schema
            .data
            .into_json()
            .expect("create schema response should serialize");
        let schema_id = create_schema_json["createFlexSchema"]["id"]
            .as_str()
            .expect("schema id should be present")
            .to_string();
        assert_eq!(
            create_schema_json["createFlexSchema"]["slug"],
            "landing_page"
        );
        assert_eq!(create_schema_json["createFlexSchema"]["name"], "Лендинг");

        let list_schemas = schema
            .execute(Request::new(
                r#"
                    query {
                        flexSchemas {
                            id
                            slug
                            name
                        }
                    }
                "#,
            ))
            .await;
        assert!(
            list_schemas.errors.is_empty(),
            "list standalone schemas failed: {:?}",
            list_schemas.errors
        );
        let list_schemas_json = list_schemas
            .data
            .into_json()
            .expect("list schemas response should serialize");
        assert_eq!(
            list_schemas_json["flexSchemas"]
                .as_array()
                .expect("flexSchemas should be an array")
                .len(),
            1
        );

        let get_schema = schema
            .execute(
                Request::new(
                    r#"
                        query($id: UUID!) {
                            flexSchema(id: $id) {
                                id
                                slug
                                settings
                            }
                        }
                    "#,
                )
                .variables(Variables::from_json(json!({ "id": schema_id }))),
            )
            .await;
        assert!(
            get_schema.errors.is_empty(),
            "find standalone schema failed: {:?}",
            get_schema.errors
        );
        let get_schema_json = get_schema
            .data
            .into_json()
            .expect("get schema response should serialize");
        assert_eq!(get_schema_json["flexSchema"]["slug"], "landing_page");
        assert_eq!(
            get_schema_json["flexSchema"]["settings"],
            json!({"layout": "hero"})
        );

        let create_entry = schema
            .execute(
                Request::new(
                    r#"
                        mutation($input: CreateFlexEntryInput!) {
                            createFlexEntry(input: $input) {
                                id
                                schemaId
                                data
                                status
                            }
                        }
                    "#,
                )
                .variables(Variables::from_json(json!({
                    "input": {
                        "schemaId": schema_id,
                        "data": { "slug": "landing", "title": "Привет" },
                        "status": "draft"
                    }
                }))),
            )
            .await;
        assert!(
            create_entry.errors.is_empty(),
            "create standalone entry failed: {:?}",
            create_entry.errors
        );
        let create_entry_json = create_entry
            .data
            .into_json()
            .expect("create entry response should serialize");
        let entry_id = create_entry_json["createFlexEntry"]["id"]
            .as_str()
            .expect("entry id should be present")
            .to_string();
        assert_eq!(create_entry_json["createFlexEntry"]["status"], "draft");

        let update_entry = schema
            .execute(
                Request::new(
                    r#"
                        mutation($schemaId: UUID!, $id: UUID!, $input: UpdateFlexEntryInput!) {
                            updateFlexEntry(schemaId: $schemaId, id: $id, input: $input) {
                                id
                                data
                                status
                            }
                        }
                    "#,
                )
                .variables(Variables::from_json(json!({
                    "schemaId": schema_id,
                    "id": entry_id,
                    "input": {
                        "data": { "slug": "landing", "title": "Здравствуйте" },
                        "status": "published"
                    }
                }))),
            )
            .await;
        assert!(
            update_entry.errors.is_empty(),
            "update standalone entry failed: {:?}",
            update_entry.errors
        );
        let update_entry_json = update_entry
            .data
            .into_json()
            .expect("update entry response should serialize");
        assert_eq!(update_entry_json["updateFlexEntry"]["status"], "published");
        assert_eq!(
            update_entry_json["updateFlexEntry"]["data"],
            json!({"slug": "landing", "title": "Здравствуйте"})
        );

        let list_entries = schema
            .execute(
                Request::new(
                    r#"
                        query($schemaId: UUID!) {
                            flexEntries(schemaId: $schemaId) {
                                id
                                status
                            }
                        }
                    "#,
                )
                .variables(Variables::from_json(json!({ "schemaId": schema_id }))),
            )
            .await;
        assert!(
            list_entries.errors.is_empty(),
            "list standalone entries failed: {:?}",
            list_entries.errors
        );
        let list_entries_json = list_entries
            .data
            .into_json()
            .expect("list entries response should serialize");
        assert_eq!(
            list_entries_json["flexEntries"]
                .as_array()
                .expect("flexEntries should be an array")
                .len(),
            1
        );

        let get_entry = schema
            .execute(
                Request::new(
                    r#"
                        query($schemaId: UUID!, $id: UUID!) {
                            flexEntry(schemaId: $schemaId, id: $id) {
                                id
                                status
                                data
                            }
                        }
                    "#,
                )
                .variables(Variables::from_json(json!({
                    "schemaId": schema_id,
                    "id": entry_id
                }))),
            )
            .await;
        assert!(
            get_entry.errors.is_empty(),
            "find standalone entry failed: {:?}",
            get_entry.errors
        );
        let get_entry_json = get_entry
            .data
            .into_json()
            .expect("get entry response should serialize");
        assert_eq!(get_entry_json["flexEntry"]["status"], "published");

        let delete_entry = schema
            .execute(
                Request::new(
                    r#"
                        mutation($schemaId: UUID!, $id: UUID!) {
                            deleteFlexEntry(schemaId: $schemaId, id: $id) {
                                success
                            }
                        }
                    "#,
                )
                .variables(Variables::from_json(json!({
                    "schemaId": schema_id,
                    "id": entry_id
                }))),
            )
            .await;
        assert!(
            delete_entry.errors.is_empty(),
            "delete standalone entry failed: {:?}",
            delete_entry.errors
        );
        let delete_entry_json = delete_entry
            .data
            .into_json()
            .expect("delete entry response should serialize");
        assert_eq!(delete_entry_json["deleteFlexEntry"]["success"], true);

        let delete_schema = schema
            .execute(
                Request::new(
                    r#"
                        mutation($id: UUID!) {
                            deleteFlexSchema(id: $id) {
                                success
                            }
                        }
                    "#,
                )
                .variables(Variables::from_json(json!({ "id": schema_id }))),
            )
            .await;
        assert!(
            delete_schema.errors.is_empty(),
            "delete standalone schema failed: {:?}",
            delete_schema.errors
        );
        let delete_schema_json = delete_schema
            .data
            .into_json()
            .expect("delete schema response should serialize");
        assert_eq!(delete_schema_json["deleteFlexSchema"]["success"], true);
    }

    #[tokio::test]
    async fn create_flex_entry_requires_explicit_entry_permission() {
        let db = setup_flex_graphql_test_db().await;
        let app_ctx = test_app_context(db.clone());
        let tenant = tenants::ActiveModel::new("Flex Tenant", "flex-standalone-entry-denied")
            .insert(&db)
            .await
            .expect("tenant should insert for standalone entry denial test");

        let admin_schema = build_test_schema(
            app_ctx.clone(),
            tenant_context(&tenant),
            auth_context(
                tenant.id,
                vec![
                    Permission::FLEX_SCHEMAS_CREATE,
                    Permission::FLEX_SCHEMAS_READ,
                    Permission::FLEX_ENTRIES_READ,
                ],
            ),
        );
        let create_schema = admin_schema
            .execute(
                Request::new(
                    r#"
                        mutation($input: CreateFlexSchemaInput!) {
                            createFlexSchema(input: $input) {
                                id
                            }
                        }
                    "#,
                )
                .variables(Variables::from_json(json!({
                    "input": {
                        "slug": "landing_page",
                        "name": "Лендинг",
                        "fieldsConfig": [
                            {
                                "fieldKey": "slug",
                                "fieldType": "text",
                                "label": { "en": "Slug" },
                                "isLocalized": false,
                                "isRequired": true
                            }
                        ]
                    }
                }))),
            )
            .await;
        assert!(
            create_schema.errors.is_empty(),
            "schema setup should succeed: {:?}",
            create_schema.errors
        );
        let create_schema_json = create_schema
            .data
            .into_json()
            .expect("create schema response should serialize");
        let schema_id = create_schema_json["createFlexSchema"]["id"]
            .as_str()
            .expect("schema id should be present")
            .to_string();

        let denied_schema = build_test_schema(
            app_ctx,
            tenant_context(&tenant),
            auth_context(tenant.id, vec![Permission::FLEX_ENTRIES_READ]),
        );
        let response = denied_schema
            .execute(
                Request::new(
                    r#"
                        mutation($input: CreateFlexEntryInput!) {
                            createFlexEntry(input: $input) {
                                id
                            }
                        }
                    "#,
                )
                .variables(Variables::from_json(json!({
                    "input": {
                        "schemaId": schema_id,
                        "data": { "slug": "landing" },
                        "status": "draft"
                    }
                }))),
            )
            .await;

        assert_eq!(response.errors.len(), 1);
        assert!(
            response.errors[0].message.contains("required"),
            "unexpected error message: {}",
            response.errors[0].message
        );
    }
}
