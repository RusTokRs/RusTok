pub mod queries;

#[cfg(target_arch = "wasm32")]
use gloo_storage::Storage as GlooStorage;
use leptos::prelude::*;
#[cfg(not(any(
    all(target_arch = "wasm32", feature = "csr", not(feature = "hydrate")),
    feature = "ssr"
)))]
use leptos_graphql::GraphqlHttpError;
#[cfg(any(
    all(target_arch = "wasm32", feature = "csr", not(feature = "hydrate")),
    feature = "ssr"
))]
use leptos_graphql::{
    execute as execute_graphql, persisted_query_extension, GraphqlHttpError, GraphqlRequest,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct ApiRequestContext {
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ServerGraphqlRequest {
    query: String,
    variables: Value,
    persisted_query_sha256: Option<String>,
    context: ApiRequestContext,
}

pub fn get_graphql_url() -> String {
    if let Some(url) = option_env!("RUSTOK_GRAPHQL_URL") {
        return url.to_string();
    }

    if let Some(base) = option_env!("RUSTOK_API_URL") {
        return format!("{}/api/graphql", base.trim_end_matches('/'));
    }

    #[cfg(target_arch = "wasm32")]
    {
        let origin = web_sys::window()
            .and_then(|w| w.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:5150".to_string());
        format!("{}/api/graphql", origin)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let base =
            std::env::var("RUSTOK_API_URL").unwrap_or_else(|_| "http://localhost:5150".to_string());
        format!("{}/api/graphql", base)
    }
}

pub fn api_base_url() -> String {
    get_graphql_url()
        .trim_end_matches("/api/graphql")
        .trim_end_matches('/')
        .to_string()
}

pub fn get_graphql_ws_url() -> String {
    let graphql_url = get_graphql_url();
    let ws_base = if let Some(rest) = graphql_url.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = graphql_url.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        graphql_url
    };

    format!("{}/ws", ws_base.trim_end_matches('/'))
}

pub type ApiError = GraphqlHttpError;

pub fn combine_native_and_graphql_error(
    server_err: leptos::prelude::ServerFnError,
    graphql_err: ApiError,
) -> ApiError {
    let payload = serde_json::json!({
        "kind": "dual_path_failure",
        "native": server_err.to_string(),
        "graphql": graphql_err.to_string(),
    });
    ApiError::Graphql(format!("dual-path failure {}", payload))
}

/// Read the admin UI locale from LocalStorage.
/// Returns None in non-WASM environments or if the key is absent.
pub fn get_stored_locale() -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        gloo_storage::LocalStorage::get::<String>("rustok-admin-locale").ok()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        None
    }
}

fn build_request_context(token: Option<String>, tenant_slug: Option<String>) -> ApiRequestContext {
    ApiRequestContext {
        token,
        tenant_slug,
        locale: get_stored_locale(),
    }
}

#[cfg(any(
    all(target_arch = "wasm32", feature = "csr", not(feature = "hydrate")),
    feature = "ssr"
))]
async fn execute_server_graphql(request: ServerGraphqlRequest) -> Result<Value, GraphqlHttpError> {
    let mut graphql_request = GraphqlRequest::new(request.query, Some(request.variables));

    if let Some(sha256_hash) = request.persisted_query_sha256.as_deref() {
        graphql_request = graphql_request.with_extensions(persisted_query_extension(sha256_hash));
    }

    execute_graphql(
        &get_graphql_url(),
        graphql_request,
        request.context.token,
        request.context.tenant_slug,
        request.context.locale,
    )
    .await
}

async fn execute_admin_graphql(request: ServerGraphqlRequest) -> Result<Value, ApiError> {
    #[cfg(all(target_arch = "wasm32", feature = "csr", not(feature = "hydrate")))]
    {
        execute_server_graphql(request)
            .await
            .map_err(ApiError::from)
    }

    #[cfg(not(all(target_arch = "wasm32", feature = "csr", not(feature = "hydrate"))))]
    {
        admin_graphql(request).await.map_err(map_server_fn_error)
    }
}

#[cfg(not(all(target_arch = "wasm32", feature = "csr", not(feature = "hydrate"))))]
fn map_server_fn_error(error: ServerFnError) -> ApiError {
    let message = error.to_string();

    normalize_server_fn_error_message(&message)
}

