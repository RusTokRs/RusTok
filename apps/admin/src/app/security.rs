use axum::{
    extract::Request,
    http::{HeaderValue, request::Parts},
    middleware::Next,
    response::Response,
};
use leptos::prelude::use_context;
use rustok_web::CspNonce;

const API_CSP: &str =
    "default-src 'none'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'";
const RICHTEXT_FRAME_CSP: &str = "default-src 'none'; script-src 'self'; script-src-attr 'none'; style-src 'self'; style-src-attr 'unsafe-inline'; img-src 'none'; font-src 'none'; connect-src 'none'; media-src 'none'; object-src 'none'; frame-src 'none'; child-src 'none'; worker-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'self'";
const ADMIN_UI_CSP_TEMPLATE: &str = "default-src 'self'; script-src 'self' 'wasm-unsafe-eval' {nonce}; script-src-attr 'none'; style-src 'self' {nonce}; style-src-attr 'none'; img-src 'self' data: blob: https:; font-src 'self' data:; connect-src {connect_sources}; object-src 'none'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'";
/// The document a Page Builder preview serves. The route inside `rustok-pages-admin` derives the
/// `style-src` hash from the stylesheet the document carries and already emits the policy; this
/// one is for the failure responses that route answers with, which carry no document and must not
/// fall back to the API policy — that would serve a blank frame under a policy nobody can read.
const PAGE_PREVIEW_CSP: &str =
    "default-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'self'";
const SECURE_CONNECT_SOURCES: &str = "'self' https: wss:";
const DEVELOPMENT_CONNECT_SOURCES: &str = "'self' https: ws: wss:";
const HSTS: &str = "max-age=31536000; includeSubDomains";

/// Installs a request-scoped nonce and fail-closed security headers for the standalone Leptos
/// admin host. The main RusToK server has its own outer middleware because it also owns the CSP
/// report collector; this adapter intentionally emits no report-only endpoint that the standalone
/// process cannot receive.
pub async fn admin_security_headers(mut request: Request, next: Next) -> Response {
    let path = request.uri().path().to_string();
    let csp_nonce =
        (!is_api_surface(&path) && !is_richtext_frame_surface(&path)).then(CspNonce::generate);
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
    // A preview route derives its policy from the stylesheet the document it is answering with
    // actually carries (`style-src 'sha256-…'`), which this layer cannot know. Overwriting it with
    // the fallback would serve the document under a policy that names no style source at all, so
    // the preview would render unstyled rather than wrong-but-visible. The fallback is installed
    // only when the handler did not produce one — a response that failed before rendering, for
    // which the fallback is the honest answer.
    if !is_page_preview_surface(&path) || !headers.contains_key("content-security-policy") {
        headers.insert("content-security-policy", policy_header(policy.as_str()));
    }
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        "x-frame-options",
        HeaderValue::from_static(
            if is_richtext_frame_surface(&path) || is_page_preview_surface(&path) {
                "SAMEORIGIN"
            } else {
                "DENY"
            },
        ),
    );
    headers.insert("x-xss-protection", HeaderValue::from_static("0"));
    headers.insert(
        "referrer-policy",
        HeaderValue::from_static(if is_richtext_frame_surface(&path) {
            "no-referrer"
        } else {
            "strict-origin-when-cross-origin"
        }),
    );
    headers.insert(
        "permissions-policy",
        HeaderValue::from_static(
            "accelerometer=(), camera=(), geolocation=(), gyroscope=(), \
             magnetometer=(), microphone=(), payment=(), usb=()",
        ),
    );
    if is_richtext_frame_surface(&path) {
        headers.insert(
            "cache-control",
            HeaderValue::from_static(richtext_cache_control(path.as_str())),
        );
    }
    if is_page_preview_surface(&path) {
        // The body is the caller's working copy. A proxy must not hand it to another caller, and
        // the answer differs per caller, so it is derived from the request rather than the path.
        headers.insert("cache-control", HeaderValue::from_static("no-store"));
        headers.insert("vary", HeaderValue::from_static("Cookie, Authorization"));
    }
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
            "RUSTOK_HTTPS must be set to true for the standalone production admin host".to_string(),
        );
    }
    Ok(())
}

fn is_api_surface(path: &str) -> bool {
    path.starts_with("/api/")
}

/// A Page Builder preview is read in a frame of this host, under the page's policy rather than the
/// console's, so it is the other surface that is not always `DENY`.
fn is_page_preview_surface(path: &str) -> bool {
    let Some(rest) = path.strip_prefix("/api/admin/pages/") else {
        return false;
    };
    rest.ends_with("/preview")
}

