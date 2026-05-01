use std::collections::HashSet;

use leptos::prelude::*;
use leptos_auth::hooks::{use_tenant, use_token};

use crate::app::modules::core_module_slugs;
use crate::features::modules::api;

fn local_resource<S, Fut, T>(
    source: impl Fn() -> S + 'static,
    fetcher: impl Fn(S) -> Fut + 'static,
) -> LocalResource<T>
where
    S: 'static,
    Fut: std::future::Future<Output = T> + 'static,
    T: 'static,
{
    LocalResource::new(move || fetcher(source()))
}

#[derive(Clone)]
pub struct EnabledModulesContext {
    pub modules: RwSignal<HashSet<String>>,
    pub is_loading: RwSignal<bool>,
    pub error: RwSignal<Option<String>>,
}

impl EnabledModulesContext {
    pub fn new() -> Self {
        Self {
            modules: RwSignal::new(HashSet::new()),
            is_loading: RwSignal::new(true),
            error: RwSignal::new(None),
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

impl Default for EnabledModulesContext {
    fn default() -> Self {
        Self::new()
    }
}

#[component]
pub fn EnabledModulesProvider(children: Children) -> impl IntoView {
    let context = EnabledModulesContext::new();
    provide_context(context.clone());

    let token = use_token();
    let tenant = use_tenant();

    let resource = local_resource(
        move || (token.get(), tenant.get()),
        move |(token_value, tenant_value)| async move {
            if token_value.is_none() || tenant_value.is_none() {
                return Ok(Vec::new());
            }

            api::fetch_enabled_modules(token_value, tenant_value).await
        },
    );

    let context_for_effect = context.clone();
    Effect::new(move |_| match resource.get() {
        Some(Ok(modules)) => {
            context_for_effect.replace_modules(
                modules
                    .into_iter()
                    .chain(core_module_slugs().iter().map(|slug| slug.to_string())),
            );
            context_for_effect.error.set(None);
            context_for_effect.is_loading.set(false);
        }
        Some(Err(err)) => {
            context_for_effect
                .replace_modules(core_module_slugs().iter().map(|slug| slug.to_string()));
            context_for_effect.error.set(Some(format!("{}", err)));
            context_for_effect.is_loading.set(false);
        }
        None => {
            context_for_effect.is_loading.set(true);
        }
    });

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
pub fn ModuleGuard(slug: &'static str, children: ChildrenFn) -> impl IntoView {
    let is_enabled = use_is_module_enabled(slug);

    view! {
        <Show
            when=move || is_enabled.get()
            fallback=|| view! {
                <div class="rounded-xl border border-border bg-card p-6 text-card-foreground shadow-sm">
                    <h3 class="text-lg font-semibold">"Module unavailable"</h3>
                    <p class="mt-2 text-sm text-muted-foreground">
                        "This module is disabled for the current tenant."
                    </p>
                </div>
            }
        >
            {children()}
        </Show>
    }
}
