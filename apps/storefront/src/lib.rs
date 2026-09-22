/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

#![recursion_limit = "256"]

#[cfg(any(feature = "ssr", not(feature = "blog-comment-island")))]
pub mod app;
#[cfg(any(feature = "ssr", not(feature = "blog-comment-island")))]
pub mod entities;
#[cfg(any(feature = "ssr", not(feature = "blog-comment-island")))]
pub mod modules;
#[cfg(any(feature = "ssr", not(feature = "blog-comment-island")))]
pub mod pages;
#[cfg(any(feature = "ssr", not(feature = "blog-comment-island")))]
pub mod shared;
#[cfg(any(feature = "ssr", not(feature = "blog-comment-island")))]
pub mod widgets;

#[cfg(all(feature = "blog-comment-island", target_arch = "wasm32"))]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn start_blog_comment_client() {
    use leptos::prelude::*;
    use wasm_bindgen::JsCast;

    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    let Some(root) = document
        .query_selector("[data-blog-comment-island='true']")
        .ok()
        .flatten()
    else {
        return;
    };
    let Some(post_id) = root
        .get_attribute("data-post-id")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    else {
        return;
    };
    let content_locale = root
        .get_attribute("data-content-locale")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "en".to_string());
    let Ok(root) = root.dyn_into::<web_sys::HtmlElement>() else {
        return;
    };

    console_error_panic_hook::set_once();
    root.set_inner_html("");
    mount_to(root, move || {
        view! {
            <leptos_auth::AuthProvider>
                <rustok_blog_storefront::BlogCommentComposer post_id content_locale />
            </leptos_auth::AuthProvider>
        }
    })
    .forget();
}

#[cfg(feature = "ssr")]
mod forum_category_route;
#[cfg(feature = "ssr")]
mod forum_topic_route;

#[cfg(feature = "ssr")]
use axum::extract::{Request, State};
#[cfg(feature = "ssr")]
use axum::http::{
    HeaderMap, HeaderValue, StatusCode,
    header::{CACHE_CONTROL, ETAG, IF_NONE_MATCH, LOCATION},
};
#[cfg(feature = "ssr")]
use axum::middleware::{Next, from_fn_with_state};
#[cfg(feature = "ssr")]
use axum::response::{Html, IntoResponse, Redirect, Response};
#[cfg(feature = "ssr")]
use axum::{Extension, Router, extract::Path, routing::get};
#[cfg(feature = "ssr")]
use leptos::prelude::{Owner, RenderHtml, provide_context};
#[cfg(feature = "ssr")]
use leptos::view;
#[cfg(feature = "ssr")]
use rustok_navigation_storefront::{
    StorefrontMenuLocation, StorefrontNavigationSnapshot, fetch_active_menu,
};
#[cfg(feature = "ssr")]
use rustok_web::CspNonce;

#[cfg(feature = "ssr")]
use crate::app::{StorefrontModulePage, StorefrontShell};
#[cfg(feature = "ssr")]
use crate::shared::context::canonical_route::{build_redirect_location, fetch_canonical_route};
#[cfg(feature = "ssr")]
use crate::shared::context::enabled_modules::fetch_enabled_modules;
#[cfg(feature = "ssr")]
use crate::shared::context::pages_composition::{
    PAGES_STOREFRONT_REVALIDATE_CACHE_CONTROL, if_none_match_matches,
    pages_storefront_composition_etag,
};
#[cfg(feature = "ssr")]
use crate::shared::context::seo_page_context::{ResolvedSeoPageContext, fetch_seo_page_context};

#[cfg(feature = "ssr")]
const DEFAULT_STOREFRONT_LOCALE: &str = "en";
#[cfg(feature = "ssr")]
const PAGES_ROUTE_SEGMENT: &str = "pages";
#[cfg(feature = "ssr")]
const PRIVATE_NO_STORE: &str = "private, no-store";

