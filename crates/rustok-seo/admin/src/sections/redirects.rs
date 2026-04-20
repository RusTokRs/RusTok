use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use rustok_seo::SeoRedirectRecord;

use crate::api::ApiError;
use crate::i18n::t;
use crate::model::SeoRedirectForm;

#[component]
pub fn SeoRedirectsPane(
    ui_locale: Option<String>,
    redirect_form: RwSignal<SeoRedirectForm>,
    redirects: Resource<Result<Vec<SeoRedirectRecord>, ApiError>>,
    busy_key: RwSignal<Option<String>>,
    on_save: Callback<SubmitEvent>,
) -> impl IntoView {
    let busy = Signal::derive(move || busy_key.get().is_some());
    let title = t(ui_locale.as_deref(), "seo.redirects.title", "Redirects");
    let subtitle = t(
        ui_locale.as_deref(),
        "seo.redirects.subtitle",
        "Manual redirect rules stay tenant-scoped and run before canonical resolution. Content-specific metadata now belongs to owner-module editors.",
    );

    view! {
        <div class="grid gap-6 xl:grid-cols-[minmax(0,0.9fr)_minmax(0,1.1fr)]">
            <form class="space-y-4 rounded-2xl border border-border bg-card p-6 shadow-sm" on:submit=move |ev| on_save.run(ev)>
                <div class="space-y-2">
                    <h2 class="text-lg font-semibold text-card-foreground">{title}</h2>
                    <p class="text-sm text-muted-foreground">{subtitle}</p>
                </div>

                <label class="grid gap-2 text-sm">
                    <span>"Match type"</span>
                    <select
                        class="rounded-lg border border-input bg-background px-3 py-2"
                        prop:value=move || redirect_form.get().match_type_value().to_string()
                        on:change=move |ev| redirect_form.update(|draft| draft.set_match_type_from_str(event_target_value(&ev).as_str()))
                    >
                        <option value="exact">"exact"</option>
                        <option value="wildcard">"wildcard"</option>
                    </select>
                </label>
                <label class="grid gap-2 text-sm">
                    <span>"Source pattern"</span>
                    <input
                        class="rounded-lg border border-input bg-background px-3 py-2"
                        prop:value=move || redirect_form.get().source_pattern.clone()
                        on:input=move |ev| redirect_form.update(|draft| draft.source_pattern = event_target_value(&ev))
                    />
                </label>
                <label class="grid gap-2 text-sm">
                    <span>"Target URL"</span>
                    <input
                        class="rounded-lg border border-input bg-background px-3 py-2"
                        prop:value=move || redirect_form.get().target_url.clone()
                        on:input=move |ev| redirect_form.update(|draft| draft.target_url = event_target_value(&ev))
                    />
                </label>
                <label class="grid gap-2 text-sm">
                    <span>"Status code"</span>
                    <input
                        class="rounded-lg border border-input bg-background px-3 py-2"
                        prop:value=move || redirect_form.get().status_code.clone()
                        on:input=move |ev| redirect_form.update(|draft| draft.status_code = event_target_value(&ev))
                    />
                </label>

                <div class="flex justify-end">
                    <button
                        type="submit"
                        class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground disabled:opacity-60"
                        disabled=move || busy.get()
                    >
                        "Save redirect"
                    </button>
                </div>
            </form>

            <section class="space-y-4 rounded-2xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-2">
                    <h2 class="text-lg font-semibold text-card-foreground">"Redirect registry"</h2>
                    <p class="text-sm text-muted-foreground">
                        "Existing rules are listed with status code, match mode, and destination to keep redirect policy reviewable."
                    </p>
                </div>

                <Suspense fallback=move || view! { <p class="text-sm text-muted-foreground">"Loading redirects..."</p> }>
                    {move || match redirects.get() {
                        Some(Ok(items)) if !items.is_empty() => view! {
                            <ul class="space-y-3 text-sm">
                                {items.into_iter().map(render_redirect_item).collect_view()}
                            </ul>
                        }.into_any(),
                        Some(Ok(_)) => view! { <p class="text-sm text-muted-foreground">"No redirects yet."</p> }.into_any(),
                        Some(Err(err)) => view! { <p class="text-sm text-destructive">{err.to_string()}</p> }.into_any(),
                        None => view! { <p class="text-sm text-muted-foreground">"No redirects yet."</p> }.into_any(),
                    }}
                </Suspense>
            </section>
        </div>
    }
}

fn render_redirect_item(item: SeoRedirectRecord) -> impl IntoView {
    let expires_at = item
        .expires_at
        .map(|value| value.to_rfc3339())
        .unwrap_or_else(|| "never".to_string());
    let state = if item.is_active { "active" } else { "inactive" };

    view! {
        <li class="rounded-xl border border-border/80 bg-background/60 px-4 py-3">
            <div class="font-medium text-foreground">
                {format!("{} {} {}", item.status_code, item.match_type.as_str(), item.source_pattern)}
            </div>
            <div class="mt-1 break-all text-sm text-muted-foreground">{item.target_url}</div>
            <div class="mt-2 text-xs uppercase tracking-wide text-muted-foreground">
                {format!("{state} · expires {expires_at}")}
            </div>
        </li>
    }
}