#[cfg(not(all(target_arch = "wasm32", feature = "csr", not(feature = "hydrate"))))]
fn normalize_server_fn_error_message(message: &str) -> ApiError {
    if message == "Unauthorized" {
        return ApiError::Unauthorized;
    }
    if message == "Network error" {
        return ApiError::Network;
    }
    if let Some(value) = message.strip_prefix("Http error: ") {
        return ApiError::Http(value.to_string());
    }
    if let Some(value) = message.strip_prefix("GraphQL error: ") {
        return ApiError::Graphql(value.to_string());
    }

    ApiError::Graphql(message.to_string())
}

#[cfg(all(test, not(all(target_arch = "wasm32", feature = "csr", not(feature = "hydrate")))))]
mod map_server_fn_error_tests {
    use super::{
        combine_native_and_graphql_error, map_server_fn_error, normalize_server_fn_error_message,
    };
    use leptos::prelude::ServerFnError;
    use leptos_graphql::GraphqlHttpError;

    #[test]
    fn maps_well_known_transport_errors() {
        assert!(matches!(
            map_server_fn_error(ServerFnError::new("Unauthorized")),
            GraphqlHttpError::Unauthorized
        ));
        assert!(matches!(
            map_server_fn_error(ServerFnError::new("Network error")),
            GraphqlHttpError::Network
        ));
    }

    #[test]
    fn maps_graphql_prefix_without_losing_error_taxonomy() {
        let mapped = map_server_fn_error(ServerFnError::new(
            "GraphQL error: MODULE_HAS_DEPENDENTS: module 'checkout' has dependents",
        ));
        assert!(matches!(mapped, GraphqlHttpError::Graphql(message) if message.contains("MODULE_HAS_DEPENDENTS")));
    }

    #[test]
    fn lifecycle_runtime_taxonomy_matrix_is_forwarded_without_remapping() {
        let cases = [
            "UNKNOWN_MODULE: module 'ghost' not found",
            "CORE_MODULE: module 'core' cannot be disabled",
            "MISSING_DEPENDENCIES: module 'checkout' requires 'catalog'",
            "HAS_DEPENDENTS: module 'catalog' required by 'checkout'",
            "MODULE_HOOK_FAILED: post-disable hook failed",
        ];

        for case in cases {
            let mapped = map_server_fn_error(ServerFnError::new(format!("GraphQL error: {case}")));
            assert!(
                matches!(mapped, GraphqlHttpError::Graphql(message) if message == case),
                "expected runtime taxonomy message to be forwarded unchanged for case {case}"
            );
        }
    }

    #[test]
    fn lifecycle_journal_metadata_fragments_are_forwarded_without_parsing() {
        let payload = "GraphQL error: MODULE_HOOK_FAILED {\"extensions\":{\"code\":\"MODULE_HOOK_FAILED\",\"correlation_id\":\"9d7987b8-0740-45ea-bf2f-8b3329e1733e\",\"requested_by\":\"admin:user-77\",\"status\":\"failed\",\"previous_effective_enabled\":true}}";
        let mapped = map_server_fn_error(ServerFnError::new(payload));

        assert!(
            matches!(mapped, GraphqlHttpError::Graphql(message)
                if message.contains("correlation_id")
                && message.contains("requested_by")
                && message.contains("previous_effective_enabled")
                && message.contains("\"status\":\"failed\"")),
            "expected server-owned journal metadata fragments to pass through unchanged"
        );
    }

    #[test]
    fn lifecycle_operation_status_matrix_is_forwarded_without_local_parsing() {
        let statuses = ["validated", "running", "committed", "failed"];

        for status in statuses {
            let payload = format!(
                "GraphQL error: MODULE_HOOK_FAILED {{\"extensions\":{{\"code\":\"MODULE_HOOK_FAILED\",\"status\":\"{status}\",\"correlation_id\":\"74bf5a96-4997-48ea-a0f0-a8eb9a34189f\"}}}}"
            );
            let mapped = map_server_fn_error(ServerFnError::new(payload.clone()));

            assert!(
                matches!(mapped, GraphqlHttpError::Graphql(message)
                    if message.contains(&format!("\"status\":\"{status}\""))
                    && message.contains("\"correlation_id\":\"74bf5a96-4997-48ea-a0f0-a8eb9a34189f\"")),
                "status `{status}` must pass through unchanged without local remapping"
            );
        }
    }

