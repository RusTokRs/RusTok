use axum::{
    extract::State,
    http::{StatusCode, header::CONTENT_TYPE},
    response::{IntoResponse, Response},
    routing::get,
};
use serde_json::Value;
use std::collections::HashSet;
use utoipa::OpenApi;
use utoipa::openapi::OpenApi as OpenApiDoc;
use utoipa::openapi::security::{
    ApiKey, ApiKeyValue, SecurityRequirement, SecurityScheme,
};

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
        crate::controllers::auth::request_reset,
        crate::controllers::auth::confirm_reset,
        crate::controllers::auth::request_verification,
        crate::controllers::auth::confirm_verification,
        crate::controllers::auth::list_sessions,
        crate::controllers::auth::revoke_all_sessions,
        crate::controllers::auth::change_password,
        crate::controllers::auth::update_profile,
        crate::controllers::auth::login_history,
        crate::controllers::auth::revoke_session,
        // OAuth
        crate::controllers::oauth::token_handler,
        crate::controllers::oauth::authorize_handler,
        crate::controllers::oauth::authorize_browser_handler,
        crate::controllers::oauth::consent_handler,
        crate::controllers::oauth::create_browser_session_handler,
        crate::controllers::oauth::clear_browser_session_handler,
        crate::controllers::oauth::revoke_handler,
        crate::controllers::oauth::userinfo_handler,
        crate::controllers::oauth::userinfo_post_handler,
        // Health
        crate::controllers::health::health,
        crate::controllers::health::live,
        crate::controllers::health::ready,
        crate::controllers::health::runtime,
        crate::controllers::health::modules,
        // Metrics
        crate::controllers::metrics::metrics,
        // Marketplace
        crate::controllers::marketplace_registry::catalog,
        crate::controllers::marketplace_registry::catalog_module,
        crate::controllers::marketplace_registry::publish,
        crate::controllers::marketplace_registry::publish_status,
        crate::controllers::marketplace_registry::upload_publish_artifact,
        crate::controllers::marketplace_registry::download_publish_artifact,
        crate::controllers::marketplace_registry::stage_external_prebuilt,
        crate::controllers::marketplace_registry::stage_platform_build,
        crate::controllers::marketplace_registry::record_author_signature_evidence,
        crate::controllers::marketplace_registry::validate_publish_request_step,
        crate::controllers::marketplace_registry::approve_publish_request,
        crate::controllers::marketplace_registry::reject_publish_request,
        crate::controllers::marketplace_registry::request_changes_publish_request,
        crate::controllers::marketplace_registry::hold_publish_request,
        crate::controllers::marketplace_registry::resume_publish_request,
        crate::controllers::marketplace_registry::report_validation_stage,
        crate::controllers::marketplace_registry::claim_remote_validation_stage,
        crate::controllers::marketplace_registry::heartbeat_remote_validation_stage,
        crate::controllers::marketplace_registry::complete_remote_validation_stage,
        crate::controllers::marketplace_registry::fail_remote_validation_stage,
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
        // Users
        crate::controllers::users::list_users,
        crate::controllers::users::get_user,
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
            crate::controllers::auth::ConfirmResetParams,
            crate::controllers::auth::InviteAcceptResponse,
            crate::controllers::auth::RequestResetParams,
            crate::controllers::auth::RequestVerificationParams,
            crate::controllers::auth::ConfirmVerificationParams,
            crate::controllers::auth::VerificationRequestResponse,
            crate::controllers::auth::ResetRequestResponse,
            crate::controllers::auth::GenericStatusResponse,
            crate::controllers::auth::ChangePasswordParams,
            crate::controllers::auth::UpdateProfileParams,
            crate::controllers::auth::SessionItem,
            crate::controllers::auth::SessionsResponse,
            crate::controllers::auth::UserResponse,
            crate::controllers::auth::AuthResponse,
            crate::controllers::auth::UserInfo,
            crate::controllers::auth::LogoutResponse,

            // OAuth
            crate::controllers::oauth::TokenRequest,
            crate::controllers::oauth::AuthorizeRequest,
            crate::controllers::oauth::BrowserAuthorizeRequest,
            crate::controllers::oauth::ConsentRequest,
            crate::controllers::oauth::RevokeRequest,
            crate::controllers::oauth::TokenResponse,
            crate::controllers::oauth::TokenErrorResponse,
            crate::controllers::oauth::BrowserSessionResponse,

            // Users
            crate::controllers::users::UserItem,
            crate::controllers::users::UsersListParams,
            crate::controllers::users::UsersResponse,

            // Common
            crate::common::PaginationMeta,
            crate::common::ApiError,
            // Marketplace
            crate::services::marketplace_catalog::RegistryCatalogResponse,
            crate::services::marketplace_catalog::RegistryCatalogModule,
            crate::services::marketplace_catalog::RegistryCatalogVersion,
            crate::services::marketplace_catalog::RegistryCatalogArtifactRelease,
            crate::services::marketplace_catalog::RegistryCatalogEvidenceReference,
            crate::services::marketplace_catalog::RegistryMutationResponse,
            crate::services::marketplace_catalog::RegistryPublishRequest,
            crate::services::marketplace_catalog::RegistryPublishDecisionRequest,
            crate::services::marketplace_catalog::RegistryPublishValidationRequest,
            crate::services::marketplace_catalog::RegistryPublishStatusResponse,
            crate::services::marketplace_catalog::RegistryExternalPrebuiltStageRequest,
            crate::services::marketplace_catalog::RegistryExternalPrebuiltStageResponse,
            crate::services::marketplace_catalog::RegistryPlatformBuildStageRequest,
            crate::services::marketplace_catalog::RegistryPlatformBuildStageResponse,
            crate::services::marketplace_catalog::RegistryAuthorSignatureEvidenceRequest,
            crate::services::marketplace_catalog::RegistryPublishArtifactOrigin,
            crate::services::marketplace_catalog::RegistryPublishModuleRequest,
            crate::services::marketplace_catalog::RegistryPublishMarketplaceRequest,
            crate::services::marketplace_catalog::RegistryPublishUiPackagesRequest,
            crate::services::marketplace_catalog::RegistryPublishUiPackageRequest,
            crate::services::marketplace_catalog::RegistryYankRequest,
            crate::services::marketplace_catalog::RegistryValidationStageReportRequest,
            crate::services::marketplace_catalog::RegistryRunnerClaimRequest,
            crate::services::marketplace_catalog::RegistryRunnerClaimResponse,
            crate::services::marketplace_catalog::RegistryRunnerClaimPayload,
            crate::services::marketplace_catalog::RegistryRunnerHeartbeatRequest,
            crate::services::marketplace_catalog::RegistryRunnerCompletionRequest,
            crate::services::marketplace_catalog::RegistryRunnerMutationResponse,
            crate::services::marketplace_catalog::RegistryOwnerTransferRequest,
            // Runtime guardrails
            crate::services::runtime_guardrails::RuntimeGuardrailSnapshot,
            crate::services::runtime_guardrails::RuntimeGuardrailStatus,
            crate::services::runtime_guardrails::RuntimeGuardrailRollout,
            crate::services::runtime_guardrails::RateLimitGuardrailSnapshot,
            crate::services::runtime_guardrails::RateLimitPolicySnapshot,
            crate::services::runtime_guardrails::EventBusGuardrailSnapshot,
            crate::services::runtime_guardrails::EventTransportGuardrailSnapshot,
            crate::services::runtime_guardrails::RemoteExecutorGuardrailSnapshot,
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
        (name = "admin", description = "Admin operations"),
        (name = "users", description = "User administration endpoints")
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
    "/catalog",
    "/catalog/{slug}",
    "/api/openapi.json",
    "/api/openapi.yaml",
];