#[cfg(feature = "ssr")]
fn render_document(
    locale: &str,
    title: &str,
    extra_head: &str,
    app_html: String,
    csp_nonce: Option<&CspNonce>,
) -> String {
    #[cfg(feature = "blog-comment-assets")]
    let comment_bootstrap = if app_html.contains("data-blog-comment-island=\"true\"") {
        csp_nonce
            .map(|nonce| {
                format!(
                    r#"<script nonce="{}" type="module" src="/assets/blog-comment-bootstrap.js"></script>"#,
                    nonce.as_str()
                )
            })
            .unwrap_or_default()
    } else {
        String::new()
    };
    #[cfg(not(feature = "blog-comment-assets"))]
    let comment_bootstrap = {
        let _ = csp_nonce;
        String::new()
    };
    format!(
        r#"<!DOCTYPE html>
<html lang="{locale}">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>{title}</title>
  {extra_head}
  <link rel="stylesheet" href="/assets/app.css" />
</head>
<body>
  <div id="app">{app_html}</div>
  {comment_bootstrap}
</body>
</html>"#,
        locale = locale,
        title = rustok_core::html_escape(title),
        extra_head = extra_head,
        app_html = app_html,
        comment_bootstrap = comment_bootstrap,
    )
}

#[cfg(feature = "ssr")]
async fn enabled_modules_or_empty() -> Vec<String> {
    match fetch_enabled_modules().await {
        Ok(modules) => modules,
        Err(err) => {
            eprintln!("failed to fetch enabled modules for storefront SSR: {err}");
            Vec::new()
        }
    }
}

#[cfg(feature = "ssr")]
pub async fn render_shell(
    locale: &str,
    query_params: std::collections::HashMap<String, String>,
) -> String {
    let locale_owned = locale.to_string();
    let enabled_modules = enabled_modules_or_empty().await;

    let owner = Owner::new();
    let app_html = owner.with(|| {
        let locale = locale_owned.clone();
        view! {
            <StorefrontShell
                locale=locale
                enabled_modules=enabled_modules
                query_params=query_params
            />
        }
        .to_html()
    });
    render_document(locale, "RusToK Storefront", "", app_html, None)
}

#[cfg(feature = "ssr")]
async fn render_shell_response(
    locale: &str,
    query_params: std::collections::HashMap<String, String>,
) -> Response {
    Html(render_shell(locale, query_params).await).into_response()
}

#[cfg(feature = "ssr")]
pub async fn render_module_page(
    locale: &str,
    route_segment: &str,
    query_params: std::collections::HashMap<String, String>,
    seo_context: Option<&ResolvedSeoPageContext>,
) -> String {
    render_module_page_with_nonce(locale, route_segment, query_params, seo_context, None, None)
        .await
}

#[cfg(feature = "ssr")]
async fn render_module_page_with_nonce(
    locale: &str,
    route_segment: &str,
    query_params: std::collections::HashMap<String, String>,
    seo_context: Option<&ResolvedSeoPageContext>,
    csp_nonce: Option<&CspNonce>,
    navigation_snapshot: Option<StorefrontNavigationSnapshot>,
) -> String {
    let locale_owned = locale.to_string();
    let route_segment_owned = route_segment.to_string();
    let enabled_modules = enabled_modules_or_empty().await;

    let owner = Owner::new();
    let app_html = owner.with(|| {
        if let Some(snapshot) = navigation_snapshot {
            provide_context(snapshot);
        }
        let locale = locale_owned.clone();
        let route_segment = route_segment_owned.clone();
        view! {
            <StorefrontModulePage
                locale=locale
                enabled_modules=enabled_modules
                route_segment=route_segment
                query_params=query_params
            />
        }
        .to_html()
    });
    let title = seo_context
        .map(|context| {
            if context.document.title.trim().is_empty() {
                "RusToK Module Storefront".to_string()
            } else {
                context.document.title.clone()
            }
        })
        .unwrap_or_else(|| "RusToK Module Storefront".to_string());
    let head_html = seo_context
        .map(|context| build_seo_head(context, csp_nonce))
        .unwrap_or_default();
    render_document(
        locale,
        title.as_str(),
        head_html.as_str(),
        app_html,
        csp_nonce,
    )
}