    #[test]
    fn lifecycle_retryable_issue_fragments_are_forwarded_without_local_parsing() {
        let payload = "GraphQL error: MODULE_HOOK_FAILED {\"extensions\":{\"code\":\"MODULE_HOOK_FAILED\",\"status\":\"failed\",\"retryable_issue\":true,\"retryable\":true,\"operation_issue\":\"post_hook_failed\",\"correlation_id\":\"12f63773-912f-432a-8fa4-c40448626d13\"}}";
        let mapped = map_server_fn_error(ServerFnError::new(payload));

        assert!(
            matches!(mapped, GraphqlHttpError::Graphql(message)
                if message.contains("\"retryable_issue\":true")
                && message.contains("\"retryable\":true")
                && message.contains("\"operation_issue\":\"post_hook_failed\"")
                && message.contains("\"correlation_id\":\"12f63773-912f-432a-8fa4-c40448626d13\"")),
            "retryable issue fragments must pass through unchanged without adapter-side interpretation"
        );
    }

    #[test]
    fn lifecycle_taxonomy_extensions_are_forwarded_without_local_normalization() {
        let payload = "GraphQL error: MISSING_DEPENDENCIES {\"extensions\":{\"code\":\"MISSING_DEPENDENCIES\",\"reason_code\":\"dependency_missing\",\"requested_by\":\"admin:user-2\"}}";
        let mapped = map_server_fn_error(ServerFnError::new(payload));

        assert!(
            matches!(mapped, GraphqlHttpError::Graphql(message)
                if message.contains("\"code\":\"MISSING_DEPENDENCIES\"")
                && message.contains("\"reason_code\":\"dependency_missing\"")
                && message.contains("\"requested_by\":\"admin:user-2\"")),
            "expected extensions payload to be forwarded unchanged without local normalization"
        );
    }

    #[test]
    fn graphql_prefixed_lifecycle_payload_is_not_misclassified_as_transport_error() {
        let mapped = map_server_fn_error(ServerFnError::new(
            "GraphQL error: Unauthorized due to MODULE_HOOK_FAILED",
        ));
        assert!(
            matches!(mapped, GraphqlHttpError::Graphql(message)
                if message == "Unauthorized due to MODULE_HOOK_FAILED"),
            "GraphQL-prefixed lifecycle payload must remain Graphql variant and not become Unauthorized transport error"
        );
    }

    #[test]
    fn plain_lifecycle_payload_with_unauthorized_word_stays_graphql_variant() {
        let mapped = map_server_fn_error(ServerFnError::new(
            "MODULE_HOOK_FAILED: Unauthorized downstream permission cache",
        ));
        assert!(
            matches!(mapped, GraphqlHttpError::Graphql(message)
                if message == "MODULE_HOOK_FAILED: Unauthorized downstream permission cache"),
            "non-transport lifecycle payload containing Unauthorized must not be normalized to transport Unauthorized variant"
        );
    }

    #[test]
    fn plain_lifecycle_payload_with_network_word_stays_graphql_variant() {
        let mapped = map_server_fn_error(ServerFnError::new(
            "MODULE_HOOK_FAILED: Network dependency probe failed in post-hook",
        ));
        assert!(
            matches!(mapped, GraphqlHttpError::Graphql(message)
                if message == "MODULE_HOOK_FAILED: Network dependency probe failed in post-hook"),
            "non-transport lifecycle payload containing Network must not be normalized to transport Network variant"
        );
    }

    #[test]
    fn composition_runtime_taxonomy_matrix_is_forwarded_without_remapping() {
        let cases = [
            "REVISION_CONFLICT: Platform composition revision conflict: expected 7, current 11",
            "INVALID_MODULE: module 'bad slug' is invalid",
            "REQUIRED_MODULE: module 'pages' is required",
            "UNKNOWN_DEPENDENCY: module 'checkout' references unknown dependency 'pricing'",
            "INTERNAL_ERROR: failed to enqueue build",
        ];

        for case in cases {
            let mapped = map_server_fn_error(ServerFnError::new(format!("GraphQL error: {case}")));
            assert!(
                matches!(mapped, GraphqlHttpError::Graphql(message) if message == case),
                "expected composition taxonomy message to be forwarded unchanged for case {case}"
            );
        }
    }

