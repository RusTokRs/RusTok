use leptos::prelude::*;

use crate::entities::module::{InstalledModule, MarketplaceModule};

fn short_checksum(value: Option<&str>) -> Option<String> {
    let value = value?;
    if value.len() > 16 {
        Some(format!("{}...", &value[..12]))
    } else {
        Some(value.to_string())
    }
}

#[component]
pub fn ModuleUpdateCard(
    module: MarketplaceModule,
    installed_module: InstalledModule,
    #[prop(into)] platform_loading: Signal<bool>,
    #[prop(default = Signal::derive(|| false))] platform_busy: Signal<bool>,
    #[prop(optional)] on_inspect: Option<Callback<String>>,
    on_upgrade: Callback<(String, String)>,
) -> impl IntoView {
    let current_version = installed_module
        .version
        .clone()
        .unwrap_or_else(|| "unpinned".to_string());
    let module_slug = module.slug.clone();
    let module_slug_for_inspect = module_slug.clone();
    let module_slug_for_upgrade = module_slug.clone();
    let target_version = module.latest_version.clone();
    let target_version_label = target_version.clone();
    let module_name = module.name.clone();
    let module_description = module.description.clone();
    let module_dependencies = module.dependencies.clone();
    let module_ownership = module.ownership.clone();
    let module_trust_level = module.trust_level.clone();
    let recommended_admin_surfaces = module.recommended_admin_surfaces.clone();
    let showcase_admin_surfaces = module.showcase_admin_surfaces.clone();
    let source_label = installed_module.source.clone();
    let publisher = module
        .publisher
        .clone()
        .unwrap_or_else(|| "Workspace / unknown".to_string());
    let checksum = short_checksum(module.checksum_sha256.as_deref());
    let version_trail = module
        .versions
        .clone()
        .into_iter()
        .take(3)
        .collect::<Vec<_>>();

    view! {
        <div class="rounded-xl border border-border bg-card text-card-foreground shadow-sm">
            <div class="p-6 pb-3">
                <div class="flex items-start justify-between gap-3">
                    <div>
                        <h3 class="text-base font-semibold text-card-foreground">{module_name}</h3>
                        <p class="mt-1 text-sm text-muted-foreground">{module_description}</p>
                    </div>
                    <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-muted-foreground">
                        {source_label}
                    </span>
                </div>
            </div>
            <div class="space-y-4 px-6 pb-6">
                <div class="flex flex-wrap items-center gap-2 text-xs">
                    <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 font-semibold text-secondary-foreground">
                        {module_ownership.replace('_', " ")}
                    </span>
                    <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                        {module_trust_level.replace('_', " ")}
                    </span>
                    <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                        {if module.compatible { "Compatible" } else { "Compatibility risk" }}
                    </span>
                    {module.signature_present.then(|| view! {
                        <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 font-semibold text-secondary-foreground">
                            "Signed"
                        </span>
                    })}
                    <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 font-semibold text-secondary-foreground">
                        {format!("Current v{}", current_version)}
                    </span>
                    <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                        {format!("Latest v{}", target_version_label)}
                    </span>
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
                                module.rustok_min_version.as_ref().map(|value| format!(">= {}", value)).unwrap_or_else(|| "no min".to_string()),
                                module.rustok_max_version.as_ref().map(|value| format!(", <= {}", value)).unwrap_or_else(|| ", no max".to_string())
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

                {(!module_dependencies.is_empty()).then(|| {
                    let deps = module_dependencies.join(", ");
                    view! {
                        <p class="text-xs text-muted-foreground">{format!("Depends on: {}", deps)}</p>
                    }
                })}

                <p class="text-xs text-muted-foreground">
                    "Upgrade writes the target version into modules.toml and queues a platform rebuild for both admin stacks."
                </p>

                <div class="flex justify-end">
                    <div class="flex w-full items-center justify-between gap-3">
                        <button
                            type="button"
                            class="inline-flex items-center justify-center rounded-md px-3 py-2 text-sm font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                            disabled=move || platform_loading.get()
                            on:click=move |_| {
                                if let Some(cb) = on_inspect.clone() {
                                    cb.run(module_slug_for_inspect.clone());
                                }
                            }
                        >
                            "Details"
                        </button>
                        <button
                            type="button"
                            class="inline-flex items-center justify-center rounded-md bg-primary px-3 py-2 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:pointer-events-none disabled:opacity-50"
                            disabled=move || platform_busy.get() || platform_loading.get()
                            on:click=move |_| on_upgrade.run((module_slug_for_upgrade.clone(), target_version.clone()))
                        >
                            {move || {
                                if platform_loading.get() {
                                    "Queueing...".to_string()
                                } else {
                                    format!("Upgrade to v{}", target_version_label)
                                }
                            }}
                        </button>
                    </div>
                </div>
            </div>
        </div>
    }
}