fn is_richtext_frame_surface(path: &str) -> bool {
    path == "/richtext/frame" || path.starts_with("/richtext/frame/")
}

fn richtext_cache_control(path: &str) -> &'static str {
    if matches!(
        path,
        "/richtext/frame" | "/richtext/frame/" | "/richtext/frame/leptos-adapter.mjs"
    ) {
        "no-store"
    } else {
        "public, max-age=31536000, immutable"
    }
}

fn select_csp(path: &str, csp_nonce: Option<&CspNonce>, allow_plaintext_websocket: bool) -> String {
    if is_page_preview_surface(path) {
        return PAGE_PREVIEW_CSP.to_string();
    } else if is_richtext_frame_surface(path) {
        return RICHTEXT_FRAME_CSP.to_string();
    } else if is_api_surface(path) {
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
    use super::{API_CSP, admin_security_headers, richtext_cache_control, select_csp};
    use axum::{
        Extension, Router,
        body::{Body, to_bytes},
        http::{Request, StatusCode},
        middleware,
        routing::get,
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
        assert!(script.contains("'wasm-unsafe-eval'"));
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

    #[test]
    fn page_preview_is_framed_by_its_own_host_and_nothing_else() {
        let policy = select_csp("/api/admin/pages/page-1/preview", None, false);
        assert!(policy.contains("frame-ancestors 'self'"), "{policy}");
        assert!(!policy.contains("frame-ancestors 'none'"), "{policy}");
        // The route itself emits the document policy; this one must not blank it if it leaks
        // through, so it names no script and no style rather than inheriting the console's.
        assert!(!policy.contains("script-src"), "{policy}");
        assert!(!policy.contains("style-src"), "{policy}");
    }

    #[test]
    fn a_preview_is_not_the_api_surface_that_shares_its_prefix() {
        for path in [
            "/api/admin/pages",
            "/api/admin/pages/page-1",
            "/api/admin/pages/page-1/builder/intents",
            "/api/admin/pages/page-1/preview/extra",
        ] {
            assert_eq!(select_csp(path, None, false), API_CSP, "{path}");
        }
    }

    #[test]
    fn richtext_frame_uses_isolated_policy() {
        let policy = select_csp("/richtext/frame", None, false);
        assert!(policy.contains("script-src 'self'"));
        assert!(policy.contains("style-src-attr 'unsafe-inline'"));
        assert!(policy.contains("connect-src 'none'"));
        assert!(policy.contains("frame-ancestors 'self'"));
    }

    #[test]
    fn unhashed_richtext_bootstrap_is_not_classified_as_immutable() {
        for path in [
            "/richtext/frame",
            "/richtext/frame/",
            "/richtext/frame/leptos-adapter.mjs",
        ] {
            assert_eq!(richtext_cache_control(path), "no-store");
        }
        assert_eq!(
            richtext_cache_control("/richtext/frame/frame.0123456789abcdef.html"),
            "public, max-age=31536000, immutable"
        );
    }

    #[tokio::test]
    async fn a_preview_response_keeps_the_policy_its_handler_derived() {
        // The handler's policy names the hash of the stylesheet the document carries. The fallback
        // this layer knows cannot: replacing the handler's header would serve the document under
        // `PAGE_PREVIEW_CSP`, which names no style source, and the preview would render unstyled.
        const DERIVED: &str = "default-src 'none'; style-src 'sha256-abc='; frame-ancestors 'self'";
        let app = Router::new()
            .route(
                "/api/admin/pages/page-1/preview",
                get(|_request: Request| async move {
                    ([("content-security-policy", DERIVED)], Body::empty())
                }),
            )
            .layer(middleware::from_fn(admin_security_headers));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/admin/pages/page-1/preview")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(
            response.headers().get("content-security-policy").unwrap(),
            DERIVED
        );
        // The frame still has to be openable by this host, which is the layer's job.
        assert_eq!(
            response.headers().get("x-frame-options").unwrap(),
            "SAMEORIGIN"
        );
        assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
    }

    #[tokio::test]
    async fn a_preview_that_fails_before_rendering_still_gets_a_policy() {
        // A failure response carries no document, so nothing derives a policy for it. Serving it
        // with none at all would let the browser apply another layer's default.
        let app = Router::new()
            .route(
                "/api/admin/pages/page-1/preview",
                get(|_request: Request| async move { StatusCode::NOT_FOUND }),
            )
            .layer(middleware::from_fn(admin_security_headers));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/admin/pages/page-1/preview")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response.headers().get("content-security-policy").unwrap(),
            super::PAGE_PREVIEW_CSP
        );
    }

    #[tokio::test]
    async fn standalone_admin_middleware_shares_nonce_with_render_context() {
        let app =
            Router::new()
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
