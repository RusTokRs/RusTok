use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use rustok_seo_targets::SeoTargetSlug;

use crate::api;
use crate::components::{SeoSnippetPreviewCard, SeoSummaryTile};
use crate::i18n::{
    recommendation, recommendations_count_label, source_label, tr, validation_error, working_label,
};
use crate::locale::normalize_locale_tag;
use crate::model::{validate_target_id, SeoCompletenessReport, SeoEntityForm, SeoMetaView};

#[component]
pub fn SeoEntityPanel(
    target_kind: SeoTargetSlug,
    target_id: Signal<Option<String>>,
    locale: Signal<String>,
    #[prop(optional, into)] panel_title: Option<TextProp>,
    #[prop(optional, into)] panel_subtitle: Option<TextProp>,
    #[prop(optional, into)] empty_message: Option<TextProp>,
) -> impl IntoView {
    let token = use_token();
    let tenant = use_tenant();
    let current_locale = normalized_locale(locale.get_untracked()).unwrap_or_default();
    let form = RwSignal::new(SeoEntityForm::new(current_locale));
    let loaded_meta = RwSignal::new(Option::<SeoMetaView>::None);
    let busy_key = RwSignal::new(Option::<String>::None);
    let status_message = RwSignal::new(Option::<String>::None);

    let title_locale = locale.clone();
    let title_override = panel_title;
    let panel_title = Memo::new(move |_| {
        title_override
            .as_ref()
            .map(|text| text.get().to_string())
            .unwrap_or_else(|| tr(Some(title_locale.get().as_str()), "SEO", "SEO"))
    });

    let subtitle_locale = locale.clone();
    let subtitle_override = panel_subtitle;
    let panel_subtitle = Memo::new(move |_| {
        subtitle_override.as_ref().map(|text| text.get().to_string()).unwrap_or_else(|| {
            tr(
                Some(subtitle_locale.get().as_str()),
                "This entity-owned panel persists explicit SEO overrides through the shared rustok-seo runtime.",
                "Эта entity-owned панель сохраняет явные SEO-overrides через общий runtime rustok-seo.",
            )
        })
    });

    let empty_locale = locale.clone();
    let empty_override = empty_message;
    let empty_message = Memo::new(move |_| {
        empty_override.as_ref().map(|text| text.get().to_string()).unwrap_or_else(|| {
            tr(
                Some(empty_locale.get().as_str()),
                "Open or create an entity first, then configure its explicit SEO metadata here.",
                "Сначала откройте или создайте сущность, затем настройте здесь её явные SEO-метаданные.",
            )
        })
    });

    let load_target_kind = target_kind.clone();
    let load_meta = Callback::new(move |(entity_id, next_locale): (String, String)| {
        let target_kind = load_target_kind.clone();
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        busy_key.set(Some("load".to_string()));
        status_message.set(None);
        spawn_local(async move {
            match api::fetch_seo_meta(
                token_value,
                tenant_value,
                target_kind.clone(),
                entity_id,
                Some(next_locale.clone()),
            )
            .await
            {
                Ok(Some(meta)) => {
                    form.update(|draft| draft.apply_record(&meta));
                    loaded_meta.set(Some(meta.clone()));
                    status_message.set(Some(format!(
                        "{} {}.",
                        tr(
                            Some(next_locale.as_str()),
                            "Loaded explicit SEO metadata from",
                            "Загружены явные SEO-метаданные из",
                        ),
                        source_label(Some(next_locale.as_str()), meta.source.as_str())
                    )));
                }
                Ok(None) => {
                    form.update(|draft| {
                        draft.apply_locale(next_locale.clone());
                        draft.clear_content();
                    });
                    loaded_meta.set(None);
                    status_message.set(Some(tr(
                        Some(next_locale.as_str()),
                        "No explicit SEO record yet. Entity/domain fallback is still active.",
                        "Явной SEO-записи пока нет. Фоллбек сущности/домена остаётся активным.",
                    )));
                }
                Err(err) => {
                    loaded_meta.set(None);
                    form.update(|draft| {
                        draft.apply_locale(next_locale.clone());
                        draft.clear_content();
                    });
                    status_message.set(Some(err.to_string()));
                }
            }
            busy_key.set(None);
        });
    });

    let initial_locale = locale.clone();
    let initial_target = target_id.clone();
    let load_meta_for_effect = load_meta.clone();
    Effect::new(move |_| {
        let entity_id = initial_target.get().unwrap_or_default();
        let raw_locale = initial_locale.get();
        let next_locale = normalized_locale(raw_locale.clone());
        form.update(|draft| draft.apply_locale(next_locale.clone().unwrap_or_default()));
        loaded_meta.set(None);

        if entity_id.trim().is_empty() {
            status_message.set(None);
            form.update(|draft| draft.clear_content());
            return;
        }

        if let Err(err) = validate_target_id(entity_id.as_str()) {
            status_message.set(Some(validation_error(
                Some(raw_locale.as_str()),
                err.as_str(),
            )));
            form.update(|draft| draft.clear_content());
            return;
        }

        let Some(next_locale) = next_locale else {
            status_message.set(Some(tr(
                Some(raw_locale.as_str()),
                "SEO panel is waiting for the host locale.",
                "SEO-панель ждёт локаль от host-приложения.",
            )));
            form.update(|draft| draft.clear_content());
            return;
        };

        load_meta_for_effect.run((entity_id, next_locale));
    });

    let completeness = Memo::new(move |_| form.get().completeness_report());
    let save_target_kind = target_kind.clone();
    let save_meta = Callback::new(move |ev: SubmitEvent| {
        ev.prevent_default();
        status_message.set(None);
        let ui_locale = locale.get_untracked();

        let entity_id = match target_id.get_untracked() {
            Some(value) if !value.trim().is_empty() => value,
            _ => {
                status_message.set(Some(tr(
                    Some(ui_locale.as_str()),
                    "Select an entity before saving SEO metadata.",
                    "Сначала выберите сущность, затем сохраняйте SEO-метаданные.",
                )));
                return;
            }
        };

        let input = match form
            .get_untracked()
            .build_input(save_target_kind.clone(), entity_id.as_str())
        {
            Ok(input) => input,
            Err(err) => {
                status_message.set(Some(validation_error(
                    Some(ui_locale.as_str()),
                    err.as_str(),
                )));
                return;
            }
        };

        busy_key.set(Some("save".to_string()));
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        spawn_local(async move {
            match api::save_seo_meta(token_value, tenant_value, input).await {
                Ok(meta) => {
                    form.update(|draft| draft.apply_record(&meta));
                    loaded_meta.set(Some(meta));
                    status_message.set(Some(tr(
                        Some(ui_locale.as_str()),
                        "SEO metadata saved.",
                        "SEO-метаданные сохранены.",
                    )));
                }
                Err(err) => status_message.set(Some(err.to_string())),
            }
            busy_key.set(None);
        });
    });

    let publish_locale = locale.clone();
    let publish_target_kind = target_kind.clone();
    let publish_revision = Callback::new(move |_| {
        status_message.set(None);
        let ui_locale = publish_locale.get_untracked();
        let Some(entity_id) = target_id.get_untracked() else {
            status_message.set(Some(tr(
                Some(ui_locale.as_str()),
                "Select an entity before publishing a revision.",
                "Сначала выберите сущность, затем публикуйте ревизию.",
            )));
            return;
        };

        if let Err(err) = validate_target_id(entity_id.as_str()) {
            status_message.set(Some(validation_error(
                Some(ui_locale.as_str()),
                err.as_str(),
            )));
            return;
        }

        busy_key.set(Some("publish".to_string()));
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let publish_target_kind = publish_target_kind.clone();
        spawn_local(async move {
            match api::publish_seo_revision(
                token_value,
                tenant_value,
                publish_target_kind,
                entity_id,
                None,
            )
            .await
            {
                Ok(revision) => {
                    status_message.set(Some(format!(
                        "{} {}.",
                        tr(
                            Some(ui_locale.as_str()),
                            "Published SEO revision",
                            "Опубликована SEO-ревизия",
                        ),
                        revision.revision
                    )));
                }
                Err(err) => status_message.set(Some(err.to_string())),
            }
            busy_key.set(None);
        });
    });

    view! {
        <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
            <div class="space-y-2">
                <h3 class="text-lg font-semibold text-card-foreground">{move || panel_title.get()}</h3>
                <p class="text-sm text-muted-foreground">{move || panel_subtitle.get()}</p>
            </div>

            <Show when=move || status_message.get().is_some()>
                <div class="mt-4 rounded-xl border border-border bg-background px-4 py-3 text-sm text-muted-foreground">
                    {move || status_message.get().unwrap_or_default()}
                </div>
            </Show>

            <Show
                when=move || target_id.get().map(|value| !value.trim().is_empty()).unwrap_or(false)
                fallback=move || view! {
                    <div class="mt-5 rounded-2xl border border-dashed border-border px-4 py-6 text-sm text-muted-foreground">
                        {move || empty_message.get()}
                    </div>
                }
            >
                <div class="mt-5 space-y-5">
                    <SeoPanelSummary
                        loaded_meta=loaded_meta
                        completeness=completeness
                        locale=locale
                    />
                    <SeoSnippetPreviewCard form=form locale=locale />

                    <form class="space-y-5" on:submit=move |ev| save_meta.run(ev)>
                        <div class="grid gap-4 md:grid-cols-2">
                            <label class="block space-y-2">
                                <span class="text-sm font-medium text-card-foreground">
                                    {move || tr(Some(locale.get().as_str()), "SEO title", "SEO-заголовок")}
                                </span>
                                <input
                                    type="text"
                                    class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                                    prop:value=move || form.get().title
                                    on:input=move |ev| form.update(|draft| draft.title = event_target_value(&ev))
                                />
                            </label>
                            <label class="block space-y-2">
                                <span class="text-sm font-medium text-card-foreground">
                                    {move || tr(Some(locale.get().as_str()), "Canonical URL", "Canonical URL")}
                                </span>
                                <input
                                    type="text"
                                    class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                                    prop:value=move || form.get().canonical_url
                                    on:input=move |ev| form.update(|draft| draft.canonical_url = event_target_value(&ev))
                                />
                            </label>
                        </div>

                        <label class="block space-y-2">
                            <span class="text-sm font-medium text-card-foreground">
                                {move || tr(Some(locale.get().as_str()), "Description", "Описание")}
                            </span>
                            <textarea
                                class="min-h-24 w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                                prop:value=move || form.get().description
                                on:input=move |ev| form.update(|draft| draft.description = event_target_value(&ev))
                            />
                        </label>

                        <label class="block space-y-2">
                            <span class="text-sm font-medium text-card-foreground">
                                {move || tr(Some(locale.get().as_str()), "Keywords", "Ключевые слова")}
                            </span>
                            <input
                                type="text"
                                class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                                prop:value=move || form.get().keywords
                                on:input=move |ev| form.update(|draft| draft.keywords = event_target_value(&ev))
                            />
                        </label>

                        <div class="rounded-2xl border border-border bg-background/70 p-4">
                            <div class="space-y-1">
                                <h4 class="text-sm font-semibold text-card-foreground">
                                    {move || tr(Some(locale.get().as_str()), "Open Graph", "Open Graph")}
                                </h4>
                                <p class="text-xs text-muted-foreground">
                                    {move || tr(
                                        Some(locale.get().as_str()),
                                        "Social preview metadata stays next to the entity editor, not in a central SEO hub.",
                                        "Метаданные социальных превью остаются рядом с редактором сущности, а не в центральном SEO-хабе.",
                                    )}
                                </p>
                            </div>
                            <div class="mt-4 grid gap-4 md:grid-cols-2">
                                <label class="block space-y-2">
                                    <span class="text-sm font-medium text-card-foreground">
                                        {move || tr(Some(locale.get().as_str()), "OG title", "OG title")}
                                    </span>
                                    <input
                                        type="text"
                                        class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                                        prop:value=move || form.get().og_title
                                        on:input=move |ev| form.update(|draft| draft.og_title = event_target_value(&ev))
                                    />
                                </label>
                                <label class="block space-y-2">
                                    <span class="text-sm font-medium text-card-foreground">
                                        {move || tr(Some(locale.get().as_str()), "OG image", "OG image")}
                                    </span>
                                    <input
                                        type="text"
                                        class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                                        prop:value=move || form.get().og_image
                                        on:input=move |ev| form.update(|draft| draft.og_image = event_target_value(&ev))
                                    />
                                </label>
                            </div>
                            <label class="mt-4 block space-y-2">
                                <span class="text-sm font-medium text-card-foreground">
                                    {move || tr(Some(locale.get().as_str()), "OG description", "OG description")}
                                </span>
                                <textarea
                                    class="min-h-20 w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                                    prop:value=move || form.get().og_description
                                    on:input=move |ev| form.update(|draft| draft.og_description = event_target_value(&ev))
                                />
                            </label>
                        </div>

                        <div class="rounded-2xl border border-border bg-background/70 p-4">
                            <div class="space-y-1">
                                <h4 class="text-sm font-semibold text-card-foreground">
                                    {move || tr(Some(locale.get().as_str()), "Advanced", "Расширенные настройки")}
                                </h4>
                                <p class="text-xs text-muted-foreground">
                                    {move || tr(
                                        Some(locale.get().as_str()),
                                        "Structured data and robots directives remain additive overrides from the shared SEO runtime.",
                                        "Structured data и robots directives остаются аддитивными overrides из общего SEO runtime.",
                                    )}
                                </p>
                            </div>
                            <label class="mt-4 block space-y-2">
                                <span class="text-sm font-medium text-card-foreground">
                                    {move || tr(
                                        Some(locale.get().as_str()),
                                        "Structured data (JSON)",
                                        "Structured data (JSON)",
                                    )}
                                </span>
                                <textarea
                                    class="min-h-32 w-full rounded-xl border border-border bg-background px-3 py-2 font-mono text-xs text-foreground outline-none transition focus:border-primary"
                                    prop:value=move || form.get().structured_data
                                    on:input=move |ev| form.update(|draft| draft.structured_data = event_target_value(&ev))
                                />
                            </label>
                            <div class="mt-4 flex flex-wrap gap-4 text-sm text-card-foreground">
                                <label class="inline-flex items-center gap-2">
                                    <input
                                        type="checkbox"
                                        prop:checked=move || form.get().noindex
                                        on:change=move |ev| form.update(|draft| draft.noindex = event_target_checked(&ev))
                                    />
                                    "noindex"
                                </label>
                                <label class="inline-flex items-center gap-2">
                                    <input
                                        type="checkbox"
                                        prop:checked=move || form.get().nofollow
                                        on:change=move |ev| form.update(|draft| draft.nofollow = event_target_checked(&ev))
                                    />
                                    "nofollow"
                                </label>
                            </div>
                        </div>

                        <div class="flex flex-wrap gap-3">
                            <button
                                type="submit"
                                class="inline-flex rounded-xl bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50"
                                disabled=move || busy_key.get().is_some() || normalized_locale(locale.get()).is_none()
                            >
                                {move || tr(Some(locale.get().as_str()), "Save SEO", "Сохранить SEO")}
                            </button>
                            <button
                                type="button"
                                class="inline-flex rounded-xl border border-border px-4 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50"
                                disabled=move || busy_key.get().is_some() || normalized_locale(locale.get()).is_none()
                                on:click=move |_| publish_revision.run(())
                            >
                                {move || tr(Some(locale.get().as_str()), "Publish revision", "Опубликовать ревизию")}
                            </button>
                            <Show when=move || busy_key.get().is_some()>
                                <span class="inline-flex items-center text-xs text-muted-foreground">
                                    {move || working_label(
                                        Some(locale.get().as_str()),
                                        busy_key.get().unwrap_or_default().as_str(),
                                    )}
                                </span>
                            </Show>
                        </div>
                    </form>
                </div>
            </Show>
        </section>
    }
}

