use axum::{
    extract::Request,
    http::{request::Parts, HeaderValue},
    middleware::Next,
    response::Response,
};
use leptos::prelude::use_context;
use rustok_web::CspNonce;

const API_CSP: &str =
    "default-src 'none'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'";
const ADMIN_UI_CSP_TEMPLATE: &str = "default-src 'self'; script-src 'self' {nonce}; script-src-attr 'none'; style-src 'self' {nonce}; style-src-attr 'none'; img-src 'self' data: blob: https:; font-src 'self' data:; connect-src {connect_sources}; object-src 'none'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'";
const SECURE_CONNECT_SOURCES: &str = "'self' https: wss:";
const DEVELOPMENT_CONNECT_SOURCES: &str = "'self' https: ws: wss:";
const HSTS: &str = "max-age=31536000; includeSubDomains";

/// Installs a request-scoped nonce and fail-closed security headers for the standalone Leptos
/// admin host. The main RusToK server has its own outer middleware because it also owns the CSP
/// report collector; this adapter intentionally emits no report-only endpoint that the standalone
/// process cannot receive.
pub async fn admin_security_headers(mut request: Request, next: Next) -> Response {
    let path = request.uri().path().to_string();
    let csp_nonce = (!is_api_surface(&path)).then(CspNonce::generate);
    if let Some(nonce) = csp_nonce.as_ref() {
        request.extensions_mut().insert(nonce.clone());
    }

    let mut response = next.run(request).await;
    let policy = select_csp(
        path.as_str(),
        csp_nonce.as_ref(),
        !is_production_environment(),
    );
    let headers = response.headers_mut();
    headers.insert("content-security-policy", policy_header(policy.as_str()));
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("x-frame-options", HeaderValue::from_static("DENY"));
    headers.insert("x-xss-protection", HeaderValue::from_static("0"));
    headers.insert(
        "referrer-policy",
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    headers.insert(
        "permissions-policy",
        HeaderValue::from_static(
            "accelerometer=(), camera=(), geolocation=(), gyroscope=(), \
             magnetometer=(), microphone=(), payment=(), usb=()",
        ),
    );
    if hsts_enabled() {
        headers.insert("strict-transport-security", HeaderValue::from_static(HSTS));
    }

    response
}

/// Copies the nonce installed by [`admin_security_headers`] from Axum request parts into the
/// Leptos render context. Missing state remains `None`; the enforced policy then allows no inline
/// script or style element because it never falls back to a blanket `unsafe-inline` source.
pub fn request_csp_nonce() -> Option<CspNonce> {
    use_context::<Parts>().and_then(|parts| parts.extensions.get::<CspNonce>().cloned())
}

pub fn validate_admin_security_profile() -> Result<(), String> {
    if is_production_environment() && !hsts_enabled() {
        return Err(
            "RUSTOK_HTTPS must be set to true for the standalone production admin host"
                .to_string(),
        );
    }
    Ok(())
}

fn is_api_surface(path: &str) -> bool {
    path.starts_with("/api/")
}

fn select_csp(
    path: &str,
    csp_nonce: Option<&CspNonce>,
    allow_plaintext_websocket: bool,
) -> String {
    if is_api_surface(path) {
        return API_CSP.to_string();
    }
    let Some(csp_nonce) = csp_nonce else {
        return API_CSP.to_string();
    };
    let connect_sources = if allow_plaintext_websocket {
        DEVELOPMENT_CONNECT_SOURCES
    } else {
        SECURE_CONNECT_SOURCES
    };
    ADMIN_UI_CSP_TEMPLATE
        .replace("{nonce}", csp_nonce.source_expression().as_str())
        .replace("{connect_sources}", connect_sources)
}

fn policy_header(policy: &str) -> HeaderValue {
    match HeaderValue::try_from(policy) {
        Ok(value) => value,
        Err(error) => {
            log::error!("generated standalone admin CSP header is invalid: {error}");
            HeaderValue::from_static(API_CSP)
        }
    }
}

fn hsts_enabled() -> bool {
    std::env::var("RUSTOK_HTTPS")
        .map(|value| parse_env_flag(value.as_str()))
        .unwrap_or(false)
}


fn parse_env_flag(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn is_production_environment() -> bool {
    ["RUSTOK_ENV", "RUST_ENV", "APP_ENV"].iter().any(|key| {
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

#[cfg(test)]
mod tests {
    use super::{admin_security_headers, select_csp, API_CSP};
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
            .find(|item| item.starts_with(name))
    }

    #[test]
    fn standalone_admin_ui_policy_requires_nonce_and_secure_production_connections() {
        let nonce = CspNonce::generate();
        let production = select_csp("/dashboard", Some(&nonce), false);
        let development = select_csp("/dashboard", Some(&nonce), true);
        let script = directive(production.as_str(), "script-src").expect("script-src");
        let style = directive(production.as_str(), "style-src").expect("style-src");
        let production_connect =
            directive(production.as_str(), "connect-src").expect("connect-src");
        let development_connect =
            directive(development.as_str(), "connect-src").expect("connect-src");

        assert!(script.contains(nonce.source_expression().as_str()));
        assert!(!script.contains("'unsafe-inline'"));
        assert!(!script.contains("'unsafe-eval'"));
        assert!(style.contains(nonce.source_expression().as_str()));
        assert!(!style.contains("'unsafe-inline'"));
        assert!(production.contains("script-src-attr 'none'"));
        assert!(production.contains("style-src-attr 'none'"));
        assert!(!production_connect.contains(" ws:"));
        assert!(production_connect.contains(" wss:"));
        assert!(development_connect.contains(" ws:"));
    }

    #[test]
    fn standalone_admin_always_enforces_strict_style_attributes() {
        let nonce = CspNonce::generate();
        let policy = select_csp("/dashboard", Some(&nonce), false);

        assert!(policy.contains("style-src-attr 'none'"));
        assert!(!policy.contains("style-src-attr 'unsafe-inline'"));
        assert!(policy.contains(nonce.source_expression().as_str()));
    }

    #[test]
    fn standalone_admin_api_policy_is_scriptless() {
        assert_eq!(select_csp("/api/admin/pages", None, false), API_CSP);
    }

    #[tokio::test]
    async fn standalone_admin_middleware_shares_nonce_with_render_context() {
        let app = Router::new()
            .route(
                "/dashboard",
                get(|Extension(nonce): Extension<CspNonce>| async move {
                    nonce.as_str().to_string()
                }),
            )
            .layer(middleware::from_fn(admin_security_headers));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/dashboard")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let policy = response
            .headers()
            .get("content-security-policy")
            .expect("CSP header")
            .to_str()
            .expect("valid CSP")
            .to_string();
        let nonce = String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("nonce body")
                .to_vec(),
        )
        .expect("UTF-8 nonce");

        assert!(policy.contains(format!("'nonce-{nonce}'").as_str()));
    }
}
