/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

#[cfg(all(feature = "ssr", not(any(feature = "csr", feature = "hydrate"))))]
#[tokio::main]
async fn main() {
    use axum::{
        Extension, Json, Router,
        extract::Path,
        http::{HeaderMap, StatusCode, header::AUTHORIZATION},
        middleware,
        routing::{get, post},
    };
    use leptos::logging::log;
    use leptos::prelude::*;
    use leptos_auth::{ServerAuthSnapshot, provide_server_auth_snapshot};
    use leptos_axum::{LeptosRoutes, generate_route_list};
    use rustok_admin::app::{
        App, DRAFT_SESSION_PRUNE_INTERVAL, admin_security_headers,
        auth_ssr::verify_auth_snapshot, request_auth_snapshot, request_csp_nonce, shell,
        spawn_draft_session_prune, validate_admin_security_profile,
    };
    use rustok_pages_admin::{
        BrowserIntentEnvelope, DraftSessionOwner, FLY_CSRF_HEADER_NAME,
        PagesBrowserIntentAccessError, PagesBrowserIntentProblem, PagesBrowserIntentResponse,
        PagesBuilderSaveSnapshot, csrf_secret_from_access_token,
        dispatch_pages_browser_intent_with_capabilities, fetch_pages_builder_rollout_snapshot,
        pages_editor_capabilities_for_snapshot, pages_editor_capability_policy_from_effective,
        verify_csrf_token,
    };
    use rustok_ui_auth::AuthError;
    use serde_json::{Value, json};

    async fn page_builder_intent(
        Path(page_id): Path<String>,
        // Inserted by `verify_auth_snapshot`, which performed the
        // single server-side refresh-cookie exchange for this request
        // and propagates rotation on the response.
        Extension(auth): Extension<ServerAuthSnapshot>,
        headers: HeaderMap,
        Json(envelope): Json<BrowserIntentEnvelope>,
    ) -> Result<Json<PagesBrowserIntentResponse>, (StatusCode, Json<Value>)> {
        // One authority per request. An `authorization: Bearer` request is an API client, which
        // may state its tenant in `x-tenant-slug` (the documented transport contract); a request
        // without one is the standalone admin's own browser session, and then BOTH the token and
        // the tenant come from that session alone, and a valid `x-csrf-token` header derived from
        // the session token is required to protect ambient cookie authentication against cross-origin
        // request forgery (CLEAN-060 / P1-10).
        let (token, tenant_slug) = match bearer_token(&headers) {
            Some(token) => {
                let tenant_slug = header_value(&headers, "x-tenant-slug")
                    .or_else(|| auth.session.as_ref().map(|session| session.tenant.clone()))
                    .ok_or_else(|| {
                        auth_error(StatusCode::BAD_REQUEST, "Page Builder tenant is missing")
                    })?;
                (token, tenant_slug)
            }
            None => {
                let session = auth.session.as_ref().ok_or_else(|| {
                    auth_error(
                        StatusCode::UNAUTHORIZED,
                        "Page Builder access token is missing",
                    )
                })?;
                let csrf_header = header_value(&headers, FLY_CSRF_HEADER_NAME);
                // The CSRF secret is the session id: stable across refresh-token rotation
                // (unlike the access token), unguessable, and only ever embedded into pages this
                // host renders for the authenticated browser. The page that embeds it derives it
                // with the same helper, so the two sides cannot drift apart.
                let csrf_secret = csrf_secret_from_access_token(&session.token).ok_or_else(|| {
                    auth_error(
                        StatusCode::FORBIDDEN,
                        "Page Builder session has no session id to verify a CSRF token against",
                    )
                })?;
                match csrf_header {
                    Some(client_csrf) if verify_csrf_token(&csrf_secret, &client_csrf) => {
                        (session.token.clone(), session.tenant.clone())
                    }
                    Some(_) => {
                        return Err(auth_error(
                            StatusCode::FORBIDDEN,
                            "Page Builder CSRF token is invalid",
                        ));
                    }
                    None => {
                        return Err(auth_error(
                            StatusCode::FORBIDDEN,
                            "Page Builder intent requires a valid CSRF token",
                        ));
                    }
                }
            }
        };
        let verified_user =
            leptos_auth::transport::fetch_current_user(token.clone(), tenant_slug.clone())
                .await
                .map_err(auth_transport_error)?
                .ok_or_else(|| {
                    auth_error(StatusCode::UNAUTHORIZED, "Authenticated user was not found")
                })?;
        let role_capabilities =
            pages_editor_capability_policy_from_effective(&verified_user.permissions).evaluate();
        let rollout =
            fetch_pages_builder_rollout_snapshot(Some(token.clone()), Some(tenant_slug.clone()))
                .await
                .map_err(|error| {
                    auth_error(
                        StatusCode::SERVICE_UNAVAILABLE,
                        format!("Pages Page Builder rollout snapshot unavailable: {error}"),
                    )
                })?;
        let editor_capabilities =
            pages_editor_capabilities_for_snapshot(role_capabilities, &rollout);
        let default_locale = header_value(&headers, "accept-language")
            .and_then(|header| {
                rustok_ui_i18n::preferred_catalog_locale_from_accept_language(Some(header.as_str()))
            })
            .unwrap_or_else(|| "en".to_string());
        let snapshot = PagesBuilderSaveSnapshot {
            token: Some(token),
            tenant_slug: Some(tenant_slug.clone()),
            page_id,
            default_locale,
            // Bind the draft session to the verified caller (audit §2.5):
            // other users — including cross-tenant ones — cannot load or
            // commit this session even with the token.
            owner: Some(DraftSessionOwner {
                tenant_id: tenant_slug,
                user_id: verified_user.id,
            }),
        };
        dispatch_pages_browser_intent_with_capabilities(snapshot, envelope, editor_capabilities)
            .await
            .map(Json)
            .map_err(page_builder_error)
    }

    fn bearer_token(headers: &HeaderMap) -> Option<String> {
        headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
    }

    fn header_value(headers: &HeaderMap, name: &str) -> Option<String> {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
    }

    fn auth_transport_error(error: AuthError) -> (StatusCode, Json<Value>) {
        let status = match &error {
            AuthError::Unauthorized | AuthError::InvalidCredentials => StatusCode::UNAUTHORIZED,
            AuthError::Network => StatusCode::BAD_GATEWAY,
            AuthError::Http(status) => {
                StatusCode::from_u16(*status).unwrap_or(StatusCode::BAD_GATEWAY)
            }
        };
        auth_error(status, error.to_string())
    }

    fn auth_error(status: StatusCode, message: impl Into<String>) -> (StatusCode, Json<Value>) {
        (
            status,
            Json(json!({
                "error": message.into(),
                "status": status.as_u16(),
            })),
        )
    }

    /// The parameters of a preview address.
    struct PreviewQuery {
        draft: Option<String>,
        runtime_context: Value,
        runtime_scenario: Option<String>,
    }

    /// Parses `?draft=…&runtime_context=…&runtime_scenario=…` without a serde extractor, because a
    /// malformed runtime context is a 400 with a message rather than a rejection the browser cannot
    /// read.
    ///
    /// A context that cannot be parsed is refused rather than replaced: the preview would otherwise
    /// render under a context the caller did not ask for, which is a preview of a different page
    /// than the editor is showing.
    fn preview_query(query: Option<&str>) -> Result<PreviewQuery, (StatusCode, Json<Value>)> {
        let mut draft = None;
        let mut runtime_scenario = None;
        let mut runtime_context = Value::Object(serde_json::Map::new());
        let pairs: Vec<(String, String)> = serde_urlencoded::from_str(query.unwrap_or_default())
            .map_err(|error| {
                auth_error(
                    StatusCode::BAD_REQUEST,
                    format!("The preview address could not be read: {error}"),
                )
            })?;
        for (key, value) in pairs {
            match key.as_str() {
                "draft" if !value.is_empty() => draft = Some(value),
                "runtime_scenario" if !value.is_empty() => runtime_scenario = Some(value),
                "runtime_context" if !value.is_empty() => {
                    runtime_context = serde_json::from_str(&value).map_err(|error| {
                        auth_error(
                            StatusCode::BAD_REQUEST,
                            format!("The preview runtime context is not valid JSON: {error}"),
                        )
                    })?;
                }
                _ => {}
            }
        }
        Ok(PreviewQuery {
            draft,
            runtime_context,
            runtime_scenario,
        })
    }

    fn page_builder_preview_error(
        error: rustok_pages_admin::PagesBrowserIntentError,
    ) -> (StatusCode, Json<Value>) {
        use rustok_pages_admin::PagesBrowserIntentError as Error;
        let (status, message) = match &error {
            Error::PageNotFound => (StatusCode::NOT_FOUND, "The page to preview was not found"),
            Error::DocumentUnavailable => (
                StatusCode::FORBIDDEN,
                "The document of this page is not available to this principal",
            ),
            Error::Draft(rustok_page_builder_admin::SsrDraftSessionError::OwnerMismatch) => (
                StatusCode::FORBIDDEN,
                "The draft belongs to another caller",
            ),
            Error::Draft(rustok_page_builder_admin::SsrDraftSessionError::PageMismatch { .. }) => (
                StatusCode::NOT_FOUND,
                "The draft does not belong to this page",
            ),
            _ => (StatusCode::BAD_REQUEST, "The preview could not be rendered"),
        };
        auth_error(status, message)
    }

    fn page_builder_error(error: PagesBrowserIntentAccessError) -> (StatusCode, Json<Value>) {
        let problem = PagesBrowserIntentProblem::from(&error);
        let status =
            StatusCode::from_u16(problem.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        match serde_json::to_value(problem) {
            Ok(payload) => (status, Json(payload)),
            Err(serialization_error) => auth_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Page Builder problem serialization failed: {serialization_error}"),
            ),
        }
    }

    /// Serves the server preview of a Page Builder draft as its own document.
    ///
    /// The editor's frame loads this instead of an inherited `srcdoc` document: a `srcdoc` frame is
    /// governed by the policy of the console that embeds it, and that policy names no script source
    /// and forbids style attributes, so an embed block's aspect ratio, a provider frame and a remote
    /// font would be dropped silently. Serving the preview as its own response lets it carry the
    /// published artifact's policy, derived from the stylesheet the document itself holds.
    ///
    /// The caller is the verified browser session. A frame cannot send a request header, so this
    /// read cannot require the CSRF header the intent endpoint requires of a write — and it does
    /// not need one: it changes nothing, and the session cookie is `HttpOnly; SameSite=Lax`, so a
    /// cross-site frame does not carry it and a cross-site link that does bring it can only cause a
    /// read the browser renders in a frame the other site cannot see into.
    async fn page_builder_preview(
        Path(page_id): Path<String>,
        Extension(auth): Extension<ServerAuthSnapshot>,
        headers: HeaderMap,
        axum::extract::RawQuery(query): axum::extract::RawQuery,
    ) -> Result<axum::response::Response, (StatusCode, Json<Value>)> {
        let session = auth.session.as_ref().ok_or_else(|| {
            auth_error(StatusCode::UNAUTHORIZED, "Page Builder access token is missing")
        })?;
        let query = preview_query(query.as_deref())?;
        // The principal is the one this request's single verified exchange produced. The intent
        // endpoint re-reads the profile because it creates or commits a draft session; a read
        // does not need a second round trip to know who is asking, and the draft session the
        // editor minted carries the id of this same exchange, so the owner check still matches.
        let verified_user = auth.user.clone().ok_or_else(|| {
            auth_error(StatusCode::UNAUTHORIZED, "Authenticated user was not found")
        })?;
        let default_locale = header_value(&headers, "accept-language")
            .and_then(|header| {
                rustok_ui_i18n::preferred_catalog_locale_from_accept_language(Some(header.as_str()))
            })
            .unwrap_or_else(|| "en".to_string());

        let preview = rustok_pages_admin::render_server_preview(
            PagesBuilderSaveSnapshot {
                token: Some(session.token.clone()),
                tenant_slug: Some(session.tenant.clone()),
                page_id,
                default_locale,
                // The owner is the caller this host verified. Without it the store would return a
                // session to whoever holds its token.
                owner: Some(DraftSessionOwner {
                    tenant_id: session.tenant.clone(),
                    user_id: verified_user.id,
                }),
            },
            rustok_pages_admin::pages_server_preview_store(),
            query.draft.as_deref(),
            query.runtime_context,
            query.runtime_scenario,
        )
        .await
        .map_err(page_builder_preview_error)?;

        rustok_pages_admin::pages_server_preview_response(&preview.html).map_err(|error| {
            auth_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
        })
    }

    validate_admin_security_profile().expect("valid standalone admin security profile");
    let cargo_toml = if std::path::Path::new("apps/admin/Cargo.toml").exists() {
        Some("apps/admin/Cargo.toml")
    } else {
        None
    };
    let configuration = get_configuration(cargo_toml).expect("Leptos SSR configuration");
    let address = configuration.leptos_options.site_addr;
    let options = configuration.leptos_options;
    let routes = generate_route_list(App);
    let mut seen_paths = std::collections::HashSet::new();
    let mut unique_routes = Vec::new();
    for route in routes {
        if seen_paths.insert(route.path().to_string()) {
            unique_routes.push(route);
        }
    }
    let application = Router::new()
        .route("/health", get(|| async { StatusCode::OK }))
        .route(
            "/api/admin/pages/{page_id}/builder/intents",
            post(page_builder_intent),
        )
        // A preview is its own document with its own policy, so it is a route and not a value
        // embedded into the console's document.
        .route(
            "/api/admin/pages/{page_id}/preview",
            get(page_builder_preview),
        )
        .leptos_routes_with_context(
            &options,
            unique_routes,
            || {
                let snapshot = use_context::<axum::http::request::Parts>()
                    .map(|parts| request_auth_snapshot(&parts))
                    .unwrap_or_default();
                provide_server_auth_snapshot(snapshot);
                if let Some(nonce) = request_csp_nonce() {
                    // Module surfaces cannot name the Axum-side nonce type. They read the UI
                    // contract and put the value on the inline elements they render.
                    if let Some(ui_nonce) = rustok_ui_core::UiCspNonce::new(nonce.as_str()) {
                        provide_context(ui_nonce);
                    }
                    provide_context(nonce);
                }
            },
            {
                let options = options.clone();
                move || shell(options.clone())
            },
        )
        .fallback(leptos_axum::file_and_error_handler(shell))
        .with_state(options)
        .layer(middleware::from_fn(verify_auth_snapshot))
        .layer(middleware::from_fn(admin_security_headers));

    // Draft sessions expire and are capped only when something calls `prune`; the intent
    // endpoint touches a session only for a browser that asks. Without this loop an abandoned
    // draft outlives its TTL until the process restarts.
    spawn_draft_session_prune(DRAFT_SESSION_PRUNE_INTERVAL);

    log!("RusTok admin SSR listening on http://{address}");
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .expect("bind admin SSR address");
    axum::serve(listener, application.into_make_service())
        .await
        .expect("serve admin SSR application");
}

