use leptos::prelude::*;
use rustok_seo::SeoRobotsPreviewRecord;

use crate::api::ApiError;
use crate::i18n::t;

#[component]
pub fn SeoRobotsPane(
    ui_locale: Option<String>,
    robots_preview: Resource<Result<SeoRobotsPreviewRecord, ApiError>>,
) -> impl IntoView {
    let title = t(ui_locale.as_deref(), "seo.robots.title", "Robots");
    let subtitle = t(
        ui_locale.as_deref(),
        "seo.robots.subtitle",
        "This pane previews the tenant-level robots.txt response and linked public URLs. Entity-level meta robots remain part of owner-module SEO panels.",
    );

    view! {
        <section class="space-y-4 rounded-2xl border border-border bg-card p-6 shadow-sm">
            <div class="space-y-2">
                <h2 class="text-lg font-semibold text-card-foreground">{title}</h2>
                <p class="max-w-3xl text-sm text-muted-foreground">{subtitle}</p>
            </div>

            <Suspense fallback=move || view! { <p class="text-sm text-muted-foreground">"Loading robots preview..."</p> }>
                {move || match robots_preview.get() {
                    Some(Ok(preview)) => view! {
                        <div class="grid gap-6 lg:grid-cols-[minmax(0,0.8fr)_minmax(0,1.2fr)]">
                            <dl class="grid gap-3 text-sm">
                                <div class="rounded-xl border border-border/80 bg-background/60 px-4 py-3">
                                    <dt class="text-xs uppercase tracking-wide text-muted-foreground">"Public robots URL"</dt>
                                    <dd class="mt-1 break-all font-medium text-foreground">{preview.public_url.clone()}</dd>
                                </div>
                                <div class="rounded-xl border border-border/80 bg-background/60 px-4 py-3">
                                    <dt class="text-xs uppercase tracking-wide text-muted-foreground">"Sitemap index"</dt>
                                    <dd class="mt-1 break-all font-medium text-foreground">
                                        {preview.sitemap_index_url.clone().unwrap_or_else(|| "disabled".to_string())}
                                    </dd>
                                </div>
                            </dl>

                            <div class="space-y-3">
                                <h3 class="text-base font-semibold text-card-foreground">"robots.txt preview"</h3>
                                <pre class="overflow-x-auto rounded-xl border border-border/80 bg-background/60 px-4 py-3 text-sm text-foreground whitespace-pre-wrap">{preview.body}</pre>
                            </div>
                        </div>
                    }.into_any(),
                    Some(Err(err)) => view! { <p class="text-sm text-destructive">{err.to_string()}</p> }.into_any(),
                    None => view! { <p class="text-sm text-muted-foreground">"No robots preview available."</p> }.into_any(),
                }}
            </Suspense>
        </section>
    }
}
