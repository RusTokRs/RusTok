#![recursion_limit = "256"]

pub mod app;
pub mod entities;
pub mod modules;
pub mod pages;
pub mod shared;
pub mod widgets;

use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::{extract::Path, routing::get, Router};
use futures::StreamExt;
use leptos::prelude::RenderHtml;
use leptos::view;

use crate::app::{StorefrontModulePage, StorefrontShell};
use crate::shared::context::canonical_route::{build_redirect_location, fetch_canonical_route};
use crate::shared::context::enabled_modules::fetch_enabled_modules;

const DEFAULT_STOREFRONT_LOCALE: &str = "en";

fn render_document(locale: &str, title: &str, app_html: String) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="{locale}">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>{title}</title>
  <link rel="stylesheet" href="/assets/app.css" />
</head>
<body>
  <div id="app">{app_html}</div>
</body>
</html>"#,
        locale = locale,
        title = title,
        app_html = app_html
    )
}

async fn enabled_modules_or_empty() -> Vec<String> {
    match fetch_enabled_modules().await {
        Ok(modules) => modules,
        Err(err) => {
            eprintln!("failed to fetch enabled modules for storefront SSR: {err}");
            Vec::new()
        }
    }
}

pub async fn render_shell(
    locale: &str,
    query_params: std::collections::HashMap<String, String>,
) -> String {
    let locale_owned = locale.to_string();
    let enabled_modules = enabled_modules_or_empty().await;

    let app_html = {
        let locale = locale_owned.clone();
        view! {
            <StorefrontShell
                locale=locale
                enabled_modules=enabled_modules
                query_params=query_params
            />
        }
        .to_html_stream_in_order()
        .collect::<String>()
        .await
    };
    render_document(locale, "RusToK Storefront", app_html)
}

async fn render_shell_response(
    locale: &str,
    query_params: std::collections::HashMap<String, String>,
) -> Response {
    Html(render_shell(locale, query_params).await).into_response()
}

pub async fn render_module_page(
    locale: &str,
    route_segment: &str,
    query_params: std::collections::HashMap<String, String>,
) -> String {
    let locale_owned = locale.to_string();
    let route_segment_owned = route_segment.to_string();
    let enabled_modules = enabled_modules_or_empty().await;

    let app_html = {
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
        .to_html_stream_in_order()
        .collect::<String>()
        .await
    };
    render_document(locale, "RusToK Module Storefront", app_html)
}

async fn render_module_page_response(
    locale: &str,
    route_segment: &str,
    query_params: std::collections::HashMap<String, String>,
    locale_path_prefix: Option<&str>,
) -> Response {
    match fetch_canonical_route(locale, route_segment, &query_params).await {
        Ok(Some(resolved)) if resolved.redirect_required => Redirect::permanent(
            build_redirect_location(&resolved, locale_path_prefix, &query_params).as_str(),
        )
        .into_response(),
        Ok(_) => {
            Html(render_module_page(locale, route_segment, query_params).await).into_response()
        }
        Err(err) => {
            eprintln!("failed to resolve canonical module route for storefront SSR: {err}");
            Html(render_module_page(locale, route_segment, query_params).await).into_response()
        }
    }
}

fn normalize_storefront_locale(raw: &str) -> Option<String> {
    let candidate = raw.trim().replace('_', "-");
    if candidate.is_empty() || candidate.len() > 16 {
        return None;
    }

    if !candidate
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    {
        return None;
    }

    let mut parts = candidate.split('-');
    let language = parts.next()?.trim();
    if language.len() < 2 || language.len() > 8 {
        return None;
    }

    let mut normalized = language.to_ascii_lowercase();
    for part in parts {
        if part.is_empty() || part.len() > 8 {
            return None;
        }

        normalized.push('-');
        if part.len() == 2 && part.chars().all(|ch| ch.is_ascii_alphabetic()) {
            normalized.push_str(&part.to_ascii_uppercase());
        } else {
            normalized.push_str(&part.to_ascii_lowercase());
        }
    }

    Some(normalized)
}

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