pub fn build_openapi_document(settings: &RustokSettings) -> OpenApiDoc {
    let mut openapi = ApiDoc::openapi();
    #[cfg(not(feature = "mod-flex"))]
    prune_disabled_flex_surface(&mut openapi);
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
        prune_unused_components(&mut openapi);
    }
    openapi
}

#[cfg(not(feature = "mod-flex"))]
fn prune_disabled_flex_surface(openapi: &mut OpenApiDoc) {
    openapi
        .paths
        .paths
        .retain(|path, _| !path.starts_with("/api/v1/flex/"));
    prune_unused_components(openapi);
}

fn prune_unused_components(openapi: &mut OpenApiDoc) {
    let path_document = match serde_json::to_value(&openapi.paths) {
        Ok(value) => value,
        Err(error) => {
            tracing::error!(%error, "Failed to serialize OpenAPI paths for component pruning");
            return;
        }
    };

    let mut schema_names = HashSet::new();
    let mut security_names = HashSet::new();
    let mut tag_names = HashSet::new();
    collect_component_references(
        &path_document,
        &mut schema_names,
        &mut security_names,
        &mut tag_names,
    );

    if let Some(global_security) = openapi.security.as_ref() {
        if let Ok(value) = serde_json::to_value(global_security) {
            let mut unused_schema_names = HashSet::new();
            let mut unused_tag_names = HashSet::new();
            collect_component_references(
                &value,
                &mut unused_schema_names,
                &mut security_names,
                &mut unused_tag_names,
            );
        }
    }

    if let Some(components) = openapi.components.as_mut() {
        // Schema components can recursively reference other schema components.
        let mut pending: Vec<String> = schema_names.iter().cloned().collect();
        let mut index = 0;
        while index < pending.len() {
            let name = &pending[index];
            index += 1;

            let Some(schema) = components.schemas.get(name) else {
                continue;
            };
            let schema_document = match serde_json::to_value(schema) {
                Ok(value) => value,
                Err(error) => {
                    tracing::error!(
                        %error,
                        schema = %name,
                        "Failed to serialize OpenAPI schema during component pruning"
                    );
                    continue;
                }
            };

            let mut nested_schema_names = HashSet::new();
            let mut unused_security_names = HashSet::new();
            let mut unused_tag_names = HashSet::new();
            collect_component_references(
                &schema_document,
                &mut nested_schema_names,
                &mut unused_security_names,
                &mut unused_tag_names,
            );

            for nested in nested_schema_names {
                if schema_names.insert(nested.clone()) {
                    pending.push(nested);
                }
            }
        }

        components
            .schemas
            .retain(|name, _| schema_names.contains(name));
        components
            .security_schemes
            .retain(|name, _| security_names.contains(name));
    }

    if let Some(tags) = openapi.tags.as_mut() {
        tags.retain(|tag| tag_names.contains(&tag.name));
    }
}

