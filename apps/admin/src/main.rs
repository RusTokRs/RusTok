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
        extract::Path,
        http::{header::AUTHORIZATION, HeaderMap, StatusCode},
        routing::post,
        Json, Router,
    };
    use leptos::logging::log;
    use leptos::prelude::*;
    use leptos_auth::{provide_server_auth_snapshot, AuthError};
    use leptos_axum::{generate_route_list, LeptosRoutes};
    use rustok_admin::app::{
        auth_ssr::auth_snapshot_from_headers, request_auth_snapshot, shell, App,
    };
    use rustok_pages_admin::{
        dispatch_pages_browser_intent_with_capabilities,
        pages_editor_capability_policy_for_role, BrowserIntentEnvelope, PagesBrowserIntentError,
        PagesBrowserIntentResponse, PagesBuilderSaveSnapshot,
    };
    use serde_json::{json, Value};

    async fn page_builder_intent(
        Path(page_id): Path<String>,
        headers: HeaderMap,
        Json(envelope): Json<BrowserIntentEnvelope>,
    ) -> Result<Json<PagesBrowserIntentResponse>, (StatusCode, Json<Value>)> {
        let auth = auth_snapshot_from_headers(&headers);
        let token = bearer_token(&headers)
            .or_else(|| compatibility_token(&headers))
            .or_else(|| auth.session.as_ref().map(|session| session.token.clone()))
            .ok_or_else(|| auth_error(StatusCode::UNAUTHORIZED, "Page Builder access token is missing"))?;
        let tenant_slug = header_value(&headers, "x-tenant-slug")
            .or_else(|| auth.session.as_ref().map(|session| session.tenant.clone()))
            .ok_or_else(|| auth_error(StatusCode::BAD_REQUEST, "Page Builder tenant is missing"))?;
        let verified_user = leptos_auth::api::fetch_current_user(
            token.clone(),
            tenant_slug.clone(),
        )
        .await
        .map_err(auth_transport_error)?
        .ok_or_else(|| auth_error(StatusCode::UNAUTHORIZED, "Authenticated user was not found"))?;
        let editor_capabilities = pages_editor_capability_policy_for_role(Some(
            verified_user.role.as_str(),
        ))
        .evaluate();
        let default_locale = header_value(&headers, "accept-language")
            .and_then(|language| language.split(',').next().map(str::to_string))
            .unwrap_or_else(|| "en".to_string());
        let snapshot = PagesBuilderSaveSnapshot {
            token: Some(token),
            tenant_slug: Some(tenant_slug),
            page_id,
            default_locale,
        };
        dispatch_pages_browser_intent_with_capabilities(
            snapshot,
            envelope,
            editor_capabilities,
        )
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

    fn compatibility_token(headers: &HeaderMap) -> Option<String> {
        header_value(headers, "x-fly-access-token")
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
            AuthError::Http(status) => StatusCode::from_u16(*status)
                .unwrap_or(StatusCode::BAD_GATEWAY),
        };
        auth_error(status, error.to_string())
    }

    fn auth_error(
        status: StatusCode,
        message: impl Into<String>,
    ) -> (StatusCode, Json<Value>) {
        (
            status,
            Json(json!({
                "error": message.into(),
                "status": status.as_u16(),
            })),
        )
    }

    fn page_builder_error(error: PagesBrowserIntentError) -> (StatusCode, Json<Value>) {
        let status = match &error {
            PagesBrowserIntentError::PageNotFound => StatusCode::NOT_FOUND,
            PagesBrowserIntentError::PageMismatch { .. } => StatusCode::BAD_REQUEST,
            PagesBrowserIntentError::Dispatch(
                rustok_page_builder_admin::BrowserIntentDispatchError::RevisionConflict { .. }
                | rustok_page_builder_admin::BrowserIntentDispatchError::ProjectHashConflict { .. },
            ) => StatusCode::CONFLICT,
            PagesBrowserIntentError::Dispatch(
                rustok_page_builder_admin::BrowserIntentDispatchError::Authoring(message),
            ) if message.contains("requires editor capability") => StatusCode::FORBIDDEN,
            PagesBrowserIntentError::Draft(
                rustok_page_builder_admin::SsrDraftSessionError::GenerationConflict { .. }
                | rustok_page_builder_admin::SsrDraftSessionError::PageMismatch { .. },
            ) => StatusCode::CONFLICT,
            PagesBrowserIntentError::Facade(error)
                if error.stable_code.as_deref() == Some("REVISION_CONFLICT") =>
            {
                StatusCode::CONFLICT
            }
            PagesBrowserIntentError::Transport(_) => StatusCode::BAD_GATEWAY,
            _ => StatusCode::UNPROCESSABLE_ENTITY,
        };
        (
            status,
            Json(json!({
                "error": error.to_string(),
                "status": status.as_u16(),
            })),
        )
    }

    let configuration = get_configuration(None).expect("Leptos SSR configuration");
    let address = configuration.leptos_options.site_addr;
    let options = configuration.leptos_options;
    let routes = generate_route_list(App);
    let application = Router::new()
        .route(
            "/api/admin/pages/{page_id}/builder/intents",
            post(page_builder_intent),
        )
        .leptos_routes_with_context(
            &options,
            routes,
            || provide_server_auth_snapshot(request_auth_snapshot()),
            {
                let options = options.clone();
                move || shell(options.clone())
            },
        )
        .fallback(leptos_axum::file_and_error_handler(shell))
        .with_state(options);

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
    mount_to_body(|| view! { <App /> });
}

#[cfg(not(any(feature = "ssr", feature = "csr", feature = "hydrate")))]
fn main() {
    eprintln!("Enable one of the `ssr`, `csr`, or `hydrate` admin runtime features.");
}
