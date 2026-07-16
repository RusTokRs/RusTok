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
    use leptos_axum::{generate_route_list, LeptosRoutes};
    use rustok_admin::app::{shell, App};
    use rustok_pages_admin::{
        dispatch_pages_browser_intent, BrowserIntentEnvelope, PagesBrowserIntentError,
        PagesBrowserIntentResponse, PagesBuilderSaveSnapshot,
    };
    use serde_json::{json, Value};

    async fn page_builder_intent(
        Path(page_id): Path<String>,
        headers: HeaderMap,
        Json(envelope): Json<BrowserIntentEnvelope>,
    ) -> Result<Json<PagesBrowserIntentResponse>, (StatusCode, Json<Value>)> {
        let token = bearer_token(&headers).or_else(|| compatibility_token(&headers));
        let tenant_slug = header_value(&headers, "x-tenant-slug");
        let default_locale = header_value(&headers, "accept-language")
            .and_then(|language| language.split(',').next().map(str::to_string))
            .unwrap_or_else(|| "en".to_string());
        let snapshot = PagesBuilderSaveSnapshot {
            token,
            tenant_slug,
            page_id,
            default_locale,
        };
        dispatch_pages_browser_intent(snapshot, envelope)
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

    fn page_builder_error(error: PagesBrowserIntentError) -> (StatusCode, Json<Value>) {
        let status = match &error {
            PagesBrowserIntentError::PageNotFound => StatusCode::NOT_FOUND,
            PagesBrowserIntentError::PageMismatch { .. } => StatusCode::BAD_REQUEST,
            PagesBrowserIntentError::Dispatch(
                rustok_page_builder_admin::BrowserIntentDispatchError::RevisionConflict { .. }
                | rustok_page_builder_admin::BrowserIntentDispatchError::ProjectHashConflict { .. },
            ) => StatusCode::CONFLICT,
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
        .leptos_routes(&options, routes, {
            let options = options.clone();
            move || shell(options.clone())
        })
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
