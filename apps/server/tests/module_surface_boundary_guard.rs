use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("apps/server should live under workspace root")
        .to_path_buf()
}

#[test]
fn optional_module_transport_shims_do_not_live_in_server() {
    let root = repo_root().join("apps/server/src");
    let forbidden_paths = [
        "controllers/blog",
        "controllers/commerce",
        "controllers/forum",
        "controllers/media",
        "controllers/pages.rs",
        "controllers/workflow",
        "graphql/blog",
        "graphql/commerce",
        "graphql/forum",
        "graphql/media",
        "graphql/pages",
        "graphql/workflow",
    ];

    for relative in forbidden_paths {
        let path = root.join(relative);
        assert!(
            !path.exists(),
            "optional module transport surface must be owned by its module crate, not apps/server: {}",
            path.display()
        );
    }
}

#[test]
fn module_route_codegen_prefers_owner_crates_over_server_wrappers() {
    let build_rs = std::fs::read_to_string(repo_root().join("apps/server/build.rs"))
        .expect("server build.rs should be readable");

    assert!(
        !build_rs.contains("crate::controllers::{slug}::routes()"),
        "route codegen must not prefer apps/server controller wrappers for optional modules"
    );
    assert!(
        !build_rs.contains("crate::controllers::{slug}::webhook_routes()"),
        "webhook route codegen must not prefer apps/server controller wrappers for optional modules"
    );
}

#[test]
fn module_graphql_codegen_uses_manifest_as_source_of_truth() {
    let build_rs = std::fs::read_to_string(repo_root().join("apps/server/build.rs"))
        .expect("server build.rs should be readable");

    assert!(
        build_rs.matches("!has_package_manifest && has_any").count() >= 2,
        "GraphQL query/mutation entrypoints must not be inferred when rustok-module.toml exists"
    );
}

#[test]
fn optional_module_openapi_definitions_do_not_live_in_server() {
    let swagger =
        std::fs::read_to_string(repo_root().join("apps/server/src/controllers/swagger.rs"))
            .expect("server swagger controller should be readable");

    for forbidden in [
        "rustok_blog::controllers",
        "rustok_forum::controllers",
        "rustok_pages::controllers",
        "rustok_commerce::controllers",
        "rustok_media::controllers",
        "rustok_workflow::controllers",
        "rustok_product::dto::CreateProductInput",
        "rustok_cart::dto::CartResponse",
    ] {
        assert!(
            !swagger.contains(forbidden),
            "module-owned OpenAPI paths/components must be defined in owner crates, not apps/server: {forbidden}"
        );
    }
}