#[cfg(any(feature = "csr", feature = "hydrate"))]
fn main() {
    use leptos::prelude::*;
    use rustok_admin::app::App;

    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(log::Level::Debug);
    mount_to_body(|| {
        // The server host hands the response nonce to module surfaces from the request. In the
        // browser there is no request: the host that served this document put the nonce on the
        // script that booted the application, and the same contract is filled from there.
        if let Some(nonce) = document_csp_nonce() {
            provide_context(nonce);
        }
        view! { <App /> }
    });
}

/// The nonce of the document the application runs in, read from a script the host trusted.
///
/// A browser empties the `nonce` content attribute of an element once it is connected under a
/// policy delivered in a header, and keeps the value on the `nonce` property. The attribute
/// stays present, so the selector finds the element and the property gives the value. A document
/// served without a nonce (a development server) has no such script and provides nothing.
#[cfg(any(feature = "csr", feature = "hydrate"))]
fn document_csp_nonce() -> Option<rustok_ui_core::UiCspNonce> {
    use wasm_bindgen::JsValue;
    use web_sys::js_sys::Reflect;

    let script = web_sys::window()?
        .document()?
        .query_selector("script[nonce]")
        .ok()??;
    let nonce = Reflect::get(&script, &JsValue::from_str("nonce"))
        .ok()?
        .as_string()?;
    rustok_ui_core::UiCspNonce::new(&nonce)
}

#[cfg(not(any(feature = "ssr", feature = "csr", feature = "hydrate")))]
fn main() {
    eprintln!("Enable one of the `ssr`, `csr`, or `hydrate` admin runtime features.");
}
