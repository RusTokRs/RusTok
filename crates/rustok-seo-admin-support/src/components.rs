use leptos::prelude::*;

use crate::i18n::{
    delivery_status_label, recommendation, remediation_action_label, remediation_reason, tr,
};
use crate::model::{
    SeoCompletenessReport, SeoEntityForm, SeoEventDeliveryStatus, SeoEventDeliverySummary,
    SeoRemediationHint,
};

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
pub fn SeoSchemaPreviewCard(
    form: RwSignal<SeoEntityForm>,
    locale: Signal<String>,
) -> impl IntoView {
    let preview_json = Memo::new(move |_| {
        let form_value = form.get();
        let schema_type = form_value.structured_data_type.trim();
        let payload = form_value.structured_data_payload.trim();

        let mut object = if payload.is_empty() {
            serde_json::Map::new()
        } else {
            serde_json::from_str::<serde_json::Value>(payload)
                .ok()
                .and_then(|v| v.as_object().cloned())
                .unwrap_or_default()
        };

        if !schema_type.is_empty() {
            object.insert(
                "@type".to_string(),
                serde_json::Value::String(schema_type.to_string()),
            );
        }
        object.insert(
            "@context".to_string(),
            serde_json::Value::String("https://schema.org".to_string()),
        );

        serde_json::to_string_pretty(&serde_json::Value::Object(object))
            .unwrap_or_else(|_| "{}".to_string())
    });

    let validation_issues = Memo::new(move |_| {
        let form_value = form.get();
        let schema_type = form_value.structured_data_type.trim();
        let payload = form_value.structured_data_payload.trim();

        let mut issues = Vec::new();
        let mut object = if payload.is_empty() {
            serde_json::Map::new()
        } else {
            serde_json::from_str::<serde_json::Value>(payload)
                .ok()
                .and_then(|v| v.as_object().cloned())
                .unwrap_or_default()
        };

        if !schema_type.is_empty() {
            object.insert(
                "@type".to_string(),
                serde_json::Value::String(schema_type.to_string()),
            );
        }

        let required = required_fields_for_type_name(schema_type);
        for field in required {
            if !object.contains_key(*field) {
                issues.push(format!("Missing required field: {}", field));
            }
        }

        match schema_type {
            "BreadcrumbList" | "ItemList" => {
                if let Some(value) = object.get("itemListElement") {
                    if !value.is_array() {
                        issues.push("itemListElement must be an array.".to_string());
                    }
                }
            }
            "FAQPage" => {
                if let Some(value) = object.get("mainEntity") {
                    if !value.is_array() {
                        issues.push("mainEntity must be an array.".to_string());
                    }
                }
            }
            _ => {}
        }

        issues
    });

    view! {
        <div class="mt-4 rounded-2xl border border-border bg-background/70 p-4 md:col-span-2 xl:col-span-2">
            <div class="space-y-1">
                <h4 class="text-sm font-semibold text-card-foreground">
                    {move || tr(Some(locale.get().as_str()), "Schema preview", "Превью схемы")}
                </h4>
                <p class="text-xs text-muted-foreground">
                    {move || tr(
                        Some(locale.get().as_str()),
                        "Live JSON-LD preview and validation from the current schema fields.",
                        "Live JSON-LD preview и валидация текущих schema-полей.",
                    )}
                </p>
            </div>
            <pre class="mt-3 max-h-64 overflow-auto rounded-xl border border-border bg-card p-3 text-xs font-mono text-foreground">
                {move || preview_json.get()}
            </pre>
            {move || {
                let issues = validation_issues.get();
                if issues.is_empty() {
                    view! {
                        <p class="mt-2 text-xs text-emerald-700">
                            {tr(
                                Some(locale.get().as_str()),
                                "Schema structure looks valid.",
                                "Структура схемы выглядит валидной.",
                            )}
                        </p>
                    }.into_any()
                } else {
                    let locale_value = locale.get();
                    view! {
                        <ul class="mt-2 space-y-1 text-xs text-amber-700">
                            {issues.into_iter().map(|issue| {
                                view! {
                                    <li class="flex items-start gap-1">
                                        <span>"⚠"</span>
                                        <span>{issue}</span>
                                    </li>
                                }
                            }).collect_view()}
                        </ul>
                    }.into_any()
                }
            }}
        </div>
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


#[component]
pub fn SeoDeliveryStatusCards(
    summary: Signal<SeoEventDeliverySummary>,
    locale: Signal<String>,
) -> impl IntoView {
    let statuses = [
        SeoEventDeliveryStatus::Pending,
        SeoEventDeliveryStatus::Sent,
        SeoEventDeliveryStatus::Retry,
        SeoEventDeliveryStatus::Failed,
    ];

    view! {
        <article class="rounded-2xl border border-border bg-background/70 p-4 md:col-span-2 xl:col-span-2">
            <h4 class="text-sm font-semibold text-card-foreground">
                {move || tr(Some(locale.get().as_str()), "Delivery status", "Статус доставки")}
            </h4>
            <div class="mt-3 grid gap-2 sm:grid-cols-2 xl:grid-cols-4">
                {statuses.into_iter().map(|status| {
                    let status_label = move || delivery_status_label(Some(locale.get().as_str()), status);
                    let status_value = move || {
                        let summary = summary.get();
                        match status {
                            SeoEventDeliveryStatus::Pending => summary.pending,
                            SeoEventDeliveryStatus::Sent => summary.sent,
                            SeoEventDeliveryStatus::Retry => summary.retry,
                            SeoEventDeliveryStatus::Failed => summary.failed,
                        }
                    };
                    view! {
                        <div class="rounded-xl border border-border bg-card px-3 py-2">
                            <p class="text-[11px] uppercase tracking-wide text-muted-foreground">{status_label}</p>
                            <p class="mt-1 text-sm font-semibold text-card-foreground">{move || status_value().to_string()}</p>
                        </div>
                    }
                }).collect_view()}
            </div>
        </article>
    }
}

#[component]
pub fn SeoRemediationHintCard(
    hint: Signal<SeoRemediationHint>,
    locale: Signal<String>,
) -> impl IntoView {
    view! {
        <article class="rounded-2xl border border-border bg-background/70 p-4 md:col-span-2 xl:col-span-2">
            <h4 class="text-sm font-semibold text-card-foreground">
                {move || tr(Some(locale.get().as_str()), "Remediation hint", "Подсказка remediation")}
            </h4>
            <p class="mt-2 text-sm text-foreground">
                {move || remediation_action_label(Some(locale.get().as_str()), hint.get().action)}
            </p>
            <p class="mt-1 text-xs text-muted-foreground">
                {move || remediation_reason(Some(locale.get().as_str()), hint.get().reason_key.as_str())}
            </p>
            <p class="mt-2 text-xs text-muted-foreground">
                {move || format!("issue_code: {}", hint.get().issue_code)}
            </p>
        </article>
    }
}

fn required_fields_for_type_name(type_name: &str) -> &'static [&'static str] {
    match type_name.trim() {
        "Product" => &["name"],
        "Offer" => &["price", "priceCurrency"],
        "BreadcrumbList" => &["itemListElement"],
        "ItemList" => &["itemListElement"],
        "FAQPage" => &["mainEntity"],
        "HowTo" => &["name"],
        "Organization" => &["name"],
        "LocalBusiness" => &["name"],
        "WebSite" => &["name"],
        "Article" | "BlogPosting" | "NewsArticle" => &["headline"],
        "DiscussionForumPosting" => &["headline"],
        "WebPage" | "CollectionPage" => &["name"],
        _ => &[],
    }
}
