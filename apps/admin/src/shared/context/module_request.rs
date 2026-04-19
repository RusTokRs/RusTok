use std::collections::BTreeMap;

use leptos::prelude::*;
use leptos_router::hooks::{use_location, use_navigate, use_query_map};
use leptos_router::NavigateOptions;
use leptos_ui_routing::RouteQueryPolicy;
use rustok_api::{sanitize_admin_route_query, UiRouteContext};

use crate::{use_i18n, Locale};

#[component]
pub fn ModuleRequestProvider(
    route_segment: Option<String>,
    subpath: Option<String>,
    children: Children,
) -> impl IntoView {
    let query_map = use_query_map();
    let location = use_location();
    let navigate = use_navigate();
    let raw_query = Signal::derive(move || {
        query_map
            .get()
            .latest_values()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect::<BTreeMap<_, _>>()
    });
    let route_segment_for_sanitize = route_segment.clone();
    let subpath_for_sanitize = subpath.clone();
    let sanitized_query = Signal::derive(move || {
        sanitize_admin_route_query(
            route_segment_for_sanitize.as_deref(),
            subpath_for_sanitize.as_deref(),
            &raw_query.get(),
        )
    });
    let locale = match use_i18n().get_locale() {
        Locale::en => Some("en".to_string()),
        Locale::ru => Some("ru".to_string()),
    };

    Effect::new(move |_| {
        let raw_query = raw_query.get();
        let sanitized_query = sanitized_query.get();
        if raw_query == sanitized_query {
            return;
        }

        let pathname = location.pathname.get();
        let href = if sanitized_query.is_empty() {
            pathname
        } else {
            let query = serde_urlencoded::to_string(sanitized_query).unwrap_or_default();
            format!("{pathname}?{query}")
        };
        navigate(
            &href,
            NavigateOptions {
                replace: true,
                ..NavigateOptions::default()
            },
        );
    });

    provide_context(RouteQueryPolicy::new(sanitize_admin_route_query));
    provide_context(UiRouteContext {
        locale,
        route_segment,
        subpath,
        query: sanitized_query.get_untracked(),
    });

    children()
}
