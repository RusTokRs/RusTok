use std::collections::BTreeMap;
use std::sync::Arc;

use leptos::prelude::*;
use leptos_router::hooks::{use_location, use_navigate, use_query_map};
use leptos_router::NavigateOptions;
use rustok_api::UiRouteContext;

type RouteQuerySanitizeFn =
    dyn Fn(Option<&str>, Option<&str>, &BTreeMap<String, String>) -> BTreeMap<String, String>
        + Send
        + Sync;

#[derive(Clone)]
pub struct RouteQueryPolicy {
    sanitize: Arc<RouteQuerySanitizeFn>,
}

impl RouteQueryPolicy {
    pub fn new<F>(sanitize: F) -> Self
    where
        F: Fn(Option<&str>, Option<&str>, &BTreeMap<String, String>) -> BTreeMap<String, String>
            + Send
            + Sync
            + 'static,
    {
        Self {
            sanitize: Arc::new(sanitize),
        }
    }

    pub fn sanitize(
        &self,
        route_segment: Option<&str>,
        subpath: Option<&str>,
        query: &BTreeMap<String, String>,
    ) -> BTreeMap<String, String> {
        (self.sanitize)(route_segment, subpath, query)
    }
}

#[derive(Clone)]
pub struct RouteQueryWriter {
    apply: Arc<dyn Fn(Vec<(String, Option<String>)>, bool) + Send + Sync>,
}

impl RouteQueryWriter {
    pub fn update(&self, updates: Vec<(String, Option<String>)>, replace: bool) {
        (self.apply)(updates, replace);
    }

    pub fn push_value(&self, key: impl Into<String>, value: impl Into<String>) {
        self.update(vec![(key.into(), Some(value.into()))], false);
    }

    pub fn replace_value(&self, key: impl Into<String>, value: impl Into<String>) {
        self.update(vec![(key.into(), Some(value.into()))], true);
    }

    pub fn clear_key(&self, key: impl AsRef<str>) {
        self.update(vec![(key.as_ref().to_string(), None)], true);
    }
}

pub fn read_route_query_value(route_context: &UiRouteContext, key: &str) -> Option<String> {
    route_context.query_value(key).map(str::to_owned)
}

pub fn use_route_query_value(key: &'static str) -> Signal<Option<String>> {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let policy = use_context::<RouteQueryPolicy>();
    let raw_query = use_query_map();
    let route_segment = route_context.route_segment.clone();
    let subpath = route_context.subpath.clone();

    Signal::derive(move || {
        let query = raw_query
            .get()
            .latest_values()
            .map(|(query_key, query_value)| (query_key.to_string(), query_value.to_string()))
            .collect::<BTreeMap<_, _>>();
        sanitize_route_query(policy.as_ref(), route_segment.as_deref(), subpath.as_deref(), &query)
            .get(key)
            .cloned()
    })
}

pub fn use_route_query_writer() -> RouteQueryWriter {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let policy = use_context::<RouteQueryPolicy>();
    let raw_query = use_query_map();
    let location = use_location();
    let navigate = use_navigate();
    let route_segment = route_context.route_segment.clone();
    let subpath = route_context.subpath.clone();

    RouteQueryWriter {
        apply: Arc::new(move |updates, replace| {
            let mut next_query = raw_query
                .get_untracked()
                .latest_values()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect::<BTreeMap<_, _>>();

            for (key, value) in updates {
                match value
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty())
                {
                    Some(value) => {
                        next_query.insert(key, value);
                    }
                    None => {
                        next_query.remove(&key);
                    }
                }
            }

            let next_query = sanitize_route_query(
                policy.as_ref(),
                route_segment.as_deref(),
                subpath.as_deref(),
                &next_query,
            );
            let pathname = location.pathname.get_untracked();
            let href = if next_query.is_empty() {
                pathname
            } else {
                let query = serde_urlencoded::to_string(next_query)
                    .unwrap_or_default();
                format!("{pathname}?{query}")
            };

            navigate(
                &href,
                NavigateOptions {
                    replace,
                    ..NavigateOptions::default()
                },
            );
        }),
    }
}

fn sanitize_route_query(
    policy: Option<&RouteQueryPolicy>,
    route_segment: Option<&str>,
    subpath: Option<&str>,
    query: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    policy
        .map(|policy| policy.sanitize(route_segment, subpath, query))
        .unwrap_or_else(|| query.clone())
}
