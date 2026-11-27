/// Security Headers Middleware
///
/// Adds OWASP-recommended security response headers to every HTTP response:
/// - `Content-Security-Policy` — restricts resource loading
/// - `Content-Security-Policy-Report-Only` — surfaces strict UI policy violations
/// - `X-Content-Type-Options: nosniff` — prevents MIME sniffing
/// - `X-Frame-Options: DENY` — prevents clickjacking
/// - `X-XSS-Protection: 0` — disables legacy XSS filter (modern browsers use CSP)
/// - `Referrer-Policy: strict-origin-when-cross-origin`
/// - `Permissions-Policy` — disables unused browser features
/// - `Strict-Transport-Security` — enforces HTTPS (only in production)
///
/// Mounted globally in application router composition via `axum::middleware::from_fn`.
use axum::http::{HeaderValue, header};
use axum::{extract::Request, middleware::Next, response::Response};
use rustok_web::CspNonce;

use super::csp_reports;

/// Default CSP for API/server-only surfaces.
const API_CSP: &str =
    "default-src 'none'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'";
/// The OAuth authorization and consent endpoints render server-generated HTML
/// forms. Keep the document isolated from scripts and external resources while
/// permitting its nonce-bearing style block and same-origin form submission.
const OAUTH_BROWSER_CSP_TEMPLATE: &str = "default-src 'none'; script-src 'none'; style-src {nonce}; img-src 'none'; font-src 'none'; connect-src 'none'; worker-src 'none'; frame-src 'none'; object-src 'none'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'";
/// GraphQL Playground is a development-only HTML document. Its upstream template
/// loads fixed CDN assets and has inline bootstrap/style markup; keep this policy
/// narrow and never use it for GraphQL POST or WebSocket requests.
const GRAPHQL_PLAYGROUND_CSP_TEMPLATE: &str = "default-src 'none'; base-uri 'none'; object-src 'none'; frame-ancestors 'none'; form-action 'none'; script-src {nonce} https://cdn.jsdelivr.net; script-src-attr 'none'; style-src 'unsafe-inline' https://cdn.jsdelivr.net https://fonts.googleapis.com; style-src-attr 'unsafe-inline'; img-src https://cdn.jsdelivr.net data:; font-src https://fonts.gstatic.com data:; connect-src 'self' ws: wss:; worker-src 'none'";
const RICHTEXT_FRAME_CSP: &str = "default-src 'none'; script-src 'self'; script-src-attr 'none'; style-src 'self'; style-src-attr 'unsafe-inline'; img-src 'none'; font-src 'none'; connect-src 'none'; media-src 'none'; object-src 'none'; frame-src 'none'; child-src 'none'; worker-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'self'";
/// UI-compatible enforced CSP for embedded admin/storefront shells.
///
/// Trusted inline scripts and style elements receive one per-response nonce. Inline event handlers,
/// dynamic string compilation, and inline style attributes are prohibited.
const UI_CSP_TEMPLATE: &str = "default-src 'self'; script-src 'self' 'wasm-unsafe-eval' {nonce}; script-src-attr 'none'; style-src 'self' {nonce}; style-src-attr 'none'; img-src 'self' data: blob: https:; font-src 'self' data:; connect-src {connect_sources}; object-src 'none'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'";
const SECURE_UI_CONNECT_SOURCES: &str = "'self' https: wss:";
const DEVELOPMENT_UI_CONNECT_SOURCES: &str = "'self' https: ws: wss:";
const RICHTEXT_FRAME_PATH: &str = "/richtext/frame";
const RICHTEXT_FRAME_ASSET_PREFIX: &str = "/richtext/frame/";
const RICHTEXT_ADAPTER_ASSET: &str = "leptos-adapter.mjs";

/// Reporting mirror for strict UI violations. It carries the same trusted nonce as the enforced
/// policy, additionally fixes worker sources, and reports violations without weakening enforcement.
const UI_CSP_REPORT_ONLY_TEMPLATE: &str = "default-src 'self'; script-src 'self' 'wasm-unsafe-eval' {nonce}; script-src-attr 'none'; style-src 'self' {nonce}; style-src-attr 'none'; img-src 'self' data: blob: https:; font-src 'self' data:; connect-src 'self' https: wss:; worker-src 'self' blob:; object-src 'none'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'; report-uri /api/security/csp-report; report-to rustok-csp";
const REPORTING_ENDPOINTS: &str = "rustok-csp=\"/api/security/csp-report\"";

