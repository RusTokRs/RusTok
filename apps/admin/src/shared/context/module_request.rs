use std::collections::BTreeMap;

use leptos::prelude::*;
use leptos_router::params::ParamsMap;
use rustok_api::UiRouteContext;

use crate::{use_i18n, Locale};

#[component]
pub fn ModuleRequestProvider(
    route_segment: Option<String>,
    subpath: Option<String>,
    query_params: ParamsMap,
    children: Children,
) -> impl IntoView {
    let query = query_params
        .latest_values()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect::<BTreeMap<_, _>>();
    let locale = match use_i18n().get_locale() {
        Locale::en => Some("en".to_string()),
        Locale::ru => Some("ru".to_string()),
    };

    provide_context(UiRouteContext {
        locale,
        route_segment,
        subpath,
        query,
    });

    children()
}
