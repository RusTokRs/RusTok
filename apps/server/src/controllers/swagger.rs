use axum::{
    extract::State,
    http::{StatusCode, header::CONTENT_TYPE},
    response::{IntoResponse, Response},
    routing::get,
};
use utoipa::OpenApi;
use utoipa::openapi::OpenApi as OpenApiDoc;

use crate::common::settings::RustokSettings;
use crate::error::{Error, Result};
use crate::services::server_runtime_context::ServerRuntimeContext;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "RusTok API",
        version = "1.0.0",
        description = "Unified API for RusTok CMS & Commerce"
    ),
    paths(
        // Auth
        crate::controllers::auth::login,
        crate::controllers::auth::register,
        crate::controllers::auth::refresh,
        crate::controllers::auth::logout,
        crate::controllers::auth::me,
        crate::controllers::auth::accept_invite,
        crate::controllers::auth::request_verification,
        crate::controllers::auth::confirm_verification,
        // Health
        crate::controllers::health::health,
        crate::controllers::health::live,
        crate::controllers::health::ready,
        crate::controllers::health::modules,
        // Metrics
        crate::controllers::metrics::metrics,
        // Marketplace
        crate::controllers::marketplace_registry::catalog,
        crate::controllers::marketplace_registry::catalog_module,
        crate::controllers::marketplace_registry::publish,
        crate::controllers::marketplace_registry::publish_status,
        crate::controllers::marketplace_registry::upload_publish_artifact,
        crate::controllers::marketplace_registry::stage_external_prebuilt,
        crate::controllers::marketplace_registry::stage_platform_build,
        crate::controllers::marketplace_registry::validate_publish_request_step,
        crate::controllers::marketplace_registry::approve_publish_request,
        crate::controllers::marketplace_registry::reject_publish_request,
        crate::controllers::marketplace_registry::report_validation_stage,
        crate::controllers::marketplace_registry::transfer_owner,
        crate::controllers::marketplace_registry::yank,
        // RBAC artifact permissions
        crate::controllers::artifact_permissions::grant_artifact_permission,
        crate::controllers::artifact_permissions::revoke_artifact_permission,
        // Swagger
        crate::controllers::swagger::openapi_json,
        crate::controllers::swagger::openapi_yaml,
        // Admin Events
        crate::controllers::admin_events::list_dlq,
        crate::controllers::admin_events::replay_dlq_event,
        // Flex standalone
        crate::controllers::flex::list_schemas,
        crate::controllers::flex::get_schema,
        crate::controllers::flex::create_schema,
        crate::controllers::flex::update_schema,
        crate::controllers::flex::delete_schema,
        crate::controllers::flex::list_entries,
        crate::controllers::flex::get_entry,
        crate::controllers::flex::create_entry,
        crate::controllers::flex::update_entry,
        crate::controllers::flex::delete_entry,
    ),
    components(
        schemas(
            crate::controllers::auth::LoginParams,
            crate::controllers::auth::RegisterParams,
            crate::controllers::auth::RefreshRequest,
            crate::controllers::auth::AcceptInviteParams,
            crate::controllers::auth::InviteAcceptResponse,
            crate::controllers::auth::RequestVerificationParams,
            crate::controllers::auth::ConfirmVerificationParams,
            crate::controllers::auth::VerificationRequestResponse,
            crate::controllers::auth::GenericStatusResponse,
            crate::controllers::auth::UserResponse,
            crate::controllers::auth::AuthResponse,
            crate::controllers::auth::UserInfo,
            crate::controllers::auth::LogoutResponse,

            // Common
            crate::common::PaginationMeta,
            crate::common::ApiError,
            // Marketplace
            crate::services::marketplace_catalog::RegistryCatalogResponse,
            crate::services::marketplace_catalog::RegistryCatalogModule,
            crate::services::marketplace_catalog::RegistryCatalogVersion,
            crate::services::marketplace_catalog::RegistryMutationResponse,
            crate::services::marketplace_catalog::RegistryPublishRequest,
            crate::services::marketplace_catalog::RegistryPublishDecisionRequest,
            crate::services::marketplace_catalog::RegistryPublishStatusResponse,
            crate::services::marketplace_catalog::RegistryExternalPrebuiltStageRequest,
            crate::services::marketplace_catalog::RegistryExternalPrebuiltStageResponse,
            crate::services::marketplace_catalog::RegistryPlatformBuildStageRequest,
            crate::services::marketplace_catalog::RegistryPlatformBuildStageResponse,
            crate::services::marketplace_catalog::RegistryPublishArtifactOrigin,
            crate::services::marketplace_catalog::RegistryPublishModuleRequest,
            crate::services::marketplace_catalog::RegistryPublishMarketplaceRequest,
            crate::services::marketplace_catalog::RegistryPublishUiPackagesRequest,
            crate::services::marketplace_catalog::RegistryPublishUiPackageRequest,
            crate::services::marketplace_catalog::RegistryYankRequest,
            // RBAC artifact permissions
            crate::controllers::artifact_permissions::ArtifactRolePermissionAssignmentRequest,
            crate::controllers::artifact_permissions::ArtifactRolePermissionAssignmentResponse,
            crate::modules::ModuleSettingSpec,

            // Health
            crate::controllers::health::HealthResponse,
            crate::controllers::health::ModuleHealth,
            crate::controllers::health::ModulesHealthResponse,

            // Admin Events
            crate::controllers::admin_events::DlqEventItem,
            crate::controllers::admin_events::DlqListResponse,
            crate::controllers::admin_events::DlqReplayResponse,

            // Flex standalone
            flex::rest::CreateFlexSchemaRequest,
            flex::rest::UpdateFlexSchemaRequest,
            flex::rest::CreateFlexEntryRequest,
            flex::rest::UpdateFlexEntryRequest,
            flex::rest::FlexSchemaResponse,
            flex::rest::FlexEntryResponse,
            flex::rest::DeleteFlexResponse,
        )
    ),
    modifiers(&SecurityAddon),
    tags(
        (name = "auth", description = "Authentication endpoints"),
        (name = "marketplace", description = "Marketplace registry and catalog endpoints"),
        (name = "rbac", description = "Role-based access control endpoints"),
        (name = "flex", description = "Flex standalone schemas and entries endpoints"),
        (name = "health", description = "Health check endpoints"),
        (name = "observability", description = "Observability and metrics endpoints"),
        (name = "admin", description = "Admin operations")
    )
)]
pub struct ApiDoc;

