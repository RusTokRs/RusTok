use leptos::ev::SubmitEvent;
use leptos::html;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_ui_routing::{use_route_query_value, use_route_query_writer};
use rustok_ui_core::{AdminQueryKey, UiRouteContext};

use crate::core::{
    MediaAdminBusyKey, MediaDetailLabels, MediaListCardLabels, MediaTranslationFormState,
    MediaUsageLabels, is_busy_key, media_admin_context_error, media_detail_lines,
    media_list_card_view_model, media_upload_success_state, media_usage_stat_cards,
    page_count_label, selected_translation_form_state,
};
use crate::i18n::t;
use crate::model::{MediaListItem, MediaUsageSnapshot};
use crate::transport;
use crate::transport::ApiError;

fn local_resource<S, Fut, T>(
    source: impl Fn() -> S + 'static,
    fetcher: impl Fn(S) -> Fut + 'static,
) -> LocalResource<T>
where
    S: 'static,
    Fut: std::future::Future<Output = T> + 'static,
    T: 'static,
{
    LocalResource::new(move || fetcher(source()))
}

#[component]
pub fn MediaAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let ui_locale = route_context.locale.clone();
    let selected_media_query = use_route_query_value(AdminQueryKey::MediaId.as_str());
    let requested_locale_query = use_route_query_value(AdminQueryKey::Locale.as_str());
    let query_writer = use_route_query_writer();
    let token = use_token();
    let tenant = use_tenant();
    let badge_text = t(ui_locale.as_deref(), "media.badge", "media");
    let title_text = t(ui_locale.as_deref(), "media.title", "Media Library");
    let subtitle_text = t(
        ui_locale.as_deref(),
        "media.subtitle",
        "Module-owned media operations surface. Native server functions handle list/detail/translations/delete, while upload keeps the existing REST path.",
    );
    let upload_title = t(ui_locale.as_deref(), "media.upload.title", "Upload");
    let upload_subtitle = t(
        ui_locale.as_deref(),
        "media.upload.subtitle",
        "Upload stays on the existing REST /api/media path. Native #[server] calls cover the read and metadata management flows.",
    );
    let upload_action = t(ui_locale.as_deref(), "media.upload.action", "Upload Asset");
    let assets_title = t(ui_locale.as_deref(), "media.assets.title", "Assets");
    let prev_label = t(ui_locale.as_deref(), "media.pagination.prev", "Prev");
    let next_label = t(ui_locale.as_deref(), "media.pagination.next", "Next");
    let page_label_template = t(
        ui_locale.as_deref(),
        "media.pagination.page",
        "Page {count}",
    );
    let detail_title = t(ui_locale.as_deref(), "media.detail.title", "Asset Detail");
    let detail_delete = t(ui_locale.as_deref(), "media.detail.delete", "Delete");
    let detail_empty = t(
        ui_locale.as_deref(),
        "media.detail.empty",
        "Select an asset to inspect translations.",
    );
    let translation_locale_label = t(ui_locale.as_deref(), "media.translation.locale", "Locale");
    let translation_title_placeholder = t(ui_locale.as_deref(), "media.translation.title", "Title");
    let translation_alt_placeholder = t(
        ui_locale.as_deref(),
        "media.translation.altText",
        "Alt text",
    );
    let translation_save = t(
        ui_locale.as_deref(),
        "media.translation.save",
        "Save Translation",
    );
    let load_library_error = t(
        ui_locale.as_deref(),
        "media.error.loadLibrary",
        "Failed to load media library",
    );
    let load_detail_error = t(
        ui_locale.as_deref(),
        "media.error.loadDetail",
        "Failed to load media detail",
    );
    let load_translations_error = t(
        ui_locale.as_deref(),
        "media.error.loadTranslations",
        "Failed to load translations",
    );

    let (page, set_page) = signal(1_i32);
    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (selected_media_id, set_selected_media_id) = signal(selected_media_query.get_untracked());
    let (selected_locale, set_selected_locale) = signal(
        requested_locale_query
            .get_untracked()
            .or_else(|| ui_locale.clone())
            .unwrap_or_default(),
    );
    let (title, set_title) = signal(String::new());
    let (alt_text, set_alt_text) = signal(String::new());
    let (caption, set_caption) = signal(String::new());
    let (upload_error, set_upload_error) = signal(Option::<String>::None);
    let (mutation_error, set_mutation_error) = signal(Option::<String>::None);
    let (busy_key, set_busy_key) = signal(Option::<String>::None);
    let media_query_writer = query_writer.clone();
    let locale_query_writer = query_writer.clone();
    let locale_chip_query_writer = query_writer.clone();
    let upload_query_writer = query_writer.clone();
    let delete_query_writer = query_writer.clone();
    let select_media_query_writer = media_query_writer.clone();
    let select_locale_chip_query_writer = locale_chip_query_writer.clone();
    let route_effect_locale = ui_locale.clone();
    let file_input: NodeRef<html::Input> = NodeRef::new();

    let library = local_resource(
        move || (token.get(), tenant.get(), page.get(), refresh_nonce.get()),
        move |(token_value, tenant_value, page_value, _)| async move {
            transport::fetch_media_library(page_value, 12, token_value, tenant_value).await
        },
    );

    let usage = local_resource(
        move || (token.get(), tenant.get(), refresh_nonce.get()),
        move |(token_value, tenant_value, _)| async move {
            transport::fetch_media_usage(token_value, tenant_value).await
        },
    );

    let detail = local_resource(
        move || {
            (
                token.get(),
                tenant.get(),
                selected_media_id.get(),
                refresh_nonce.get(),
            )
        },
        move |(token_value, tenant_value, media_id, _)| async move {
            match media_id {
                Some(media_id) => {
                    transport::fetch_media_detail(media_id, token_value, tenant_value).await
                }
                None => Ok(None),
            }
        },
    );

    let translations = local_resource(
        move || {
            (
                token.get(),
                tenant.get(),
                selected_media_id.get(),
                refresh_nonce.get(),
            )
        },
        move |(token_value, tenant_value, media_id, _)| async move {
            match media_id {
                Some(media_id) => {
                    transport::fetch_media_translations(media_id, token_value, tenant_value).await
                }
                None => Ok(Vec::new()),
            }
        },
    );

    Effect::new(move |_| {
        set_selected_media_id.set(selected_media_query.get());
        set_selected_locale.set(
            requested_locale_query
                .get()
                .or_else(|| route_effect_locale.clone())
                .unwrap_or_default(),
        );
    });

    Effect::new(move |_| {
        if let Some(Ok(items)) = translations.get() {
            let form_state = selected_translation_form_state(&items, &selected_locale.get());
            set_title.set(form_state.title);
            set_alt_text.set(form_state.alt_text);
            set_caption.set(form_state.caption);
        }
    });

    let upload_ui_locale = ui_locale.clone();
    let upload_selected = move |_| {
        set_upload_error.set(None);
        let upload_ui_locale = upload_ui_locale.clone();
        let upload_query_writer = upload_query_writer.clone();
        let Some(input) = file_input.get() else {
            set_upload_error.set(Some(t(
                upload_ui_locale.as_deref(),
                "media.error.uploadInputUnavailable",
                "Upload input is not available.",
            )));
            return;
        };
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        set_busy_key.set(Some(MediaAdminBusyKey::Upload.as_storage_key()));
        spawn_local(async move {
            match read_selected_file(input).await {
                Ok(Some(file)) => {
                    match transport::upload_media(
                        file.name,
                        file.content_type,
                        file.bytes,
                        token_value,
                        tenant_value,
                    )
                    .await
                    {
                        Ok(item) => {
                            let upload_state = media_upload_success_state(item.id);
                            set_selected_media_id.set(Some(upload_state.selected_media_id.clone()));
                            if upload_state.should_refresh {
                                set_refresh_nonce.update(|value| *value += 1);
                            }
                            upload_query_writer.replace_value(
                                AdminQueryKey::MediaId.as_str(),
                                upload_state.selected_media_id,
                            );
                        }
                        Err(err) => set_upload_error.set(Some(format!(
                            "{}: {err}",
                            t(
                                upload_ui_locale.as_deref(),
                                "media.error.uploadFailed",
                                "Upload failed",
                            )
                        ))),
                    }
                }
                Ok(None) => set_upload_error.set(Some(t(
                    upload_ui_locale.as_deref(),
                    "media.error.chooseFileFirst",
                    "Choose a file first.",
                ))),
                Err(err) => set_upload_error.set(Some(format!(
                    "{}: {err}",
                    t(
                        upload_ui_locale.as_deref(),
                        "media.error.readFile",
                        "Failed to read file",
                    )
                ))),
            }
            set_busy_key.set(None);
        });
    };

    let translation_ui_locale = ui_locale.clone();
    let save_translation = move |ev: SubmitEvent| {
        ev.prevent_default();
        set_mutation_error.set(None);
        let translation_ui_locale = translation_ui_locale.clone();
        let Some(media_id) = selected_media_id.get_untracked() else {
            set_mutation_error.set(Some(t(
                translation_ui_locale.as_deref(),
                "media.error.selectAsset",
                "Select an asset first.",
            )));
            return;
        };
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let payload = MediaTranslationFormState {
            title: title.get_untracked(),
            alt_text: alt_text.get_untracked(),
            caption: caption.get_untracked(),
        }
        .to_upsert_payload(selected_locale.get_untracked());
        set_busy_key.set(Some(MediaAdminBusyKey::Translation.as_storage_key()));
        spawn_local(async move {
            match transport::upsert_translation(media_id, payload, token_value, tenant_value).await
            {
                Ok(_) => set_refresh_nonce.update(|value| *value += 1),
                Err(err) => set_mutation_error.set(Some(format!(
                    "{}: {err}",
                    t(
                        translation_ui_locale.as_deref(),
                        "media.error.saveTranslation",
                        "Failed to save translation",
                    )
                ))),
            }
            set_busy_key.set(None);
        });
    };

    let delete_ui_locale = ui_locale.clone();
    let delete_selected = move |_| {
        set_mutation_error.set(None);
        let delete_ui_locale = delete_ui_locale.clone();
        let delete_query_writer = delete_query_writer.clone();
        let Some(media_id) = selected_media_id.get_untracked() else {
            return;
        };
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        set_busy_key.set(Some(
            MediaAdminBusyKey::Delete(media_id.clone()).as_storage_key(),
        ));
        spawn_local(async move {
            match transport::delete_media(media_id, token_value, tenant_value).await {
                Ok(true) => {
                    delete_query_writer.clear_key(AdminQueryKey::MediaId.as_str());
                    set_selected_media_id.set(None);
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Ok(false) => set_mutation_error.set(Some(t(
                    delete_ui_locale.as_deref(),
                    "media.error.deleteRejected",
                    "Delete request was rejected.",
                ))),
                Err(err) => set_mutation_error.set(Some(format!(
                    "{}: {err}",
                    t(
                        delete_ui_locale.as_deref(),
                        "media.error.deleteAsset",
                        "Failed to delete asset",
                    )
                ))),
            }
            set_busy_key.set(None);
        });
    };

    view! {
        <div class="space-y-6">
            <header class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-2">
                    <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground">
                        {badge_text.clone()}
                    </span>
                    <h1 class="text-2xl font-semibold text-card-foreground">
                        {title_text.clone()}
                    </h1>
                    <p class="max-w-3xl text-sm text-muted-foreground">{subtitle_text.clone()}</p>
                </div>
            </header>

            <Suspense fallback=move || view! { <div class="h-24 animate-pulse rounded-2xl bg-muted"></div> }>
                {move || usage.get().map(render_usage)}
            </Suspense>

            <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                <div class="flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between">
                    <div>
                        <h2 class="text-lg font-semibold text-card-foreground">
                            {upload_title.clone()}
                        </h2>
                        <p class="text-sm text-muted-foreground">{upload_subtitle.clone()}</p>
                    </div>
                    <div class="flex flex-col gap-3 sm:flex-row sm:items-center">
                        <input
                            node_ref=file_input
                            type="file"
                            class="rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground"
                        />
                        <button
                            type="button"
                            class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground disabled:opacity-60"
                            disabled=move || is_busy_key(busy_key.get().as_deref(), MediaAdminBusyKey::Upload)
                            on:click=upload_selected
                        >
                            {upload_action.clone()}
                        </button>
                    </div>
                </div>
                {move || upload_error.get().map(render_error)}
            </section>

            {move || mutation_error.get().map(render_error)}

            <div class="grid gap-6 xl:grid-cols-[1.4fr_1fr]">
                <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                    <div class="mb-4 flex items-center justify-between gap-4">
                        <h2 class="text-lg font-semibold text-card-foreground">{assets_title.clone()}</h2>
                        <div class="flex items-center gap-2">
                            <button
                                type="button"
                                class="rounded-lg border border-border px-3 py-2 text-sm disabled:opacity-60"
                                disabled=move || page.get() <= 1
                                on:click=move |_| set_page.update(|value| *value = (*value - 1).max(1))
                            >
                                {prev_label.clone()}
                            </button>
                            <span class="text-sm text-muted-foreground">
                                {move || page_count_label(&page_label_template, page.get())}
                            </span>
                            <button
                                type="button"
                                class="rounded-lg border border-border px-3 py-2 text-sm"
                                on:click=move |_| set_page.update(|value| *value += 1)
                            >
                                {next_label.clone()}
                            </button>
                        </div>
                    </div>
                    <Suspense fallback=move || view! { <div class="h-64 animate-pulse rounded-xl bg-muted"></div> }>
                        {move || {
                            library.get().map(|result| match result {
                                Ok(payload) => view! {
                                    <div class="space-y-3">
                                        <div class="text-sm text-muted-foreground">
                                            {format!("{} assets", payload.total)}
                                        </div>
                                        <div class="space-y-2">
                                            {payload.items.into_iter().map(|item| {
                                                let item_id = item.id.clone();
                                                let media_query_writer = select_media_query_writer.clone();
                                                view! {
                                                    <button
                                                        type="button"
                                                        class="w-full rounded-xl border border-border px-4 py-3 text-left transition hover:border-primary/50 hover:bg-accent/40"
                                                        on:click=move |_| media_query_writer.push_value(AdminQueryKey::MediaId.as_str(), item_id.clone())
                                                    >
                                                        <MediaListCard item=item />
                                                    </button>
                                                }
                                            }).collect_view()}
                                        </div>
                                    </div>
                                }.into_any(),
                                Err(err) => render_error_view(media_admin_context_error(&load_library_error, err).message),
                            })
                        }}
                    </Suspense>
                </section>

                <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                    <div class="mb-4 flex items-center justify-between">
                        <h2 class="text-lg font-semibold text-card-foreground">{detail_title.clone()}</h2>
                        <button
                            type="button"
                            class="rounded-lg border border-destructive/40 px-3 py-2 text-sm text-destructive disabled:opacity-60"
                            disabled=move || selected_media_id.get().is_none()
                            on:click=delete_selected
                        >
                            {detail_delete.clone()}
                        </button>
                    </div>
                    <Suspense fallback=move || view! { <div class="h-72 animate-pulse rounded-xl bg-muted"></div> }>
                        {move || {
                            detail.get().map(|result| match result {
                                Ok(Some(item)) => view! { <MediaDetailCard item=item /> }.into_any(),
                                Ok(None) => view! {
                                    <div class="rounded-xl border border-dashed border-border px-4 py-8 text-sm text-muted-foreground">{detail_empty.clone()}</div>
                                }.into_any(),
                                Err(err) => render_error_view(media_admin_context_error(&load_detail_error, err).message),
                            })
                        }}
                    </Suspense>

                    <div class="mt-6 border-t border-border pt-6">
                        <div class="mb-4 flex items-center gap-3">
                            <label class="text-sm font-medium text-card-foreground" for="translation-locale">{translation_locale_label.clone()}</label>
                            <input
                                id="translation-locale"
                                class="rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground"
                                prop:value=selected_locale
                                on:input=move |ev| {
                                    let next_value = event_target_value(&ev);
                                    set_selected_locale.set(next_value.clone());
                                    if next_value.trim().is_empty() {
                                        locale_query_writer.clear_key(AdminQueryKey::Locale.as_str());
                                    } else {
                                        locale_query_writer.replace_value(AdminQueryKey::Locale.as_str(), next_value);
                                    }
                                }
                            />
                        </div>
                        <Suspense fallback=move || view! { <div class="h-20 animate-pulse rounded-xl bg-muted"></div> }>
                            {move || {
                                translations.get().map(|result| match result {
                                    Ok(items) => view! {
                                        <div class="mb-4 flex flex-wrap gap-2">
                                            {items.into_iter().map(|item| {
                                                let locale = item.locale.clone();
                                                let locale_label = locale.clone();
                                                let locale_chip_query_writer = select_locale_chip_query_writer.clone();
                                                view! {
                                                    <button
                                                        type="button"
                                                        class="rounded-full border border-border px-3 py-1 text-xs text-muted-foreground"
                                                        on:click=move |_| {
                                                            set_selected_locale.set(locale.clone());
                                                            locale_chip_query_writer.replace_value(AdminQueryKey::Locale.as_str(), locale.clone());
                                                        }
                                                    >
                                                        {locale_label}
                                                    </button>
                                                }
                                            }).collect_view()}
                                        </div>
                                    }.into_any(),
                                    Err(err) => render_error_view(media_admin_context_error(&load_translations_error, err).message),
                                })
                            }}
                        </Suspense>
                        <form class="space-y-3" on:submit=save_translation>
                            <input
                                class="w-full rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground"
                                placeholder=translation_title_placeholder.clone()
                                prop:value=title
                                on:input=move |ev| set_title.set(event_target_value(&ev))
                            />
                            <input
                                class="w-full rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground"
                                placeholder=translation_alt_placeholder.clone()
                                prop:value=alt_text
                                on:input=move |ev| set_alt_text.set(event_target_value(&ev))
                            />
                            <textarea
                                class="min-h-24 w-full rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground"
                                prop:value=caption
                                on:input=move |ev| set_caption.set(event_target_value(&ev))
                            />
                            <button
                                type="submit"
                                class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground disabled:opacity-60"
                                disabled=move || is_busy_key(busy_key.get().as_deref(), MediaAdminBusyKey::Translation)
                            >
                                {translation_save.clone()}
                            </button>
                        </form>
                    </div>
                </section>
            </div>
        </div>
    }
}

