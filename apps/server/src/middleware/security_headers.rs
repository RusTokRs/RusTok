use axum::http::HeaderValue;
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
use axum::{extract::Request, middleware::Next, response::Response};
use rustok_web::CspNonce;

use super::csp_reports;

/// Default CSP for API/server-only surfaces.
const API_CSP: &str =
    "default-src 'none'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'";
/// UI-compatible enforced CSP for embedded admin/storefront shells.
///
/// Trusted inline scripts and style elements receive one per-response nonce. Inline event handlers
/// and all dynamic string compilation are prohibited. Inline style attributes remain temporary
/// migration debt until component-level style usage is moved to classes or hashes.
const UI_CSP_TEMPLATE: &str = "default-src 'self'; script-src 'self' {nonce}; script-src-attr 'none'; style-src 'self' {nonce}; style-src-attr 'unsafe-inline'; img-src 'self' data: blob: https:; font-src 'self' data:; connect-src {connect_sources}; object-src 'none'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'";
const UI_CSP_STRICT_STYLE_TEMPLATE: &str = "default-src 'self'; script-src 'self' {nonce}; script-src-attr 'none'; style-src 'self' {nonce}; style-src-attr 'none'; img-src 'self' data: blob: https:; font-src 'self' data:; connect-src {connect_sources}; object-src 'none'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'";
const SECURE_UI_CONNECT_SOURCES: &str = "'self' https: wss:";
const DEVELOPMENT_UI_CONNECT_SOURCES: &str = "'self' https: ws: wss:";

/// Target UI policy used during style-attribute migration. It carries the same trusted nonce as the
/// enforced policy, blocks inline style attributes and plaintext connection schemes, and remains
/// report-only until the component style inventory is clean.
const UI_CSP_REPORT_ONLY_TEMPLATE: &str = "default-src 'self'; script-src 'self' {nonce}; script-src-attr 'none'; style-src 'self' {nonce}; style-src-attr 'none'; img-src 'self' data: blob: https:; font-src 'self' data:; connect-src 'self' https: wss:; worker-src 'self' blob:; object-src 'none'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'; report-uri /api/security/csp-report; report-to rustok-csp";
const REPORTING_ENDPOINTS: &str = "rustok-csp=\"/api/security/csp-report\"";

/// HSTS: 1 year, include subdomains.
/// Injected when `RUSTOK_HTTPS` explicitly declares an HTTPS deployment. The
/// executable host rejects production startup without the same declaration.
const HSTS: &str = "max-age=31536000; includeSubDomains";

pub async fn security_headers(mut request: Request, next: Next) -> Response {
    let path = request.uri().path().to_string();
    let csp_nonce = (!is_api_surface(&path)).then(CspNonce::generate);
    if let Some(nonce) = csp_nonce.as_ref() {
        request.extensions_mut().insert(nonce.clone());
    }

    // This middleware is the outermost application layer, so the fixed report
    // endpoint is handled before tenant/auth routing and never inherits a tenant.
    let mut response = if csp_reports::is_report_request(&request) {
        csp_reports::handle(request).await
    } else {
        next.run(request).await
    };
    let headers = response.headers_mut();

    // Content-Security-Policy. Missing nonce state on a UI path falls back to the API deny policy
    // rather than restoring an inline-script or inline-style-element allowance.
    let enforced_policy = select_csp(
        &path,
        csp_nonce.as_ref(),
        plaintext_websocket_allowed(),
        strict_style_attributes_enabled(),
    );
    headers.insert(
        "content-security-policy",
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
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );

    // X-Frame-Options
    headers.insert("x-frame-options", HeaderValue::from_static("DENY"));

    // X-XSS-Protection — disabled per OWASP recommendation (CSP is the modern replacement)
    headers.insert("x-xss-protection", HeaderValue::from_static("0"));

    // Referrer-Policy
    headers.insert(
        "referrer-policy",
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );

    // Permissions-Policy — disable all unused browser features
    headers.insert(
        "permissions-policy",
        HeaderValue::from_static(
            "accelerometer=(), camera=(), geolocation=(), gyroscope=(), \
             magnetometer=(), microphone=(), payment=(), usb=()",
        ),
    );

    // Strict-Transport-Security — only for an explicitly declared HTTPS deployment.
    if hsts_enabled() {
        headers.insert("strict-transport-security", HeaderValue::from_static(HSTS));
    }

    response
}

fn policy_header(policy: &str) -> HeaderValue {
    match HeaderValue::try_from(policy) {
        Ok(value) => value,
        Err(error) => {
            tracing::error!(%error, "Generated CSP policy was not a valid HTTP header");
            HeaderValue::from_static(API_CSP)
        }
    }
}

pub(crate) fn hsts_enabled() -> bool {
    std::env::var("RUSTOK_HTTPS")
        .map(|value| parse_env_flag(&value))
        .unwrap_or(false)
}

fn strict_style_attributes_enabled() -> bool {
    std::env::var("RUSTOK_CSP_STRICT_STYLE_ATTRIBUTES")
        .map(|value| parse_env_flag(value.as_str()))
        .unwrap_or(false)
}