#[cfg(feature = "ssr")]
async fn resolve_pages_route_response(
    locale: &str,
    route_segment: &str,
    query_params: &std::collections::HashMap<String, String>,
    locale_path_prefix: Option<&str>,
) -> Result<Option<rustok_pages_storefront::StorefrontPageRouteDecision>, Response> {
    if route_segment != PAGES_ROUTE_SEGMENT {
        return Ok(None);
    }
    let Some(page_slug) = query_params
        .get("slug")
        .map(String::as_str)
        .map(str::trim)
        .filter(|slug| !slug.is_empty())
    else {
        return Ok(None);
    };

    let decision = match rustok_pages_storefront::resolve_storefront_page_route(
        page_slug.to_string(),
        Some(locale.to_string()),
    )
    .await
    {
        Ok(decision) => decision,
        Err(error) => {
            eprintln!("failed to resolve Pages host route: {error}");
            return Err(private_status_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "Page route resolution is temporarily unavailable",
            ));
        }
    };

    if let Some(response) =
        pages_route_response_from_decision(locale, locale_path_prefix, page_slug, &decision)
    {
        Err(response)
    } else {
        Ok(Some(decision))
    }
}

#[cfg(feature = "ssr")]
fn pages_route_response_from_decision(
    locale: &str,
    locale_path_prefix: Option<&str>,
    requested_slug: &str,
    decision: &rustok_pages_storefront::StorefrontPageRouteDecision,
) -> Option<Response> {
    use rustok_pages_storefront::StorefrontPageRouteDisposition;

    match decision.disposition {
        StorefrontPageRouteDisposition::Canonical => {
            let canonical_path = match decision.canonical_path.as_deref() {
                Some(path) => path,
                None => {
                    return Some(private_status_response(
                        StatusCode::CONFLICT,
                        "Canonical page route is incomplete",
                    ));
                }
            };
            let canonical_slug = match decision.canonical_slug.as_deref() {
                Some(slug) => slug,
                None => {
                    return Some(private_status_response(
                        StatusCode::CONFLICT,
                        "Canonical page route is incomplete",
                    ));
                }
            };
            let locale_is_canonical = locale_path_prefix == Some(locale);
            let slug_is_canonical = requested_slug == canonical_slug;
            if locale_is_canonical && slug_is_canonical {
                None
            } else {
                Some(private_permanent_redirect(canonical_path))
            }
        }
        StorefrontPageRouteDisposition::Redirect => decision
            .canonical_path
            .as_deref()
            .map(private_permanent_redirect)
            .or_else(|| {
                Some(private_status_response(
                    StatusCode::CONFLICT,
                    "Redirect page route is missing its canonical target",
                ))
            }),
        StorefrontPageRouteDisposition::Gone => Some(private_status_response(
            StatusCode::GONE,
            "This page route is no longer available",
        )),
        StorefrontPageRouteDisposition::NotFound => Some(private_status_response(
            StatusCode::NOT_FOUND,
            "Page route not found",
        )),
        StorefrontPageRouteDisposition::Conflict => Some(private_status_response(
            StatusCode::CONFLICT,
            "Page route cannot be resolved safely",
        )),
    }
}

#[cfg(feature = "ssr")]
async fn fetch_pages_navigation_snapshot(locale: &str) -> StorefrontNavigationSnapshot {
    let locale = Some(locale.to_string());
    let (header, footer) = tokio::join!(
        fetch_active_menu(StorefrontMenuLocation::Header, locale.clone()),
        fetch_active_menu(StorefrontMenuLocation::Footer, locale),
    );
    StorefrontNavigationSnapshot {
        header: match header {
            Ok(menu) => menu,
            Err(error) => {
                tracing::warn!(%error, "Pages storefront header navigation load failed");
                None
            }
        },
        footer: match footer {
            Ok(menu) => menu,
            Err(error) => {
                tracing::warn!(%error, "Pages storefront footer navigation load failed");
                None
            }
        },
    }
}

