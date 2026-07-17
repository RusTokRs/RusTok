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

use super::csp_reports;

/// Default CSP for API/server-only surfaces.
const API_CSP: &str =
    "default-src 'none'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'";
/// UI-compatible enforced CSP for embedded admin/storefront shells.
///
/// Inline script/style allowances remain temporarily for the current SSR/bootstrap
/// path and must be replaced with nonce/hash-based directives before the platform
/// is declared production-ready. Plaintext browser connections and plugin/object
/// content are intentionally prohibited now.
const UI_CSP: &str = "default-src 'self'; script-src 'self' 'unsafe-inline' 'unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob: https:; font-src 'self' data:; connect-src 'self' https: ws: wss:; object-src 'none'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'";

/// Target UI policy used during migration. It intentionally omits all inline/eval
/// allowances and plaintext connection schemes, but remains report-only until the
/// SSR/bootstrap surfaces are nonce/hash compatible.
const UI_CSP_REPORT_ONLY: &str = "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data: blob: https:; font-src 'self' data:; connect-src 'self' https: wss:; worker-src 'self' blob:; object-src 'none'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'; report-uri /api/security/csp-report; report-to rustok-csp";
const REPORTING_ENDPOINTS: &str = "rustok-csp=\"/api/security/csp-report\"";

/// HSTS: 1 year, include subdomains.
/// Injected when `RUSTOK_HTTPS` explicitly declares an HTTPS deployment. The
/// executable host rejects production startup without the same declaration.
const HSTS: &str = "max-age=31536000; includeSubDomains";

pub async fn security_headers(request: Request, next: Next) -> Response {
    let path = request.uri().path().to_string();
    // This middleware is the outermost application layer, so the fixed report
    // endpoint is handled before tenant/auth routing and never inherits a tenant.
    let mut response = if csp_reports::is_report_request(&request) {
        csp_reports::handle(request).await
    } else {
        next.run(request).await
    };
    let headers = response.headers_mut();

    // Content-Security-Policy
    headers.insert(
        "content-security-policy",
        HeaderValue::from_static(select_csp(&path)),
    );

    // Run the future nonce/hash-compatible UI policy without blocking users. API
    // surfaces already use the stricter enforced policy and do not need a duplicate.
    if let Some(policy) = select_report_only_csp(&path) {
        headers.insert(
            "content-security-policy-report-only",
            HeaderValue::from_static(policy),
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

pub(crate) fn hsts_enabled() -> bool {
    std::env::var("RUSTOK_HTTPS")
        .map(|value| parse_env_flag(&value))
        .unwrap_or(false)
}

fn parse_env_flag(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn is_api_surface(path: &str) -> bool {
    path.starts_with("/api/")
        || path == "/metrics"
        || path.starts_with("/health")
        || path.starts_with("/swagger")
        || path.starts_with("/api/openapi")
}

fn select_csp(path: &str) -> &'static str {
    if is_api_surface(path) {
        API_CSP
    } else {
        UI_CSP
    }
}

fn select_report_only_csp(path: &str) -> Option<&'static str> {
    (!is_api_surface(path)).then_some(UI_CSP_REPORT_ONLY)
}

#[cfg(test)]
mod tests {
    use super::{
        parse_env_flag, select_csp, select_report_only_csp, API_CSP, REPORTING_ENDPOINTS, UI_CSP,
        UI_CSP_REPORT_ONLY,
    };

    #[test]
    fn api_and_operator_paths_use_strict_csp() {
        assert_eq!(select_csp("/api/graphql"), API_CSP);
        assert_eq!(select_csp("/metrics"), API_CSP);
        assert_eq!(select_csp("/health/ready"), API_CSP);
        assert_eq!(select_csp("/swagger/index.html"), API_CSP);
        assert_eq!(select_report_only_csp("/api/graphql"), None);
    }

    #[test]
    fn ui_paths_use_enforced_and_report_only_policies() {
        for path in ["/admin", "/", "/assets/app.js"] {
            assert_eq!(select_csp(path), UI_CSP);
            assert_eq!(select_report_only_csp(path), Some(UI_CSP_REPORT_ONLY));
        }
    }

    #[test]
    fn enforced_ui_csp_blocks_plaintext_connections_and_plugin_content() {
        assert!(!UI_CSP.contains(" http:"));
        assert!(UI_CSP.contains("object-src 'none'"));
        assert!(UI_CSP.contains("form-action 'self'"));
    }

    #[test]
    fn report_only_ui_csp_exposes_inline_eval_and_plaintext_dependencies() {
        assert!(!UI_CSP_REPORT_ONLY.contains("'unsafe-inline'"));
        assert!(!UI_CSP_REPORT_ONLY.contains("'unsafe-eval'"));
        assert!(!UI_CSP_REPORT_ONLY.contains(" http:"));
        assert!(!UI_CSP_REPORT_ONLY.contains(" ws:"));
        assert!(UI_CSP_REPORT_ONLY.contains("worker-src 'self' blob:"));
        assert!(UI_CSP_REPORT_ONLY.contains("report-uri /api/security/csp-report"));
        assert!(UI_CSP_REPORT_ONLY.contains("report-to rustok-csp"));
        assert_eq!(
            REPORTING_ENDPOINTS,
            "rustok-csp=\"/api/security/csp-report\""
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
