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
    for forbidden in [
        "crate::context",
        "crate::services",
        "loco_rs",
        "apps/server",
    ] {
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
        "loco_rs::app::AppContext",
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
        "loco_rs::app::AppContext",
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
    assert!(schema
        .contains("rustok_auth::graphql::{AuthMutation, AuthQuery, OAuthMutation, OAuthQuery}"));

    let forbidden = [
        "crate::context",
        "crate::models",
        "crate::services",
        "DatabaseConnection",
        "sea_orm",
        "loco_rs",
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
        "loco_rs",
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