#[component]
fn MediaListCard(item: MediaListItem) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let card = media_list_card_view_model(
        &item,
        MediaListCardLabels {
            bytes_template: t(locale.as_deref(), "media.asset.bytes", "{count} bytes"),
            dimensions_not_available: t(locale.as_deref(), "media.asset.notAvailable", "n/a"),
        },
    );
    view! {
        <div class="flex items-start justify-between gap-4">
            <div class="min-w-0 space-y-1">
                <div class="truncate text-sm font-semibold text-card-foreground">{card.original_name}</div>
                <div class="truncate text-xs text-muted-foreground">{card.public_url}</div>
                <div class="flex flex-wrap gap-2 text-xs text-muted-foreground">
                    <span>{card.mime_type}</span>
                    <span>{card.size_label}</span>
                    <span>{card.dimensions_label}</span>
                </div>
            </div>
            <span class="rounded-full border border-border px-2 py-1 text-[11px] text-muted-foreground">
                {card.storage_driver}
            </span>
        </div>
    }
}

#[component]
fn MediaDetailCard(item: MediaListItem) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let lines = media_detail_lines(
        &item,
        MediaDetailLabels {
            original_name: t(
                locale.as_deref(),
                "media.detail.originalName",
                "Original Name",
            ),
            id: t(locale.as_deref(), "media.detail.id", "ID"),
            mime: t(locale.as_deref(), "media.detail.mime", "MIME"),
            storage: t(locale.as_deref(), "media.detail.storage", "Storage"),
            public_url: t(locale.as_deref(), "media.detail.publicUrl", "Public URL"),
            size: t(locale.as_deref(), "media.detail.size", "Size"),
            created: t(locale.as_deref(), "media.detail.created", "Created"),
        },
        &t(locale.as_deref(), "media.asset.bytes", "{count} bytes"),
    );

    view! {
        <div class="space-y-3 text-sm">
            {lines.into_iter().map(|line| view! {
                <DetailLine label=line.label value=line.value />
            }).collect_view()}
        </div>
    }
}

