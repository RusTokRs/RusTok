use leptos::prelude::*;

use crate::i18n::{recommendation, tr};
use crate::model::{SeoCompletenessReport, SeoEntityForm};

#[component]
pub fn SeoSummaryTile(
    label: Signal<String>,
    value: Signal<String>,
    detail: Signal<String>,
) -> impl IntoView {
    view! {
        <article class="rounded-2xl border border-border bg-background/70 p-4">
            <p class="text-[11px] font-semibold uppercase tracking-[0.22em] text-muted-foreground">
                {move || label.get()}
            </p>
            <p class="mt-2 text-sm font-medium text-card-foreground">{move || value.get()}</p>
            <p class="mt-2 text-xs leading-5 text-muted-foreground">{move || detail.get()}</p>
        </article>
    }
}

#[component]
pub fn SeoSnippetPreviewCard(
    form: RwSignal<SeoEntityForm>,
    locale: Signal<String>,
) -> impl IntoView {
    view! {
        <article class="rounded-2xl border border-border bg-background/70 p-4 md:col-span-2 xl:col-span-2">
            <div class="space-y-1">
                <h4 class="text-sm font-semibold text-card-foreground">
                    {move || tr(Some(locale.get().as_str()), "Snippet preview", "Превью сниппета")}
                </h4>
                <p class="text-xs text-muted-foreground">
                    {move || tr(
                        Some(locale.get().as_str()),
                        "Search-style preview of the current SEO fields.",
                        "Поисковое превью текущих SEO-полей.",
                    )}
                </p>
            </div>
            <div class="mt-4 rounded-xl border border-border bg-card px-4 py-3">
                <p class="text-base font-medium leading-6 text-blue-700">
                    {move || {
                        let value = form.get().title.trim().to_string();
                        if value.is_empty() {
                            tr(
                                Some(locale.get().as_str()),
                                "SEO title preview",
                                "Превью SEO-заголовка",
                            )
                        } else {
                            value
                        }
                    }}
                </p>
                <p class="mt-1 text-xs text-emerald-700">
                    {move || {
                        let value = form.get().canonical_url.trim().to_string();
                        if value.is_empty() {
                            tr(
                                Some(locale.get().as_str()),
                                "Canonical URL not set",
                                "Canonical URL не задан",
                            )
                        } else {
                            value
                        }
                    }}
                </p>
                <p class="mt-2 text-sm leading-6 text-muted-foreground">
                    {move || {
                        let value = form.get().description.trim().to_string();
                        if value.is_empty() {
                            tr(
                                Some(locale.get().as_str()),
                                "Meta description preview will appear here once you fill it in.",
                                "Превью meta description появится здесь, когда вы её заполните.",
                            )
                        } else {
                            value
                        }
                    }}
                </p>
            </div>
        </article>
    }
}

#[component]
pub fn SeoRecommendationsCard(
    completeness: Memo<SeoCompletenessReport>,
    locale: Signal<String>,
) -> impl IntoView {
    view! {
        <article class="rounded-2xl border border-border bg-background/70 p-4 md:col-span-2 xl:col-span-2">
            <h4 class="text-sm font-semibold text-card-foreground">
                {move || tr(Some(locale.get().as_str()), "Recommendations", "Рекомендации")}
            </h4>
            {move || {
                let report = completeness.get();
                if report.recommendations.is_empty() {
                    view! {
                        <p class="mt-2 text-sm text-emerald-700">
                            {tr(
                                Some(locale.get().as_str()),
                                "Core snippet and social metadata are in a healthy state.",
                                "Базовый сниппет и social metadata в хорошем состоянии.",
                            )}
                        </p>
                    }
                        .into_any()
                } else {
                    let locale_value = locale.get();
                    view! {
                        <ul class="mt-2 space-y-2 text-sm text-muted-foreground">
                            {report
                                .recommendations
                                .into_iter()
                                .map(|item| {
                                    let text = recommendation(Some(locale_value.as_str()), &item);
                                    view! {
                                        <li class="rounded-xl border border-border bg-card px-3 py-2">{text}</li>
                                    }
                                })
                                .collect_view()}
                        </ul>
                    }
                        .into_any()
                }
            }}
        </article>
    }
}