#[cfg(feature = "ssr")]
async fn render_canonical_pages_response(
    locale: &str,
    route_segment: &str,
    query_params: std::collections::HashMap<String, String>,
    decision: rustok_pages_storefront::StorefrontPageRouteDecision,
    csp_nonce: Option<&CspNonce>,
    if_none_match: Option<&str>,
) -> Response {
    let seo_context = match fetch_seo_page_context(locale, route_segment, &query_params).await {
        Ok(Some(resolved)) if resolved.route.redirect.is_some() => {
            let redirect = resolved
                .route
                .redirect
                .as_ref()
                .expect("checked is_some above");
            return redirect_response(redirect.target_url.as_str(), Some(redirect.status_code));
        }
        Ok(seo_context) => seo_context,
        Err(error) => {
            tracing::warn!(%error, "Pages storefront SEO composition load failed");
            None
        }
    };
    let navigation = fetch_pages_navigation_snapshot(locale).await;
    let html = render_module_page_with_nonce(
        locale,
        route_segment,
        query_params,
        seo_context.as_ref(),
        csp_nonce,
        Some(navigation.clone()),
    )
    .await;
    let etag = pages_storefront_composition_etag(
        locale,
        &decision,
        &seo_context,
        &navigation,
        html.as_str(),
    );
    if let Some(etag) = etag.as_deref()
        && if_none_match_matches(if_none_match, etag)
    {
        return not_modified_composition_response(etag);
    }

    let mut response = Html(html).into_response();
    if let Some(etag) = etag {
        apply_composition_headers(&mut response, etag.as_str());
    }
    response
}

#[cfg(feature = "ssr")]
fn apply_composition_headers(response: &mut Response, etag: &str) {
    let Ok(etag) = HeaderValue::from_str(etag) else {
        return;
    };
    response.headers_mut().insert(ETAG, etag);
    response.headers_mut().insert(
        CACHE_CONTROL,
        HeaderValue::from_static(PAGES_STOREFRONT_REVALIDATE_CACHE_CONTROL),
    );
}

#[cfg(feature = "ssr")]
fn not_modified_composition_response(etag: &str) -> Response {
    let mut response = StatusCode::NOT_MODIFIED.into_response();
    apply_composition_headers(&mut response, etag);
    response
}

#[cfg(feature = "ssr")]
fn private_permanent_redirect(location: &str) -> Response {
    (
        StatusCode::PERMANENT_REDIRECT,
        [
            (LOCATION, location.to_string()),
            (CACHE_CONTROL, PRIVATE_NO_STORE.to_string()),
        ],
        "",
    )
        .into_response()
}

#[cfg(feature = "ssr")]
fn private_status_response(status: StatusCode, message: &'static str) -> Response {
    (status, [(CACHE_CONTROL, PRIVATE_NO_STORE)], message).into_response()
}

#[cfg(feature = "ssr")]
async fn render_module_page_response(
    locale: &str,
    route_segment: &str,
    query_params: std::collections::HashMap<String, String>,
    locale_path_prefix: Option<&str>,
    csp_nonce: Option<&CspNonce>,
    if_none_match: Option<&str>,
) -> Response {
    let pages_decision = match resolve_pages_route_response(
        locale,
        route_segment,
        &query_params,
        locale_path_prefix,
    )
    .await
    {
        Ok(decision) => decision,
        Err(response) => return response,
    };
    if let Some(decision) = pages_decision {
        return render_canonical_pages_response(
            locale,
            route_segment,
            query_params,
            decision,
            csp_nonce,
            if_none_match,
        )
        .await;
    }

    match fetch_seo_page_context(locale, route_segment, &query_params).await {
        Ok(Some(resolved)) if resolved.route.redirect.is_some() => {
            let redirect = resolved
                .route
                .redirect
                .as_ref()
                .expect("checked is_some above");
            redirect_response(redirect.target_url.as_str(), Some(redirect.status_code))
        }
        Ok(seo_context) => Html(
            render_module_page_with_nonce(
                locale,
                route_segment,
                query_params,
                seo_context.as_ref(),
                csp_nonce,
                None,
            )
            .await,
        )
        .into_response(),
        Err(err) => {
            eprintln!("failed to resolve SEO page context for storefront SSR: {err}");
            match fetch_canonical_route(locale, route_segment, &query_params).await {
                Ok(Some(resolved)) if resolved.redirect_required => Redirect::permanent(
                    build_redirect_location(&resolved, locale_path_prefix, &query_params).as_str(),
                )
                .into_response(),
                _ => Html(
                    render_module_page_with_nonce(
                        locale,
                        route_segment,
                        query_params,
                        None,
                        csp_nonce,
                        None,
                    )
                    .await,
                )
                .into_response(),
            }
        }
    }
}

