use leptos::prelude::*;

use crate::entities::module::MarketplaceModule;

fn short_checksum(value: Option<&str>) -> Option<String> {
    let value = value?;
    if value.len() > 16 {
        Some(format!("{}...", &value[..12]))
    } else {
        Some(value.to_string())
    }
}

fn humanize_token(value: &str) -> String {
    value
        .split(['-', '_'])
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[component]
pub fn module_detail_panel(
    admin_surface: String,
    selected_slug: String,
    module: Option<MarketplaceModule>,
    #[prop(into)] loading: Signal<bool>,
    on_close: Callback<()>,
) -> impl IntoView {
    let detail = module.clone();
    let detail_for_body = module;
    let admin_surface_for_body = admin_surface.clone();

    view! {
        <div class="rounded-xl border border-primary/20 bg-primary/5 p-6 shadow-sm">
            <div class="flex items-start justify-between gap-3">
                <div class="space-y-1">
                    <h3 class="text-base font-semibold text-card-foreground">"Module detail"</h3>
                    <p class="text-sm text-muted-foreground">
                        {match detail.as_ref() {
                            Some(module) => format!(
                                "{} metadata from the internal marketplace catalog.",
                                module.name
                            ),
                            None if loading.get() => format!(
                                "Loading {} from the internal marketplace catalog.",
                                selected_slug
                            ),
                            None => format!("No catalog entry resolved for {}.", selected_slug),
                        }}
                    </p>
                </div>
                <button
                    type="button"
                    class="inline-flex items-center justify-center rounded-md border border-border bg-background px-3 py-2 text-sm font-medium text-foreground transition-colors hover:bg-accent"
                    on:click=move |_| on_close.run(())
                >
                    "Close"
                </button>
            </div>

            <Show
                when=move || detail.is_some()
                fallback=move || view! {
                    <p class="mt-4 text-sm text-muted-foreground">
                        "The selected module is not available in the current catalog snapshot."
                    </p>
                }
            >
                {move || {
                    detail_for_body.clone().map(|module| {
                        let version_trail = module.versions.clone().into_iter().take(5).collect::<Vec<_>>();
                        let checksum = short_checksum(module.checksum_sha256.as_deref());
                        let primary_here = module
                            .recommended_admin_surfaces
                            .iter()
                            .any(|surface| surface == &admin_surface_for_body);
                        let showcase_here = module
                            .showcase_admin_surfaces
                            .iter()
                            .any(|surface| surface == &admin_surface_for_body);
                        view! {
                            <div class="mt-4 space-y-4">
                                <div class="space-y-2">
                                    <div class="flex flex-wrap items-center gap-2">
                                        <h4 class="text-lg font-semibold text-card-foreground">{module.name.clone()}</h4>
                                        <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-muted-foreground">
                                            {format!("v{}", module.latest_version)}
                                        </span>
                                        <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 text-xs font-semibold text-secondary-foreground">
                                            {humanize_token(&module.source)}
                                        </span>
                                        <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-muted-foreground">
                                            {humanize_token(&module.category)}
                                        </span>
                                        <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-muted-foreground">
                                            {if module.compatible { "Compatible" } else { "Compatibility risk" }}
                                        </span>
                                        {module.signature_present.then(|| view! {
                                            <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 text-xs font-semibold text-secondary-foreground">
                                                "Signed"
                                            </span>
                                        })}
                                        {module.installed.then(|| view! {
                                            <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 text-xs font-semibold text-secondary-foreground">
                                                {format!(
                                                    "Installed{}",
                                                    module
                                                        .installed_version
                                                        .as_ref()
                                                        .map(|value| format!(" v{}", value))
                                                        .unwrap_or_default()
                                                )}
                                            </span>
                                        })}
                                        {module.update_available.then(|| view! {
                                            <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-muted-foreground">
                                                "Update available"
                                            </span>
                                        })}
                                    </div>
                                    <p class="text-sm text-muted-foreground">{module.description.clone()}</p>
                                </div>

                                <div class="flex flex-wrap items-center gap-2 text-xs">
                                    <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 font-semibold text-secondary-foreground">
                                        {humanize_token(&module.ownership)}
                                    </span>
                                    <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                                        {humanize_token(&module.trust_level)}
                                    </span>
                                    <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                                        {if primary_here {
                                            "Primary for this admin"
                                        } else if showcase_here {
                                            "Showcase for this admin"
                                        } else {
                                            "No dedicated UI for this admin"
                                        }}
                                    </span>
                                </div>

                                <div class="grid gap-4 lg:grid-cols-2">
                                    <div class="rounded-lg border border-border bg-background/70 p-4 text-sm">
                                        <p class="text-xs uppercase tracking-wide text-muted-foreground">"Package metadata"</p>
                                        <dl class="mt-3 space-y-2">
                                            <div class="flex items-start justify-between gap-3">
                                                <dt class="text-muted-foreground">"Slug"</dt>
                                                <dd class="font-mono text-right">{module.slug.clone()}</dd>
                                            </div>
                                            <div class="flex items-start justify-between gap-3">
                                                <dt class="text-muted-foreground">"Crate"</dt>
                                                <dd class="font-mono text-right">{module.crate_name.clone()}</dd>
                                            </div>
                                            <div class="flex items-start justify-between gap-3">
                                                <dt class="text-muted-foreground">"Publisher"</dt>
                                                <dd class="text-right">{module.publisher.clone().unwrap_or_else(|| "Workspace / unknown".to_string())}</dd>
                                            </div>
                                            <div class="flex items-start justify-between gap-3">
                                                <dt class="text-muted-foreground">"RusTok range"</dt>
                                                <dd class="text-right">
                                                    {format!(
                                                        "{}{}",
                                                        module
                                                            .rustok_min_version
                                                            .as_ref()
                                                            .map(|value| format!(">= {}", value))
                                                            .unwrap_or_else(|| "no min".to_string()),
                                                        module
                                                            .rustok_max_version
                                                            .as_ref()
                                                            .map(|value| format!(", <= {}", value))
                                                            .unwrap_or_else(|| ", no max".to_string())
                                                    )}
                                                </dd>
                                            </div>
                                            <div class="flex items-start justify-between gap-3">
                                                <dt class="text-muted-foreground">"Checksum"</dt>
                                                <dd class="font-mono text-right">{checksum.unwrap_or_else(|| "Not published".to_string())}</dd>
                                            </div>
                                        </dl>
                                    </div>

                                    <div class="rounded-lg border border-border bg-background/70 p-4 text-sm">
                                        <p class="text-xs uppercase tracking-wide text-muted-foreground">"Surface policy"</p>
                                        <div class="mt-3 space-y-3">
                                            <div class="flex flex-wrap gap-2">
                                                {if module.recommended_admin_surfaces.is_empty() {
                                                    view! {
                                                        <span class="text-xs text-muted-foreground">
                                                            "No primary admin surface declared."
                                                        </span>
                                                    }
                                                        .into_any()
                                                } else {
                                                    module
                                                        .recommended_admin_surfaces
                                                        .clone()
                                                        .into_iter()
                                                        .map(|surface| {
                                                            view! {
                                                                <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-muted-foreground">
                                                                    {format!("Primary: {}", humanize_token(&surface))}
                                                                </span>
                                                            }
                                                        })
                                                        .collect_view()
                                                        .into_any()
                                                }}
                                            </div>
                                            <div class="flex flex-wrap gap-2">
                                                {if module.showcase_admin_surfaces.is_empty() {
                                                    view! {
                                                        <span class="text-xs text-muted-foreground">
                                                            "No showcase admin surface declared."
                                                        </span>
                                                    }
                                                        .into_any()
                                                } else {
                                                    module
                                                        .showcase_admin_surfaces
                                                        .clone()
                                                        .into_iter()
                                                        .map(|surface| {
                                                            view! {
                                                                <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-muted-foreground">
                                                                    {format!("Showcase: {}", humanize_token(&surface))}
                                                                </span>
                                                            }
                                                        })
                                                        .collect_view()
                                                        .into_any()
                                                }}
                                            </div>
                                            <div class="text-xs text-muted-foreground">
                                                {if module.dependencies.is_empty() {
                                                    "No module dependencies declared.".to_string()
                                                } else {
                                                    format!("Depends on: {}", module.dependencies.join(", "))
                                                }}
                                            </div>
                                        </div>
                                    </div>
                                </div>

                                <div class="rounded-lg border border-border bg-background/70 p-4">
                                    <div class="flex items-center gap-2">
                                        <p class="text-xs uppercase tracking-wide text-muted-foreground">"Version history"</p>
                                        <Show when=move || loading.get()>
                                            <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-muted-foreground">
                                                "Refreshing"
                                            </span>
                                        </Show>
                                    </div>
                                    {if version_trail.is_empty() {
                                        view! {
                                            <p class="mt-3 text-sm text-muted-foreground">
                                                "No version history has been published for this module yet."
                                            </p>
                                        }
                                            .into_any()
                                    } else {
                                        view! {
                                            <div class="mt-3 space-y-3">
                                                {version_trail.into_iter().map(|version| {
                                                    let checksum = short_checksum(version.checksum_sha256.as_deref());
                                                    view! {
                                                        <div class="flex flex-col gap-2 rounded-lg border border-border px-3 py-3 text-sm">
                                                            <div class="flex flex-wrap items-center gap-2">
                                                                <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-muted-foreground">
                                                                    {format!("v{}", version.version)}
                                                                </span>
                                                                {version.yanked.then(|| view! {
                                                                    <span class="inline-flex items-center rounded-full bg-destructive px-2.5 py-0.5 text-xs font-semibold text-destructive-foreground">
                                                                        "Yanked"
                                                                    </span>
                                                                })}
                                                                {version.signature_present.then(|| view! {
                                                                    <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 text-xs font-semibold text-secondary-foreground">
                                                                        "Signed"
                                                                    </span>
                                                                })}
                                                                <span class="text-xs text-muted-foreground">
                                                                    {version.published_at.unwrap_or_else(|| "Unknown".to_string())}
                                                                </span>
                                                            </div>
                                                            {version.changelog.map(|changelog| view! {
                                                                <p class="text-sm text-muted-foreground">{changelog}</p>
                                                            })}
                                                            {checksum.map(|checksum| view! {
                                                                <div class="text-xs text-muted-foreground">
                                                                    <span class="font-mono">{format!("sha256 {}", checksum)}</span>
                                                                </div>
                                                            })}
                                                        </div>
                                                    }
                                                }).collect_view()}
                                            </div>
                                        }
                                            .into_any()
                                    }}
                                </div>
                            </div>
                        }
                    })
                }}
            </Show>
        </div>
    }
}