/// HSTS: 1 year, include subdomains.
/// Injected when `RUSTOK_HTTPS` explicitly declares an HTTPS deployment. The
/// executable host rejects production startup without the same declaration.
const HSTS: &str = "max-age=31536000; includeSubDomains";

/// Handles the fixed CSP report endpoint at a layer below the configured public
/// rate limiter but above tenant/auth middleware, so reports remain unauthenticated and
/// tenant-independent without bypassing the public API abuse budget.
pub async fn handle_csp_report(request: Request, next: Next) -> Response {
    if csp_reports::is_report_request(&request) {
        csp_reports::handle(request).await
    } else {
        next.run(request).await
    }
}

pub async fn security_headers(mut request: Request, next: Next) -> Response {
    let path = request.uri().path().to_string();
    let method = request.method().clone();
    let credential_response = is_credential_response_path(&path, &method);
    tracing::debug!(
        path_group = super::http_stack::trace_path_family(&path),
        "security_headers entry"
    );
    let oauth_browser_document = is_oauth_browser_document(&path, &method);
    let graphql_playground_document = is_graphql_playground_document(&path, &method);
    let csp_nonce = ((!is_api_surface(&path)
        || oauth_browser_document
        || graphql_playground_document)
        && !is_richtext_frame_surface(&path))
    .then(CspNonce::generate);
    if let Some(nonce) = csp_nonce.as_ref() {
        request.extensions_mut().insert(nonce.clone());
    }

    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    // Content-Security-Policy. Missing nonce state on a UI path falls back to the API deny policy
    // rather than restoring an inline-script or inline-style-element allowance.
    let enforced_policy = select_request_csp(
        &path,
        &method,
        csp_nonce.as_ref(),
        plaintext_websocket_allowed(),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        policy_header(enforced_policy.as_str()),
    );

    // Run the future style-attribute-compatible UI policy without blocking users. API
    // surfaces already use the stricter enforced policy and do not need a duplicate.
    if let Some(policy) = select_report_only_csp(&path, csp_nonce.as_ref()) {
        headers.insert(
            "content-security-policy-report-only",
            policy_header(policy.as_str()),
        );
        headers.insert(
            "reporting-endpoints",
            HeaderValue::from_static(REPORTING_ENDPOINTS),
        );
    }

    // X-Content-Type-Options
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );

    // X-Frame-Options
    headers.insert(
        header::X_FRAME_OPTIONS,
        HeaderValue::from_static(if is_richtext_frame_surface(&path) {
            "SAMEORIGIN"
        } else {
            "DENY"
        }),
    );

    // X-XSS-Protection — disabled per OWASP recommendation (CSP is the modern replacement)
    headers.insert(header::X_XSS_PROTECTION, HeaderValue::from_static("0"));

    // Referrer-Policy
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static(if is_richtext_frame_surface(&path) {
            "no-referrer"
        } else {
            "strict-origin-when-cross-origin"
        }),
    );

    // Permissions-Policy — disable all unused browser features
    headers.insert(
        "permissions-policy",
        HeaderValue::from_static(
            "accelerometer=(), camera=(), geolocation=(), gyroscope=(), \
             magnetometer=(), microphone=(), payment=(), usb=()",
        ),
    );
    // Operational responses are time-sensitive and may vary by credentials.
    // Set this at the outer edge, including rate-limit/auth/timeout failures.
    if credential_response {
        // Enforce the policy even when tenant/auth/rate-limit/timeout or
        // invite acceptance middleware returns without entering a controller.
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        headers.insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    } else if super::metrics_auth::is_observability_auth_path(&path)
        || matches!(path.as_str(), "/health" | "/health/live")
    {
        headers.insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("private, no-store"),
        );
    } else if let Some(cache_control) = richtext_cache_control(&path) {
        headers.insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static(cache_control),
        );
    }

    // Strict-Transport-Security — only for an explicitly declared HTTPS deployment.
    if hsts_enabled() {
        headers.insert(
            header::STRICT_TRANSPORT_SECURITY,
            HeaderValue::from_static(HSTS),
        );
    }

    response
}

fn policy_header(policy: &str) -> HeaderValue {
    match HeaderValue::try_from(policy) {
        Ok(value) => value,
        Err(_error) => {
            tracing::error!(
                error_code = "security_csp_header_invalid",
                "Generated CSP policy was not a valid HTTP header"
            );
            HeaderValue::from_static(API_CSP)
        }
    }
}

pub(crate) fn hsts_enabled() -> bool {
    std::env::var("RUSTOK_HTTPS")
        .map(|value| parse_env_flag(&value))
        .unwrap_or(false)
}

