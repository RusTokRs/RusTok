use leptos::prelude::*;

use crate::entities::module::ModuleInfo;
use crate::shared::i18n::translate;

#[component]
pub fn ModuleCard(
    module: ModuleInfo,
    #[prop(into)] loading: Signal<bool>,
    #[prop(optional)] on_toggle: Option<Callback<(String, bool)>>,
) -> impl IntoView {
    let is_core = module.is_core();
    let enabled = module.enabled;
    let slug = module.module_slug.clone();
    let slug_for_toggle = slug.clone();

    let opacity_class = if !enabled && !is_core {
        "opacity-60"
    } else {
        ""
    };

    view! {
        <div class=format!(
            "rounded-xl border border-border bg-card text-card-foreground shadow-sm transition-all hover:shadow-md {}",
            opacity_class
        )>
            <div class="p-6 pb-3">
                <div class="flex items-start justify-between">
                    <div class="flex items-center gap-2">
                        {if enabled {
                            view! {
                                <svg class="h-5 w-5 shrink-0 text-primary" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                    <path d="M16.88 3.549a8.94 8.94 0 0 1 2.18 2.661c.497.951.79 2.007.844 3.09.054 1.078-.1 2.168-.452 3.192a9.01 9.01 0 0 1-1.678 2.856M7.12 3.549a8.94 8.94 0 0 0-2.18 2.661A9.016 9.016 0 0 0 4.096 9.3c-.054 1.078.1 2.168.452 3.192a9.01 9.01 0 0 0 1.678 2.856M12 2v2M12 18v4M8 22h8" />
                                </svg>
                            }.into_any()
                        } else {
                            view! {
                                <svg class="h-5 w-5 shrink-0 text-muted-foreground" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                    <path d="M2 2l20 20M16.88 3.549a8.94 8.94 0 0 1 2.18 2.661c.497.951.79 2.007.844 3.09M7.12 3.549a8.94 8.94 0 0 0-2.18 2.661A9.016 9.016 0 0 0 4.096 9.3M12 2v2M12 18v4M8 22h8" />
                                </svg>
                            }.into_any()
                        }}
                        <h3 class="text-base font-semibold text-card-foreground">{module.name.clone()}</h3>
                    </div>
                    <div class="flex items-center gap-2">
                        {is_core.then(|| view! {
                            <span class="inline-flex items-center rounded-full bg-primary px-2.5 py-0.5 text-xs font-semibold text-primary-foreground">
                                {translate("modules.badge.core")}
                            </span>
                        })}
                        <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-muted-foreground">
                            {format!("v{}", module.version)}
                        </span>
                    </div>
                </div>
                <p class="mt-2 text-sm text-muted-foreground">{module.description.clone()}</p>
            </div>
            <div class="px-6 pb-6">
                <div class="flex items-center justify-between">
                    <div class="text-xs text-muted-foreground">
                        {(!module.dependencies.is_empty()).then(|| {
                            let deps = module.dependencies.join(", ");
                            view! {
                                <span>{format!("{}: {}", translate("modules.depends_on"), deps)}</span>
                            }
                        })}
                    </div>
                    {if is_core {
                        view! {
                            <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 text-xs font-semibold text-secondary-foreground">
                                {translate("modules.always_on")}
                            </span>
                        }.into_any()
                    } else {
                        let is_loading = loading;
                        view! {
                            <div class="flex items-center gap-2">
                                <span class="text-xs text-muted-foreground">
                                    {if enabled {
                                        translate("modules.enabled")
                                    } else {
                                        translate("modules.disabled")
                                    }}
                                </span>
                                <button
                                    type="button"
                                    class=move || {
                                        let base = "relative inline-flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-full border-2 border-transparent transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background disabled:cursor-not-allowed disabled:opacity-50";
                                        if enabled {
                                            format!("{} bg-primary", base)
                                        } else {
                                            format!("{} bg-input", base)
                                        }
                                    }
                                    disabled=move || is_loading.get()
                                    on:click=move |_| {
                                        if let Some(cb) = on_toggle {
                                            cb.run((slug_for_toggle.clone(), !enabled));
                                        }
                                    }
                                >
                                    <span
                                        class=move || {
                                            let base = "pointer-events-none block h-4 w-4 rounded-full bg-background shadow-lg ring-0 transition-transform";
                                            if enabled {
                                                format!("{} translate-x-4", base)
                                            } else {
                                                format!("{} translate-x-0", base)
                                            }
                                        }
                                    />
                                </button>
                            </div>
                        }.into_any()
                    }}
                </div>
            </div>
        </div>
    }
}
