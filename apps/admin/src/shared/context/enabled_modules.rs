use std::collections::HashSet;

use leptos::prelude::*;
use leptos_auth::hooks::{use_tenant, use_token};

use crate::features::modules::transport;

/// The module-availability decision, derived from the request that resolved it.
///
/// The signals are memos over the resource, not cells written by an effect: an effect is a task for
/// a later tick and is not driven before the server produces the response, so a cell it wrote reads
/// as "nothing is enabled" in every SSR document. A memo is a pure function of a value the render is
/// waiting on, so the response and the browser see the same decision without a second fetch.
#[derive(Clone)]
pub struct EnabledModulesContext {
    pub modules: Memo<HashSet<String>>,
    pub is_loading: Memo<bool>,
    pub error: Memo<Option<String>>,
    refresh_generation: RwSignal<u64>,
}

impl EnabledModulesContext {
    /// Derives the availability signals from the request's resource.
    ///
    /// `refresh_generation` is owned by the caller because the resource's source reads it: the
    /// provider has to create the signal before the resource, so it cannot be created here.
    pub fn new(
        resource: Resource<Result<Vec<String>, String>>,
        refresh_generation: RwSignal<u64>,
    ) -> Self {
        Self {
            modules: Memo::new(move |_| {
                resource
                    .get()
                    .and_then(|result| result.ok())
                    .map(|modules| modules.into_iter().collect())
                    .unwrap_or_default()
            }),
            is_loading: Memo::new(move |_| resource.get().is_none()),
            error: Memo::new(move |_| resource.get().and_then(|result| result.err())),
            refresh_generation,
        }
    }

    /// Re-resolves the exact owner-issued availability decision after a
    /// lifecycle command instead of locally inferring effective enablement.
    pub fn refresh(&self) {
        self.refresh_generation
            .update(|generation| *generation = generation.wrapping_add(1));
    }
}

#[component]
pub fn EnabledModulesProvider(children: Children) -> impl IntoView {
    let token = use_token();
    let tenant = use_tenant();
    let refresh_generation = RwSignal::new(0_u64);

    // A `Resource` awaited by the render, not a `LocalResource` fed into an `Effect`. A
    // `LocalResource` is `pending().await` under `ssr`, and an `Effect` is a task spawned for a
    // later tick that is not driven before the response is produced. Either one leaves every route
    // reading `is_loading`, which is how `ModuleGuard` came to report "module unavailable" for every
    // module on the standalone host.
    let resource = Resource::new(
        move || (token.get(), tenant.get(), refresh_generation.get()),
        move |(token_value, tenant_value, _refresh_generation): (
            Option<String>,
            Option<String>,
            u64,
        )| async move {
            if token_value.is_none() || tenant_value.is_none() {
                return Ok::<_, String>(Vec::new());
            }

            transport::fetch_module_effective_policy(token_value, tenant_value)
                .await
                .map(|policy| policy.enabled_module_slugs())
                .map_err(|error| error.to_string())
        },
    );

    provide_context(EnabledModulesContext::new(resource, refresh_generation));

    children()
}

#[derive(Clone)]
pub struct EnabledModulesResource(EnabledModulesContext);

impl EnabledModulesResource {
    /// The enabled slugs once the request resolved them; `None` while it has not.
    ///
    /// Takes `&self`: the render closure that calls this re-runs on every source change, and a
    /// by-value receiver would make that closure `FnOnce`.
    pub fn modules(&self) -> Option<HashSet<String>> {
        (!self.0.is_loading.get()).then(|| self.0.modules.get())
    }

    /// The failure the request reported, if it failed.
    pub fn error(&self) -> Option<String> {
        self.0.error.get()
    }
}

/// Reads the availability decision of the current request.
///
/// A view that renders this decision reads it inside a `Suspense`, so the response carries the
/// decision instead of an empty set. Reading the decision outside a `Suspense` is what the
/// standalone host did before: an effect that had not run yet, and therefore "no modules".
pub fn use_enabled_modules_resource() -> EnabledModulesResource {
    EnabledModulesResource(use_enabled_modules_context())
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
    let enabled_modules = use_enabled_modules_resource();

    view! {
        // The guard asserts a decision, so it says which decision. "Unavailable" while the request
        // is still resolving is a different statement from "disabled for this tenant", and only one
        // of them is a reason to hide a surface.
        <Suspense fallback=|| view! {
            <div class="rounded-xl border border-border bg-card p-6 shadow-sm" aria-busy="true">
                <div class="h-6 w-48 animate-pulse rounded-lg bg-muted"></div>
                <div class="mt-3 h-4 w-64 animate-pulse rounded-lg bg-muted"></div>
            </div>
        }>
            {move || match enabled_modules.modules() {
                None => view! {
                    <div class="rounded-xl border border-destructive/30 bg-destructive/10 p-6 text-sm text-destructive" role="alert">
                        {enabled_modules
                            .error()
                            .unwrap_or_else(|| "Module availability was not resolved".to_string())}
                    </div>
                }
                .into_any(),
                Some(modules) if modules.contains(slug) => children().into_any(),
                Some(_) => view! {
                    <div class="rounded-xl border border-border bg-card p-6 text-card-foreground shadow-sm">
                        <h3 class="text-lg font-semibold">"Module unavailable"</h3>
                        <p class="mt-2 text-sm text-muted-foreground">
                            "This module is disabled for the current tenant."
                        </p>
                    </div>
                }
                .into_any(),
            }}
        </Suspense>
    }
}
