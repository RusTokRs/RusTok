use std::collections::HashSet;

use leptos::prelude::*;
use rustok_auth_ui::ui::leptos::context::AuthContext;
use rustok_auth_ui::ui::leptos::hooks::{use_auth, use_tenant, use_token};

use crate::features::modules::transport;
use crate::shared::ui::Button;
use crate::use_admin_locale;

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

type PolicyResult = (u64, u64, Result<Vec<String>, String>);

// Resource retains its previous value during a refetch. Only publish a result
// whose credential-free auth and refresh stamps still match the current scope.
fn current_policy_result(
    result: Option<PolicyResult>,
    scope_ready: bool,
    scope_generation: u64,
    refresh_generation: u64,
) -> Option<Result<Vec<String>, String>> {
    result.and_then(|(result_scope, result_refresh, value)| {
        (scope_ready && result_scope == scope_generation && result_refresh == refresh_generation)
            .then_some(value)
    })
}

impl EnabledModulesContext {
    /// Derives availability only from a resource resolved for the current scope.
    fn new(
        resource: Resource<PolicyResult>,
        refresh_generation: RwSignal<u64>,
        auth: AuthContext,
    ) -> Self {
        let decision = Memo::new(move |_| {
            let scope = auth.client_scope();
            current_policy_result(
                resource.get(),
                scope.is_ready(),
                scope.generation,
                refresh_generation.get(),
            )
        });
        Self {
            modules: Memo::new(move |_| {
                decision
                    .get()
                    .and_then(|result| result.ok())
                    .map(|modules| modules.into_iter().collect())
                    .unwrap_or_default()
            }),
            is_loading: Memo::new(move |_| decision.get().is_none()),
            error: Memo::new(move |_| decision.get().and_then(|result| result.err())),
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
    let auth = use_auth();
    let source_auth = auth.clone();
    let refresh_generation = RwSignal::new(0_u64);

    // A `Resource` awaited by the render, not a `LocalResource` fed into an `Effect`. A
    // `LocalResource` is `pending().await` under `ssr`, and an `Effect` is a task spawned for a
    // later tick that is not driven before the response is produced. Either one leaves every route
    // reading `is_loading`, which is how `ModuleGuard` came to report "module unavailable" for every
    // module on the standalone host.
    let resource = Resource::new(
        move || {
            let scope = source_auth.client_scope();
            (
                token.get(),
                tenant.get(),
                scope.generation,
                scope.is_ready(),
                refresh_generation.get(),
            )
        },
        move |(token_value, tenant_value, scope_generation, scope_ready, refresh): (
            Option<String>,
            Option<String>,
            u64,
            bool,
            u64,
        )| async move {
            let result = if !scope_ready
                || token_value.as_deref().is_none_or(str::is_empty)
                || tenant_value.as_deref().is_none_or(str::is_empty)
            {
                Err("module availability scope is missing".to_string())
            } else {
                transport::fetch_module_effective_policy(token_value, tenant_value)
                    .await
                    .map(|policy| policy.enabled_module_slugs())
                    .map_err(|error| error.to_string())
            };
            // Never serialize the bearer token or tenant input into the resource result.
            (scope_generation, refresh, result)
        },
    );

    provide_context(EnabledModulesContext::new(resource, refresh_generation, auth));

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
        (!self.0.is_loading.get() && self.0.error.get().is_none())
            .then(|| self.0.modules.get())
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
    // INVARIANT: EnabledModulesProvider is mounted in App shell before child component rendering
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

/// Retry a failed availability read without disclosing transport errors or
/// presenting the failure as an owner-issued disabled decision.
#[component]
pub fn ModulePolicyReadError() -> impl IntoView {
    let retry_context = use_enabled_modules_context();
    let i18n = use_admin_locale();
    view! {
        <div class="rounded-xl border border-destructive/30 bg-destructive/10 p-6 text-sm text-destructive" role="alert">
            <p>{move || i18n.translate("modules.policy.error.description")}</p>
            <div class="mt-4">
                <Button on_click=move |_| retry_context.refresh()>
                    {move || i18n.translate("modules.policy.error.retry")}
                </Button>
            </div>
        </div>
    }
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
                None if enabled_modules.error().is_some() => view! {
                    <ModulePolicyReadError />
                }.into_any(),
                None => view! {
                    <div class="rounded-xl border border-border bg-card p-6 shadow-sm" aria-busy="true">
                        <div class="h-6 w-48 animate-pulse rounded-lg bg-muted"></div>
                        <div class="mt-3 h-4 w-64 animate-pulse rounded-lg bg-muted"></div>
                    </div>
                }.into_any(),
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

#[cfg(test)]
mod tests {
    use super::current_policy_result;

    #[test]
    fn availability_requires_matching_ready_scope_and_refresh() {
        let old = || Some((3, 7, Ok(vec!["forum".to_string()])));
        assert_eq!(
            current_policy_result(old(), true, 3, 7),
            Some(Ok(vec!["forum".to_string()]))
        );
        assert_eq!(current_policy_result(old(), false, 3, 7), None);
        assert_eq!(current_policy_result(old(), true, 4, 7), None);
        assert_eq!(current_policy_result(old(), true, 3, 8), None);
        assert_eq!(current_policy_result(None, true, 3, 7), None);

        let error = Some((3, 7, Err("read failed".to_string())));
        assert_eq!(
            current_policy_result(error.clone(), true, 3, 7),
            Some(Err("read failed".to_string()))
        );
        assert_eq!(current_policy_result(error, true, 4, 7), None);
    }
}
