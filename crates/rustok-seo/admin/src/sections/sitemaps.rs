use leptos::prelude::*;
use rustok_seo::SeoSitemapStatusRecord;

use crate::api::ApiError;
use crate::i18n::t;

#[component]
pub fn SeoSitemapsPane(
    ui_locale: Option<String>,
    sitemap_status: Resource<Result<SeoSitemapStatusRecord, ApiError>>,
    busy_key: RwSignal<Option<String>>,
    on_generate: Callback<()>,
) -> impl IntoView {
    let busy = Signal::derive(move || busy_key.get().is_some());
    let title = t(ui_locale.as_deref(), "seo.sitemaps.title", "Sitemaps");
    let subtitle = t(
        ui_locale.as_deref(),
        "seo.sitemaps.subtitle",
        "Latest sitemap job status, file inventory, and manual regeneration live in one operator-focused panel. Entity SEO authoring is intentionally out of scope here.",
    );

    view! {
        <section class="space-y-4 rounded-2xl border border-border bg-card p-6 shadow-sm">
            <div class="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
                <div class="space-y-2">
                    <h2 class="text-lg font-semibold text-card-foreground">{title}</h2>
                    <p class="max-w-3xl text-sm text-muted-foreground">{subtitle}</p>
                </div>
                <button
                    type="button"
                    class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground disabled:opacity-60"
                    disabled=move || {
                        busy.get()
                            || matches!(
                                sitemap_status.get(),
                                Some(Ok(SeoSitemapStatusRecord { enabled: false, .. }))
                            )
                    }
                    on:click=move |_| on_generate.run(())
                >
                    "Generate"
                </button>
            </div>

            <Suspense fallback=move || view! { <p class="text-sm text-muted-foreground">"Loading sitemap status..."</p> }>
                {move || match sitemap_status.get() {
                    Some(Ok(status)) => {
                        let enabled = status.enabled;
                        let generated_at = status
                            .generated_at
                            .map(|value| value.to_rfc3339())
                            .unwrap_or_else(|| "n/a".to_string());
                        let latest_job = status
                            .latest_job_id
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "n/a".to_string());
                        let status_label = status.status.clone().unwrap_or_else(|| "n/a".to_string());
                        let file_view = if status.files.is_empty() {
                            view! {
                                <p class="text-sm text-muted-foreground">
                                    "No generated sitemap files yet."
                                </p>
                            }
                                .into_any()
                        } else {
                            let items = status
                                .files
                                .into_iter()
                                .map(|file| view! {
                                    <li class="rounded-xl border border-border/80 bg-background/60 px-4 py-3">
                                        <div class="font-medium text-foreground">{file.path}</div>
                                        <div class="mt-1 text-sm text-muted-foreground">
                                            {format!("URLs: {} · created at {}", file.url_count, file.created_at.to_rfc3339())}
                                        </div>
                                    </li>
                                })
                                .collect_view();
                            view! { <ul class="space-y-3">{items}</ul> }.into_any()
                        };

                        view! {
                            <div class="space-y-4">
                                <Show when=move || !enabled>
                                    <div class="rounded-xl border border-amber-300/70 bg-amber-50 px-4 py-3 text-sm text-amber-900">
                                        "Sitemap generation is disabled in SEO defaults. Existing status stays visible, but regeneration is intentionally blocked until the setting is re-enabled."
                                    </div>
                                </Show>

                                <div class="grid gap-6 lg:grid-cols-[minmax(0,0.8fr)_minmax(0,1.2fr)]">
                                    <dl class="grid gap-3 text-sm">
                                        <div class="rounded-xl border border-border/80 bg-background/60 px-4 py-3">
                                            <dt class="text-xs uppercase tracking-wide text-muted-foreground">"Enabled"</dt>
                                            <dd class="mt-1 font-medium text-foreground">{status.enabled.to_string()}</dd>
                                        </div>
                                        <div class="rounded-xl border border-border/80 bg-background/60 px-4 py-3">
                                            <dt class="text-xs uppercase tracking-wide text-muted-foreground">"Status"</dt>
                                            <dd class="mt-1 font-medium text-foreground">{status_label}</dd>
                                        </div>
                                        <div class="rounded-xl border border-border/80 bg-background/60 px-4 py-3">
                                            <dt class="text-xs uppercase tracking-wide text-muted-foreground">"Files"</dt>
                                            <dd class="mt-1 font-medium text-foreground">{status.file_count.to_string()}</dd>
                                        </div>
                                        <div class="rounded-xl border border-border/80 bg-background/60 px-4 py-3">
                                            <dt class="text-xs uppercase tracking-wide text-muted-foreground">"Latest job"</dt>
                                            <dd class="mt-1 break-all font-medium text-foreground">{latest_job}</dd>
                                        </div>
                                        <div class="rounded-xl border border-border/80 bg-background/60 px-4 py-3">
                                            <dt class="text-xs uppercase tracking-wide text-muted-foreground">"Generated at"</dt>
                                            <dd class="mt-1 font-medium text-foreground">{generated_at}</dd>
                                        </div>
                                    </dl>

                                    <div class="space-y-3">
                                        <h3 class="text-base font-semibold text-card-foreground">"Generated files"</h3>
                                        {file_view}
                                    </div>
                                </div>
                            </div>
                        }.into_any()
                    }
                    Some(Err(err)) => view! { <p class="text-sm text-destructive">{err.to_string()}</p> }.into_any(),
                    None => view! { <p class="text-sm text-muted-foreground">"No sitemap status available."</p> }.into_any(),
                }}
            </Suspense>
        </section>
    }
}