#[cfg(feature = "ssr")]
fn redirect_response(location: &str, status_code: Option<i32>) -> Response {
    match status_code.unwrap_or(308) {
        301 | 308 => Redirect::permanent(location).into_response(),
        _ => Redirect::temporary(location).into_response(),
    }
}

#[cfg(feature = "ssr")]
fn build_seo_head(context: &ResolvedSeoPageContext, csp_nonce: Option<&CspNonce>) -> String {
    let context = crate::shared::context::seo_page_context::to_seo_page_context(context);
    nonce_structured_data_scripts(rustok_seo_render::render_head_html(&context), csp_nonce)
}

#[cfg(feature = "ssr")]
fn nonce_structured_data_scripts(head: String, csp_nonce: Option<&CspNonce>) -> String {
    let Some(csp_nonce) = csp_nonce else {
        return head;
    };
    let trusted_opening_tag = r#"<script type="application/ld+json">"#;
    let nonce_opening_tag = format!(
        r#"<script nonce="{}" type="application/ld+json">"#,
        csp_nonce.as_str()
    );
    head.replace(trusted_opening_tag, nonce_opening_tag.as_str())
}

#[cfg(feature = "ssr")]
fn normalize_storefront_locale(raw: &str) -> Option<String> {
    rustok_api::normalize_locale_tag(raw)
}

#[cfg(feature = "ssr")]
fn resolve_storefront_locale(
    locale_path_prefix: Option<&str>,
    query_params: &std::collections::HashMap<String, String>,
) -> String {
    locale_path_prefix
        .and_then(normalize_storefront_locale)
        .or_else(|| {
            query_params
                .get("lang")
                .and_then(|value| normalize_storefront_locale(value))
        })
        .unwrap_or_else(|| DEFAULT_STOREFRONT_LOCALE.to_string())
}

#[cfg(feature = "ssr")]
async fn provide_leptos_request_context(
    State(runtime): State<rustok_api::HostRuntimeContext>,
    request: Request,
    next: Next,
) -> Response {
    let (request, parts) = leptos_axum::generate_request_and_parts(request);
    let owner = Owner::new();
    let future = owner.with(|| {
        provide_context(runtime);
        provide_context(parts);
        provide_context(leptos_axum::ResponseOptions::default());
        leptos::reactive::computed::ScopedFuture::new(next.run(request))
    });
    future.await
}