const REGISTRY_ONLY_OPENAPI_PATHS: &[&str] = &[
    "/health",
    "/health/live",
    "/health/ready",
    "/health/runtime",
    "/health/modules",
    "/metrics",
    "/v1/catalog",
    "/v1/catalog/{slug}",
    "/api/openapi.json",
    "/api/openapi.yaml",
];

pub fn build_openapi_document(settings: &RustokSettings) -> OpenApiDoc {
    let mut openapi = ApiDoc::openapi();
    #[cfg(feature = "mod-blog")]
    openapi.merge(rustok_blog::openapi::openapi_document());
    #[cfg(feature = "mod-forum")]
    openapi.merge(rustok_forum::openapi::openapi_document());
    #[cfg(feature = "mod-pages")]
    openapi.merge(rustok_pages::openapi::openapi_document());
    #[cfg(feature = "mod-commerce")]
    openapi.merge(rustok_commerce::openapi::openapi_document());
    if settings.runtime.is_registry_only() {
        openapi
            .paths
            .paths
            .retain(|path, _| REGISTRY_ONLY_OPENAPI_PATHS.contains(&path.as_str()));
    }
    openapi
}

/// GET /api/openapi.json — OpenAPI specification in JSON format
#[utoipa::path(
    get,
    path = "/api/openapi.json",
    tag = "observability",
    responses(
        (status = 200, description = "OpenAPI specification in JSON format", content_type = "application/json"),
    )
)]
pub async fn openapi_json(State(ctx): State<ServerRuntimeContext>) -> Result<Response> {
    let spec = build_openapi_document(ctx.settings())
        .to_json()
        .map_err(|e| Error::Message(format!("Failed to serialize OpenAPI spec: {e}")))?;
    Ok((
        StatusCode::OK,
        [(CONTENT_TYPE, "application/json; charset=utf-8")],
        spec,
    )
        .into_response())
}