fn parse_env_flag(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn plaintext_websocket_allowed() -> bool {
    !["RUSTOK_ENV", "RUST_ENV", "APP_ENV"]
        .iter()
        .any(|key| {
            std::env::var(key)
                .map(|value| {
                    matches!(
                        value.trim().to_ascii_lowercase().as_str(),
                        "prod" | "production"
                    )
                })
                .unwrap_or(false)
        })
}

fn is_api_surface(path: &str) -> bool {
    path.starts_with("/api/")
        || path == "/metrics"
        || path.starts_with("/health")
        || path.starts_with("/swagger")
        || path.starts_with("/api/openapi")
}

fn select_csp(
    path: &str,
    csp_nonce: Option<&CspNonce>,
    allow_plaintext_websocket: bool,
    strict_style_attributes: bool,
) -> String {
    if is_api_surface(path) {
        API_CSP.to_string()
    } else if let Some(csp_nonce) = csp_nonce {
        let connect_sources = if allow_plaintext_websocket {
            DEVELOPMENT_UI_CONNECT_SOURCES
        } else {
            SECURE_UI_CONNECT_SOURCES
        };
        let template = if strict_style_attributes {
            UI_CSP_STRICT_STYLE_TEMPLATE
        } else {
            UI_CSP_TEMPLATE
        };
        template
            .replace("{nonce}", csp_nonce.source_expression().as_str())
            .replace("{connect_sources}", connect_sources)
    } else {
        API_CSP.to_string()
    }
}

fn select_report_only_csp(path: &str, csp_nonce: Option<&CspNonce>) -> Option<String> {
    if is_api_surface(path) {
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
        parse_env_flag, security_headers, select_csp, select_report_only_csp, API_CSP,
        REPORTING_ENDPOINTS,
    };
    use crate::middleware::csp_reports::CSP_REPORT_PATH;
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
        middleware,
        routing::get,
        Extension, Router,
    };
    use rustok_web::CspNonce;
    use tower::ServiceExt;

    fn directive<'a>(policy: &'a str, name: &str) -> Option<&'a str> {
        policy
            .split(';')
            .map(str::trim)
            .find(|directive| directive.starts_with(name))
    }

    #[test]
    fn api_and_operator_paths_use_strict_csp() {
        assert_eq!(select_csp("/api/graphql", None, false, false), API_CSP);
        assert_eq!(select_csp("/metrics", None, false, false), API_CSP);
        assert_eq!(select_csp("/health/ready", None, false, false), API_CSP);
        assert_eq!(select_csp("/swagger/index.html", None, false, false), API_CSP);
        assert_eq!(select_report_only_csp("/api/graphql", None), None);
    }

    #[test]
    fn ui_paths_use_nonce_backed_script_and_style_element_policies() {
        for path in ["/admin", "/", "/assets/app.js"] {
            let nonce = CspNonce::generate();
            let enforced = select_csp(path, Some(&nonce), false, false);
            let report_only = select_report_only_csp(path, Some(&nonce)).expect("UI policy");
            let script = directive(enforced.as_str(), "script-src").expect("script-src");
            let style = directive(enforced.as_str(), "style-src").expect("style-src");

            assert!(script.contains(nonce.source_expression().as_str()));
            assert!(!script.contains("'unsafe-inline'"));
            assert!(!script.contains("'unsafe-eval'"));
            assert!(style.contains(nonce.source_expression().as_str()));
            assert!(!style.contains("'unsafe-inline'"));
            assert!(enforced.contains("script-src-attr 'none'"));
            assert!(enforced.contains("style-src-attr 'unsafe-inline'"));
            assert!(report_only.contains(nonce.source_expression().as_str()));
            assert!(report_only.contains("style-src-attr 'none'"));
        }
    }

    #[test]
    fn strict_style_attribute_profile_enforces_none() {
        let nonce = CspNonce::generate();
        let relaxed = select_csp("/admin", Some(&nonce), false, false);
        let strict = select_csp("/admin", Some(&nonce), false, true);

        assert!(relaxed.contains("style-src-attr 'unsafe-inline'"));
        assert!(!relaxed.contains("style-src-attr 'none'"));
        assert!(strict.contains("style-src-attr 'none'"));
        assert!(!strict.contains("style-src-attr 'unsafe-inline'"));
        assert!(strict.contains(nonce.source_expression().as_str()));
    }

    #[test]
    fn production_ui_csp_forbids_plaintext_websocket() {
        let nonce = CspNonce::generate();
        let production = select_csp("/", Some(&nonce), false, false);
        let development = select_csp("/", Some(&nonce), true, false);
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
        let policy = select_csp("/admin", Some(&nonce), false, false);

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
    async fn outer_security_layer_shares_one_nonce_with_ui_handler_and_header() {
        let app = Router::new()
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
    async fn outer_security_layer_collects_report_without_registered_route() {
        let app = Router::new()
            .route("/probe", get(|| async { StatusCode::OK }))
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
        for value in ["true", "TRUE", "1", "yes", "on", " on "] {
            assert!(parse_env_flag(value), "expected {value:?} to enable HSTS");
        }
        for value in ["", "false", "0", "no", "off", "https"] {
            assert!(!parse_env_flag(value), "expected {value:?} to disable HSTS");
        }
    }
}