#[cfg(feature = "ssr")]
pub fn router(runtime: rustok_api::HostRuntimeContext) -> Router {
    let _ = any_spawner::Executor::init_tokio();
    Router::new()
        .route(
            "/",
            get(
                |axum::extract::Query(params): axum::extract::Query<
                    std::collections::HashMap<String, String>,
                >| async move {
                    let locale = resolve_storefront_locale(None, &params);
                    render_shell_response(locale.as_str(), params).await
                },
            ),
        )
        .route(
            "/{locale}",
            get(
                |Path(locale_path_prefix): Path<String>,
                 axum::extract::Query(params): axum::extract::Query<
                    std::collections::HashMap<String, String>,
                >| async move {
                    let locale =
                        resolve_storefront_locale(Some(locale_path_prefix.as_str()), &params);
                    render_shell_response(locale.as_str(), params).await
                },
            ),
        )
        .route(
            "/{locale}/forum/c/{slug}",
            get(
                |Path((locale_path_prefix, slug)): Path<(String, String)>,
                 original_uri: axum::extract::OriginalUri,
                 nonce: Option<Extension<CspNonce>>,
                 axum::extract::Query(params): axum::extract::Query<
                    std::collections::HashMap<String, String>,
                >| async move {
                    let effective_locale =
                        resolve_storefront_locale(Some(locale_path_prefix.as_str()), &params);
                    let nonce = nonce.as_ref().map(|Extension(value)| value);
                    forum_category_route::render_forum_category_route_response(
                        original_uri.0.path().to_string(),
                        locale_path_prefix,
                        effective_locale,
                        slug,
                        params,
                        nonce,
                    )
                    .await
                },
            ),
        )
        .route(
            "/{locale}/forum/t/{short_id}/{slug}",
            get(
                |Path((locale_path_prefix, short_id, slug)): Path<(String, String, String)>,
                 original_uri: axum::extract::OriginalUri,
                 nonce: Option<Extension<CspNonce>>,
                 axum::extract::Query(params): axum::extract::Query<
                    std::collections::HashMap<String, String>,
                >| async move {
                    let effective_locale =
                        resolve_storefront_locale(Some(locale_path_prefix.as_str()), &params);
                    let nonce = nonce.as_ref().map(|Extension(value)| value);
                    forum_topic_route::render_forum_topic_route_response(
                        original_uri.0.path().to_string(),
                        locale_path_prefix,
                        effective_locale,
                        short_id,
                        slug,
                        params,
                        nonce,
                    )
                    .await
                },
            ),
        )
        .route(
            "/modules/{route_segment}",
            get(
                |Path(route_segment): Path<String>,
                 headers: HeaderMap,
                 nonce: Option<Extension<CspNonce>>,
                 axum::extract::Query(params): axum::extract::Query<
                    std::collections::HashMap<String, String>,
                >| async move {
                    let locale = resolve_storefront_locale(None, &params);
                    let nonce = nonce.as_ref().map(|Extension(value)| value);
                    let if_none_match = headers
                        .get(IF_NONE_MATCH)
                        .and_then(|value| value.to_str().ok());
                    render_module_page_response(
                        locale.as_str(),
                        route_segment.as_str(),
                        params,
                        None,
                        nonce,
                        if_none_match,
                    )
                    .await
                },
            ),
        )
        .route(
            "/{locale}/modules/{route_segment}",
            get(
                |Path((locale_path_prefix, route_segment)): Path<(String, String)>,
                 headers: HeaderMap,
                 nonce: Option<Extension<CspNonce>>,
                 axum::extract::Query(params): axum::extract::Query<
                    std::collections::HashMap<String, String>,
                >| async move {
                    let locale =
                        resolve_storefront_locale(Some(locale_path_prefix.as_str()), &params);
                    let nonce = nonce.as_ref().map(|Extension(value)| value);
                    let if_none_match = headers
                        .get(IF_NONE_MATCH)
                        .and_then(|value| value.to_str().ok());
                    render_module_page_response(
                        locale.as_str(),
                        route_segment.as_str(),
                        params,
                        Some(locale_path_prefix.as_str()),
                        nonce,
                        if_none_match,
                    )
                    .await
                },
            ),
        )
        .layer(from_fn_with_state(runtime, provide_leptos_request_context))
}

#[cfg(feature = "ssr")]
#[cfg(test)]
mod tests {
    use super::{
        apply_composition_headers, nonce_structured_data_scripts, normalize_storefront_locale,
        not_modified_composition_response, pages_route_response_from_decision,
        resolve_storefront_locale,
    };
    use axum::http::{
        StatusCode,
        header::{CACHE_CONTROL, ETAG, LOCATION},
    };
    use axum::response::IntoResponse;
    use rustok_pages_storefront::{StorefrontPageRouteDecision, StorefrontPageRouteDisposition};
    use rustok_web::CspNonce;
    use std::collections::HashMap;

    fn route_decision(disposition: StorefrontPageRouteDisposition) -> StorefrontPageRouteDecision {
        StorefrontPageRouteDecision {
            disposition,
            canonical_path: Some("/en/modules/pages?slug=about".to_string()),
            canonical_slug: Some("about".to_string()),
            canonical_page_id: Some("00000000-0000-0000-0000-000000000000".to_string()),
            canonical_locale: Some("en".to_string()),
            channel_id: Some("00000000-0000-0000-0000-000000000001".to_string()),
            route_generation: Some(1),
            page_generation: Some(2),
            artifact_generation: Some(3),
        }
    }

    #[test]
    fn resolves_locale_from_path_before_legacy_lang_query() {
        let params = HashMap::from([("lang".to_string(), "en".to_string())]);

        let locale = resolve_storefront_locale(Some("ru"), &params);

        assert_eq!(locale, "ru");
    }