fn collect_component_references(
    value: &Value,
    schema_names: &mut HashSet<String>,
    security_names: &mut HashSet<String>,
    tag_names: &mut HashSet<String>,
) {
    match value {
        Value::Array(values) => {
            for value in values {
                collect_component_references(
                    value,
                    schema_names,
                    security_names,
                    tag_names,
                );
            }
        }
        Value::Object(map) => {
            if let Some(Value::String(reference)) = map.get("$ref") {
                const SCHEMA_PREFIX: &str = "#/components/schemas/";
                if let Some(name) = reference.strip_prefix(SCHEMA_PREFIX) {
                    schema_names.insert(name.to_string());
                }
            }

            if let Some(Value::Array(requirements)) = map.get("security") {
                for requirement in requirements {
                    if let Value::Object(entries) = requirement {
                        security_names.extend(entries.keys().cloned());
                    }
                }
            }

            if let Some(Value::Array(tags)) = map.get("tags") {
                for tag in tags {
                    if let Value::String(name) = tag {
                        tag_names.insert(name.clone());
                    }
                }
            }

            for value in map.values() {
                collect_component_references(
                    value,
                    schema_names,
                    security_names,
                    tag_names,
                );
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
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
                SecurityScheme::Http(
                    utoipa::openapi::security::HttpBuilder::new()
                        .scheme(utoipa::openapi::security::HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .build(),
                ),
            );
            components.add_security_scheme(
                "runner_token",
                SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::with_description(
                    "x-rustok-runner-token",
                    "Shared token required for remote registry validation runner operations.",
                ))),
            );
        }

        // Remote runner operations are authenticated by a dedicated shared
        // header rather than a user/session bearer token. Keep that distinction
        // explicit in the machine-readable contract.
        for path in [
            "/v2/catalog/runner/claim",
            "/v2/catalog/runner/{claim_id}/heartbeat",
            "/v2/catalog/runner/{claim_id}/complete",
            "/v2/catalog/runner/{claim_id}/fail",
        ] {
            if let Some(operation) = openapi.paths.paths.get_mut(path).and_then(|item| item.post.as_mut()) {
                operation.security = Some(vec![SecurityRequirement::new(
                    "runner_token",
                    std::iter::empty::<String>(),
                )]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ApiDoc, build_openapi_document};
    use crate::common::settings::{RuntimeHostMode, RustokSettings};
    use utoipa::OpenApi;



    #[test]
    fn openapi_includes_all_documented_core_paths() {
        let openapi = ApiDoc::openapi();

        let expected_paths = [
            "/api/auth/register",
            "/api/auth/login",
            "/api/auth/refresh",
            "/api/auth/logout",
            "/api/auth/me",
            "/api/auth/invite/accept",
            "/api/auth/reset/request",
            "/api/auth/reset/confirm",
            "/api/auth/verify/request",
            "/api/auth/verify/confirm",
            "/api/auth/sessions",
            "/api/auth/sessions/revoke-all",
            "/api/auth/change-password",
            "/api/auth/profile",
            "/api/auth/history",
            "/api/auth/sessions/{id}",
            "/health",
            "/health/live",
            "/health/ready",
            "/health/runtime",
            "/health/modules",
            "/metrics",
            "/catalog",
            "/catalog/{slug}",
            "/v2/catalog/publish",
            "/v2/catalog/publish/{request_id}",
            "/v2/catalog/publish/{request_id}/artifact",
            "/v2/catalog/publish/{request_id}/artifact/download",
            "/v2/catalog/publish/{request_id}/external-prebuilt-stage",
            "/v2/catalog/publish/{request_id}/platform-build-stage",
            "/v2/catalog/publish/{request_id}/author-signature",
            "/v2/catalog/publish/{request_id}/validate",
            "/v2/catalog/publish/{request_id}/stages",
            "/v2/catalog/publish/{request_id}/approve",
            "/v2/catalog/publish/{request_id}/reject",
            "/v2/catalog/publish/{request_id}/request-changes",
            "/v2/catalog/publish/{request_id}/hold",
            "/v2/catalog/publish/{request_id}/resume",
            "/v2/catalog/runner/claim",
            "/v2/catalog/runner/{claim_id}/heartbeat",
            "/v2/catalog/runner/{claim_id}/complete",
            "/v2/catalog/runner/{claim_id}/fail",
            "/v2/catalog/yank",
            "/v2/catalog/owner-transfer",
            "/api/rbac/artifact-permissions/roles/{role_id}",
            "/api/admin/events/dlq",
            "/api/admin/events/dlq/{id}/replay",
            "/api/users",
            "/api/users/{id}",
            "/api/v1/flex/schemas",
            "/api/v1/flex/schemas/{schema_id}",
            "/api/v1/flex/schemas/{schema_id}/entries",
            "/api/v1/flex/schemas/{schema_id}/entries/{entry_id}",
            "/api/openapi.json",
            "/api/openapi.yaml",
        ];

        for path in expected_paths {
            assert!(
                openapi.paths.paths.contains_key(path),
                "OpenAPI spec must include documented core path {path}"
            );
        }
    }

    #[test]
    fn openapi_marks_remote_runner_operations_with_runner_auth() {
        let openapi = ApiDoc::openapi();

        let security = openapi
            .paths
            .get_path_operation(
                "/v2/catalog/runner/claim",
                utoipa::openapi::HttpMethod::Post,
            )
            .and_then(|operation| operation.security.as_ref())
            .expect("runner claim must require runner_token");

        assert_eq!(
            serde_json::to_value(security).expect("security serializes"),
            serde_json::json!([{ "runner_token": [] }])
        );

        let runner_scheme = openapi
            .components
            .as_ref()
            .and_then(|components| components.security_schemes.get("runner_token"))
            .expect("runner_token security scheme must exist");

        assert_eq!(
            serde_json::to_value(runner_scheme).expect("runner scheme serializes"),
            serde_json::json!({
                "type": "apiKey",
                "in": "header",
                "name": "x-rustok-runner-token",
                "description": "Shared token required for remote registry validation runner operations."
            })
        );
    }

    #[test]
    fn openapi_marks_marketplace_registry_auth_boundaries() {
        let openapi = ApiDoc::openapi();

        use utoipa::openapi::HttpMethod;

        for (path, method) in [
            ("/v2/catalog/publish/{request_id}", HttpMethod::Get),
            ("/v2/catalog/publish/{request_id}/artifact", HttpMethod::Put),
            (
                "/v2/catalog/publish/{request_id}/external-prebuilt-stage",
                HttpMethod::Post,
            ),
            (
                "/v2/catalog/publish/{request_id}/platform-build-stage",
                HttpMethod::Post,
            ),
            (
                "/v2/catalog/publish/{request_id}/author-signature",
                HttpMethod::Post,
            ),
            ("/v2/catalog/publish/{request_id}/validate", HttpMethod::Post),
            ("/v2/catalog/publish/{request_id}/stages", HttpMethod::Post),
            ("/v2/catalog/publish/{request_id}/approve", HttpMethod::Post),
            ("/v2/catalog/publish/{request_id}/reject", HttpMethod::Post),
            (
                "/v2/catalog/publish/{request_id}/request-changes",
                HttpMethod::Post,
            ),
            ("/v2/catalog/publish/{request_id}/hold", HttpMethod::Post),
            ("/v2/catalog/publish/{request_id}/resume", HttpMethod::Post),
            ("/v2/catalog/yank", HttpMethod::Post),
            ("/v2/catalog/owner-transfer", HttpMethod::Post),
        ] {
            let security = openapi
                .paths
                .get_path_operation(path, method)
                .and_then(|operation| operation.security.as_ref())
                .unwrap_or_else(|| panic!("marketplace operation must require bearer auth: {path}"));

            assert_eq!(
                serde_json::to_value(security).expect("security serializes"),
                serde_json::json!([{ "bearer_auth": [] }]),
                "unexpected security contract for {path}"
            );
        }

        assert!(
            openapi
                .paths
                .get_path_operation("/v2/catalog/publish", HttpMethod::Post)
                .and_then(|operation| operation.security.as_ref())
                .is_none(),
            "publish dry-run remains anonymously documented"
        );

        assert!(
            openapi
                .paths
                .get_path_operation("/v2/catalog/publish/{request_id}", HttpMethod::Get)
                .and_then(|operation| operation.security.as_ref())
                .is_none(),
            "publish status remains anonymously readable"
        );

        let download_security = openapi
            .paths
            .get_path_operation(
                "/v2/catalog/publish/{request_id}/artifact/download",
                HttpMethod::Get,
            )
            .and_then(|operation| operation.security.as_ref())
            .expect("artifact download must expose bearer-or-runner authentication");

        assert_eq!(
            serde_json::to_value(download_security).expect("security serializes"),
            serde_json::json!([
                { "bearer_auth": [] },
                { "runner_token": [] }
            ])
        );
    }

    #[test]
    fn openapi_documents_expected_methods_on_shared_paths() {
        let openapi = ApiDoc::openapi();

        use utoipa::openapi::HttpMethod;

        for (path, method, method_name) in [
            (
                "/api/rbac/artifact-permissions/roles/{role_id}",
                HttpMethod::Put,
                "PUT",
            ),
            (
                "/api/rbac/artifact-permissions/roles/{role_id}",
                HttpMethod::Delete,
                "DELETE",
            ),
            ("/api/users", HttpMethod::Get, "GET"),
            ("/api/v1/flex/schemas", HttpMethod::Get, "GET"),
            ("/api/v1/flex/schemas", HttpMethod::Post, "POST"),
            (
                "/api/v1/flex/schemas/{schema_id}",
                HttpMethod::Get,
                "GET",
            ),
            (
                "/api/v1/flex/schemas/{schema_id}",
                HttpMethod::Put,
                "PUT",
            ),
            (
                "/api/v1/flex/schemas/{schema_id}",
                HttpMethod::Delete,
                "DELETE",
            ),
            (
                "/api/v1/flex/schemas/{schema_id}/entries",
                HttpMethod::Get,
                "GET",
            ),
            (
                "/api/v1/flex/schemas/{schema_id}/entries",
                HttpMethod::Post,
                "POST",
            ),
            (
                "/api/v1/flex/schemas/{schema_id}/entries/{entry_id}",
                HttpMethod::Get,
                "GET",
            ),
            (
                "/api/v1/flex/schemas/{schema_id}/entries/{entry_id}",
                HttpMethod::Put,
                "PUT",
            ),
            (
                "/api/v1/flex/schemas/{schema_id}/entries/{entry_id}",
                HttpMethod::Delete,
                "DELETE",
            ),
        ] {
            assert!(
                openapi.paths.get_path_operation(path, method).is_some(),
                "OpenAPI spec must include {method_name} operation for {path}"
            );
        }
    }

    #[test]
    fn openapi_declares_documented_runtime_schemas() {
        let openapi = ApiDoc::openapi();
        let schemas = openapi
            .components
            .as_ref()
            .expect("OpenAPI components must exist");

        for name in [
            "ResetRequestResponse",
            "SessionsResponse",
            "RuntimeGuardrailSnapshot",
            "RegistryPublishValidationRequest",
            "RegistryValidationStageReportRequest",
            "RegistryRunnerClaimRequest",
            "RegistryRunnerClaimResponse",
            "RegistryRunnerClaimPayload",
            "RegistryRunnerHeartbeatRequest",
            "RegistryRunnerCompletionRequest",
            "RegistryRunnerMutationResponse",
            "RegistryOwnerTransferRequest",
        ] {
            assert!(
                schemas.schemas.contains_key(name),
                "OpenAPI components must contain schema {name}"
            );
        }
    }

    #[test]
    fn openapi_includes_registry_catalog_path() {
        let openapi = ApiDoc::openapi();

        assert!(
            openapi.paths.paths.contains_key("/catalog"),
            "OpenAPI spec must include /catalog"
        );
        assert!(
            openapi.paths.paths.contains_key("/catalog/{slug}"),
            "OpenAPI spec must include /catalog/{{slug}}"
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
                .contains_key("/v2/catalog/publish/{request_id}/author-signature"),
            "OpenAPI spec must include /v2/catalog/publish/{{request_id}}/author-signature"
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

        assert!(openapi.paths.paths.contains_key("/catalog"));
        assert!(openapi.paths.paths.contains_key("/catalog/{slug}"));
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

    #[cfg(not(feature = "mod-flex"))]
    #[test]
    fn disabled_flex_openapi_surface_is_not_advertised() {
        let settings = RustokSettings::default();
        let openapi = build_openapi_document(&settings);

        assert!(!openapi.paths.paths.contains_key("/api/v1/flex/schemas"));
        assert!(
            !openapi
                .paths
                .paths
                .keys()
                .any(|path| path.starts_with("/api/v1/flex/")),
            "disabled Flex must not appear in the public OpenAPI path set"
        );

        let components = openapi
            .components
            .as_ref()
            .expect("OpenAPI components must exist");
        assert!(
            !components.schemas.contains_key("CreateFlexSchemaRequest"),
            "disabled Flex request schemas must not remain publicly advertised"
        );
        assert!(
            !components
                .schemas
                .contains_key("FlexSchemaResponse"),
            "disabled Flex response schemas must not remain publicly advertised"
        );
    }

    #[cfg(feature = "mod-blog")]
    #[test]
    fn blog_openapi_document_builds_independently() {
        let openapi = rustok_blog::openapi::openapi_document();

        assert!(openapi.paths.paths.contains_key("/api/blog/posts"));
    }

    #[cfg(feature = "mod-forum")]
    #[test]
    fn forum_openapi_document_builds_independently() {
        let openapi = rustok_forum::openapi::openapi_document();

        assert!(openapi.paths.paths.contains_key("/api/forum/categories"));
    }

    #[cfg(feature = "mod-pages")]
    #[test]
    fn pages_openapi_document_builds_independently() {
        let openapi = rustok_pages::openapi::openapi_document();

        assert!(openapi.paths.paths.contains_key("/api/pages"));
    }

    #[cfg(feature = "mod-commerce")]
    #[test]
    fn commerce_openapi_document_builds_independently() {
        let openapi = rustok_commerce::openapi::openapi_document();

        assert!(openapi.paths.paths.contains_key("/store/carts"));
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
