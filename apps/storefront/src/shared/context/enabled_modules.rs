use std::collections::HashSet;

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::shared::api::{request, ApiError};

const ENABLED_MODULES_QUERY: &str = "query EnabledModules { enabledModules }";

#[derive(Clone, Debug, Deserialize, Serialize)]
struct EnabledModulesResponse {
    #[serde(rename = "enabledModules")]
    enabled_modules: Vec<String>,
}

#[derive(Clone)]
pub struct EnabledModulesContext {
    pub modules: RwSignal<HashSet<String>>,
}

impl EnabledModulesContext {
    pub fn new<I>(modules: I) -> Self
    where
        I: IntoIterator<Item = String>,
    {
        Self {
            modules: RwSignal::new(modules.into_iter().collect()),
        }
    }

    pub fn replace_modules<I>(&self, modules: I)
    where
        I: IntoIterator<Item = String>,
    {
        self.modules.set(modules.into_iter().collect());
    }

    pub fn set_module_enabled(&self, slug: &str, enabled: bool) {
        self.modules.update(|modules| {
            if enabled {
                modules.insert(slug.to_string());
            } else {
                modules.remove(slug);
            }
        });
    }
}

fn configured_tenant_slug() -> Option<String> {
    [
        "RUSTOK_TENANT_SLUG",
        "NEXT_PUBLIC_TENANT_SLUG",
        "NEXT_PUBLIC_DEFAULT_TENANT_SLUG",
    ]
    .into_iter()
    .find_map(|key| {
        std::env::var(key).ok().and_then(|value| {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        })
    })
}

pub async fn fetch_enabled_modules() -> Result<Vec<String>, ApiError> {
    let Some(tenant_slug) = configured_tenant_slug() else {
        return Ok(Vec::new());
    };

    let response: EnabledModulesResponse =
        request(ENABLED_MODULES_QUERY, (), None, Some(tenant_slug)).await?;
    Ok(response.enabled_modules)
}

#[component]
pub fn EnabledModulesProvider(initial_modules: Vec<String>, children: Children) -> impl IntoView {
    let context = EnabledModulesContext::new(initial_modules);
    provide_context(context);
    children()
}

pub fn use_enabled_modules_context() -> EnabledModulesContext {
    use_context::<EnabledModulesContext>().expect(
        "EnabledModulesContext not found. Make sure to wrap your app with <EnabledModulesProvider>",
    )
}

pub fn use_enabled_modules() -> Signal<HashSet<String>> {
    let context = use_enabled_modules_context();
    Signal::derive(move || context.modules.get())
}

pub fn use_is_module_enabled(slug: &'static str) -> Signal<bool> {
    let context = use_enabled_modules_context();
    Signal::derive(move || context.modules.get().contains(slug))
}

#[component]
pub fn ModuleGuard(slug: &'static str, children: Children) -> impl IntoView {
    let is_enabled = use_is_module_enabled(slug);

    move || {
        if is_enabled.get() {
            children().into_any()
        } else {
            view! {
                <div class="rounded-xl border border-border bg-card p-6 text-card-foreground shadow-sm">
                    <h3 class="text-lg font-semibold">"Module unavailable"</h3>
                    <p class="mt-2 text-sm text-muted-foreground">
                        "This module is disabled for the current tenant."
                    </p>
                </div>
            }
            .into_any()
        }
    }
}
