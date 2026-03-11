use leptos::prelude::*;

use crate::entities::module::{MarketplaceModule, ModuleInfo};
use crate::{t, t_string, use_i18n};

fn short_checksum(value: Option<&str>) -> Option<String> {
    let value = value?;
    if value.len() > 16 {
        Some(format!("{}...", &value[..12]))
    } else {
        Some(value.to_string())
    }
}

#[component]
pub fn ModuleCard(
    module: ModuleInfo,
    #[prop(optional)] catalog_module: Option<MarketplaceModule>,
    #[prop(into)] tenant_loading: Signal<bool>,
    #[prop(into)] platform_loading: Signal<bool>,
    #[prop(into)] platform_installed: Signal<bool>,
    #[prop(default = Signal::derive(|| false))] platform_busy: Signal<bool>,
    #[prop(default = Signal::derive(|| None))] platform_version: Signal<Option<String>>,
    #[prop(default = Signal::derive(|| None))] recommended_version: Signal<Option<String>>,
    #[prop(optional)] on_toggle: Option<Callback<(String, bool)>>,
    #[prop(optional)] on_install: Option<Callback<(String, String)>>,
    #[prop(optional)] on_inspect: Option<Callback<String>>,
    #[prop(optional)] on_uninstall: Option<Callback<String>>,
) -> impl IntoView {
    let i18n = use_i18n();
    let is_core = module.is_core();
    let module_name = module.name.clone();
    let module_description = module.description.clone();
    let module_version = module.version.clone();
    let module_dependencies = module.dependencies.clone();
    let module_ownership = module.ownership.clone();
    let module_trust_level = module.trust_level.clone();
    let recommended_admin_surfaces = module.recommended_admin_surfaces.clone();
    let showcase_admin_surfaces = module.showcase_admin_surfaces.clone();
    let catalog_module_value = catalog_module.clone();
    let slug = module.module_slug.clone();
    let slug_for_toggle = slug.clone();
    let slug_for_install = slug.clone();
    let slug_for_inspect = slug.clone();
    let slug_for_uninstall = slug.clone();
    let version_for_install = module.version.clone();
    let raw_enabled = module.enabled;

    let current_version = platform_version;
    let latest_version = recommended_version;
    let update_available =
        Signal::derive(
            move || match (current_version.get(), latest_version.get()) {
                (Some(current), Some(latest)) => current != latest,
                _ => false,
            },
        );
    let tenant_enabled =
        Signal::derive(move || is_core || (platform_installed.get() && raw_enabled));
    let opacity_class = move || {
        if !tenant_enabled.get() && !is_core {
            "opacity-60"
        } else {
            ""
        }
    };

    view! {
        <div class=move || format!(
            "rounded-xl border border-border bg-card text-card-foreground shadow-sm transition-all hover:shadow-md {}",
            opacity_class()
        )>
            <div class="p-6 pb-3">
                <div class="flex items-start justify-between gap-3">
                    <div class="flex items-center gap-2">
                        {move || {
                            if tenant_enabled.get() {
                                view! {
                                    <svg class="h-5 w-5 shrink-0 text-primary" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                        <path d="M16.88 3.549a8.94 8.94 0 0 1 2.18 2.661c.497.951.79 2.007.844 3.09.054 1.078-.1 2.168-.452 3.192a9.01 9.01 0 0 1-1.678 2.856M7.12 3.549a8.94 8.94 0 0 0-2.18 2.661A9.016 9.016 0 0 0 4.096 9.3c-.054 1.078.1 2.168.452 3.192a9.01 9.01 0 0 0 1.678 2.856M12 2v2M12 18v4M8 22h8" />
                                    </svg>
                                }
                                    .into_any()
                            } else {
                                view! {
                                    <svg class="h-5 w-5 shrink-0 text-muted-foreground" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                        <path d="M2 2l20 20M16.88 3.549a8.94 8.94 0 0 1 2.18 2.661c.497.951.79 2.007.844 3.09M7.12 3.549a8.94 8.94 0 0 0-2.18 2.661A9.016 9.016 0 0 0 4.096 9.3M12 2v2M12 18v4M8 22h8" />
                                    </svg>
                                }
                                    .into_any()
                            }
                        }}
                        <h3 class="text-base font-semibold text-card-foreground">{module_name}</h3>
                    </div>
                    <div class="flex flex-wrap items-center gap-2">
                        {is_core.then(|| view! {
                            <span class="inline-flex items-center rounded-full bg-primary px-2.5 py-0.5 text-xs font-semibold text-primary-foreground">
                                {t!(i18n, modules.badge.core)}
                            </span>
                        })}
                        {(!is_core).then(|| view! {
                            <Show when=move || !platform_installed.get()>
                                <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 text-xs font-semibold text-secondary-foreground">
                                    "Not installed"
                                </span>
                            </Show>
                        })}
                        {(!is_core).then(|| view! {
                            <Show when=move || platform_installed.get() && platform_version.get().is_some()>
                                <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 text-xs font-semibold text-secondary-foreground">
                                    {move || format!("Manifest v{}", platform_version.get().unwrap_or_default())}
                                </span>
                            </Show>
                        })}
                        <Show when=move || update_available.get()>
                            <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-muted-foreground">
                                {move || format!("Update v{}", recommended_version.get().unwrap_or_default())}
                            </span>
                        </Show>
                        <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-muted-foreground">
                            {format!("v{}", module_version)}
                        </span>
                    </div>
                </div>
                <p class="mt-2 text-sm text-muted-foreground">{module_description}</p>
            </div>
            <div class="space-y-4 px-6 pb-6">
                <div class="flex flex-wrap items-center gap-2 text-xs">
                    <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 font-semibold text-secondary-foreground">
                        {module_ownership.replace('_', " ")}
                    </span>
                    <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                        {module_trust_level.replace('_', " ")}
                    </span>
                    {catalog_module_value.as_ref().map(|catalog| {
                        view! {
                            <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                                {if catalog.compatible { "Compatible" } else { "Compatibility risk" }}
                            </span>
                        }
                    })}
                    {catalog_module_value.as_ref().is_some_and(|catalog| catalog.signature_present).then(|| view! {
                        <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 font-semibold text-secondary-foreground">
                            "Signed"
                        </span>
                    })}
                    {recommended_admin_surfaces
                        .into_iter()
                        .map(|surface| {
                            view! {
                                <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                                    {format!("Primary: {}", surface.replace('-', " "))}
                                </span>
                            }
                        })
                        .collect_view()}
                    {showcase_admin_surfaces
                        .into_iter()
                        .map(|surface| {
                            view! {
                                <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                                    {format!("Showcase: {}", surface.replace('-', " "))}
                                </span>
                            }
                        })
                        .collect_view()}
                </div>

                {catalog_module_value.clone().map(|catalog| {
                    let publisher = catalog
                        .publisher
                        .clone()
                        .unwrap_or_else(|| "Workspace / unknown".to_string());
                    let checksum = short_checksum(catalog.checksum_sha256.as_deref());
                    let version_trail = catalog.versions.into_iter().take(3).collect::<Vec<_>>();
                    view! {
                        <div class="grid gap-2 rounded-lg border border-border/60 bg-muted/30 p-3 text-xs">
                            <div class="flex flex-wrap items-center gap-2">
                                <span class="text-muted-foreground">"Publisher:"</span>
                                <span>{publisher}</span>
                                {checksum.map(|checksum| view! {
                                    <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-mono font-medium text-muted-foreground">
                                        {format!("sha256 {}", checksum)}
                                    </span>
                                })}
                            </div>
                            <div class="flex flex-wrap items-center gap-2">
                                <span class="text-muted-foreground">"RusTok:"</span>
                                <span>
                                    {format!(
                                        "{}{}",
                                        catalog.rustok_min_version.as_ref().map(|value| format!(">= {}", value)).unwrap_or_else(|| "no min".to_string()),
                                        catalog.rustok_max_version.as_ref().map(|value| format!(", <= {}", value)).unwrap_or_else(|| ", no max".to_string())
                                    )}
                                </span>
                            </div>
                            {(!version_trail.is_empty()).then(|| {
                                view! {
                                    <div class="flex flex-wrap items-center gap-2">
                                        <span class="text-muted-foreground">"Versions:"</span>
                                        {version_trail.into_iter().map(|version| {
                                            view! {
                                                <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                                                    {format!("v{}{}", version.version, if version.yanked { " yanked" } else { "" })}
                                                </span>
                                            }
                                        }).collect_view()}
                                    </div>
                                }
                            })}
                        </div>
                    }
                })}

                <div class="flex items-center justify-between gap-3">
                    <div class="text-xs text-muted-foreground">
                        {(!module_dependencies.is_empty()).then(|| {
                            let deps = module_dependencies.join(", ");
                            view! {
                                <span>{format!("{}: {}", t_string!(i18n, modules.depends_on), deps)}</span>
                            }
                        })}
                    </div>
                    {if is_core {
                        view! {
                            <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 text-xs font-semibold text-secondary-foreground">
                                {t!(i18n, modules.always_on)}
                            </span>
                        }
                            .into_any()
                    } else {
                        let on_toggle = on_toggle.clone();
                        view! {
                            <div class="flex items-center gap-2">
                                <span class="text-xs text-muted-foreground">
                                    {move || {
                                        if !platform_installed.get() {
                                            "Unavailable".to_string()
                                        } else if tenant_enabled.get() {
                                            t_string!(i18n, modules.enabled).to_string()
                                        } else {
                                            t_string!(i18n, modules.disabled).to_string()
                                        }
                                    }}
                                </span>
                                <button
                                    type="button"
                                    class=move || {
                                        let base = "relative inline-flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-full border-2 border-transparent transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background disabled:cursor-not-allowed disabled:opacity-50";
                                        if tenant_enabled.get() {
                                            format!("{} bg-primary", base)
                                        } else {
                                            format!("{} bg-input", base)
                                        }
                                    }
                                    disabled=move || tenant_loading.get() || platform_loading.get() || !platform_installed.get()
                                    on:click=move |_| {
                                        if let Some(cb) = on_toggle.clone() {
                                            cb.run((slug_for_toggle.clone(), !tenant_enabled.get()));
                                        }
                                    }
                                >
                                    <span
                                        class=move || {
                                            let base = "pointer-events-none block h-4 w-4 rounded-full bg-background shadow-lg ring-0 transition-transform";
                                            if tenant_enabled.get() {
                                                format!("{} translate-x-4", base)
                                            } else {
                                                format!("{} translate-x-0", base)
                                            }
                                        }
                                    />
                                </button>
                            </div>
                        }
                            .into_any()
                    }}
                </div>

                {(!is_core).then(|| {
                    let on_install = on_install.clone();
                    let on_uninstall = on_uninstall.clone();
                    let on_inspect = on_inspect.clone();
                    view! {
                        <div class="flex items-center justify-between gap-3 border-t pt-3">
                            <button
                                type="button"
                                class="inline-flex items-center justify-center rounded-md px-3 py-2 text-sm font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                disabled=move || platform_loading.get()
                                on:click=move |_| {
                                    if let Some(cb) = on_inspect.clone() {
                                        cb.run(slug_for_inspect.clone());
                                    }
                                }
                            >
                                "Details"
                            </button>
                            <div class="flex items-center gap-3">
                                <div class="text-xs text-muted-foreground">
                                    {move || {
                                        if platform_installed.get() {
                                            match platform_version.get() {
                                                Some(version) => format!("Installed in platform manifest as v{}", version),
                                                None => "Installed in platform manifest".to_string(),
                                            }
                                        } else {
                                            "Missing from platform manifest".to_string()
                                        }
                                    }}
                                </div>
                                <Show
                                    when=move || platform_installed.get()
                                    fallback=move || view! {
                                        <button
                                            type="button"
                                            class="inline-flex items-center justify-center rounded-md bg-primary px-3 py-2 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:pointer-events-none disabled:opacity-50"
                                            disabled=move || platform_busy.get() || platform_loading.get()
                                            on:click=move |_| {
                                                if let Some(cb) = on_install.clone() {
                                                    cb.run((slug_for_install.clone(), version_for_install.clone()));
                                                }
                                            }
                                        >
                                            {move || if platform_loading.get() { "Queueing..." } else { "Install" }}
                                        </button>
                                    }
                                >
                                    <button
                                        type="button"
                                        class="inline-flex items-center justify-center rounded-md border border-border bg-background px-3 py-2 text-sm font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                        disabled=move || platform_busy.get() || platform_loading.get()
                                        on:click=move |_| {
                                            if let Some(cb) = on_uninstall.clone() {
                                                cb.run(slug_for_uninstall.clone());
                                            }
                                        }
                                    >
                                        {move || if platform_loading.get() { "Queueing..." } else { "Uninstall" }}
                                    </button>
                                </Show>
                            </div>
                        </div>
                    }
                })}
                {is_core.then(|| {
                    let on_inspect = on_inspect.clone();
                    view! {
                        <div class="flex items-center justify-between gap-3 border-t pt-3">
                            <button
                                type="button"
                                class="inline-flex items-center justify-center rounded-md px-3 py-2 text-sm font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                disabled=move || platform_loading.get()
                                on:click=move |_| {
                                    if let Some(cb) = on_inspect.clone() {
                                        cb.run(slug.clone());
                                    }
                                }
                            >
                                "Details"
                            </button>
                            <span class="text-xs text-muted-foreground">
                                "Built into the platform manifest"
                            </span>
                        </div>
                    }
                })}
            </div>
        </div>
    }
}
