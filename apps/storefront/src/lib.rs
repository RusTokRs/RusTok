pub mod app;
pub mod entities;
pub mod modules;
pub mod pages;
pub mod shared;
pub mod widgets;

use axum::{routing::get, Router};
use leptos::prelude::RenderHtml;
use leptos::view;

use crate::app::StorefrontShell;

pub fn render_shell(locale: &str) -> String {
    let locale_owned = locale.to_string();
    let app_html = {
        let locale = locale_owned.clone();
        view! { <StorefrontShell locale=locale /> }.to_html()
    };
    format!(
        r#"<!DOCTYPE html>
<html lang="{locale}">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>RusToK Storefront</title>
  <link rel="stylesheet" href="/assets/app.css" />
</head>
<body>
  <div id="app">{app_html}</div>
</body>
</html>"#,
        locale = locale,
        app_html = app_html
    )
}

pub fn router() -> Router {
    Router::new().route(
        "/",
        get(
            |axum::extract::Query(params): axum::extract::Query<
                std::collections::HashMap<String, String>,
            >| async move {
                let locale = params
                    .get("lang")
                    .map(|value| value.to_lowercase())
                    .unwrap_or_else(|| "en".to_string());
                render_shell(locale.as_str())
            },
        ),
    )
}