    #[test]
    fn resolves_locale_from_legacy_lang_query_when_path_is_absent() {
        let params = HashMap::from([("lang".to_string(), "ru-ru".to_string())]);

        let locale = resolve_storefront_locale(None, &params);

        assert_eq!(locale, "ru-RU");
    }

    #[test]
    fn falls_back_to_default_locale_for_invalid_values() {
        let params = HashMap::from([("lang".to_string(), "***".to_string())]);

        let locale = resolve_storefront_locale(Some(""), &params);

        assert_eq!(locale, "en");
    }

    #[test]
    fn normalizes_storefront_locale_tags() {
        assert_eq!(
            normalize_storefront_locale("ru-ru").as_deref(),
            Some("ru-RU")
        );
        assert_eq!(
            normalize_storefront_locale("en_us").as_deref(),
            Some("en-US")
        );
    }

    #[test]
    fn exact_localized_canonical_page_route_continues_ssr() {
        let decision = route_decision(StorefrontPageRouteDisposition::Canonical);
        assert!(pages_route_response_from_decision("en", Some("en"), "about", &decision).is_none());
    }

    #[test]
    fn legacy_or_noncanonical_page_route_redirects_privately() {
        let decision = route_decision(StorefrontPageRouteDisposition::Canonical);
        let response = pages_route_response_from_decision("en", None, "About", &decision)
            .expect("legacy route must redirect");

        assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT);
        assert_eq!(
            response
                .headers()
                .get(LOCATION)
                .and_then(|value| value.to_str().ok()),
            Some("/en/modules/pages?slug=about")
        );
        assert_eq!(
            response
                .headers()
                .get(CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("private, no-store")
        );
    }

    #[test]
    fn alias_gone_missing_and_conflict_stop_before_ssr() {
        for (disposition, status) in [
            (
                StorefrontPageRouteDisposition::Redirect,
                StatusCode::PERMANENT_REDIRECT,
            ),
            (StorefrontPageRouteDisposition::Gone, StatusCode::GONE),
            (
                StorefrontPageRouteDisposition::NotFound,
                StatusCode::NOT_FOUND,
            ),
            (
                StorefrontPageRouteDisposition::Conflict,
                StatusCode::CONFLICT,
            ),
        ] {
            let decision = route_decision(disposition);
            let response =
                pages_route_response_from_decision("en", Some("en"), "historical", &decision)
                    .expect("terminal route decision must return a response");
            assert_eq!(response.status(), status);
            assert_eq!(
                response
                    .headers()
                    .get(CACHE_CONTROL)
                    .and_then(|value| value.to_str().ok()),
                Some("private, no-store")
            );
        }
    }

    #[test]
    fn canonical_composition_responses_use_private_revalidation_etags() {
        let etag = "\"pages_storefront_composition_v1-deadbeef\"";
        let mut ok = StatusCode::OK.into_response();
        apply_composition_headers(&mut ok, etag);
        assert_eq!(
            ok.headers().get(ETAG).and_then(|value| value.to_str().ok()),
            Some(etag)
        );
        assert_eq!(
            ok.headers()
                .get(CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("private, no-cache")
        );

        let not_modified = not_modified_composition_response(etag);
        assert_eq!(not_modified.status(), StatusCode::NOT_MODIFIED);
        assert_eq!(
            not_modified
                .headers()
                .get(ETAG)
                .and_then(|value| value.to_str().ok()),
            Some(etag)
        );
    }

    #[test]
    fn nonces_only_trusted_structured_data_scripts() {
        let nonce = CspNonce::generate();
        let head = r#"<script type="application/ld+json">{"@type":"Product"}</script><script>alert(1)</script>"#.to_string();

        let rendered = nonce_structured_data_scripts(head, Some(&nonce));

        assert!(
            rendered.contains(
                format!(
                    r#"<script nonce="{}" type="application/ld+json">"#,
                    nonce.as_str()
                )
                .as_str()
            )
        );
        assert!(rendered.contains("<script>alert(1)</script>"));
    }
}
