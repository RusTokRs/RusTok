use std::collections::HashSet;

use leptos::prelude::*;
use rustok_ui_transport::UiTransportPath;
use serde::{Deserialize, Serialize};

use crate::shared::api::{ApiError, configured_tenant_slug};

fn selected_transport_path() -> UiTransportPath {
    if cfg!(all(target_arch = "wasm32", not(feature = "hydrate"))) {
        UiTransportPath::Graphql
    } else {
        UiTransportPath::NativeServer
    }
}

const ENABLED_MODULES_QUERY: &str = "query EnabledModules { enabledModules(limit: 100) }";

// GraphQL provides neither pagination nor a completeness marker. The native
// owner reader is not subject to this transport cap.
fn validate_graphql_projection(modules: Vec<String>) -> Result<Vec<String>, ApiError> {
    let mut seen = HashSet::new();
    if modules.len() >= 100
        || modules
            .iter()
            .any(|slug| !rustok_api::is_valid_module_slug(slug) || !seen.insert(slug))
    {
        return Err(ApiError::from(ServerFnError::new(
            "module availability response is invalid or may be incomplete",
        )));
    }
    Ok(modules)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct EnabledModulesResponse {
    #[serde(rename = "enabledModules")]
    enabled_modules: Vec<String>,
}

#[derive(Clone)]
pub struct EnabledModulesContext {
    // All replacements go through the methods below so independent client islands are fenced
    // synchronously, before the new projection becomes visible to host UI.
    modules: RwSignal<HashSet<String>>,
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
        let modules = modules.into_iter().collect::<HashSet<_>>();
        if self.modules.try_get_untracked().is_none() {
            return;
        }
        crate::enabled_module_events::publish(&modules);
        // Event listeners run synchronously and may dispose the provider.
        drop(self.modules.try_set(modules));
    }

    pub fn set_module_enabled(&self, slug: &str, enabled: bool) {
        let Some(mut modules) = self.modules.try_get_untracked() else {
            return;
        };
        if enabled {
            modules.insert(slug.to_string());
        } else {
            modules.remove(slug);
        }
        crate::enabled_module_events::publish(&modules);
        drop(self.modules.try_set(modules));
    }
}

/// Clear any unbound DOM projection before a standalone CSR policy read.
/// Other independently mounted islands must fail closed while the read is pending.
pub fn invalidate_pending_projection() {
    crate::enabled_module_events::publish(&HashSet::new());
}

pub async fn fetch_enabled_modules() -> Result<Vec<String>, ApiError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => fetch_enabled_modules_server().await,
        UiTransportPath::Graphql => {
            let Some(tenant_slug) = configured_tenant_slug() else {
                return Err(ApiError::from(ServerFnError::new(
                    "module availability scope is missing",
                )));
            };
            fetch_enabled_modules_graphql(tenant_slug).await
        }
    }
}

pub async fn fetch_enabled_modules_server() -> Result<Vec<String>, ApiError> {
    super::enabled_modules_native_server_adapter::list_enabled_modules()
        .await
        .map_err(ApiError::from)
}

