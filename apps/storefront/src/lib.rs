#![recursion_limit = "256"]

pub mod app;
pub mod entities;
pub mod modules;
pub mod pages;
pub mod shared;
pub mod widgets;

#[cfg(feature = "ssr")]
use axum::response::{Html, IntoResponse, Redirect, Response};
#[cfg(feature = "ssr")]
use axum::{extract::Path, routing::get, Router};
#[cfg(feature = "ssr")]
use leptos::prelude::{Owner, RenderHtml};
#[cfg(feature = "ssr")]
use leptos::view;

#[cfg(feature = "ssr")]
use crate::app::{StorefrontModulePage, StorefrontShell};
#[cfg(feature = "ssr")]
use crate::shared::context::canonical_route::{build_redirect_location, fetch_canonical_route};
#[cfg(feature = "ssr")]
use crate::shared::context::enabled_modules::fetch_enabled_modules;
#[cfg(feature = "ssr")]
use crate::shared::context::seo_page_context::{fetch_seo_page_context, ResolvedSeoPageContext};

#[cfg(feature = "ssr")]
const DEFAULT_STOREFRONT_LOCALE: &str = "en";

#[cfg(feature = "ssr")]
fn render_document(locale: &str, title: &str, extra_head: &str, app_html: String) -> String {
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
</body>
</html>"#,
        locale = locale,
        title = rustok_core::html_escape(title),
        extra_head = extra_head,
        app_html = app_html
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
    render_document(locale, "RusToK Storefront", "", app_html)
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
    let locale_owned = locale.to_string();
    let route_segment_owned = route_segment.to_string();
    let enabled_modules = enabled_modules_or_empty().await;

    let owner = Owner::new();
    let app_html = owner.with(|| {
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
    let head_html = seo_context.map(build_seo_head).unwrap_or_default();
    render_document(locale, title.as_str(), head_html.as_str(), app_html)
}

#[cfg(feature = "ssr")]
async fn render_module_page_response(
    locale: &str,
    route_segment: &str,
    query_params: std::collections::HashMap<String, String>,
    locale_path_prefix: Option<&str>,
) -> Response {
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
            render_module_page(locale, route_segment, query_params, seo_context.as_ref()).await,
        )
        .into_response(),
        Err(err) => {
            eprintln!("failed to resolve SEO page context for storefront SSR: {err}");
            match fetch_canonical_route(locale, route_segment, &query_params).await {
                Ok(Some(resolved)) if resolved.redirect_required => Redirect::permanent(
                    build_redirect_location(&resolved, locale_path_prefix, &query_params).as_str(),
                )
                .into_response(),
                _ => Html(render_module_page(locale, route_segment, query_params, None).await)
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
fn build_seo_head(context: &ResolvedSeoPageContext) -> String {
    #[cfg(feature = "ssr")]
    {
        let context = crate::shared::context::seo_page_context::to_seo_page_context(context);
        rustok_seo_render::render_head_html(&context)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = context;
        String::new()
    }
}

#[cfg(feature = "ssr")]
fn normalize_storefront_locale(raw: &str) -> Option<String> {
    rustok_core::normalize_locale_tag(raw)
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
pub fn router() -> Router {
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
            "/modules/{route_segment}",
            get(
                |Path(route_segment): Path<String>,
                 axum::extract::Query(params): axum::extract::Query<
                    std::collections::HashMap<String, String>,
                >| async move {
                    let locale = resolve_storefront_locale(None, &params);
                    render_module_page_response(
                        locale.as_str(),
                        route_segment.as_str(),
                        params,
                        None,
                    )
                    .await
                },
            ),
        )
        .route(
            "/{locale}/modules/{route_segment}",
            get(
                |Path((locale_path_prefix, route_segment)): Path<(String, String)>,
                 axum::extract::Query(params): axum::extract::Query<
                    std::collections::HashMap<String, String>,
                >| async move {
                    let locale =
                        resolve_storefront_locale(Some(locale_path_prefix.as_str()), &params);
                    render_module_page_response(
                        locale.as_str(),
                        route_segment.as_str(),
                        params,
                        Some(locale_path_prefix.as_str()),
                    )
                    .await
                },
            ),
        )
}

#[cfg(feature = "ssr")]
#[cfg(test)]
mod tests {
    use super::{normalize_storefront_locale, resolve_storefront_locale};
    use std::collections::HashMap;

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
}
