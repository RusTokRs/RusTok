/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

#[cfg(all(feature = "ssr", not(feature = "csr")))]
use std::sync::Arc;

#[cfg(all(feature = "ssr", not(feature = "csr")))]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("DATABASE_URL")?;
    let database = sea_orm::Database::connect(database_url).await?;
    let event_bus = rustok_outbox::TransactionalEventBus::new(Arc::new(
        rustok_outbox::OutboxTransport::new(database.clone()),
    ));
    let runtime = rustok_api::HostRuntimeContext::new(database).with_shared_value(event_bus);
    let app = rustok_storefront::router(runtime);

    let port = std::env::var("STOREFRONT_LEPTOS_PORT")
        .or_else(|_| std::env::var("PORT"))
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3101);
    let host = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let addr = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Storefront SSR running on http://{addr}");
    axum::serve(listener, app.into_make_service()).await?;
    Ok(())
}

#[cfg(feature = "csr")]
fn main() {
    use leptos::prelude::*;
    use rustok_auth_ui::ui::leptos::AuthProvider;
    use rustok_storefront::app::{StorefrontModulePage, StorefrontShell};
    use rustok_storefront::shared::api::configured_tenant_slug;
    use rustok_storefront::shared::context::enabled_modules::{
        ModulePolicyReadError, invalidate_pending_projection,
    };

    console_error_panic_hook::set_once();
    invalidate_pending_projection();

    let (locale, route_segment, query_params) = browser_route_context();
    let Some(tenant_slug) = configured_tenant_slug() else {
        mount_to_body(move || view! { <ModulePolicyReadError locale=locale.clone() /> });
        return;
    };
    let debug_modules = resolve_csr_debug_modules(&query_params);
    let fetch_initial = debug_modules.is_none();
    let enabled_modules = debug_modules.unwrap_or_default();

    mount_to_body(move || {
        view! {
            <AuthProvider tenant_slug=tenant_slug.clone()>
                {if let Some(route_segment) = route_segment.clone() {
                    view! {
                        <StorefrontModulePage
                            locale=locale.clone()
                            enabled_modules=enabled_modules.clone()
                            fetch_initial=fetch_initial
                            route_segment=route_segment
                            query_params=query_params.clone()
                        />
                    }
                    .into_any()
                } else {
                    view! {
                        <StorefrontShell
                            locale=locale.clone()
                            enabled_modules=enabled_modules.clone()
                            fetch_initial=fetch_initial
                            query_params=query_params.clone()
                        />
                    }
                    .into_any()
                }}
            </AuthProvider>
        }
    });
}

#[cfg(feature = "csr")]
fn browser_route_context() -> (
    String,
    Option<String>,
    std::collections::HashMap<String, String>,
) {
    // INVARIANT: web_sys::window() is guaranteed in CSR browser runtime
    let location = web_sys::window()
        .expect("window is available in the storefront CSR runtime")
        .location();
    let pathname = location.pathname().unwrap_or_else(|_| "/".to_string());
    let search = location.search().unwrap_or_default();
    let query_params = parse_query_params(search.as_str());
    let segments = pathname
        .trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    let locale = segments
        .first()
        .filter(|segment| looks_like_locale(segment))
        .copied()
        .unwrap_or("en")
        .to_string();
    let route_offset = usize::from(
        segments
            .first()
            .is_some_and(|segment| looks_like_locale(segment)),
    );
    let route_segment = match segments.as_slice().get(route_offset..route_offset + 2) {
        Some(["modules", segment]) => Some((*segment).to_string()),
        _ => None,
    };

    (locale, route_segment, query_params)
}

#[cfg(feature = "csr")]
fn parse_query_params(search: &str) -> std::collections::HashMap<String, String> {
    // Match Axum's SSR query parsing: decode percent-encoded Unicode and '+'
    // before the module passes category, tag and slug filters to GraphQL.
    serde_urlencoded::from_str(search.trim_start_matches('?')).unwrap_or_default()
}

#[cfg(feature = "csr")]
fn looks_like_locale(segment: &str) -> bool {
    let len = segment.len();
    (len == 2 || len == 5)
        && segment
            .chars()
            .all(|ch| ch.is_ascii_alphabetic() || ch == '-')
}

// A module-list meta tag is not bound to a tenant. Standalone CSR must read
// the policy for its configured tenant rather than trusting such a bootstrap.
#[cfg(feature = "csr")]
fn resolve_csr_debug_modules(
    _query_params: &std::collections::HashMap<String, String>,
) -> Option<Vec<String>> {
    #[cfg(debug_assertions)]
    if _query_params.get("rustok_debug_modules").map(|v| v.as_str()) == Some("true") {
        return Some(vec![
            "content".into(),
            "cart".into(),
            "customer".into(),
            "product".into(),
            "taxonomy".into(),
            "region".into(),
            "pricing".into(),
            "inventory".into(),
            "order".into(),
            "payment".into(),
            "fulfillment".into(),
            "commerce".into(),
            "pages".into(),
            "blog".into(),
            "forum".into(),
            "search".into(),
        ]);
    }

    // Missing bootstrap is pending, not an authoritative empty projection.
    None
}

#[cfg(not(any(feature = "ssr", feature = "csr")))]
fn main() {}

#[cfg(all(test, feature = "csr"))]
mod csr_query_parity_tests {
    use super::{parse_query_params, resolve_csr_debug_modules};

    #[test]
    fn standalone_csr_requires_a_policy_read_unless_explicit_debug_is_enabled() {
        assert!(resolve_csr_debug_modules(&Default::default()).is_none());
        #[cfg(debug_assertions)]
        assert!(resolve_csr_debug_modules(&std::collections::HashMap::from([
            ("rustok_debug_modules".to_string(), "true".to_string())
        ])).is_some());
    }

    #[test]
    fn decodes_unicode_and_form_encoded_query_values() {
        let query =
            parse_query_params("?tag=%D0%BD%D0%BE%D0%B2%D0%BE%D1%81%D1%82%D0%B8&q=hello+world");
        assert_eq!(query.get("tag").map(String::as_str), Some("новости"));
        assert_eq!(query.get("q").map(String::as_str), Some("hello world"));
    }
}