#[test]
fn content_orchestration_bridge_does_not_live_in_server() {
    let server_impl = repo_root().join("apps/server/src/services/content_orchestration.rs");
    assert!(
        !server_impl.exists(),
        "content orchestration bridge is cross-module domain logic and must live outside apps/server"
    );

    let services_dir = repo_root().join("apps/server/src/services");
    let mut offenders = Vec::new();
    for entry in std::fs::read_dir(&services_dir).expect("server services dir should be readable") {
        let entry = entry.expect("server service entry should be readable");
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("server service source should read");
        for forbidden in [
            "rustok_blog::",
            "rustok_forum::",
            "rustok_taxonomy::",
            "rustok_comments::",
        ] {
            if source.contains(forbidden) {
                offenders.push(format!("{} contains {forbidden}", path.display()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "server services must not own content/blog/forum orchestration internals: {offenders:?}"
    );
}

#[test]
fn module_owned_graphql_types_and_resolvers_do_not_live_in_server() {
    let graphql_dir = repo_root().join("apps/server/src/graphql");
    assert!(
        !graphql_dir.join("common.rs").exists(),
        "shared GraphQL helpers must be imported from rustok-api, not re-exported by apps/server"
    );
    assert!(
        !graphql_dir.join("errors.rs").exists(),
        "shared GraphQL errors must be imported from rustok-api, not re-exported by apps/server"
    );
    assert!(
        !graphql_dir.join("oauth").exists(),
        "OAuth GraphQL query/mutation/types must live in rustok-auth, not apps/server"
    );
    assert!(
        !graphql_dir.join("auth").exists(),
        "Auth lifecycle GraphQL query/mutation/types must live in rustok-auth, not apps/server"
    );
    assert!(
        !graphql_dir.join("mcp").exists(),
        "MCP GraphQL query/mutation/types must live in rustok-mcp, not apps/server"
    );
    assert!(
        !graphql_dir.join("connection.rs").exists(),
        "module-specific concrete GraphQL connections must live in owner crates"
    );

    let forbidden = [
        ("queries.rs", "resolve_canonical_route"),
        ("mutations.rs", "promote_topic_to_post"),
        ("mutations.rs", "demote_post_to_topic"),
        ("mutations.rs", "split_topic"),
        ("mutations.rs", "merge_topics"),
        ("types.rs", "ResolvedCanonicalRoute"),
        ("types.rs", "ContentOrchestrationPayload"),
    ];

    for (file, symbol) in forbidden {
        let path = graphql_dir.join(file);
        let source = std::fs::read_to_string(&path).expect("server GraphQL source should read");
        assert!(
            !source.contains(symbol),
            "module-owned GraphQL symbol {symbol} must not live in {}",
            path.display()
        );
    }
}

#[test]
fn content_graphql_entity_loaders_do_not_live_in_server() {
    let graphql_dir = repo_root().join("apps/server/src/graphql");
    let forbidden = [
        "rustok_content::entities",
        "struct NodeLoader",
        "struct NodeTranslationLoader",
        "struct NodeBodyLoader",
    ];

    let mut offenders = Vec::new();
    for entry in std::fs::read_dir(&graphql_dir).expect("server GraphQL dir should be readable") {
        let entry = entry.expect("server GraphQL entry should be readable");
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("server GraphQL source should read");
        for symbol in forbidden {
            if source.contains(symbol) {
                offenders.push(format!("{} contains {symbol}", path.display()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "content GraphQL entity loaders must live in rustok-content, not apps/server: {offenders:?}"
    );
}

#[test]
fn flex_attached_payload_logic_is_owned_by_flex_crate() {
    let repo = repo_root();
    let owner_attached = std::fs::read_to_string(repo.join("crates/flex/src/attached.rs"))
        .expect("owner Flex attached source should read");
    let server_adapter =
        std::fs::read_to_string(repo.join("apps/server/src/services/flex_attached_values.rs"))
            .expect("server Flex attached adapter should read");

    for owner_owned_symbol in [
        "pub fn prepare_attached_values_create",
        "pub async fn prepare_attached_values_update",
        "pub async fn resolve_attached_payload",
        "pub async fn persist_localized_values",
        "pub async fn delete_attached_localized_values",
        "fn prepare_write(",
        "fn split_definitions(",
        "fn split_existing_metadata(",
        "fn split_patch(",
        "fn merge_patch(",
        "fn validate_schema(",
        "pub struct PreparedAttachedValuesWrite",
    ] {
        assert!(
            owner_attached.contains(owner_owned_symbol),
            "missing owner-owned attached Flex helper: {owner_owned_symbol}"
        );
    }

    for delegated_call in [
        "prepare_attached_values_create(schema, payload, locale)",
        "prepare_attached_values_update(",
        "resolve_attached_payload(",
        "persist_localized_values(db, tenant_id, entity_type, entity_id, locale, values).await",
        "delete_attached_localized_values(db, tenant_id, entity_type, entity_id).await",
    ] {
        assert!(
            server_adapter.contains(delegated_call),
            "server attached Flex adapter must delegate to owner helper: {delegated_call}"
        );
    }

    for forbidden in [
        "fn prepare_write(",
        "fn split_definitions(",
        "fn split_existing_metadata(",
        "fn split_patch(",
        "fn merge_patch(",
        "fn validate_schema(",
        "struct PreparedAttachedValuesWrite",
    ] {
        assert!(
            !server_adapter.contains(forbidden),
            "attached Flex payload ownership must live in crates/flex, not apps/server: {forbidden}"
        );
    }
}

#[test]
fn module_entity_imports_do_not_leak_into_server_graphql() {
    let graphql_dir = repo_root().join("apps/server/src/graphql");
    let forbidden = ["rustok_media::media::", "rustok_media::media::{"];

    let mut offenders = Vec::new();
    for entry in std::fs::read_dir(&graphql_dir).expect("server GraphQL dir should be readable") {
        let entry = entry.expect("server GraphQL entry should be readable");
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("server GraphQL source should read");
        for symbol in forbidden {
            if source.contains(symbol) {
                offenders.push(format!("{} contains {symbol}", path.display()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "module entity imports must stay behind owner crate APIs, not apps/server GraphQL: {offenders:?}"
    );
}

#[test]
fn media_usage_graphql_is_owned_by_media_crate() {
    let repo = repo_root();
    let system = std::fs::read_to_string(repo.join("apps/server/src/graphql/system.rs"))
        .expect("server system GraphQL source should read");
    for forbidden in ["rustok_media", "MediaUsageStats", "fn media_usage"] {
        assert!(
            !system.contains(forbidden),
            "media usage GraphQL must live in rustok-media, not server SystemQuery: {forbidden}"
        );
    }

    let media_query =
        std::fs::read_to_string(repo.join("crates/rustok-media/src/graphql/query.rs"))
            .expect("rustok-media GraphQL query should read");
    let media_types =
        std::fs::read_to_string(repo.join("crates/rustok-media/src/graphql/types.rs"))
            .expect("rustok-media GraphQL types should read");
    assert!(media_query.contains("async fn media_usage"));
    assert!(media_types.contains("pub struct MediaUsageStats"));
}

#[test]
fn order_dashboard_snapshot_is_owned_by_order_crate() {
    let repo = repo_root();
    let root_queries = std::fs::read_to_string(repo.join("apps/server/src/graphql/queries.rs"))
        .expect("server root GraphQL queries source should read");
    for forbidden in [
        "struct OrderStatsSnapshot",
        "fn load_order_stats_snapshot",
        "event_type = 'order.placed'",
    ] {
        assert!(
            !root_queries.contains(forbidden),
            "order dashboard snapshot SQL must live in rustok-order, not server RootQuery: {forbidden}"
        );
    }
    assert!(
        root_queries.contains("rustok_order::load_order_stats_snapshot"),
        "server RootQuery should compose the order-owned dashboard snapshot helper"
    );

    let order_analytics =
        std::fs::read_to_string(repo.join("crates/rustok-order/src/analytics.rs"))
            .expect("rustok-order analytics source should read");
    assert!(order_analytics.contains("pub struct OrderStatsSnapshot"));
    assert!(order_analytics.contains("pub async fn load_order_stats_snapshot"));
    assert!(order_analytics.contains("event_type = 'order.placed'"));
}

#[test]
fn content_dashboard_post_snapshot_is_owned_by_content_crate() {
    let repo = repo_root();
    let root_queries = std::fs::read_to_string(repo.join("apps/server/src/graphql/queries.rs"))
        .expect("server root GraphQL queries source should read");
    for forbidden in [
        "fn load_post_stats_snapshot",
        "FROM nodes",
        "AND kind = ?4",
        "AND kind = $4",
    ] {
        assert!(
            !root_queries.contains(forbidden),
            "post dashboard snapshot SQL must live in rustok-content, not server RootQuery: {forbidden}"
        );
    }
    assert!(
        root_queries.contains("rustok_content::load_post_stats_snapshot"),
        "server RootQuery should compose the content-owned post dashboard snapshot helper"
    );

    let content_analytics =
        std::fs::read_to_string(repo.join("crates/rustok-content/src/analytics.rs"))
            .expect("rustok-content analytics source should read");
    assert!(content_analytics.contains("pub struct ContentCountSnapshot"));
    assert!(content_analytics.contains("pub async fn load_post_stats_snapshot"));
    assert!(content_analytics.contains("FROM nodes"));
    assert!(content_analytics.contains("AND kind = $4"));
}

#[test]
fn server_dashboard_user_activity_logic_stays_out_of_graphql_root() {
    let repo = repo_root();
    let root_queries = std::fs::read_to_string(repo.join("apps/server/src/graphql/queries.rs"))
        .expect("server root GraphQL queries source should read");
    for forbidden in [
        "struct PeriodCountSnapshot",
        "fn load_period_count_snapshot",
        "FROM users",
        "recent_users = users::Entity::find()",
    ] {
        assert!(
            !root_queries.contains(forbidden),
            "dashboard user activity read logic must live in server services, not RootQuery: {forbidden}"
        );
    }
    assert!(
        root_queries.contains("dashboard_user_activity::load_user_stats_snapshot"),
        "server RootQuery should compose the user stats service helper"
    );
    assert!(
        root_queries.contains("dashboard_user_activity::load_recent_user_activity"),
        "server RootQuery should compose the recent user activity service helper"
    );

    let service_source =
        std::fs::read_to_string(repo.join("apps/server/src/services/dashboard_user_activity.rs"))
            .expect("dashboard user activity service source should read");
    assert!(service_source.contains("pub async fn load_user_stats_snapshot"));
    assert!(service_source.contains("pub async fn load_recent_user_activity"));
    assert!(service_source.contains("FROM users"));
}

#[test]
fn flex_graphql_surface_is_owned_by_flex_crate() {
    let repo = repo_root();
    assert!(
        !repo
            .join("apps/server/docs/flex-phase45-migration-guide.md")
            .exists(),
        "Flex migration notes must live in the owner docs, not apps/server docs"
    );
    let server_graphql_dir = repo.join("apps/server/src/graphql/flex");
    assert!(
        !server_graphql_dir.exists(),
        "Flex GraphQL query/mutation/types/runtime must live in crates/flex, not apps/server"
    );

    let owner_graphql_dir = repo.join("crates/flex/src/graphql");
    let runtime = std::fs::read_to_string(owner_graphql_dir.join("runtime.rs"))
        .expect("owner Flex GraphQL runtime source should read");
    assert!(runtime.contains("Arc<dyn FlexStandaloneService>"));
    assert!(runtime.contains("FieldDefRegistry"));
    assert!(runtime.contains("FieldDefinitionCachePort"));

    let owner_query = std::fs::read_to_string(owner_graphql_dir.join("query.rs"))
        .expect("owner Flex GraphQL query should read");
    let owner_mutation = std::fs::read_to_string(owner_graphql_dir.join("mutation.rs"))
        .expect("owner Flex GraphQL mutation should read");
    assert!(owner_query.contains("pub struct FlexQuery"));
    assert!(owner_query.contains("async fn field_definitions"));
    assert!(owner_query.contains("async fn field_definition"));
    assert!(owner_query.contains("async fn flex_schemas"));
    assert!(owner_query.contains("async fn flex_entries"));
    assert!(owner_mutation.contains("pub struct FlexMutation"));
    assert!(owner_mutation.contains("async fn create_field_definition"));
    assert!(owner_mutation.contains("async fn update_field_definition"));
    assert!(owner_mutation.contains("async fn delete_field_definition"));
    assert!(owner_mutation.contains("async fn reorder_field_definitions"));
    assert!(owner_mutation.contains("async fn create_flex_schema"));
    assert!(owner_mutation.contains("async fn create_flex_entry"));
    for forbidden in ["crate::context", "crate::services", "apps/server"] {
        assert!(!owner_query.contains(forbidden));
        assert!(!owner_mutation.contains(forbidden));
        assert!(!runtime.contains(forbidden));
    }

    let owner_types = std::fs::read_to_string(owner_graphql_dir.join("types.rs"))
        .expect("owner Flex GraphQL types should read");
    for owner_owned_type in [
        "struct FieldDefinitionObject",
        "struct CreateFieldDefinitionInput",
        "struct UpdateFieldDefinitionInput",
        "struct DeleteFieldDefinitionPayload",
        "struct FlexSchemaObject",
        "struct FlexEntryObject",
        "struct CreateFlexSchemaInput",
        "struct UpdateFlexSchemaInput",
        "struct CreateFlexEntryInput",
        "struct UpdateFlexEntryInput",
        "struct DeleteFlexPayload",
    ] {
        assert!(
            owner_types.contains(owner_owned_type),
            "missing owner-owned standalone Flex GraphQL DTO: {owner_owned_type}"
        );
    }

    let schema = std::fs::read_to_string(repo.join("apps/server/src/graphql/schema.rs"))
        .expect("server GraphQL schema should read");
    assert!(schema.contains("flex::graphql::FlexGraphqlRuntime"));
    assert!(schema.contains("FlexGraphqlRuntime::new"));
    assert!(!schema.contains("super::flex"));
    assert!(!schema.contains("FlexQuery"));
    assert!(!schema.contains("FlexMutation"));
    assert!(schema.contains(".data(flex_runtime)"));

    let manifest = std::fs::read_to_string(repo.join("crates/flex/rustok-module.toml"))
        .expect("Flex module manifest should read");
    assert!(manifest.contains("[provides.graphql]"));
    assert!(manifest.contains("query = \"graphql::FlexQuery\""));
    assert!(manifest.contains("mutation = \"graphql::FlexMutation\""));
}

#[test]
fn flex_rest_contract_dtos_are_owned_by_flex_crate() {
    let repo = repo_root();
    let server_controller =
        std::fs::read_to_string(repo.join("apps/server/src/controllers/flex.rs"))
            .expect("server Flex REST controller should read");

    for forbidden in [
        "pub struct CreateFlexSchemaRequest",
        "pub struct UpdateFlexSchemaRequest",
        "pub struct CreateFlexEntryRequest",
        "pub struct UpdateFlexEntryRequest",
        "pub struct FlexSchemaResponse",
        "pub struct FlexEntryResponse",
        "pub struct DeleteFlexResponse",
        "flex::CreateFlexSchemaCommand",
        "flex::UpdateFlexSchemaCommand",
        "flex::CreateFlexEntryCommand",
        "flex::UpdateFlexEntryCommand",
        "fn parse_fields_config(",
        "DeleteFlexResponse { success: true }",
        "fn map_schema(",
        "fn map_entry(",
    ] {
        assert!(
            !server_controller.contains(forbidden),
            "Flex REST DTO and view mapping ownership must live in crates/flex, not apps/server: {forbidden}"
        );
    }
    assert!(server_controller.contains("use flex::rest::{"));
    assert!(server_controller.contains("FlexSchemaResponse::from"));
    assert!(server_controller.contains("FlexEntryResponse::from"));

    let owner_rest = std::fs::read_to_string(repo.join("crates/flex/src/rest.rs"))
        .expect("owner Flex REST contract source should read");
    for owner_owned_type in [
        "pub struct CreateFlexSchemaRequest",
        "pub struct UpdateFlexSchemaRequest",
        "pub struct CreateFlexEntryRequest",
        "pub struct UpdateFlexEntryRequest",
        "pub struct FlexSchemaResponse",
        "pub struct FlexEntryResponse",
        "pub struct DeleteFlexResponse",
        "impl CreateFlexSchemaRequest",
        "impl UpdateFlexSchemaRequest",
        "impl CreateFlexEntryRequest",
        "impl UpdateFlexEntryRequest",
        "pub fn into_command",
        "impl DeleteFlexResponse",
        "pub fn success() -> Self",
        "impl From<FlexSchemaView> for FlexSchemaResponse",
        "impl From<FlexEntryView> for FlexEntryResponse",
    ] {
        assert!(
            owner_rest.contains(owner_owned_type),
            "missing owner-owned standalone Flex REST contract item: {owner_owned_type}"
        );
    }

    let swagger = std::fs::read_to_string(repo.join("apps/server/src/controllers/swagger.rs"))
        .expect("server swagger controller should be readable");
    assert!(swagger.contains("flex::rest::CreateFlexSchemaRequest"));
    assert!(swagger.contains("flex::rest::FlexSchemaResponse"));
    assert!(!swagger.contains("crate::controllers::flex::FlexSchemaResponse"));
}

#[test]
fn flex_standalone_validation_contract_is_owned_by_flex_crate() {
    let repo = repo_root();
    assert!(
        !repo
            .join("apps/server/src/services/flex_standalone_validation_service.rs")
            .exists(),
        "standalone Flex entry normalization/validation must live in crates/flex, not apps/server"
    );

    let owner_standalone = std::fs::read_to_string(repo.join("crates/flex/src/standalone.rs"))
        .expect("owner Flex standalone source should read");
    for owner_owned_symbol in [
        "pub fn normalize_and_validate_standalone_entry",
        "pub fn split_standalone_entry_data",
        "pub fn effective_standalone_entry_data",
        "pub fn merge_standalone_entry_patch",
        "pub trait StandaloneSchemaViewSource",
        "pub trait StandaloneSchemaTranslationSource",
        "pub trait StandaloneEntryViewSource",
        "pub fn standalone_schema_view_from_source",
        "pub fn standalone_entry_view_from_source",
        "schema.apply_defaults(&mut data);",
        "schema.strip_unknown(&mut data);",
        "FlexError::ValidationFailed(errors)",
    ] {
        assert!(
            owner_standalone.contains(owner_owned_symbol),
            "missing owner-owned standalone validation contract: {owner_owned_symbol}"
        );
    }

    let server_adapter =
        std::fs::read_to_string(repo.join("apps/server/src/services/flex_standalone_service.rs"))
            .expect("server Flex standalone SeaORM adapter should read");
    assert!(server_adapter.contains("schema.build_custom_fields_schema()?"));
    assert!(server_adapter.contains("flex::normalize_and_validate_standalone_entry"));
    assert!(server_adapter.contains("flex::split_standalone_entry_data"));
    assert!(server_adapter.contains("flex::merge_standalone_entry_patch"));
    assert!(server_adapter.contains("flex::standalone_schema_view_from_source"));
    assert!(server_adapter.contains("flex::standalone_entry_view_from_source"));
    assert!(!server_adapter.contains("FlexStandaloneValidationService"));
    for forbidden in [
        "fn split_entry_data(",
        "fn effective_entry_data(",
        "fn merge_entry_patch(",
        "fn schema_to_view(",
        "fn entry_to_view(",
        "flex::FlexSchemaView {",
        "flex::FlexEntryView {",
    ] {
        assert!(
            !server_adapter.contains(forbidden),
            "standalone entry/schema view and JSON split/merge ownership must live in crates/flex, not apps/server: {forbidden}"
        );
    }

    let server_schema_model =
        std::fs::read_to_string(repo.join("apps/server/src/models/flex_schemas.rs"))
            .expect("server Flex schema model helper should read");
    assert!(server_schema_model.contains("flex::parse_standalone_fields_config"));
    assert!(server_schema_model.contains("flex::build_standalone_custom_fields_schema"));
    assert!(server_schema_model.contains("impl flex::StandaloneSchemaViewSource for Model"));
    let server_entry_model =
        std::fs::read_to_string(repo.join("apps/server/src/models/flex_entries.rs"))
            .expect("server Flex entry model helper should read");
    let server_translation_model =
        std::fs::read_to_string(repo.join("apps/server/src/models/flex_schema_translations.rs"))
            .expect("server Flex schema translation model helper should read");
    assert!(server_entry_model.contains("impl flex::StandaloneEntryViewSource for Model"));
    assert!(
        server_translation_model.contains("impl flex::StandaloneSchemaTranslationSource for Model")
    );
    for forbidden in [
        "serde_json::from_value(self.fields_config.clone())",
        "CustomFieldsSchema::new(self.parse_field_definitions()?)",
    ] {
        assert!(
            !server_schema_model.contains(forbidden),
            "standalone schema fields_config interpretation must live in crates/flex, not apps/server: {forbidden}"
        );
    }

    for owner_owned_symbol in [
        "pub fn parse_standalone_fields_config",
        "pub fn build_standalone_custom_fields_schema",
        "pub fn serialize_standalone_fields_config",
        "pub fn standalone_localized_field_keys",
    ] {
        assert!(
            owner_standalone.contains(owner_owned_symbol),
            "missing owner-owned standalone fields_config helper: {owner_owned_symbol}"
        );
    }
    assert!(server_adapter.contains("flex::serialize_standalone_fields_config("));
    assert!(
        server_adapter.contains("flex::standalone_localized_field_keys(&custom_fields_schema)")
    );
    for forbidden in [
        "fn localized_field_keys(",
        "serde_json::to_value(input.fields_config).unwrap_or_default()",
    ] {
        assert!(
            !server_adapter.contains(forbidden),
            "standalone fields_config/key derivation ownership must live in crates/flex, not apps/server: {forbidden}"
        );
    }
}

#[test]
fn flex_field_definition_view_mapping_is_owned_by_flex_crate() {
    let repo = repo_root();
    let owner_registry = std::fs::read_to_string(repo.join("crates/flex/src/registry.rs"))
        .expect("owner Flex registry source should read");
    for owner_owned_symbol in [
        "pub trait FieldDefinitionViewSource",
        "impl FieldDefinitionView",
        "pub fn from_source",
        "macro_rules! impl_field_definition_command_conversions",
        "impl From<$crate::CreateFieldDefinitionCommand>",
        "impl From<$crate::UpdateFieldDefinitionCommand>",
        "pub fn validate_field_definition_create",
        "pub fn field_definition_position_or_next",
        "pub fn field_definition_type_name",
        "pub fn field_definition_label_json",
        "pub fn field_definition_description_json",
        "pub fn field_definition_validation_json",
        "pub fn field_definition_cache_invalidation_target",
        "pub fn field_definition_created_event",
        "pub fn field_definition_updated_event",
        "pub fn field_definition_deleted_event",
    ] {
        assert!(
            owner_registry.contains(owner_owned_symbol),
            "missing owner-owned field-definition view mapping contract: {owner_owned_symbol}"
        );
    }

    let server_bootstrap = std::fs::read_to_string(
        repo.join("apps/server/src/services/field_definition_registry_bootstrap.rs"),
    )
    .expect("server field-definition registry bootstrap source should read");
    assert!(server_bootstrap.contains("impl_field_definition_view_source!"));
    assert!(server_bootstrap.contains("impl_field_definition_service_adapter!"));
    assert!(server_bootstrap.contains("flex::impl_field_definition_command_conversions!"));
    assert!(server_bootstrap.contains("FieldDefinitionView::from_source"));
    assert!(!server_bootstrap.contains("UserFieldService::list_all"));
    assert!(!server_bootstrap.contains("OrderFieldService::list_all"));
    assert!(!server_bootstrap.contains("ProductFieldService::list_all"));
    assert!(!server_bootstrap.contains("TopicFieldService::list_all"));
    for forbidden in [
        "fn user_model_to_view(",
        "fn order_model_to_view(",
        "fn product_model_to_view(",
        "fn topic_model_to_view(",
        "FieldDefinitionView {\n        id:",
        "FieldDefinitionView {\r\n        id:",
        "field_key: input.field_key",
        "label: input.label",
        "is_localized: input.is_localized",
        "is_active: input.is_active",
    ] {
        assert!(
            !server_bootstrap.contains(forbidden),
            "field-definition view shape mapping must live in crates/flex, not apps/server: {forbidden}"
        );
    }

    for service_path in [
        "apps/server/src/services/user_field_service.rs",
        "apps/server/src/services/order_field_service.rs",
        "apps/server/src/services/product_field_service.rs",
        "apps/server/src/services/topic_field_service.rs",
    ] {
        let service = std::fs::read_to_string(repo.join(service_path))
            .expect("server field-definition service source should read");
        let production = service.split("#[cfg(test)]").next().unwrap_or(&service);

        for required in [
            "flex::validate_field_definition_create",
            "flex::field_definition_position_or_next",
            "flex::field_definition_type_name",
            "flex::field_definition_label_json",
            "flex::field_definition_description_json",
            "flex::field_definition_validation_json",
            "flex::field_definition_created_event",
            "flex::field_definition_updated_event",
            "flex::field_definition_deleted_event",
        ] {
            assert!(
                production.contains(required),
                "server field-definition persistence adapter should delegate owner lifecycle policy to flex: {service_path} missing {required}"
            );
        }

        for forbidden in [
            "is_valid_field_key",
            "DomainEvent::FieldDefinition",
            "EventEnvelope::new(",
            "serde_json::to_value(input.field_type)",
            "serde_json::to_value(&input.label)",
            "serde_json::to_value(d)",
            "serde_json::to_value(v)",
            "serde_json::to_value(label)",
            "serde_json::to_value(desc)",
            "serde_json::to_value(val)",
            "FlexError::TooManyFields",
            "FlexError::DuplicateFieldKey",
            "FlexError::InvalidFieldKey",
        ] {
            assert!(
                !production.contains(forbidden),
                "field-definition lifecycle policy must live in crates/flex, not apps/server: {service_path} contains {forbidden}"
            );
        }
    }

    let cache =
        std::fs::read_to_string(repo.join("apps/server/src/services/field_definition_cache.rs"))
            .expect("server field-definition cache source should read");
    let cache_production = cache.split("#[cfg(test)]").next().unwrap_or(&cache);
    assert!(
        cache_production
            .contains("flex::field_definition_cache_invalidation_target(&envelope.event)"),
        "server field-definition cache should delegate event taxonomy to flex"
    );
    for forbidden in [
        "DomainEvent::FieldDefinitionCreated",
        "DomainEvent::FieldDefinitionUpdated",
        "DomainEvent::FieldDefinitionDeleted",
    ] {
        assert!(
            !cache_production.contains(forbidden),
            "field-definition cache event taxonomy must live in crates/flex, not apps/server: {forbidden}"
        );
    }

    for model_path in [
        "apps/server/src/models/user_field_definitions.rs",
        "apps/server/src/models/order_field_definitions.rs",
        "apps/server/src/models/product_field_definitions.rs",
        "apps/server/src/models/topic_field_definitions.rs",
    ] {
        let model = std::fs::read_to_string(repo.join(model_path))
            .expect("server field-definition model helper source should read");
        assert!(model.contains("flex::impl_field_definition_source!(Model);"));
        assert!(model.contains("flex::field_definition_from_source(&self)"));
        for forbidden in [
            "serde_json::from_value(serde_json::Value::String",
            "let label: HashMap<String, String> = serde_json::from_value",
            "let validation: Option<ValidationRule> =",
            "Some(FieldDefinition {",
        ] {
            assert!(
                !model.contains(forbidden),
                "field-definition row-to-core mapping must live in crates/flex, not apps/server: {model_path} contains {forbidden}"
            );
        }
    }
}

#[test]
fn search_graphql_surface_is_owned_by_search_crate() {
    let repo = repo_root();
    let server_search_dir = repo.join("apps/server/src/graphql/search");
    assert!(
        !server_search_dir.exists(),
        "search GraphQL query/mutation/types must live in rustok-search, not apps/server"
    );

    let search_graphql_dir = repo.join("crates/rustok-search/src/graphql");
    let forbidden = [
        "crate::common",
        "crate::context",
        "crate::graphql",
        "crate::middleware",
        "crate::services",
        "RbacService",
        "transactional_event_bus_from_context",
        "SharedSearchRateLimiter",
    ];

    let mut offenders = Vec::new();
    for entry in std::fs::read_dir(&search_graphql_dir)
        .expect("rustok-search GraphQL dir should be readable")
    {
        let entry = entry.expect("rustok-search GraphQL entry should be readable");
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("rustok-search GraphQL source");
        for symbol in forbidden {
            if source.contains(symbol) {
                offenders.push(format!("{} contains {symbol}", path.display()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "rustok-search GraphQL must depend on shared owner/host contracts, not apps/server internals: {offenders:?}"
    );
}

#[test]
fn ai_graphql_surface_is_owned_by_ai_crate() {
    let repo = repo_root();
    let server_ai_dir = repo.join("apps/server/src/graphql/ai");
    assert!(
        !server_ai_dir.exists(),
        "AI GraphQL query/mutation/subscription/types must live in rustok-ai, not apps/server"
    );

    let graphql_dir = repo.join("crates/rustok-ai/src/graphql");
    for file in [
        "mod.rs",
        "query.rs",
        "mutation.rs",
        "subscription.rs",
        "types.rs",
    ] {
        assert!(graphql_dir.join(file).exists(), "missing AI GraphQL {file}");
    }

    let forbidden = [
        "crate::common",
        "crate::context",
        "crate::models",
        "crate::services",
        "rustok_rbac",
        "has_effective_permission_in_set",
        "apps/server",
    ];
    let mut offenders = Vec::new();
    for entry in std::fs::read_dir(&graphql_dir).expect("rustok-ai GraphQL dir should read") {
        let path = entry.expect("rustok-ai GraphQL entry should read").path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("rustok-ai GraphQL source");
        for symbol in forbidden {
            if source.contains(symbol) {
                offenders.push(format!("{} contains {symbol}", path.display()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "rustok-ai GraphQL must depend on owner/shared contracts: {offenders:?}"
    );

    let schema = std::fs::read_to_string(repo.join("apps/server/src/graphql/schema.rs"))
        .expect("server GraphQL schema should read");
    assert!(schema.contains("rustok_ai::graphql::{AiMutation, AiQuery, AiSubscription}"));
}

#[test]
fn rbac_graphql_surface_is_owned_by_rbac_crate() {
    let repo = repo_root();
    let server_rbac_dir = repo.join("apps/server/src/graphql/rbac");
    assert!(
        !server_rbac_dir.exists(),
        "RBAC GraphQL query/mutation/types must live in rustok-rbac, not apps/server"
    );

    let rbac_graphql_dir = repo.join("crates/rustok-rbac/src/graphql");
    let forbidden = [
        "crate::common",
        "crate::context",
        "crate::graphql",
        "crate::middleware",
        "crate::services",
        "RbacService",
    ];

    let mut offenders = Vec::new();
    for entry in std::fs::read_dir(&rbac_graphql_dir).expect("rustok-rbac GraphQL dir should read")
    {
        let entry = entry.expect("rustok-rbac GraphQL entry should read");
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("rustok-rbac GraphQL source");
        for symbol in forbidden {
            if source.contains(symbol) {
                offenders.push(format!("{} contains {symbol}", path.display()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "rustok-rbac GraphQL must depend on owner/shared contracts, not apps/server internals: {offenders:?}"
    );
}

#[test]
fn auth_graphql_surface_is_owned_by_auth_crate() {
    let repo = repo_root();
    let auth_graphql_dir = repo.join("crates/rustok-auth/src/graphql");
    assert!(auth_graphql_dir.join("query.rs").exists());
    assert!(auth_graphql_dir.join("mutation.rs").exists());
    assert!(auth_graphql_dir.join("types.rs").exists());
    assert!(auth_graphql_dir.join("auth_query.rs").exists());
    assert!(auth_graphql_dir.join("auth_mutation.rs").exists());
    assert!(auth_graphql_dir.join("auth_types.rs").exists());

    let schema = std::fs::read_to_string(repo.join("apps/server/src/graphql/schema.rs"))
        .expect("server GraphQL schema should read");
    assert!(
        schema
            .contains("rustok_auth::graphql::{AuthMutation, AuthQuery, OAuthMutation, OAuthQuery}")
    );

    let forbidden = [
        "crate::context",
        "crate::models",
        "crate::services",
        "DatabaseConnection",
        "sea_orm",
    ];
    let mut offenders = Vec::new();
    for entry in std::fs::read_dir(&auth_graphql_dir).expect("rustok-auth GraphQL dir should read")
    {
        let path = entry.expect("rustok-auth GraphQL entry should read").path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("rustok-auth GraphQL source");
        for symbol in forbidden {
            if source.contains(symbol) {
                offenders.push(format!("{} contains {symbol}", path.display()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "rustok-auth GraphQL must depend on owner/shared contracts: {offenders:?}"
    );
}

#[test]
fn auth_rest_dto_surface_is_owned_by_auth_crate() {
    let repo = repo_root();
    let rest = std::fs::read_to_string(repo.join("crates/rustok-auth/src/rest.rs"))
        .expect("rustok-auth REST contract should read");
    let controller = std::fs::read_to_string(repo.join("apps/server/src/controllers/auth.rs"))
        .expect("server auth controller should read");

    let dto_markers = [
        "pub struct LoginParams",
        "pub struct RefreshRequest",
        "pub struct RegisterParams",
        "pub struct AcceptInviteParams",
        "pub struct InviteAcceptResponse",
        "pub struct RequestResetParams",
        "pub struct ConfirmResetParams",
        "pub struct RequestVerificationParams",
        "pub struct ConfirmVerificationParams",
        "pub struct ChangePasswordParams",
        "pub struct UpdateProfileParams",
        "pub struct ResetRequestResponse",
        "pub struct VerificationRequestResponse",
        "pub struct GenericStatusResponse",
        "pub struct SessionItem",
        "pub struct SessionsResponse",
        "pub struct SessionListParams",
        "pub struct UserResponse",
        "pub struct UserInfo",
        "pub struct AuthResponse",
        "pub struct LogoutResponse",
    ];

    for marker in dto_markers {
        assert!(
            rest.contains(marker),
            "rustok-auth must own auth REST DTO marker {marker}"
        );
        assert!(
            !controller.contains(marker),
            "apps/server auth controller must not define owner DTO marker {marker}"
        );
    }

    assert!(
        rest.contains("utoipa::ToSchema"),
        "rustok-auth REST DTOs must preserve OpenAPI schema derives"
    );
    assert!(
        controller.contains("pub use rustok_auth::{"),
        "server auth controller should re-export owner DTOs for existing Swagger paths"
    );
    assert!(
        !controller.contains("utoipa::ToSchema"),
        "server auth controller must not own auth OpenAPI DTO derives"
    );
    assert!(
        !controller.contains("UserResponse::from_user_and_role"),
        "server auth controller must not rely on a host-owned UserResponse constructor"
    );
}

#[test]
fn oauth_rest_dto_surface_is_owned_by_auth_crate() {
    let repo = repo_root();
    let rest = std::fs::read_to_string(repo.join("crates/rustok-auth/src/rest.rs"))
        .expect("rustok-auth REST contract should read");
    let controller = std::fs::read_to_string(repo.join("apps/server/src/controllers/oauth.rs"))
        .expect("server OAuth controller should read");

    let dto_markers = [
        "pub struct TokenRequest",
        "pub struct AuthorizeRequest",
        "pub struct BrowserAuthorizeRequest",
        "pub struct ConsentRequest",
        "pub struct BrowserSessionResponse",
        "pub struct TokenResponse",
        "pub struct TokenErrorResponse",
        "pub struct RevokeRequest",
    ];

    for marker in dto_markers {
        assert!(
            rest.contains(marker),
            "rustok-auth must own OAuth REST DTO marker {marker}"
        );
        assert!(
            !controller.contains(marker),
            "apps/server OAuth controller must not define owner DTO marker {marker}"
        );
    }

    assert!(
        controller.contains("use rustok_auth::{"),
        "server OAuth controller should import owner DTOs from rustok-auth"
    );
    assert!(
        controller.contains("fn oauth_error_response(error: TokenErrorResponse)"),
        "server OAuth controller may keep HTTP status mapping for the owner error DTO"
    );
    assert!(
        !controller.contains("impl axum::response::IntoResponse for TokenErrorResponse"),
        "server OAuth controller must not implement external HTTP traits for owner DTOs"
    );
    assert!(
        !controller.contains("use serde::{Deserialize, Serialize}"),
        "server OAuth controller must not own serde DTO definitions"
    );
}

#[test]
fn users_rest_dto_surface_is_owned_by_auth_crate() {
    let repo = repo_root();
    let rest = std::fs::read_to_string(repo.join("crates/rustok-auth/src/rest.rs"))
        .expect("rustok-auth REST contract should read");
    let controller = std::fs::read_to_string(repo.join("apps/server/src/controllers/users.rs"))
        .expect("server users controller should read");

    let dto_markers = [
        "pub struct UserItem",
        "pub struct UsersResponse",
        "pub struct UsersListParams",
    ];

    for marker in dto_markers {
        assert!(
            rest.contains(marker),
            "rustok-auth must own users REST DTO marker {marker}"
        );
        assert!(
            !controller.contains(marker),
            "apps/server users controller must not define owner DTO marker {marker}"
        );
    }

    assert!(
        controller.contains("use rustok_auth::{UserItem, UsersListParams, UsersResponse};"),
        "server users controller should import owner DTOs from rustok-auth"
    );
    assert!(
        !controller.contains("use serde::{Deserialize, Serialize}"),
        "server users controller must not own serde DTO definitions"
    );
    assert!(
        !controller.contains("use utoipa::ToSchema"),
        "server users controller must not own OpenAPI DTO derives"
    );
}

#[test]
fn mcp_graphql_surface_is_owned_by_mcp_crate() {
    let repo = repo_root();
    let graphql_dir = repo.join("crates/rustok-mcp/src/graphql");
    for file in ["mod.rs", "query.rs", "mutation.rs", "types.rs"] {
        assert!(
            graphql_dir.join(file).exists(),
            "missing MCP GraphQL {file}"
        );
    }

    let forbidden = [
        "crate::context",
        "crate::models",
        "crate::services",
        "DatabaseConnection",
        "sea_orm",
    ];
    let mut offenders = Vec::new();
    for entry in std::fs::read_dir(&graphql_dir).expect("rustok-mcp GraphQL dir should read") {
        let path = entry.expect("rustok-mcp GraphQL entry should read").path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("rustok-mcp GraphQL source");
        for symbol in forbidden {
            if source.contains(symbol) {
                offenders.push(format!("{} contains {symbol}", path.display()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "rustok-mcp GraphQL must depend on owner/shared contracts: {offenders:?}"
    );

    let schema = std::fs::read_to_string(repo.join("apps/server/src/graphql/schema.rs"))
        .expect("server GraphQL schema should read");
    assert!(schema.contains("rustok_mcp::graphql::{McpMutation, McpQuery}"));
}

#[test]
fn mcp_rest_control_plane_dto_is_owned_by_mcp_crate() {
    let repo = repo_root();
    let management = std::fs::read_to_string(repo.join("crates/rustok-mcp/src/management.rs"))
        .expect("rustok-mcp management contract should read");
    let access = std::fs::read_to_string(repo.join("crates/rustok-mcp/src/access.rs"))
        .expect("rustok-mcp access contract should read");
    let controller = std::fs::read_to_string(repo.join("apps/server/src/controllers/mcp.rs"))
        .expect("server MCP controller should read");

    let dto_markers = [
        "pub struct BootstrapMcpRemoteSessionRequest",
        "pub struct McpRemoteToolCallRequest",
        "pub struct McpRemoteToolCallResponse",
        "pub struct CreateMcpClientRequest",
        "pub struct RotateMcpTokenRequest",
        "pub struct UpdateMcpPolicyRequest",
        "pub struct McpAuditQuery",
        "pub struct StageMcpModuleScaffoldDraftRequest",
        "pub struct ApplyMcpModuleScaffoldDraftRequest",
        "pub struct McpClientSummaryResponse",
        "pub struct McpClientDetailsResponse",
        "pub struct McpAuditEventResponse",
        "pub struct McpModuleScaffoldDraftResponse",
    ];
    for marker in dto_markers {
        assert!(
            management.contains(marker),
            "rustok-mcp must own MCP REST/control-plane DTO marker {marker}"
        );
        assert!(
            !controller.contains(marker),
            "apps/server MCP controller must not define owner DTO marker {marker}"
        );
    }

    assert!(
        access.contains("impl FromStr for McpActorType"),
        "MCP actor type parsing must stay in rustok-mcp"
    );
    assert!(
        controller.contains("McpActorType::from_str"),
        "server MCP controller must delegate actor type parsing to rustok-mcp"
    );
    assert!(
        !controller.contains("\"human_user\" => Ok(McpActorType::HumanUser)"),
        "server MCP controller must not duplicate MCP actor parsing"
    );
}

#[test]
fn channel_rest_control_plane_dto_is_owned_by_channel_crate() {
    let repo = repo_root();
    let dto = std::fs::read_to_string(repo.join("crates/rustok-channel/src/dto/mod.rs"))
        .expect("rustok-channel DTO contract should read");
    let controller = std::fs::read_to_string(repo.join("apps/server/src/controllers/channel.rs"))
        .expect("server channel controller should read");

    let owner_markers = [
        "pub struct ChannelBootstrapResponse",
        "pub struct CreateResolutionPolicySetRequest",
        "pub struct CreateResolutionRuleRequest",
        "pub struct UpdateResolutionRuleRequest",
        "pub struct ReorderResolutionRulesRequest",
        "pub struct AvailableChannelModuleItem",
        "pub struct AvailableChannelOauthAppItem",
        "pub fn create_resolution_policy_set_input",
        "pub fn create_resolution_rule_input",
        "pub fn update_resolution_rule_input",
    ];
    for marker in owner_markers {
        assert!(
            dto.contains(marker),
            "rustok-channel must own channel REST/control-plane marker {marker}"
        );
    }

    let forbidden_controller_markers = [
        "struct ChannelBootstrapResponse",
        "struct CreateResolutionPolicySetRequest",
        "struct CreateResolutionRuleRequest",
        "struct UpdateResolutionRuleRequest",
        "struct ReorderResolutionRulesRequest",
        "struct AvailableModuleItem",
        "struct AvailableOauthAppItem",
        "fn build_rule_definition",
        "fn build_update_rule_input",
        "fn normalize_optional_string",
        "ResolutionAction::ResolveToChannel",
        "ResolutionPredicate::HostEquals",
    ];
    for marker in forbidden_controller_markers {
        assert!(
            !controller.contains(marker),
            "apps/server channel controller must not own channel REST/control-plane marker {marker}"
        );
    }

    for marker in [
        "ChannelBootstrapResponse::<crate::context::ChannelContext>",
        "create_resolution_policy_set_input(tenant.id, input)",
        "create_resolution_rule_input(input)",
        "update_resolution_rule_input(input)",
    ] {
        assert!(
            controller.contains(marker),
            "apps/server channel controller must consume owner channel contract marker {marker}"
        );
    }
}

#[test]
fn product_translation_search_helper_is_not_server_owned() {
    let repo = repo_root();
    let server_helper = repo.join("apps/server/src/services/product_search.rs");
    assert!(
        !server_helper.exists(),
        "product translation search helper must not live in apps/server"
    );

    let services_mod = std::fs::read_to_string(repo.join("apps/server/src/services/mod.rs"))
        .expect("server services module should read");
    assert!(
        !services_mod.contains("pub mod product_search;"),
        "apps/server must not re-export a product_search service"
    );

    let foundation_search =
        std::fs::read_to_string(repo.join("crates/rustok-commerce-foundation/src/search.rs"))
            .expect("commerce foundation search helper should read");
    assert!(
        foundation_search.contains("pub fn product_translation_title_search_condition"),
        "product translation title search condition must remain owner/foundation-owned"
    );
}