fn parse_env_flag(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on" | "y" | "t"
    )
}

fn plaintext_websocket_allowed() -> bool {
    !crate::common::is_production_security_mode()
}

fn is_oauth_browser_document(path: &str, method: &axum::http::Method) -> bool {
    (path == "/api/oauth/authorize" && method.as_str() == "GET")
        || (path == "/api/oauth/consent" && method.as_str() == "POST")
}

fn is_graphql_playground_document(path: &str, method: &axum::http::Method) -> bool {
    path == "/api/graphql"
        && method.as_str() == "GET"
        && !crate::common::settings::is_production_security_mode()
}

fn is_api_surface(path: &str) -> bool {
    // Match reserved route families on segment boundaries. The registry's
    // mutation API and protocol discovery are not UI documents, while paths
    // such as /healthcare must retain the normal UI nonce policy.
    [
        "/api",
        "/metrics",
        "/health",
        "/swagger",
        "/openapi",
        "/catalog",
        "/v2/catalog",
        "/.well-known",
        "/ws/builds",
    ]
    .iter()
    .any(|&root| {
        path == root
            || path
                .strip_prefix(root)
                .is_some_and(|suffix| suffix.starts_with('/'))
    })
}

fn is_richtext_frame_surface(path: &str) -> bool {
    if path == RICHTEXT_FRAME_PATH {
        return true;
    }

    let Some(asset) = path.strip_prefix(RICHTEXT_FRAME_ASSET_PREFIX) else {
        return false;
    };

    if asset == RICHTEXT_ADAPTER_ASSET {
        return true;
    }

    let Some(hash_and_extension) = asset.strip_prefix("richtext-frame.") else {
        return false;
    };
    let Some((hash, extension)) = hash_and_extension.rsplit_once('.') else {
        return false;
    };

    hash.len() == 16
        && hash.as_bytes().iter().all(u8::is_ascii_hexdigit)
        && matches!(extension, "js" | "css")
}

/// Auth and OAuth routes can return credentials or private identity data.
/// The shared GraphQL POST endpoint can execute the same credential
/// mutations; playground and schema GET responses are not credential bodies.
fn is_credential_response_path(path: &str, method: &axum::http::Method) -> bool {
    ["/api/auth", "/api/oauth"].iter().any(|&root| {
        path == root
            || path
                .strip_prefix(root)
                .is_some_and(|suffix| suffix.starts_with('/'))
    }) || (path == "/api/graphql" && *method == axum::http::Method::POST)
}

fn richtext_cache_control(path: &str) -> Option<&'static str> {
    (path == RICHTEXT_FRAME_PATH).then_some("no-store")
}

fn select_csp(path: &str, csp_nonce: Option<&CspNonce>, allow_plaintext_websocket: bool) -> String {
    if is_richtext_frame_surface(path) {
        RICHTEXT_FRAME_CSP.to_string()
    } else if is_api_surface(path) {
        API_CSP.to_string()
    } else if let Some(csp_nonce) = csp_nonce {
        let connect_sources = if allow_plaintext_websocket {
            DEVELOPMENT_UI_CONNECT_SOURCES
        } else {
            SECURE_UI_CONNECT_SOURCES
        };
        UI_CSP_TEMPLATE
            .replace("{nonce}", csp_nonce.source_expression().as_str())
            .replace("{connect_sources}", connect_sources)
    } else {
        API_CSP.to_string()
    }
}

fn select_request_csp(
    path: &str,
    method: &axum::http::Method,
    csp_nonce: Option<&CspNonce>,
    allow_plaintext_websocket: bool,
) -> String {
    if is_oauth_browser_document(path, method) {
        return csp_nonce
            .map(|nonce| {
                OAUTH_BROWSER_CSP_TEMPLATE
                    .replace("{nonce}", nonce.source_expression().as_str())
            })
            .unwrap_or_else(|| API_CSP.to_string());
    }

    if is_graphql_playground_document(path, method) {
        return csp_nonce
            .map(|nonce| {
                GRAPHQL_PLAYGROUND_CSP_TEMPLATE
                    .replace("{nonce}", nonce.source_expression().as_str())
            })
            .unwrap_or_else(|| API_CSP.to_string());
    }

    select_csp(path, csp_nonce, allow_plaintext_websocket)
}

