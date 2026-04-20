use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use rustok_seo::SeoModuleSettings;

use crate::api::ApiError;
use crate::i18n::t;
use crate::model::{SeoSettingsForm, ROBOT_DIRECTIVE_PRESETS};

#[component]
pub fn SeoDefaultsPane(
    ui_locale: Option<String>,
    settings_form: RwSignal<SeoSettingsForm>,
    settings: Resource<Result<SeoModuleSettings, ApiError>>,
    busy_key: RwSignal<Option<String>>,
    on_save: Callback<SubmitEvent>,
) -> impl IntoView {
    let busy = Signal::derive(move || busy_key.get().is_some());
    let title = t(ui_locale.as_deref(), "seo.defaults.title", "Defaults");
    let subtitle = t(
        ui_locale.as_deref(),
        "seo.defaults.subtitle",
        "Tenant-scoped SEO defaults are persisted through the shared module settings contract. This pane does not own page, product, blog, or forum metadata editing.",
    );

    view! {
        <div class="grid gap-6 xl:grid-cols-[minmax(0,1.15fr)_minmax(0,0.85fr)]">
            <form class="space-y-5 rounded-2xl border border-border bg-card p-6 shadow-sm" on:submit=move |ev| on_save.run(ev)>
                <div class="space-y-2">
                    <h2 class="text-lg font-semibold text-card-foreground">{title}</h2>
                    <p class="text-sm text-muted-foreground">{subtitle}</p>
                </div>

                <label class="flex items-center justify-between gap-4 rounded-xl border border-border/80 bg-background/60 px-4 py-3">
                    <div class="space-y-1">
                        <div class="text-sm font-medium text-foreground">"Sitemap generation"</div>
                        <p class="text-sm text-muted-foreground">
                            "Disabling this turns `robots.txt` into a sitemap-free response and blocks manual generation from the control plane."
                        </p>
                    </div>
                    <input
                        type="checkbox"
                        class="h-5 w-5 accent-primary"
                        prop:checked=move || settings_form.get().sitemap_enabled
                        on:change=move |ev| settings_form.update(|draft| draft.sitemap_enabled = event_target_checked(&ev))
                    />
                </label>

                <div class="space-y-3 rounded-xl border border-border/80 bg-background/60 px-4 py-4">
                    <div class="space-y-1">
                        <h3 class="text-sm font-semibold text-foreground">"Default robots directives"</h3>
                        <p class="text-sm text-muted-foreground">
                            "Directives are stored as a normalized token list. Preset chips are shortcuts only; arbitrary directives are still allowed."
                        </p>
                    </div>

                    <div class="flex flex-wrap gap-2">
                        {ROBOT_DIRECTIVE_PRESETS.iter().map(|directive| {
                            let token = (*directive).to_string();
                            view! {
                                <button
                                    type="button"
                                    class="rounded-full border border-border px-3 py-1 text-xs font-medium text-foreground transition hover:bg-accent"
                                    on:click=move |_| settings_form.update(|draft| draft.add_robot_directive(token.clone()))
                                >
                                    {directive.to_string()}
                                </button>
                            }
                        }).collect_view()}
                    </div>

                    <div class="flex flex-wrap gap-2">
                        {move || {
                            settings_form
                                .get()
                                .default_robots
                                .into_iter()
                                .map(|directive| {
                                    let remove_directive = directive.clone();
                                    view! {
                                        <button
                                            type="button"
                                            class="rounded-full bg-secondary px-3 py-1 text-xs font-medium text-secondary-foreground transition hover:opacity-90"
                                            on:click=move |_| settings_form.update(|draft| draft.remove_robot_directive(remove_directive.as_str()))
                                        >
                                            {format!("{directive} ×")}
                                        </button>
                                    }
                                })
                                .collect_view()
                        }}
                    </div>

                    <div class="flex gap-3">
                        <input
                            class="min-w-0 flex-1 rounded-lg border border-input bg-background px-3 py-2 text-sm"
                            placeholder="custom directive"
                            prop:value=move || settings_form.get().robot_directive_input.clone()
                            on:input=move |ev| settings_form.update(|draft| draft.robot_directive_input = event_target_value(&ev))
                        />
                        <button
                            type="button"
                            class="rounded-lg border border-border px-4 py-2 text-sm font-medium text-foreground transition hover:bg-accent"
                            on:click=move |_| settings_form.update(|draft| {
                                let input = draft.robot_directive_input.clone();
                                draft.add_robot_directive(input);
                            })
                        >
                            "Add"
                        </button>
                    </div>
                </div>

                <label class="grid gap-2 text-sm">
                    <span class="font-medium text-foreground">"Allowed redirect hosts"</span>
                    <textarea
                        class="min-h-32 rounded-lg border border-input bg-background px-3 py-2"
                        prop:value=move || settings_form.get().allowed_redirect_hosts_text.clone()
                        on:input=move |ev| settings_form.update(|draft| draft.allowed_redirect_hosts_text = event_target_value(&ev))
                    />
                    <span class="text-xs text-muted-foreground">"One host per line. Empty lines are ignored."</span>
                </label>

                <label class="grid gap-2 text-sm">
                    <span class="font-medium text-foreground">"Allowed canonical hosts"</span>
                    <textarea
                        class="min-h-32 rounded-lg border border-input bg-background px-3 py-2"
                        prop:value=move || settings_form.get().allowed_canonical_hosts_text.clone()
                        on:input=move |ev| settings_form.update(|draft| draft.allowed_canonical_hosts_text = event_target_value(&ev))
                    />
                    <span class="text-xs text-muted-foreground">"One host per line. Empty lines are ignored."</span>
                </label>

                <label class="grid gap-2 text-sm">
                    <span class="font-medium text-foreground">"`x-default` locale"</span>
                    <input
                        class="rounded-lg border border-input bg-background px-3 py-2"
                        prop:value=move || settings_form.get().x_default_locale.clone()
                        on:input=move |ev| settings_form.update(|draft| draft.x_default_locale = event_target_value(&ev))
                    />
                    <span class="text-xs text-muted-foreground">"Leave empty to unset the tenant-level x-default hreflang locale."</span>
                </label>

                <div class="flex justify-end">
                    <button
                        type="submit"
                        class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground disabled:opacity-60"
                        disabled=move || busy.get()
                    >
                        "Save defaults"
                    </button>
                </div>
            </form>

            <section class="space-y-4 rounded-2xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-2">
                    <h2 class="text-lg font-semibold text-card-foreground">"Effective settings"</h2>
                    <p class="text-sm text-muted-foreground">
                        "The read-side below shows the normalized settings snapshot currently consumed by the SEO runtime."
                    </p>
                </div>

                <Suspense fallback=move || view! { <p class="text-sm text-muted-foreground">"Loading settings..."</p> }>
                    {move || match settings.get() {
                        Some(Ok(settings)) => view! {
                            <dl class="grid gap-3 text-sm">
                                <SettingsValueCard label="Default robots".to_string() value=if settings.default_robots.is_empty() {
                                    "n/a".to_string()
                                } else {
                                    settings.default_robots.join(", ")
                                } />
                                <SettingsValueCard label="Sitemap enabled".to_string() value=settings.sitemap_enabled.to_string() />
                                <SettingsValueCard label="Allowed redirect hosts".to_string() value=if settings.allowed_redirect_hosts.is_empty() {
                                    "none".to_string()
                                } else {
                                    settings.allowed_redirect_hosts.join(", ")
                                } />
                                <SettingsValueCard label="Allowed canonical hosts".to_string() value=if settings.allowed_canonical_hosts.is_empty() {
                                    "none".to_string()
                                } else {
                                    settings.allowed_canonical_hosts.join(", ")
                                } />
                                <SettingsValueCard label="x-default locale".to_string() value=settings.x_default_locale.unwrap_or_else(|| "unset".to_string()) />
                            </dl>
                        }.into_any(),
                        Some(Err(err)) => view! { <p class="text-sm text-destructive">{err.to_string()}</p> }.into_any(),
                        None => view! { <p class="text-sm text-muted-foreground">"No settings available."</p> }.into_any(),
                    }}
                </Suspense>
            </section>
        </div>
    }
}

#[component]
fn SettingsValueCard(label: String, value: String) -> impl IntoView {
    view! {
        <div class="rounded-xl border border-border/80 bg-background/60 px-4 py-3">
            <dt class="text-xs uppercase tracking-wide text-muted-foreground">{label}</dt>
            <dd class="mt-1 break-all font-medium text-foreground">{value}</dd>
        </div>
    }
}
