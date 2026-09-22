use std::collections::{BTreeMap, HashMap};

use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;

#[component]
pub fn ModuleRequestProvider(
    locale: Option<String>,
    route_segment: Option<String>,
    subpath: Option<String>,
    query_params: HashMap<String, String>,
    children: Children,
) -> impl IntoView {
    let query = query_params.into_iter().collect::<BTreeMap<_, _>>();
    provide_context(UiRouteContext {
        locale,
        route_segment,
        subpath,
        query,
    });
    children()
}