fn select_report_only_csp(path: &str, csp_nonce: Option<&CspNonce>) -> Option<String> {
    if is_api_surface(path) || is_richtext_frame_surface(path) {
        None
    } else {
        csp_nonce.map(|nonce| {
            UI_CSP_REPORT_ONLY_TEMPLATE.replace("{nonce}", nonce.source_expression().as_str())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        API_CSP, REPORTING_ENDPOINTS, RICHTEXT_FRAME_PATH, handle_csp_report,
        is_graphql_playground_document, is_oauth_browser_document,
        is_richtext_frame_surface, parse_env_flag, richtext_cache_control,
        security_headers, select_csp, select_report_only_csp, select_request_csp,
    };
    use crate::middleware::csp_reports::CSP_REPORT_PATH;
    use axum::{
        Extension, Router,
        body::{Body, to_bytes},
        http::{Method, Request, StatusCode},
        middleware,
        routing::get,
    };
    use rustok_web::CspNonce;
    use std::sync::Arc;
    use tower::ServiceExt;

    use crate::middleware::rate_limit::{
        PathRateLimitMiddlewareState, PathRateLimitPolicy, RateLimitConfig, RateLimiter,
        rate_limit_for_paths,
    };

    fn directive<'a>(policy: &'a str, name: &str) -> Option<&'a str> {
        policy
            .split(';')
            .map(str::trim)
            .find(|directive| directive.starts_with(name))
    }

    #[test]
    fn api_and_operator_paths_use_strict_csp() {
        let nonce = CspNonce::generate();
        // Passing a nonce makes classification observable rather than merely
        // exercising the missing-nonce fail-closed fallback.
        for path in [
            "/api",
            "/api/graphql",
            "/metrics",
            "/metrics/",
            "/health/ready",
            "/swagger/index.html",
            "/openapi/v3.json",
            "/catalog/modules",
            "/v2/catalog/publish",
            "/v2/catalog/publish/request/platform-build-stage",
            "/v2/catalog/runner/claim",
            "/.well-known/openid-configuration",
            "/.well-known/oauth-authorization-server",
            "/ws/builds",
        ] {
            assert_eq!(select_csp(path, Some(&nonce), false), API_CSP, "{path}");
            assert_eq!(select_report_only_csp(path, Some(&nonce)), None, "{path}");
        }
    }

    #[test]
    fn oauth_browser_csp_is_method_scoped_and_fails_closed_without_nonce() {
        let nonce = CspNonce::generate();

        for (path, method) in [
            ("/api/oauth/authorize", Method::GET),
            ("/api/oauth/consent", Method::POST),
        ] {
            assert!(is_oauth_browser_document(path, &method));
            let policy = select_request_csp(path, &method, Some(&nonce), false);
            assert!(policy.contains("form-action 'self'"), "{policy}");
            assert!(policy.contains("script-src 'none'"), "{policy}");
            assert!(policy.contains(nonce.source_expression().as_str()), "{policy}");
            assert!(!policy.contains("unsafe-inline"), "{policy}");
        }

        assert!(!is_oauth_browser_document(
            "/api/oauth/authorize",
            &Method::POST
        ));
        assert!(!is_oauth_browser_document(
            "/api/oauth/token",
            &Method::POST
        ));
        assert_eq!(
            select_request_csp(
                "/api/oauth/authorize",
                &Method::POST,
                Some(&nonce),
                false
            ),
            API_CSP
        );
        assert_eq!(
            select_request_csp(
                "/api/oauth/token",
                &Method::POST,
                Some(&nonce),
                false
            ),
            API_CSP
        );
        assert_eq!(
            select_request_csp(
                "/api/oauth/authorize",
                &Method::GET,
                None,
                false
            ),
            API_CSP
        );
    }

    #[test]
    fn graphql_playground_csp_is_dev_only_and_get_only() {
        let nonce = CspNonce::generate();
        let enabled = !crate::common::settings::is_production_security_mode();

        assert_eq!(
            is_graphql_playground_document("/api/graphql", &Method::GET),
            enabled
        );
        assert!(!is_graphql_playground_document(
            "/api/graphql",
            &Method::POST
        ));
        assert!(!is_graphql_playground_document(
            "/api/graphql/ws",
            &Method::GET
        ));

        let policy = select_request_csp(
            "/api/graphql",
            &Method::GET,
            Some(&nonce),
            false,
        );
        if enabled {
            assert!(policy.contains(nonce.source_expression().as_str()), "{policy}");
            assert!(policy.contains("https://cdn.jsdelivr.net"), "{policy}");
            assert!(policy.contains("https://fonts.googleapis.com"), "{policy}");
            assert!(policy.contains("connect-src 'self' ws: wss:"), "{policy}");
            let script_policy = policy
                .split(';')
                .find(|directive| directive.trim_start().starts_with("script-src "))
                .expect("script-src directive");
            assert!(!script_policy.contains("unsafe-inline"), "{script_policy}");
            assert_eq!(
                select_request_csp(
                    "/api/graphql",
                    &Method::GET,
                    None,
                    false
                ),
                API_CSP
            );
        } else {
            assert_eq!(policy, API_CSP);
        }

        assert_eq!(
            select_request_csp(
                "/api/graphql",
                &Method::POST,
                Some(&nonce),
                false
            ),
            API_CSP
        );
    }

    #[test]
    fn similarly_named_ui_paths_are_not_api_surfaces() {
        let nonce = CspNonce::generate();
        for path in [
            "/healthcare",
            "/catalogue",
            "/swagger-guide",
            "/openapi-guide",
            "/apiculture",
            "/metrics-dashboard",
            "/v2/catalogue",
            "/.well-known-other",
            "/ws/builds-guide",
        ] {
            assert!(
                select_csp(path, Some(&nonce), false).contains(nonce.source_expression().as_str()),
                "{path}"
            );
            assert!(
                select_report_only_csp(path, Some(&nonce)).is_some(),
                "{path}"
            );
        }
    }

    #[test]
    fn richtext_frame_uses_isolated_policy_without_report_only_nonce() {
        let policy = select_csp("/richtext/frame", None, false);
        assert!(policy.contains("script-src 'self'"));
        assert!(policy.contains("style-src-attr 'unsafe-inline'"));
        assert!(policy.contains("connect-src 'none'"));
        assert!(policy.contains("frame-ancestors 'self'"));
        assert_eq!(select_report_only_csp("/richtext/frame", None), None);
    }

    #[test]
    fn richtext_surface_is_exactly_bounded() {
        for path in [
            RICHTEXT_FRAME_PATH,
            "/richtext/frame/leptos-adapter.mjs",
            "/richtext/frame/richtext-frame.0123456789abcdef.js",
            "/richtext/frame/richtext-frame.0123456789abcdef.css",
        ] {
            assert!(is_richtext_frame_surface(path), "{path}");
        }

        for path in [
            "/richtext/frame/",
            "/richtext/frame/index.html",
            "/richtext/frame/unknown.js",
            "/richtext/frame/richtext-frame.0123456789abcde.js",
            "/richtext/frame/richtext-frame.0123456789abcdef.mjs",
            "/richtext/frame/nested/app.js",
        ] {
            assert!(!is_richtext_frame_surface(path), "{path}");
        }

        assert_eq!(
            richtext_cache_control(RICHTEXT_FRAME_PATH),
            Some("no-store")
        );
        assert_eq!(
            richtext_cache_control("/richtext/frame/leptos-adapter.mjs"),
            None
        );
        assert_eq!(
            richtext_cache_control("/richtext/frame/richtext-frame.0123456789abcdef.js"),
            None
        );
    }

    #[test]
    fn ui_paths_use_nonce_backed_script_and_style_element_policies() {
        for path in ["/admin", "/", "/assets/app.js"] {
            let nonce = CspNonce::generate();
            let enforced = select_csp(path, Some(&nonce), false);
            let report_only = select_report_only_csp(path, Some(&nonce)).expect("UI policy");
            let script = directive(enforced.as_str(), "script-src").expect("script-src");
            let style = directive(enforced.as_str(), "style-src").expect("style-src");

            assert!(script.contains(nonce.source_expression().as_str()));
            assert!(script.contains("'wasm-unsafe-eval'"));
            assert!(!script.contains("'unsafe-inline'"));
            assert!(!script.contains("'unsafe-eval'"));
            assert!(style.contains(nonce.source_expression().as_str()));
            assert!(!style.contains("'unsafe-inline'"));
            assert!(enforced.contains("script-src-attr 'none'"));
            assert!(enforced.contains("style-src-attr 'none'"));
            assert!(report_only.contains(nonce.source_expression().as_str()));
            assert!(report_only.contains("style-src-attr 'none'"));
        }
    }

    #[test]
    fn ui_policy_always_enforces_strict_style_attributes() {
        let nonce = CspNonce::generate();
        let policy = select_csp("/admin", Some(&nonce), false);

        assert!(policy.contains("style-src-attr 'none'"));
        assert!(!policy.contains("style-src-attr 'unsafe-inline'"));
        assert!(policy.contains(nonce.source_expression().as_str()));
    }

    #[test]
    fn production_ui_csp_forbids_plaintext_websocket() {
        let nonce = CspNonce::generate();
        let production = select_csp("/", Some(&nonce), false);
        let development = select_csp("/", Some(&nonce), true);
        let production_connect =
            directive(production.as_str(), "connect-src").expect("production connect-src");
        let development_connect =
            directive(development.as_str(), "connect-src").expect("development connect-src");

        assert!(!production_connect.contains(" ws:"));
        assert!(production_connect.contains(" wss:"));
        assert!(development_connect.contains(" ws:"));
    }

    #[test]
    fn enforced_ui_csp_blocks_eval_plaintext_http_and_plugin_content() {
        let nonce = CspNonce::generate();
        let policy = select_csp("/admin", Some(&nonce), false);

        assert!(!policy.contains("'unsafe-eval'"));
        assert!(!policy.contains(" http:"));
        assert!(policy.contains("object-src 'none'"));
        assert!(policy.contains("form-action 'self'"));
    }

    #[test]
    fn report_only_ui_csp_exposes_style_attributes_and_plaintext_dependencies() {
        let nonce = CspNonce::generate();
        let policy = select_report_only_csp("/", Some(&nonce)).expect("report-only policy");

        assert!(!policy.contains("'unsafe-inline'"));
        assert!(!policy.contains("'unsafe-eval'"));
        assert!(!policy.contains(" http:"));
        assert!(!policy.contains(" ws:"));
        assert!(policy.contains("style-src-attr 'none'"));
        assert!(policy.contains("worker-src 'self' blob:"));
        assert!(policy.contains("report-uri /api/security/csp-report"));
        assert!(policy.contains("report-to rustok-csp"));
        assert_eq!(
            REPORTING_ENDPOINTS,
            "rustok-csp=\"/api/security/csp-report\""
        );
    }

    #[tokio::test]
    async fn credential_surfaces_are_no_store_even_on_early_failures() {
        use axum::http::{Method, header};

        // This fixture intentionally has no controller-level cache middleware.
        // The outer edge must override even an explicit cacheable response.
        let app = Router::new()
            .fallback(|| async {
                (
                    StatusCode::TOO_MANY_REQUESTS,
                    [(header::CACHE_CONTROL, "public, max-age=60")],
                )
            })
            .layer(middleware::from_fn(security_headers));

        for (method, path, private) in [
            (Method::POST, "/api/auth/invite/accept", true),
            (Method::POST, "/api/auth/session/refresh", true),
            (Method::GET, "/api/auth/me", true),
            (Method::GET, "/api/oauth/authorize", true),
            (Method::POST, "/api/oauth/token", true),
            (Method::POST, "/api/graphql", true),
            (Method::GET, "/api/graphql", false),
            (Method::GET, "/api/graphql/schema.graphql", false),
            (Method::GET, "/api/authors", false),
            (Method::GET, "/api/oauth-other", false),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(path)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .expect("edge response");
            assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS, "{path}");
            assert_eq!(
                response
                    .headers()
                    .get(header::CACHE_CONTROL)
                    .and_then(|v| v.to_str().ok()),
                Some(if private {
                    "no-store"
                } else {
                    "public, max-age=60"
                }),
                "{path}",
            );
            assert_eq!(
                response
                    .headers()
                    .get(header::PRAGMA)
                    .and_then(|v| v.to_str().ok()),
                private.then_some("no-cache"),
                "{path}",
            );
        }
    }

    #[tokio::test]
    async fn operational_responses_are_never_cacheable() {
        let app = Router::new()
            .route("/health/ready", get(|| async { StatusCode::OK }))
            .route("/health/runtime", get(|| async { StatusCode::FORBIDDEN }))
            .route("/metrics", get(|| async { StatusCode::TOO_MANY_REQUESTS }))
            .route("/health/live", get(|| async { StatusCode::OK }))
            .route("/api/other", get(|| async { StatusCode::OK }))
            .layer(middleware::from_fn(security_headers));

        for path in [
            "/health/ready",
            "/health/runtime",
            "/metrics",
            "/health/live",
        ] {
            let response = app
                .clone()
                .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                .await
                .expect("operational response");
            assert_eq!(
                response
                    .headers()
                    .get("cache-control")
                    .and_then(|v| v.to_str().ok()),
                Some("private, no-store"),
                "{path}",
            );
        }
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/other")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("unrelated API response");
        assert!(!response.headers().contains_key("cache-control"));
    }

    async fn oauth_browser_test_page(
        Extension(nonce): Extension<CspNonce>,
    ) -> axum::response::Html<String> {
        axum::response::Html(format!("<style nonce=\"{}\"></style>", nonce.as_str()))
    }

    async fn oauth_api_test_page() -> StatusCode {
        StatusCode::OK
    }

    #[tokio::test]
    async fn oauth_browser_documents_receive_nonce_and_form_safe_csp() {
        let app = Router::new()
            .route(
                "/api/oauth/authorize",
                get(oauth_browser_test_page).post(oauth_api_test_page),
            )
            .route(
                "/api/oauth/consent",
                axum::routing::post(oauth_browser_test_page),
            )
            .layer(middleware::from_fn(security_headers));

        for (method, path, expect_browser_policy) in [
            (Method::GET, "/api/oauth/authorize", true),
            (Method::POST, "/api/oauth/consent", true),
            (Method::POST, "/api/oauth/authorize", false),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(path)
                        .body(Body::empty())
                        .expect("OAuth request"),
                )
                .await
                .expect("OAuth response");
            let policy = response
                .headers()
                .get("content-security-policy")
                .expect("CSP header")
                .to_str()
                .expect("valid CSP")
                .to_string();
            let body = String::from_utf8(
                to_bytes(response.into_body(), usize::MAX)
                    .await
                    .expect("OAuth response body")
                    .to_vec(),
            )
            .expect("HTML response body");

            if expect_browser_policy {
                let nonce = body
                    .strip_prefix("<style nonce=\"")
                    .and_then(|rest| rest.split_once('"').map(|(nonce, _)| nonce))
                    .expect("nonce-bearing OAuth style element");
                assert!(policy.contains(&format!("'nonce-{nonce}'")), "{policy}");
                assert!(policy.contains("form-action 'self'"), "{policy}");
                assert!(policy.contains("script-src 'none'"), "{policy}");
                assert!(!policy.contains("unsafe-inline"), "{policy}");
            } else {
                assert_eq!(policy, API_CSP);
            }
        }
    }

    async fn graphql_playground_test_page(
        csp_nonce: Option<Extension<CspNonce>>,
    ) -> axum::response::Html<String> {
        let nonce = csp_nonce
            .map(|Extension(nonce)| nonce.as_str().to_owned())
            .unwrap_or_else(|| "missing".to_string());
        axum::response::Html(format!("<script nonce=\"{}\"></script>", nonce))
    }

    #[tokio::test]
    async fn graphql_playground_document_receives_nonce_from_security_middleware() {
        let app = Router::new()
            .route("/api/graphql", get(graphql_playground_test_page))
            .layer(middleware::from_fn(security_headers));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/graphql")
                    .body(Body::empty())
                    .expect("GraphQL Playground request"),
            )
            .await
            .expect("GraphQL Playground response");
        let policy = response
            .headers()
            .get("content-security-policy")
            .expect("CSP header")
            .to_str()
            .expect("valid CSP")
            .to_string();
        let body = String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("GraphQL Playground body")
                .to_vec(),
        )
        .expect("HTML response body");

        if crate::common::settings::is_production_security_mode() {
            assert_eq!(policy, API_CSP);
            assert!(body.contains("nonce=\"missing\""));
        } else {
            let nonce = body
                .strip_prefix("<script nonce=\"")
                .and_then(|rest| rest.split_once('"').map(|(nonce, _)| nonce))
                .expect("nonce-bearing GraphQL bootstrap");
            assert!(policy.contains(&format!("'nonce-{nonce}'")), "{policy}");
            assert!(policy.contains("https://cdn.jsdelivr.net"), "{policy}");
            assert!(policy.contains("connect-src 'self' ws: wss:"), "{policy}");
        }
    }

    #[tokio::test]
    async fn outer_security_layer_shares_one_nonce_with_ui_handler_and_header() {
        let app =
            Router::new()
                .route(
                    "/ui",
                    get(|Extension(nonce): Extension<CspNonce>| async move {
                        nonce.as_str().to_string()
                    }),
                )
                .layer(middleware::from_fn(security_headers));
        let response = app
            .oneshot(Request::builder().uri("/ui").body(Body::empty()).unwrap())
            .await
            .expect("UI response");

        assert_eq!(response.status(), StatusCode::OK);
        let policy = response
            .headers()
            .get("content-security-policy")
            .expect("UI CSP header")
            .to_str()
            .expect("valid CSP")
            .to_string();
        let nonce = String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("nonce response")
                .to_vec(),
        )
        .expect("UTF-8 nonce");

        assert!(policy.contains(format!("'nonce-{nonce}'").as_str()));
    }

    #[tokio::test]
    async fn protocol_routes_do_not_receive_ui_nonce_or_reporting_headers() {
        let app = Router::new()
            .fallback(|request: axum::extract::Request| async move {
                if request.extensions().get::<CspNonce>().is_some() {
                    StatusCode::BAD_REQUEST
                } else {
                    StatusCode::OK
                }
            })
            .layer(middleware::from_fn(security_headers));

        for path in [
            "/v2/catalog/publish",
            "/.well-known/openid-configuration",
            "/ws/builds",
        ] {
            let response = app
                .clone()
                .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                .await
                .expect("protocol response");
            assert_eq!(response.status(), StatusCode::OK, "{path}");
            assert_eq!(
                response
                    .headers()
                    .get("content-security-policy")
                    .and_then(|value| value.to_str().ok()),
                Some(API_CSP),
                "{path}"
            );
            assert!(
                !response
                    .headers()
                    .contains_key("content-security-policy-report-only")
            );
            assert!(!response.headers().contains_key("reporting-endpoints"));
        }
    }

    #[tokio::test]
    async fn outer_security_layer_covers_edge_short_circuit_responses() {
        let app =
            crate::middleware::http_stack::apply_http_edge_stack(Router::new(), false, None, 10)
                .layer(middleware::from_fn(security_headers));

        let response = app
            .oneshot(
                Request::builder()
                    .method("OPTIONS")
                    .uri("/api/edge-probe")
                    .header("origin", "http://localhost:3000")
                    .header("access-control-request-method", "GET")
                    .body(Body::empty())
                    .expect("CORS preflight request"),
            )
            .await
            .expect("CORS preflight response");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("content-security-policy")
                .and_then(|value| value.to_str().ok()),
            Some(API_CSP)
        );
        assert_eq!(
            response
                .headers()
                .get("x-content-type-options")
                .and_then(|value| value.to_str().ok()),
            Some("nosniff")
        );
    }

    #[tokio::test]
    async fn csp_reports_are_rate_limited_before_collection_and_keep_security_headers() {
        let state = PathRateLimitMiddlewareState {
            policies: Arc::new(vec![PathRateLimitPolicy {
                limiter: Arc::new(RateLimiter::new_with_namespace(
                    RateLimitConfig::new(1, 60),
                    "test-csp-report",
                )),
                prefixes: Arc::new(vec!["/api/"]),
            }]),
            auth_config: None,
            trusted_auth_dimensions: false,
            request_trust: Default::default(),
        };
        let app = Router::new()
            .layer(middleware::from_fn(handle_csp_report))
            .layer(middleware::from_fn_with_state(state, rate_limit_for_paths))
            .layer(middleware::from_fn(security_headers));

        let request = || {
            Request::builder()
                .method("POST")
                .uri(CSP_REPORT_PATH)
                .header("content-type", "application/csp-report")
                .body(Body::from(
                    r#"{"csp-report":{"document-uri":"https://admin.example.com/orders","blocked-uri":"inline","violated-directive":"default-src","effective-directive":"script-src-elem"}}"#,
                ))
                .expect("CSP report request")
        };

        let first = app.clone().oneshot(request()).await.expect("first report");
        assert_eq!(first.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            first
                .headers()
                .get("content-security-policy")
                .and_then(|value| value.to_str().ok()),
            Some(API_CSP)
        );

        let second = app.oneshot(request()).await.expect("second report");
        assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            second
                .headers()
                .get("content-security-policy")
                .and_then(|value| value.to_str().ok()),
            Some(API_CSP)
        );
    }

    #[tokio::test]
    async fn csp_report_layer_collects_report_without_registered_route() {
        let app = Router::new()
            .route("/probe", get(|| async { StatusCode::OK }))
            .layer(middleware::from_fn(handle_csp_report))
            .layer(middleware::from_fn(security_headers));
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(CSP_REPORT_PATH)
                    .header("content-type", "application/csp-report")
                    .body(Body::from(
                        r#"{"csp-report":{"document-uri":"https://admin.example.com/orders?token=secret","blocked-uri":"inline","violated-directive":"script-src-elem"}}"#,
                    ))
                    .expect("CSP report request"),
            )
            .await
            .expect("security middleware response");

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            response
                .headers()
                .get("content-security-policy")
                .expect("API CSP header"),
            API_CSP
        );
    }

    #[test]
    fn https_flag_accepts_explicit_boolean_forms() {
        for value in ["true", "TRUE", "1", "yes", "on", " on ", "y", "t"] {
            assert!(parse_env_flag(value), "expected {value:?} to enable HSTS");
        }
        for value in ["", "false", "0", "no", "off", "https"] {
            assert!(!parse_env_flag(value), "expected {value:?} to disable HSTS");
        }
    }
}