pub async fn fetch_enabled_modules_graphql(tenant_slug: String) -> Result<Vec<String>, ApiError> {
    let response: EnabledModulesResponse =
        crate::shared::api::request(ENABLED_MODULES_QUERY, (), None, Some(tenant_slug)).await?;
    validate_graphql_projection(response.enabled_modules)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AvailabilityState {
    Loading,
    Ready,
    Failed,
}

#[component]
pub fn ModulePolicyReadError(locale: String) -> impl IntoView {
    let (title, description, retry) = crate::shared::local::module_policy_strings(&locale);
    view! {
        <main class="flex min-h-screen items-center justify-center px-4 py-16">
            <div role="alert" class="max-w-md space-y-4 rounded-xl border border-border bg-card p-6 text-card-foreground">
                <h1 class="text-xl font-semibold">{title}</h1>
                <p class="text-sm text-muted-foreground">{description}</p>
                <a href="" class="inline-block rounded-md border border-border px-4 py-2 text-sm">{retry}</a>
            </div>
        </main>
    }
}

#[component]
pub fn EnabledModulesProvider(
    initial_modules: Vec<String>,
    locale: String,
    #[prop(optional)] fetch_initial: bool,
    children: ChildrenFn,
) -> impl IntoView {
    let context = EnabledModulesContext::new(initial_modules);
    provide_context(context.clone());
    let state = RwSignal::new(if fetch_initial {
        AvailabilityState::Loading
    } else {
        AvailabilityState::Ready
    });

    #[cfg(all(feature = "csr", target_arch = "wasm32"))]
    if fetch_initial {
        let ctx = context.clone();
        leptos::task::spawn_local(async move {
            let result = fetch_enabled_modules().await;
            if state.try_get_untracked() != Some(AvailabilityState::Loading) {
                return;
            }
            match result {
                Ok(modules) => {
                    ctx.replace_modules(modules);
                    drop(state.try_set(AvailabilityState::Ready));
                }
                Err(_) => {
                    drop(state.try_set(AvailabilityState::Failed));
                }
            }
        });
    }

    view! {
        {move || match state.get() {
            AvailabilityState::Ready => children().into_any(),
            AvailabilityState::Loading => view! {
                <main aria-busy="true" class="p-6">{crate::shared::local::module_policy_loading(&locale)}</main>
            }.into_any(),
            AvailabilityState::Failed => view! { <ModulePolicyReadError locale=locale.clone() /> }.into_any(),
        }}
    }
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

#[cfg(test)]
mod tests {
    use super::validate_graphql_projection;
    #[cfg(feature = "ssr")]
    use super::{EnabledModulesContext, EnabledModulesProvider};
    #[cfg(feature = "ssr")]
    use leptos::prelude::*;

    #[test]
    #[cfg(feature = "ssr")]
    fn pending_policy_does_not_mount_children_and_empty_success_does() {
        let owner = Owner::new();
        let pending = owner.with(|| {
            view! {
                <EnabledModulesProvider initial_modules=Vec::new() locale="en".to_string() fetch_initial=true>
                    <span>"composition-ready"</span>
                </EnabledModulesProvider>
            }.to_html()
        });
        assert!(pending.contains("Checking module availability"));
        assert!(!pending.contains("composition-ready"));
        let ready = owner.with(|| {
            view! {
                <EnabledModulesProvider initial_modules=Vec::new() locale="en".to_string()>
                    <span>"composition-ready"</span>
                </EnabledModulesProvider>
            }.to_html()
        });
        assert!(ready.contains("composition-ready"));
        assert!(!ready.contains("Checking module availability"));
    }

    #[test]
    #[cfg(feature = "ssr")]
    fn disposed_projection_ignores_late_replacement() {
        let owner = Owner::new();
        let context = owner.with(|| EnabledModulesContext::new(Vec::<String>::new()));
        owner.cleanup();
        context.replace_modules(["forum".to_string()]);
        context.set_module_enabled("forum", true);
    }

    #[test]
    fn graphql_availability_rejects_invalid_and_potentially_truncated_projection() {
        assert!(validate_graphql_projection(Vec::new()).is_ok());
        assert!(validate_graphql_projection(vec![" ".to_string()]).is_err());
        assert!(validate_graphql_projection(vec!["forum?".to_string()]).is_err());
        assert!(validate_graphql_projection(vec!["forum".to_string(), "forum".to_string()]).is_err());
        let modules: Vec<String> = (0..101).map(|i| format!("module_{i}")).collect();
        assert!(validate_graphql_projection(modules[..99].to_vec()).is_ok());
        assert!(validate_graphql_projection(modules[..100].to_vec()).is_err());
        assert!(validate_graphql_projection(modules).is_err());
    }
}