#[component]
fn DetailLine(label: String, value: String) -> impl IntoView {
    view! {
        <div class="rounded-xl border border-border bg-background/60 px-3 py-2">
            <div class="text-[11px] uppercase tracking-wide text-muted-foreground">{label}</div>
            <div class="mt-1 break-all text-sm text-card-foreground">{value}</div>
        </div>
    }
}

#[component]
fn StatCard(label: String, value: String) -> impl IntoView {
    view! {
        <div class="rounded-2xl border border-border bg-card p-5 shadow-sm">
            <div class="text-sm text-muted-foreground">{label}</div>
            <div class="mt-2 text-2xl font-semibold text-card-foreground">{value}</div>
        </div>
    }
}

fn render_usage(result: Result<MediaUsageSnapshot, ApiError>) -> AnyView {
    match result {
        Ok(payload) => {
            let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
            let cards = media_usage_stat_cards(
                payload,
                MediaUsageLabels {
                    files: t(locale.as_deref(), "media.usage.files", "Files"),
                    total_bytes: t(locale.as_deref(), "media.usage.totalBytes", "Total Bytes"),
                    tenant: t(locale.as_deref(), "media.usage.tenant", "Tenant"),
                },
            );
            view! {
                <section class="grid gap-4 md:grid-cols-3">
                    {cards.into_iter().map(|card| view! {
                        <StatCard label=card.label value=card.value />
                    }).collect_view()}
                </section>
            }
            .into_any()
        }
        Err(err) => {
            render_error_view(media_admin_context_error("Failed to load media usage", err).message)
        }
    }
}

