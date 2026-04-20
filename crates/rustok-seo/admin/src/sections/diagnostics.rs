use leptos::prelude::*;
use rustok_seo::{
    SeoModuleSettings, SeoRedirectRecord, SeoRobotsPreviewRecord, SeoSitemapStatusRecord,
};

use crate::api::ApiError;
use crate::i18n::t;

#[component]
pub fn SeoDiagnosticsPane(
    ui_locale: Option<String>,
    settings: Resource<Result<SeoModuleSettings, ApiError>>,
    redirects: Resource<Result<Vec<SeoRedirectRecord>, ApiError>>,
    sitemap_status: Resource<Result<SeoSitemapStatusRecord, ApiError>>,
    robots_preview: Resource<Result<SeoRobotsPreviewRecord, ApiError>>,
) -> impl IntoView {
    let title = t(ui_locale.as_deref(), "seo.diagnostics.title", "Diagnostics");
    let subtitle = t(
        ui_locale.as_deref(),
        "seo.diagnostics.subtitle",
        "Infrastructure-only summary of current SEO runtime inputs. Per-entity scores and analysis stay in owner-module editors.",
    );

    view! {
        <section class="space-y-4 rounded-2xl border border-border bg-card p-6 shadow-sm">
            <div class="space-y-2">
                <h2 class="text-lg font-semibold text-card-foreground">{title}</h2>
                <p class="max-w-3xl text-sm text-muted-foreground">{subtitle}</p>
            </div>

            <div class="grid gap-6 xl:grid-cols-2">
                <DiagnosticsSettingsCard settings=settings />
                <DiagnosticsRedirectsCard redirects=redirects />
                <DiagnosticsSitemapCard sitemap_status=sitemap_status />
                <DiagnosticsRobotsCard robots_preview=robots_preview />
            </div>
        </section>
    }
}

#[component]
fn DiagnosticsSettingsCard(
    settings: Resource<Result<SeoModuleSettings, ApiError>>,
) -> impl IntoView {
    view! {
        <div class="space-y-3 rounded-xl border border-border/80 bg-background/60 p-4">
            <h3 class="text-base font-semibold text-card-foreground">"Defaults snapshot"</h3>
            <Suspense fallback=move || view! { <p class="text-sm text-muted-foreground">"Loading settings..."</p> }>
                {move || match settings.get() {
                    Some(Ok(settings)) => view! {
                        <ul class="space-y-2 text-sm text-foreground">
                            <li>{format!("Default robots: {}", if settings.default_robots.is_empty() { "n/a".to_string() } else { settings.default_robots.join(", ") })}</li>
                            <li>{format!("Sitemap enabled: {}", settings.sitemap_enabled)}</li>
                            <li>{format!("x-default locale: {}", settings.x_default_locale.unwrap_or_else(|| "unset".to_string()))}</li>
                            <li>{format!("Redirect host allowlist: {}", if settings.allowed_redirect_hosts.is_empty() { "none".to_string() } else { settings.allowed_redirect_hosts.join(", ") })}</li>
                            <li>{format!("Canonical host allowlist: {}", if settings.allowed_canonical_hosts.is_empty() { "none".to_string() } else { settings.allowed_canonical_hosts.join(", ") })}</li>
                        </ul>
                    }.into_any(),
                    Some(Err(err)) => view! { <p class="text-sm text-destructive">{err.to_string()}</p> }.into_any(),
                    None => view! { <p class="text-sm text-muted-foreground">"No settings snapshot available."</p> }.into_any(),
                }}
            </Suspense>
        </div>
    }
}

#[component]
fn DiagnosticsRedirectsCard(
    redirects: Resource<Result<Vec<SeoRedirectRecord>, ApiError>>,
) -> impl IntoView {
    view! {
        <div class="space-y-3 rounded-xl border border-border/80 bg-background/60 p-4">
            <h3 class="text-base font-semibold text-card-foreground">"Redirect summary"</h3>
            <Suspense fallback=move || view! { <p class="text-sm text-muted-foreground">"Loading redirects..."</p> }>
                {move || match redirects.get() {
                    Some(Ok(items)) => {
                        let total = items.len();
                        let active = items.iter().filter(|item| item.is_active).count();
                        let inactive = total.saturating_sub(active);
                        view! {
                            <ul class="space-y-2 text-sm text-foreground">
                                <li>{format!("Total rules: {total}")}</li>
                                <li>{format!("Active rules: {active}")}</li>
                                <li>{format!("Inactive rules: {inactive}")}</li>
                            </ul>
                        }.into_any()
                    }
                    Some(Err(err)) => view! { <p class="text-sm text-destructive">{err.to_string()}</p> }.into_any(),
                    None => view! { <p class="text-sm text-muted-foreground">"No redirect summary available."</p> }.into_any(),
                }}
            </Suspense>
        </div>
    }
}

#[component]
fn DiagnosticsSitemapCard(
    sitemap_status: Resource<Result<SeoSitemapStatusRecord, ApiError>>,
) -> impl IntoView {
    view! {
        <div class="space-y-3 rounded-xl border border-border/80 bg-background/60 p-4">
            <h3 class="text-base font-semibold text-card-foreground">"Sitemap summary"</h3>
            <Suspense fallback=move || view! { <p class="text-sm text-muted-foreground">"Loading sitemap status..."</p> }>
                {move || match sitemap_status.get() {
                    Some(Ok(status)) => view! {
                        <ul class="space-y-2 text-sm text-foreground">
                            <li>{format!("Enabled: {}", status.enabled)}</li>
                            <li>{format!("Status: {}", status.status.unwrap_or_else(|| "n/a".to_string()))}</li>
                            <li>{format!("Files: {}", status.file_count)}</li>
                            <li>{format!("Generated at: {}", status.generated_at.map(|value| value.to_rfc3339()).unwrap_or_else(|| "n/a".to_string()))}</li>
                        </ul>
                    }.into_any(),
                    Some(Err(err)) => view! { <p class="text-sm text-destructive">{err.to_string()}</p> }.into_any(),
                    None => view! { <p class="text-sm text-muted-foreground">"No sitemap summary available."</p> }.into_any(),
                }}
            </Suspense>
        </div>
    }
}

#[component]
fn DiagnosticsRobotsCard(
    robots_preview: Resource<Result<SeoRobotsPreviewRecord, ApiError>>,
) -> impl IntoView {
    view! {
        <div class="space-y-3 rounded-xl border border-border/80 bg-background/60 p-4">
            <h3 class="text-base font-semibold text-card-foreground">"Robots summary"</h3>
            <Suspense fallback=move || view! { <p class="text-sm text-muted-foreground">"Loading robots preview..."</p> }>
                {move || match robots_preview.get() {
                    Some(Ok(preview)) => view! {
                        <ul class="space-y-2 text-sm text-foreground">
                            <li>{format!("robots.txt URL: {}", preview.public_url)}</li>
                            <li>{format!("Sitemap index: {}", preview.sitemap_index_url.unwrap_or_else(|| "disabled".to_string()))}</li>
                            <li>{format!("Preview lines: {}", preview.body.lines().count())}</li>
                        </ul>
                    }.into_any(),
                    Some(Err(err)) => view! { <p class="text-sm text-destructive">{err.to_string()}</p> }.into_any(),
                    None => view! { <p class="text-sm text-muted-foreground">"No robots summary available."</p> }.into_any(),
                }}
            </Suspense>
        </div>
    }
}
