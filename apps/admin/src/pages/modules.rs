use leptos::prelude::*;
use rustok_auth_ui::ui::leptos::hooks::{use_auth, use_tenant, use_token};
use serde::{Deserialize, Serialize};

use crate::entities::module::{
    BuildJob, InstalledModule, MarketplaceModule, ModuleCompositionSnapshot, ModuleInfo,
    TenantModule,
};
use crate::features::modules::components::ModulesList;
use crate::features::modules::transport;
use crate::shared::api::ApiError;
use crate::shared::ui::PageHeader;
use crate::use_admin_locale;

#[derive(Clone, Deserialize, Serialize)]
struct ModulesPageData {
    modules: Vec<ModuleInfo>,
    marketplace_modules: Vec<MarketplaceModule>,
    marketplace_registry_freshness: Vec<rustok_api::MarketplaceRegistryFreshness>,
    composition_snapshot: Option<ModuleCompositionSnapshot>,
    installed_modules: Vec<InstalledModule>,
    tenant_modules: Vec<TenantModule>,
    active_build: Option<BuildJob>,
    build_history: Vec<BuildJob>,
}

fn validated_tenant_modules(
    result: Result<Vec<TenantModule>, ApiError>,
) -> Result<Vec<TenantModule>, ApiError> {
    let modules = result?;
    // Both native and GraphQL projections currently cap at 100, without a
    // cursor or completeness marker. Exactly 100 is therefore ambiguous.
    if modules.len() >= 100 {
        return Err(ApiError::Graphql(
            "Tenant module settings may be incomplete; refresh after the owner provides a complete projection".to_string(),
        ));
    }
    Ok(modules)
}

fn is_current_modules_result(ready: bool, current_generation: u64, result_generation: u64) -> bool {
    ready && current_generation == result_generation
}

#[component]
pub fn Modules() -> impl IntoView {
    let i18n = use_admin_locale();
    let token = use_token();
    let tenant = use_tenant();
    let auth = use_auth();
    let source_auth = auth.clone();

    let modules_resource = Resource::new(
        move || {
            let scope = source_auth.client_scope();
            (token.get(), tenant.get(), scope.generation, scope.is_ready())
        },
        move |(token_value, tenant_value, generation, ready)| async move {
            let result = async {
                if !ready || token_value.as_deref().is_none_or(str::is_empty)
                    || tenant_value.as_deref().is_none_or(str::is_empty)
                {
                    return Err(ApiError::Unauthorized);
                }
                let modules = transport::fetch_modules(token_value.clone(), tenant_value.clone())
                    .await
                    .unwrap_or_default();
                let marketplace_modules = transport::fetch_marketplace_modules(
                    token_value.clone(),
                    tenant_value.clone(),
                    transport::MarketplaceVariables {
                        search: None,
                        category: None,
                        tag: None,
                        source: None,
                        trust_level: None,
                        only_compatible: None,
                        installed_only: None,
                    },
                )
                .await
                .unwrap_or_default();
                let marketplace_registry_freshness = transport::fetch_marketplace_registry_freshness(
                    token_value.clone(),
                    tenant_value.clone(),
                )
                .await
                .unwrap_or_default();
                let composition_snapshot = transport::fetch_module_composition_snapshot(
                    token_value.clone(),
                    tenant_value.clone(),
                )
                .await
                .ok();
                let installed_modules =
                    transport::fetch_installed_modules(token_value.clone(), tenant_value.clone())
                        .await
                        .unwrap_or_default();
                // An unavailable or capped tenant projection cannot establish that
                // a missing module has no override; never substitute an empty list.
                let tenant_modules = validated_tenant_modules(
                    transport::fetch_tenant_modules(token_value.clone(), tenant_value.clone()).await,
                )?;
                let active_build =
                    transport::fetch_active_build(token_value.clone(), tenant_value.clone())
                        .await
                        .unwrap_or_default();
                let build_history = transport::fetch_build_history(token_value, tenant_value, 10, 0)
                    .await
                    .unwrap_or_default();

                Ok::<ModulesPageData, _>(ModulesPageData {
                    modules,
                    marketplace_modules,
                    marketplace_registry_freshness,
                    composition_snapshot,
                    installed_modules,
                    tenant_modules,
                    active_build,
                    build_history,
                })
            }
            .await;
            // Never serialize credentials in a Resource result.
            (generation, result)
        },
    );

    view! {
        <section class="flex flex-1 flex-col p-4 md:px-6">
            <PageHeader
                title=i18n.translate("modules.title")
                eyebrow=i18n.translate("modules.eyebrow").to_string()
                subtitle=i18n.translate("modules.subtitle").to_string()
            />

            <Suspense
                fallback=move || view! {
                    <div class="space-y-4">
                        <div class="grid gap-4 xl:grid-cols-[minmax(0,1.4fr)_repeat(3,minmax(0,0.8fr))]">
                            <div class="h-40 animate-pulse rounded-xl bg-muted xl:col-span-2"></div>
                            <div class="h-40 animate-pulse rounded-xl bg-muted"></div>
                            <div class="h-40 animate-pulse rounded-xl bg-muted"></div>
                            <div class="h-40 animate-pulse rounded-xl bg-muted"></div>
                        </div>
                        <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
                            {(0..6)
                                .map(|_| {
                                    view! { <div class="h-40 animate-pulse rounded-xl bg-muted"></div> }
                                })
                                .collect_view()}
                        </div>
                    </div>
                }
            >
                {move || {
                    modules_resource
                        .get()
                        .filter(|(generation, _)| {
                            let scope = auth.client_scope();
                            is_current_modules_result(scope.is_ready(), scope.generation, *generation)
                        })
                        .map(|(_, result): (u64, Result<ModulesPageData, ApiError>)| {
                        match result {
                            Ok(data) => {
                                view! {
                                    <ModulesList
                                        admin_surface="leptos-admin".to_string()
                                        modules=data.modules
                                        marketplace_modules=data.marketplace_modules
                                        marketplace_registry_freshness=data.marketplace_registry_freshness
                                        composition_snapshot=data.composition_snapshot
                                        installed_modules=data.installed_modules
                                        tenant_modules=data.tenant_modules
                                        active_build=data.active_build
                                        build_history=data.build_history
                                    />
                                }.into_any()
                            }
                            Err(err) => {
                                view! {
                                    <div class="rounded-lg border border-destructive/50 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                                        {format!("{}: {}", i18n.translate("modules.error.load"), err)}
                                    </div>
                                }.into_any()
                            }
                        }
                    })
                }}
            </Suspense>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modules(count: usize) -> Vec<TenantModule> {
        (0..count)
            .map(|index| TenantModule {
                module_slug: format!("module_{index}"),
                enabled: true,
                settings: "{}".to_string(),
                revision: 1,
            })
            .collect()
    }

    #[test]
    fn old_or_unready_auth_scopes_never_publish_modules() {
        assert!(is_current_modules_result(true, 5, 5));
        assert!(!is_current_modules_result(false, 5, 5));
        assert!(!is_current_modules_result(true, 6, 5));
    }

    #[test]
    fn tenant_projection_errors_and_cap_never_become_empty_state() {
        assert!(matches!(
            validated_tenant_modules(Err(ApiError::Unauthorized)),
            Err(ApiError::Unauthorized)
        ));
        assert_eq!(validated_tenant_modules(Ok(modules(0))).unwrap().len(), 0);
        assert_eq!(validated_tenant_modules(Ok(modules(99))).unwrap().len(), 99);
        assert!(validated_tenant_modules(Ok(modules(100))).is_err());
        assert!(validated_tenant_modules(Ok(modules(101))).is_err());
    }
}