fn render_error(error: String) -> impl IntoView {
    view! {
        <div class="mt-4 rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
            {error}
        </div>
    }
}

fn render_error_view(error: String) -> AnyView {
    view! {
        <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
            {error}
        </div>
    }
    .into_any()
}

struct SelectedUploadFile {
    name: String,
    content_type: String,
    bytes: Vec<u8>,
}

#[cfg(target_arch = "wasm32")]
async fn read_selected_file(
    input: web_sys::HtmlInputElement,
) -> Result<Option<SelectedUploadFile>, String> {
    use wasm_bindgen_futures::JsFuture;

    let Some(files) = input.files() else {
        return Ok(None);
    };
    let Some(file) = files.get(0) else {
        return Ok(None);
    };
    let buffer = JsFuture::from(file.array_buffer())
        .await
        .map_err(|err| format!("{err:?}"))?;
    let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
    let content_type = if file.type_().is_empty() {
        "application/octet-stream".to_string()
    } else {
        file.type_()
    };

    Ok(Some(SelectedUploadFile {
        name: file.name(),
        content_type,
        bytes,
    }))
}

#[cfg(not(target_arch = "wasm32"))]
async fn read_selected_file(
    _input: web_sys::HtmlInputElement,
) -> Result<Option<SelectedUploadFile>, String> {
    Ok(None)
}