#[component]
pub fn SeoCapabilityNotice(title: String, body: String) -> impl IntoView {
    view! {
        <section class="rounded-2xl border border-dashed border-border bg-card p-6 shadow-sm">
            <h3 class="text-lg font-semibold text-card-foreground">{title}</h3>
            <p class="mt-2 text-sm leading-6 text-muted-foreground">{body}</p>
        </section>
    }
}

#[component]
fn SeoPanelSummary(
    loaded_meta: RwSignal<Option<SeoMetaView>>,
    completeness: Memo<SeoCompletenessReport>,
    locale: Signal<String>,
) -> impl IntoView {
    view! {
        <div class="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
            <SeoSummaryTile
                label=Signal::derive(move || tr(Some(locale.get().as_str()), "Completeness", "Заполненность"))
                value=Signal::derive(move || format!("{}%", completeness.get().score))
                detail=Signal::derive(move || {
                    let report = completeness.get();
                    if report.score >= 80 {
                        tr(Some(locale.get().as_str()), "Strong baseline", "Хорошая базовая заполненность")
                    } else {
                        recommendations_count_label(Some(locale.get().as_str()), report.recommendations.len())
                    }
                })
            />
            <SeoSummaryTile
                label=Signal::derive(move || tr(Some(locale.get().as_str()), "Source", "Источник"))
                value=Signal::derive(move || {
                    loaded_meta
                        .get()
                        .map(|meta| source_label(Some(locale.get().as_str()), meta.source.as_str()))
                        .unwrap_or_else(|| tr(Some(locale.get().as_str()), "Entity fallback", "Фоллбек сущности"))
                })
                detail=Signal::derive(move || {
                    tr(
                        Some(locale.get().as_str()),
                        "Explicit SEO overrides win over domain fallback.",
                        "Явные SEO-overrides имеют приоритет над доменным фоллбеком.",
                    )
                })
            />
            <SeoSummaryTile
                label=Signal::derive(move || tr(Some(locale.get().as_str()), "Effective locale", "Эффективная локаль"))
                value=Signal::derive(move || {
                    loaded_meta
                        .get()
                        .map(|meta| meta.effective_locale)
                        .unwrap_or_else(|| "n/a".to_string())
                })
                detail=Signal::derive(move || {
                    tr(
                        Some(locale.get().as_str()),
                        "Locale comes from the owner editor context.",
                        "Локаль приходит из контекста owner-editor экрана.",
                    )
                })
            />
            <SeoSummaryTile
                label=Signal::derive(move || tr(Some(locale.get().as_str()), "Available locales", "Доступные локали"))
                value=Signal::derive(move || {
                    loaded_meta
                        .get()
                        .map(|meta| {
                            if meta.available_locales.is_empty() {
                                "n/a".to_string()
                            } else {
                                meta.available_locales.join(", ")
                            }
                        })
                        .unwrap_or_else(|| "n/a".to_string())
                })
                detail=Signal::derive(move || {
                    tr(
                        Some(locale.get().as_str()),
                        "Coverage for explicit or resolved SEO translations.",
                        "Покрытие для явных или разрешённых SEO-переводов.",
                    )
                })
            />

            <div class="rounded-2xl border border-border bg-background/70 p-4 md:col-span-2 xl:col-span-4">
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
                        }.into_any()
                    } else {
                        let locale_value = locale.get();
                        view! {
                            <ul class="mt-2 space-y-2 text-sm text-muted-foreground">
                                {report.recommendations.into_iter().map(|item| {
                                    let text = recommendation(Some(locale_value.as_str()), &item);
                                    view! {
                                        <li class="rounded-xl border border-border bg-card px-3 py-2">{text}</li>
                                    }
                                }).collect_view()}
                            </ul>
                        }.into_any()
                    }
                }}
            </div>
        </div>
    }
}

fn normalized_locale(value: String) -> Option<String> {
    normalize_locale_tag(value.trim())
}

#[cfg(test)]
mod tests {
    use super::normalized_locale;

    #[test]
    fn normalized_locale_canonicalizes_host_locale() {
        assert_eq!(
            normalized_locale(" pt_br ".to_string()).as_deref(),
            Some("pt-BR")
        );
        assert_eq!(
            normalized_locale("ru_ru".to_string()).as_deref(),
            Some("ru-RU")
        );
    }

    #[test]
    fn normalized_locale_rejects_empty_and_invalid_values() {
        assert_eq!(normalized_locale(String::new()), None);
        assert_eq!(normalized_locale("**".to_string()), None);
    }
}