    #[test]
    fn composition_manifest_fragments_are_forwarded_without_local_parsing() {
        let payload = "GraphQL error: REVISION_CONFLICT {\"extensions\":{\"code\":\"REVISION_CONFLICT\",\"manifest_ref\":\"platform_state:42\",\"manifest_revision\":42,\"expected_revision\":41}}";
        let mapped = map_server_fn_error(ServerFnError::new(payload));

        assert!(
            matches!(mapped, GraphqlHttpError::Graphql(message)
                if message.contains("\"manifest_ref\":\"platform_state:42\"")
                && message.contains("\"manifest_revision\":42")
                && message.contains("\"expected_revision\":41")),
            "expected manifest revision fragments to be forwarded unchanged without local parsing"
        );
    }

    #[test]
    fn graphql_prefixed_composition_payload_with_unauthorized_word_stays_graphql_variant() {
        let mapped = map_server_fn_error(ServerFnError::new(
            "GraphQL error: REVISION_CONFLICT: Unauthorized actor tried stale revision write",
        ));
        assert!(
            matches!(mapped, GraphqlHttpError::Graphql(message)
                if message == "REVISION_CONFLICT: Unauthorized actor tried stale revision write"),
            "GraphQL-prefixed composition payload must remain Graphql variant and not become Unauthorized transport error"
        );
    }

    #[test]
    fn graphql_prefixed_composition_payload_with_network_word_stays_graphql_variant() {
        let mapped = map_server_fn_error(ServerFnError::new(
            "GraphQL error: INTERNAL_ERROR: Network jitter during build enqueue",
        ));
        assert!(
            matches!(mapped, GraphqlHttpError::Graphql(message)
                if message == "INTERNAL_ERROR: Network jitter during build enqueue"),
            "GraphQL-prefixed composition payload must remain Graphql variant and not become Network transport error"
        );
    }

    #[test]
    fn lifecycle_and_composition_taxonomy_with_transport_words_keep_graphql_variant_matrix() {
        let cases = [
            "GraphQL error: UNKNOWN_MODULE: Unauthorized module slug access",
            "GraphQL error: MODULE_HOOK_FAILED: Network post-hook timeout",
            "GraphQL error: REVISION_CONFLICT: Unauthorized stale revision write",
            "GraphQL error: INTERNAL_ERROR: Network build queue unavailable",
        ];

        for case in cases {
            let mapped = map_server_fn_error(ServerFnError::new(case));
            let expected = case.trim_start_matches("GraphQL error: ").to_string();
            assert!(
                matches!(mapped, GraphqlHttpError::Graphql(message) if message == expected),
                "taxonomy payload `{case}` must stay Graphql variant even with transport-like words"
            );
        }
    }

    #[test]
    fn graphql_prefixed_composition_extensions_with_transport_words_stay_graphql_variant() {
        let payload = "GraphQL error: REVISION_CONFLICT {\"extensions\":{\"code\":\"REVISION_CONFLICT\",\"manifest_ref\":\"platform_state:44\",\"manifest_revision\":44,\"expected_revision\":43,\"detail\":\"Unauthorized actor during Network partition\"}}";
        let mapped = map_server_fn_error(ServerFnError::new(payload));

        assert!(
            matches!(mapped, GraphqlHttpError::Graphql(message)
                if message.contains("\"code\":\"REVISION_CONFLICT\"")
                && message.contains("\"manifest_ref\":\"platform_state:44\"")
                && message.contains("Unauthorized actor during Network partition")),
            "GraphQL-prefixed composition extensions payload must remain Graphql variant and preserve extension fragments verbatim"
        );
    }

    #[test]
    fn graphql_prefixed_taxonomy_payloads_with_transport_prefix_strings_stay_graphql_variant() {
        let cases = [
            "GraphQL error: REVISION_CONFLICT: Http error: 409 stale revision",
            "GraphQL error: UNKNOWN_MODULE: Http error: module not found",
            "GraphQL error: MODULE_HOOK_FAILED: Unauthorized downstream scope mismatch",
            "GraphQL error: INTERNAL_ERROR: Network saturation during enqueue",
        ];

        for case in cases {
            let mapped = map_server_fn_error(ServerFnError::new(case));
            let expected = case.trim_start_matches("GraphQL error: ").to_string();
            assert!(
                matches!(mapped, GraphqlHttpError::Graphql(message) if message == expected),
                "GraphQL-prefixed taxonomy payload `{case}` must stay Graphql variant without transport remap"
            );
        }
    }