/// GET /api/openapi.yaml — OpenAPI specification in YAML format
#[utoipa::path(
    get,
    path = "/api/openapi.yaml",
    tag = "observability",
    responses(
        (status = 200, description = "OpenAPI specification in YAML format", content_type = "text/yaml"),
    )
)]
pub async fn openapi_yaml(State(ctx): State<ServerRuntimeContext>) -> Result<Response> {
    let spec = build_openapi_document(ctx.settings())
        .to_yaml()
        .map_err(|e| Error::Message(format!("Failed to serialize OpenAPI spec to YAML: {e}")))?;
    Ok((
        StatusCode::OK,
        [(CONTENT_TYPE, "text/yaml; charset=utf-8")],
        spec,
    )
        .into_response())
}

pub fn router() -> crate::routes::ServerRouter {
    axum::Router::new()
        .route("/api/openapi.json", get(openapi_json))
        .route("/api/openapi.yaml", get(openapi_yaml))
}

pub struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                utoipa::openapi::security::SecurityScheme::Http(
                    utoipa::openapi::security::HttpBuilder::new()
                        .scheme(utoipa::openapi::security::HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .build(),
                ),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ApiDoc, build_openapi_document};
    use crate::common::settings::{RuntimeHostMode, RustokSettings};
    use utoipa::OpenApi;

    #[test]
    fn openapi_includes_registry_catalog_path() {
        let openapi = ApiDoc::openapi();

        assert!(
            openapi.paths.paths.contains_key("/v1/catalog"),
            "OpenAPI spec must include /v1/catalog"
        );
        assert!(
            openapi.paths.paths.contains_key("/v1/catalog/{slug}"),
            "OpenAPI spec must include /v1/catalog/{{slug}}"
        );
        assert!(
            openapi.paths.paths.contains_key("/v2/catalog/publish"),
            "OpenAPI spec must include /v2/catalog/publish"
        );
        assert!(
            openapi
                .paths
                .paths
                .contains_key("/v2/catalog/publish/{request_id}"),
            "OpenAPI spec must include /v2/catalog/publish/{{request_id}}"
        );
        assert!(
            openapi
                .paths
                .paths
                .contains_key("/v2/catalog/publish/{request_id}/artifact"),
            "OpenAPI spec must include /v2/catalog/publish/{{request_id}}/artifact"
        );
        assert!(
            openapi
                .paths
                .paths
                .contains_key("/v2/catalog/publish/{request_id}/validate"),
            "OpenAPI spec must include /v2/catalog/publish/{{request_id}}/validate"
        );
        assert!(
            openapi
                .paths
                .paths
                .contains_key("/v2/catalog/publish/{request_id}/approve"),
            "OpenAPI spec must include /v2/catalog/publish/{{request_id}}/approve"
        );
        assert!(
            openapi
                .paths
                .paths
                .contains_key("/v2/catalog/publish/{request_id}/reject"),
            "OpenAPI spec must include /v2/catalog/publish/{{request_id}}/reject"
        );
        assert!(
            openapi
                .paths
                .paths
                .contains_key("/v2/catalog/publish/{request_id}/stages"),
            "OpenAPI spec must include /v2/catalog/publish/{{request_id}}/stages"
        );
        assert!(
            openapi
                .paths
                .paths
                .contains_key("/v2/catalog/owner-transfer"),
            "OpenAPI spec must include /v2/catalog/owner-transfer"
        );
        assert!(
            openapi.paths.paths.contains_key("/v2/catalog/yank"),
            "OpenAPI spec must include /v2/catalog/yank"
        );
        assert!(
            openapi.paths.paths.contains_key("/api/v1/flex/schemas"),
            "OpenAPI spec must include /api/v1/flex/schemas"
        );
        assert!(
            openapi
                .paths
                .paths
                .contains_key("/api/v1/flex/schemas/{schema_id}/entries/{entry_id}"),
            "OpenAPI spec must include /api/v1/flex/schemas/{{schema_id}}/entries/{{entry_id}}"
        );
    }

    #[test]
    fn registry_only_openapi_filters_non_registry_surface() {
        let mut settings = RustokSettings::default();
        settings.runtime.host_mode = RuntimeHostMode::RegistryOnly;

        let openapi = build_openapi_document(&settings);

        assert!(openapi.paths.paths.contains_key("/v1/catalog"));
        assert!(openapi.paths.paths.contains_key("/v1/catalog/{slug}"));
        assert!(openapi.paths.paths.contains_key("/metrics"));
        assert!(openapi.paths.paths.contains_key("/api/openapi.json"));
        assert!(openapi.paths.paths.contains_key("/api/openapi.yaml"));
        assert!(!openapi.paths.paths.contains_key("/v2/catalog/publish"));
        assert!(
            !openapi
                .paths
                .paths
                .contains_key("/v2/catalog/publish/{request_id}")
        );
        assert!(
            !openapi
                .paths
                .paths
                .contains_key("/v2/catalog/publish/{request_id}/artifact")
        );
        assert!(
            !openapi
                .paths
                .paths
                .contains_key("/v2/catalog/publish/{request_id}/validate")
        );
        assert!(
            !openapi
                .paths
                .paths
                .contains_key("/v2/catalog/publish/{request_id}/approve")
        );
        assert!(
            !openapi
                .paths
                .paths
                .contains_key("/v2/catalog/publish/{request_id}/reject")
        );
        assert!(
            !openapi
                .paths
                .paths
                .contains_key("/v2/catalog/publish/{request_id}/stages")
        );
        assert!(
            !openapi
                .paths
                .paths
                .contains_key("/v2/catalog/owner-transfer")
        );
        assert!(!openapi.paths.paths.contains_key("/v2/catalog/yank"));
        assert!(!openapi.paths.paths.contains_key("/api/auth/login"));
        assert!(!openapi.paths.paths.contains_key("/api/admin/events/dlq"));
        assert!(!openapi.paths.paths.contains_key("/api/v1/flex/schemas"));
    }

    #[cfg(feature = "mod-commerce")]
    #[test]
    fn openapi_merges_commerce_surface_when_mod_commerce_enabled() {
        let openapi = build_openapi_document(&RustokSettings::default());

        assert!(
            openapi.paths.paths.contains_key("/store/carts"),
            "OpenAPI spec must include store cart create path when mod-commerce is enabled"
        );
        assert!(
            openapi.paths.paths.contains_key("/admin/products"),
            "OpenAPI spec must include admin product path when mod-commerce is enabled"
        );
        assert!(
            openapi
                .tags
                .as_ref()
                .is_some_and(|tags| tags.iter().any(|tag| tag.name == "commerce")),
            "OpenAPI spec must advertise commerce tag when mod-commerce is enabled"
        );
    }

    #[cfg(not(feature = "mod-commerce"))]
    #[test]
    fn openapi_excludes_commerce_surface_when_mod_commerce_disabled() {
        let openapi = build_openapi_document(&RustokSettings::default());

        assert!(
            !openapi.paths.paths.contains_key("/store/carts"),
            "Reduced OpenAPI must not include store cart paths when mod-commerce is disabled"
        );
        assert!(
            !openapi.paths.paths.contains_key("/admin/products"),
            "Reduced OpenAPI must not include admin product paths when mod-commerce is disabled"
        );
        assert!(
            !openapi
                .tags
                .as_ref()
                .is_some_and(|tags| tags.iter().any(|tag| tag.name == "commerce")),
            "Reduced OpenAPI must not advertise commerce tag when mod-commerce is disabled"
        );
    }

    #[cfg(all(
        not(feature = "mod-blog"),
        not(feature = "mod-forum"),
        not(feature = "mod-pages")
    ))]
    #[test]
    fn openapi_excludes_content_tags_when_content_modules_are_disabled() {
        let openapi = build_openapi_document(&RustokSettings::default());

        assert!(
            !openapi
                .tags
                .as_ref()
                .is_some_and(|tags| tags.iter().any(|tag| tag.name == "blog")),
            "Reduced OpenAPI must not advertise blog tag when mod-blog is disabled"
        );
        assert!(
            !openapi
                .tags
                .as_ref()
                .is_some_and(|tags| tags.iter().any(|tag| tag.name == "forum")),
            "Reduced OpenAPI must not advertise forum tag when mod-forum is disabled"
        );
        assert!(
            !openapi
                .tags
                .as_ref()
                .is_some_and(|tags| tags.iter().any(|tag| tag.name == "pages")),
            "Reduced OpenAPI must not advertise pages tag when mod-pages is disabled"
        );
    }
}
