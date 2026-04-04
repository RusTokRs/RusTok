mod api;
mod i18n;
mod model;

use leptos::ev::SubmitEvent;
use leptos::html;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use rustok_api::UiRouteContext;

use crate::api::ApiError;
use crate::i18n::t;
use crate::model::{MediaListItem, MediaUsageSnapshot, UpsertTranslationPayload};

#[component]
pub fn MediaAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let ui_locale = route_context.locale.clone();
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
    let (selected_media_id, set_selected_media_id) = signal(Option::<String>::None);
    let (selected_locale, set_selected_locale) =
        signal(ui_locale.clone().unwrap_or_else(|| "en".to_string()));
    let (title, set_title) = signal(String::new());
    let (alt_text, set_alt_text) = signal(String::new());
    let (caption, set_caption) = signal(String::new());
    let (upload_error, set_upload_error) = signal(Option::<String>::None);
    let (mutation_error, set_mutation_error) = signal(Option::<String>::None);
    let (busy_key, set_busy_key) = signal(Option::<String>::None);
    let file_input: NodeRef<html::Input> = NodeRef::new();

    let library = Resource::new(
        move || (token.get(), tenant.get(), page.get(), refresh_nonce.get()),
        move |(token_value, tenant_value, page_value, _)| async move {
            api::fetch_media_library(page_value, 12, token_value, tenant_value).await
        },
    );

    let usage = Resource::new(
        move || (token.get(), tenant.get(), refresh_nonce.get()),
        move |(token_value, tenant_value, _)| async move {
            api::fetch_media_usage(token_value, tenant_value).await
        },
    );

    let detail = Resource::new(
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
                    api::fetch_media_detail(media_id, token_value, tenant_value).await
                }
                None => Ok(None),
            }
        },
    );

    let translations = Resource::new(
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
                    api::fetch_media_translations(media_id, token_value, tenant_value).await
                }
                None => Ok(Vec::new()),
            }
        },
    );

    Effect::new(move |_| {
        if let Some(Ok(payload)) = library.get() {
            if selected_media_id.get_untracked().is_none() {
                if let Some(first) = payload.items.first() {
                    set_selected_media_id.set(Some(first.id.clone()));
                }
            }
        }
    });

    Effect::new(move |_| {
        if let Some(Ok(items)) = translations.get() {
            let locale = selected_locale.get();
            if let Some(current) = items.iter().find(|item| item.locale == locale) {
                set_title.set(current.title.clone().unwrap_or_default());
                set_alt_text.set(current.alt_text.clone().unwrap_or_default());
                set_caption.set(current.caption.clone().unwrap_or_default());
            } else {
                set_title.set(String::new());
                set_alt_text.set(String::new());
                set_caption.set(String::new());
            }
        }
    });

    let upload_ui_locale = ui_locale.clone();
    let upload_selected = move |_| {
        set_upload_error.set(None);
        let upload_ui_locale = upload_ui_locale.clone();
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
        set_busy_key.set(Some("upload".to_string()));
        spawn_local(async move {
            match read_selected_file(input).await {
                Ok(Some(file)) => {
                    match api::upload_media(
                        file.name,
                        file.content_type,
                        file.bytes,
                        token_value,
                        tenant_value,
                    )
                    .await
                    {
                        Ok(item) => {
                            set_selected_media_id.set(Some(item.id));
                            set_refresh_nonce.update(|value| *value += 1);
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
        let payload = UpsertTranslationPayload {
            locale: selected_locale.get_untracked(),
            title: non_empty_option(&title.get_untracked()),
            alt_text: non_empty_option(&alt_text.get_untracked()),
            caption: non_empty_option(&caption.get_untracked()),
        };
        set_busy_key.set(Some("translation".to_string()));
        spawn_local(async move {
            match api::upsert_translation(media_id, payload, token_value, tenant_value).await {
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
        let Some(media_id) = selected_media_id.get_untracked() else {
            return;
        };
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        set_busy_key.set(Some(format!("delete:{media_id}")));
        spawn_local(async move {
            match api::delete_media(media_id, token_value, tenant_value).await {
                Ok(true) => {
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
                            disabled=move || busy_key.get().as_deref() == Some("upload")
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
                                {move || page_label_template.replace("{count}", &page.get().to_string())}
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
                                                view! {
                                                    <button
                                                        type="button"
                                                        class="w-full rounded-xl border border-border px-4 py-3 text-left transition hover:border-primary/50 hover:bg-accent/40"
                                                        on:click=move |_| set_selected_media_id.set(Some(item_id.clone()))
                                                    >
                                                        <MediaListCard item=item />
                                                    </button>
                                                }
                                            }).collect_view()}
                                        </div>
                                    </div>
                                }.into_any(),
                                Err(err) => render_error_view(format!("{}: {err}", load_library_error)),
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
                                Err(err) => render_error_view(format!("{}: {err}", load_detail_error)),
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
                                on:input=move |ev| set_selected_locale.set(event_target_value(&ev))
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
                                                view! {
                                                    <button
                                                        type="button"
                                                        class="rounded-full border border-border px-3 py-1 text-xs text-muted-foreground"
                                                        on:click=move |_| set_selected_locale.set(locale.clone())
                                                    >
                                                        {locale_label}
                                                    </button>
                                                }
                                            }).collect_view()}
                                        </div>
                                    }.into_any(),
                                    Err(err) => render_error_view(format!("{}: {err}", load_translations_error)),
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
                                disabled=move || busy_key.get().as_deref() == Some("translation")
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
    let dimensions = item
        .width
        .zip(item.height)
        .map(|(width, height)| format!("{width}×{height}"))
        .unwrap_or_else(|| t(locale.as_deref(), "media.asset.notAvailable", "n/a"));
    view! {
        <div class="flex items-start justify-between gap-4">
            <div class="min-w-0 space-y-1">
                <div class="truncate text-sm font-semibold text-card-foreground">{item.original_name}</div>
                <div class="truncate text-xs text-muted-foreground">{item.public_url}</div>
                <div class="flex flex-wrap gap-2 text-xs text-muted-foreground">
                    <span>{item.mime_type}</span>
                    <span>{t(locale.as_deref(), "media.asset.bytes", "{count} bytes").replace("{count}", &item.size.to_string())}</span>
                    <span>{dimensions}</span>
                </div>
            </div>
            <span class="rounded-full border border-border px-2 py-1 text-[11px] text-muted-foreground">
                {item.storage_driver}
            </span>
        </div>
    }
}

#[component]
fn MediaDetailCard(item: MediaListItem) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    view! {
        <div class="space-y-3 text-sm">
            <DetailLine label=t(locale.as_deref(), "media.detail.originalName", "Original Name") value=item.original_name />
            <DetailLine label=t(locale.as_deref(), "media.detail.id", "ID") value=item.id />
            <DetailLine label=t(locale.as_deref(), "media.detail.mime", "MIME") value=item.mime_type />
            <DetailLine label=t(locale.as_deref(), "media.detail.storage", "Storage") value=item.storage_driver />
            <DetailLine label=t(locale.as_deref(), "media.detail.publicUrl", "Public URL") value=item.public_url />
            <DetailLine label=t(locale.as_deref(), "media.detail.size", "Size") value=t(locale.as_deref(), "media.asset.bytes", "{count} bytes").replace("{count}", &item.size.to_string()) />
            <DetailLine label=t(locale.as_deref(), "media.detail.created", "Created") value=item.created_at />
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
            view! {
                <section class="grid gap-4 md:grid-cols-3">
                    <StatCard label=t(locale.as_deref(), "media.usage.files", "Files") value=payload.file_count.to_string() />
                    <StatCard label=t(locale.as_deref(), "media.usage.totalBytes", "Total Bytes") value=payload.total_bytes.to_string() />
                    <StatCard label=t(locale.as_deref(), "media.usage.tenant", "Tenant") value=payload.tenant_id />
                </section>
            }
            .into_any()
        }
        Err(err) => render_error_view(format!("Failed to load media usage: {err}")),
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

fn non_empty_option(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
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