    #[test]
    fn pure_graphql_prefixed_http_payload_is_not_misclassified_as_transport_http() {
        let payload = "GraphQL error: Http error: 409 stale revision conflict";
        let mapped = map_server_fn_error(ServerFnError::new(payload));
        assert!(
            matches!(mapped, GraphqlHttpError::Graphql(message)
                if message == "Http error: 409 stale revision conflict"),
            "GraphQL-prefixed payload must remain Graphql variant even when inner message starts with `Http error:`"
        );
    }

    #[test]
    fn maps_http_prefix_and_preserves_payload() {
        let mapped = normalize_server_fn_error_message("Http error: 409 conflict");
        assert!(matches!(mapped, GraphqlHttpError::Http(message) if message == "409 conflict"));
    }

    #[test]
    fn unknown_server_errors_fallback_to_graphql_variant() {
        let mapped = normalize_server_fn_error_message("internal adapter panic");
        assert!(matches!(mapped, GraphqlHttpError::Graphql(message) if message == "internal adapter panic"));
    }

    #[test]
    fn combined_error_keeps_structured_dual_path_and_taxonomy() {
        let combined = combine_native_and_graphql_error(
            ServerFnError::new("native disabled"),
            GraphqlHttpError::Graphql("MODULE_HAS_DEPENDENTS".to_string()),
        );
        assert!(matches!(combined, GraphqlHttpError::Graphql(message)
            if message.contains("dual-path failure")
            && message.contains("\"kind\":\"dual_path_failure\"")
            && message.contains("\"native\":\"native disabled\"")
            && message.contains("\"graphql\":\"MODULE_HAS_DEPENDENTS\"")));
    }

    #[test]
    fn combined_error_escapes_special_chars_in_json_payload() {
        let combined = combine_native_and_graphql_error(
            ServerFnError::new("native [disabled]"),
            GraphqlHttpError::Graphql("MODULE_HAS_DEPENDENTS: \"quoted\"".to_string()),
        );
        assert!(matches!(combined, GraphqlHttpError::Graphql(message)
            if message.contains("\"native\":\"native [disabled]\"")
            && message.contains("\\\"quoted\\\"")));
    }
}

pub async fn extract_http_error(response: reqwest::Response) -> String {
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    let trimmed = text.trim();

    if trimmed.is_empty() {
        return format!("request failed with status {status}");
    }

    if let Ok(payload) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if let Some(message) = payload
            .get("message")
            .and_then(Value::as_str)
            .or_else(|| payload.get("error").and_then(Value::as_str))
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return message.to_string();
        }
    }

    trimmed.to_string()
}

#[server(prefix = "/api/fn", endpoint = "admin/graphql")]
async fn admin_graphql(request: ServerGraphqlRequest) -> Result<Value, ServerFnError> {
    execute_server_graphql(request)
        .await
        .map_err(|err| ServerFnError::ServerError(err.to_string()))
}

pub async fn request<V, T>(
    query: &str,
    variables: V,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<T, ApiError>
where
    V: Serialize,
    T: for<'de> Deserialize<'de>,
{
    let response = execute_admin_graphql(ServerGraphqlRequest {
        query: query.to_string(),
        variables: serde_json::to_value(variables)
            .map_err(|err| ApiError::Graphql(err.to_string()))?,
        persisted_query_sha256: None,
        context: build_request_context(token, tenant_slug),
    })
    .await?;

    serde_json::from_value(response).map_err(|err| ApiError::Graphql(err.to_string()))
}

pub async fn request_with_persisted<V, T>(
    query: &str,
    variables: V,
    sha256_hash: &str,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<T, ApiError>
where
    V: Serialize,
    T: for<'de> Deserialize<'de>,
{
    let response = execute_admin_graphql(ServerGraphqlRequest {
        query: query.to_string(),
        variables: serde_json::to_value(variables)
            .map_err(|err| ApiError::Graphql(err.to_string()))?,
        persisted_query_sha256: Some(sha256_hash.to_string()),
        context: build_request_context(token, tenant_slug),
    })
    .await?;

    serde_json::from_value(response).map_err(|err| ApiError::Graphql(err.to_string()))
}
