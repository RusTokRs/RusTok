use axum::http::HeaderValue;
/// Security Headers Middleware
///
/// Adds OWASP-recommended security response headers to every HTTP response:
/// - `Content-Security-Policy` — restricts resource loading
/// - `X-Content-Type-Options: nosniff` — prevents MIME sniffing
/// - `X-Frame-Options: DENY` — prevents clickjacking
/// - `X-XSS-Protection: 0` — disables legacy XSS filter (modern browsers use CSP)
/// - `Referrer-Policy: strict-origin-when-cross-origin`
/// - `Permissions-Policy` — disables unused browser features
/// - `Strict-Transport-Security` — enforces HTTPS (only in production)
///
/// Mounted globally in `app.rs::after_routes()` via `axum::middleware::from_fn`.
use axum::{extract::Request, middleware::Next, response::Response};

/// Default CSP for API/server-only surfaces.
const API_CSP: &str =
    "default-src 'none'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'";
/// UI-compatible CSP for embedded admin/storefront shells.
///
/// Inline script/style allowances remain temporarily for the current SSR/bootstrap
/// path and must be replaced with nonce/hash-based directives before the platform
/// is declared production-ready. Plaintext browser connections and plugin/object
/// content are intentionally prohibited now.
const UI_CSP: &str = "default-src 'self'; script-src 'self' 'unsafe-inline' 'unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob: https:; font-src 'self' data:; connect-src 'self' https: ws: wss:; object-src 'none'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'";

/// HSTS: 1 year, include subdomains.
/// Injected when `RUSTOK_HTTPS` explicitly declares an HTTPS deployment. The
/// executable host rejects production startup without the same declaration.
const HSTS: &str = "max-age=31536000; includeSubDomains";

pub async fn security_headers(request: Request, next: Next) -> Response {
    let path = request.uri().path().to_string();
    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    // Content-Security-Policy
    headers.insert(
        "content-security-policy",
        HeaderValue::from_static(select_csp(&path)),
    );

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

fn select_csp(path: &str) -> &'static str {
    let is_api_surface = path.starts_with("/api/")
        || path == "/metrics"
        || path.starts_with("/health")
        || path.starts_with("/swagger")
        || path.starts_with("/api/openapi");

    if is_api_surface {
        API_CSP
    } else {
        UI_CSP
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_env_flag, select_csp, API_CSP, UI_CSP};

    #[test]
    fn api_and_operator_paths_use_strict_csp() {
        assert_eq!(select_csp("/api/graphql"), API_CSP);
        assert_eq!(select_csp("/metrics"), API_CSP);
        assert_eq!(select_csp("/health/ready"), API_CSP);
        assert_eq!(select_csp("/swagger/index.html"), API_CSP);
    }

    #[test]
    fn ui_paths_use_ui_csp() {
        assert_eq!(select_csp("/admin"), UI_CSP);
        assert_eq!(select_csp("/"), UI_CSP);
        assert_eq!(select_csp("/assets/app.js"), UI_CSP);
    }

    #[test]
    fn ui_csp_blocks_plaintext_connections_and_plugin_content() {
        assert!(!UI_CSP.contains(" http:"));
        assert!(UI_CSP.contains("object-src 'none'"));
        assert!(UI_CSP.contains("form-action 'self'"));
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
