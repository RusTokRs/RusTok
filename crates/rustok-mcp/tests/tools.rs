use async_trait::async_trait;
use sea_orm_migration::MigrationTrait;

use rustok_core::module::{MigrationSource, RusToKModule};
use rustok_core::registry::ModuleRegistry;
use rustok_mcp::tools::{
    list_modules, list_modules_filtered, module_details, module_exists, McpState, McpToolResponse,
    ModuleLookupRequest, ModuleQueryRequest,
};

struct DemoModule;
struct ExtraModule;

impl MigrationSource for DemoModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        Vec::new()
    }
}

impl MigrationSource for ExtraModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        Vec::new()
    }
}

#[async_trait]
impl RusToKModule for DemoModule {
    fn slug(&self) -> &'static str {
        "demo"
    }

    fn name(&self) -> &'static str {
        "Demo"
    }

    fn description(&self) -> &'static str {
        "Demo module"
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn dependencies(&self) -> &[&'static str] {
        &["core"]
    }
}

#[async_trait]
impl RusToKModule for ExtraModule {
    fn slug(&self) -> &'static str {
        "extra"
    }

    fn name(&self) -> &'static str {
        "Extra"
    }

    fn description(&self) -> &'static str {
        "Extra module"
    }

    fn version(&self) -> &'static str {
        "0.2.0"
    }

    fn dependencies(&self) -> &[&'static str] {
        &[]
    }
}

#[tokio::test]
async fn list_modules_returns_registered_metadata() {
    let registry = ModuleRegistry::new().register(DemoModule);
    let state = Box::leak(Box::new(McpState { registry }));

    let response = list_modules(state).await;

    assert_eq!(response.modules.len(), 1);
    assert_eq!(response.modules[0].slug, "demo");
    assert_eq!(response.modules[0].name, "Demo");
    assert_eq!(response.modules[0].description, "Demo module");
    assert_eq!(response.modules[0].version, "0.1.0");
    assert_eq!(response.modules[0].dependencies, vec!["core"]);
}

#[tokio::test]
async fn module_exists_checks_registry() {
    let registry = ModuleRegistry::new().register(DemoModule);
    let state = Box::leak(Box::new(McpState { registry }));

    let response = module_exists(
        state,
        ModuleLookupRequest {
            slug: "demo".to_string(),
        },
    )
    .await;

    assert!(response.exists);
    assert_eq!(response.slug, "demo");
}

#[tokio::test]
async fn module_details_returns_module_data() {
    let registry = ModuleRegistry::new().register(DemoModule);
    let state = Box::leak(Box::new(McpState { registry }));

    let response = module_details(
        state,
        ModuleLookupRequest {
            slug: "demo".to_string(),
        },
    )
    .await;

    let module = response.module.expect("module details");
    assert_eq!(response.slug, "demo");
    assert_eq!(module.slug, "demo");
    assert_eq!(module.name, "Demo");
    assert_eq!(module.description, "Demo module");
    assert_eq!(module.version, "0.1.0");
    assert_eq!(module.dependencies, vec!["core"]);
}

#[tokio::test]
async fn module_details_returns_none_for_unknown_slug() {
    let registry = ModuleRegistry::new().register(DemoModule);
    let state = Box::leak(Box::new(McpState { registry }));

    let response = module_details(
        state,
        ModuleLookupRequest {
            slug: "missing".to_string(),
        },
    )
    .await;

    assert!(response.module.is_none());
    assert_eq!(response.slug, "missing");
}

#[test]
fn tool_response_success_sets_data() {
    let response = McpToolResponse::success("ok");

    assert!(response.ok);
    assert_eq!(response.data, Some("ok"));
    assert!(response.error.is_none());
}

#[test]
fn tool_response_error_sets_error_payload() {
    let response = McpToolResponse::<()>::error("invalid", "Bad request");

    assert!(!response.ok);
    assert!(response.data.is_none());
    let error = response.error.expect("error payload");
    assert_eq!(error.code, "invalid");
    assert_eq!(error.message, "Bad request");
}

#[tokio::test]
async fn list_modules_filtered_applies_filters_and_pagination() {
    let registry = ModuleRegistry::new()
        .register(DemoModule)
        .register(ExtraModule);
    let state = Box::leak(Box::new(McpState { registry }));

    let response = list_modules_filtered(
        state,
        ModuleQueryRequest {
            slug_prefix: Some("d".to_string()),
            dependency: Some("core".to_string()),
            limit: Some(1),
            offset: Some(0),
        },
    )
    .await;

    assert_eq!(response.modules.len(), 1);
    assert_eq!(response.modules[0].slug, "demo");
}

#[tokio::test]
async fn list_modules_filtered_paginates_results() {
    let registry = ModuleRegistry::new()
        .register(DemoModule)
        .register(ExtraModule);
    let state = Box::leak(Box::new(McpState { registry }));

    let response = list_modules_filtered(
        state,
        ModuleQueryRequest {
            slug_prefix: None,
            dependency: None,
            limit: Some(1),
            offset: Some(1),
        },
    )
    .await;

    assert_eq!(response.modules.len(), 1);
    assert_eq!(response.modules[0].slug, "extra");
}
