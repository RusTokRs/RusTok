mod api;
mod i18n;
mod model;

use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_ui_routing::{use_route_query_value, use_route_query_writer};
use rustok_api::{AdminQueryKey, UiRouteContext};
use rustok_seo::SeoTargetKind;
use rustok_seo_admin_support::SeoEntityPanel;

use crate::i18n::t;
use crate::model::{CreatePageDraft, PageListItem};

#[component]
pub fn PagesAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let selected_page_query = use_route_query_value(AdminQueryKey::PageId.as_str());
    let query_writer = use_route_query_writer();
    let token = use_token();
    let tenant = use_tenant();
    let default_locale = route_context.locale.clone().unwrap_or_default();
    let badge_text = t(route_context.locale.as_deref(), "pages.badge", "pages");
    let title_text = t(
        route_context.locale.as_deref(),
        "pages.title",
        "Pages Builder",
    );
    let subtitle_text = t(
        route_context.locale.as_deref(),
        "pages.subtitle",
        "Canonical module-owned admin slice: list, create, edit, publish and delete pages through the pages module GraphQL contract.",
    );
    let list_title_text = t(route_context.locale.as_deref(), "pages.list.title", "Pages");
    let list_subtitle_text = t(
        route_context.locale.as_deref(),
        "pages.list.subtitle",
        "This list is loaded from the module package itself, not from apps/admin.",
    );
    let load_error_text = t(
        route_context.locale.as_deref(),
        "pages.error.load",
        "Failed to load pages",
    );
    let form_subtitle_text = t(
        route_context.locale.as_deref(),
        "pages.form.subtitle",
        "A standard module-owned CRUD form that lives entirely inside the package.",
    );
    let create_new_instead_text = t(
        route_context.locale.as_deref(),
        "pages.form.createNewInstead",
        "Create new instead",
    );
    let title_label = t(route_context.locale.as_deref(), "pages.form.title", "Title");
    let slug_label = t(route_context.locale.as_deref(), "pages.form.slug", "Slug");
    let locale_label = t(
        route_context.locale.as_deref(),
        "pages.form.locale",
        "Locale",
    );
    let channel_slugs_label = t(
        route_context.locale.as_deref(),
        "pages.form.channelSlugs",
        "Channel slugs",
    );
    let channel_slugs_placeholder = t(
        route_context.locale.as_deref(),
        "pages.form.channelSlugsPlaceholder",
        "web, mobile-app",
    );
    let channel_slugs_help = t(
        route_context.locale.as_deref(),
        "pages.form.channelSlugsHelp",
        "Comma-separated allowlist. Leave empty to publish on all channels.",
    );
    let body_label = t(route_context.locale.as_deref(), "pages.form.body", "Body");
    let publish_now_label = t(
        route_context.locale.as_deref(),
        "pages.form.publishNow",
        "Publish immediately",
    );

    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (editing_page_id, set_editing_page_id) = signal(Option::<String>::None);
    let (title, set_title) = signal(String::new());
    let (slug, set_slug) = signal(String::new());
    let (body, set_body) = signal(String::new());
    let (channel_slugs_text, set_channel_slugs_text) = signal(String::new());
    let (locale, set_locale) = signal(default_locale.clone());
    let (publish_now, set_publish_now) = signal(false);
    let (busy_key, set_busy_key) = signal(Option::<String>::None);
    let (submit_error, set_submit_error) = signal(Option::<String>::None);
    let reset_form_action = Callback::new({
        let default_locale = default_locale.clone();
        move |_| {
            reset_page_form(
                set_editing_page_id,
                set_title,
                set_slug,
                set_body,
                set_channel_slugs_text,
                set_locale,
                set_publish_now,
                default_locale.as_str(),
            )
        }
    });
    let editing_banner_text = Signal::derive({
        move || {
            editing_page_id
                .get()
                .map(|page_id| {
                    t(
                        locale.get().as_str().into(),
                        "pages.form.editingBanner",
                        "Editing page {id}",
                    )
                    .replace("{id}", page_id.as_str())
                })
                .unwrap_or_default()
        }
    });
    let reset_current_page = Callback::new({
        let query_writer = query_writer.clone();
        let reset_form_action = reset_form_action.clone();
        move |_| {
            query_writer.clear_key(AdminQueryKey::PageId.as_str());
            reset_form_action.run(());
        }
    });

    let pages_resource = Resource::new(
        move || (token.get(), tenant.get(), refresh_nonce.get()),
        move |(token_value, tenant_value, _)| async move {
            api::fetch_pages(token_value, tenant_value).await
        },
    );

    let edit_default_locale = default_locale.clone();
    let edit_page = Callback::new(move |page_id: String| {
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let default_locale = edit_default_locale.clone();
        set_submit_error.set(None);
        set_busy_key.set(Some(format!("edit:{page_id}")));

        spawn_local(async move {
            match api::fetch_page(token_value, tenant_value, page_id.clone()).await {
                Ok(Some(page)) => {
                    let page_locale = page
                        .translation
                        .as_ref()
                        .map(|translation| translation.locale.clone())
                        .or_else(|| page.body.as_ref().map(|page_body| page_body.locale.clone()))
                        .unwrap_or_else(|| default_locale.clone());
                    let page_title = page
                        .translation
                        .as_ref()
                        .and_then(|translation| translation.title.clone())
                        .unwrap_or_default();
                    let page_slug = page
                        .translation
                        .as_ref()
                        .and_then(|translation| translation.slug.clone())
                        .unwrap_or_default();
                    let page_body = page
                        .body
                        .map(|page_body| page_body.content)
                        .unwrap_or_default();
                    let page_channel_slugs = page.channel_slugs.join(", ");

                    set_editing_page_id.set(Some(page_id.clone()));
                    set_locale.set(page_locale);
                    set_title.set(page_title);
                    set_slug.set(page_slug);
                    set_body.set(page_body);
                    set_channel_slugs_text.set(page_channel_slugs);
                    set_publish_now.set(page.status.eq_ignore_ascii_case("published"));
                }
                Ok(None) => {
                    reset_page_form(
                        set_editing_page_id,
                        set_title,
                        set_slug,
                        set_body,
                        set_channel_slugs_text,
                        set_locale,
                        set_publish_now,
                        default_locale.as_str(),
                    );
                    set_submit_error.set(Some("Page not found for editing.".to_string()));
                }
                Err(err) => {
                    reset_page_form(
                        set_editing_page_id,
                        set_title,
                        set_slug,
                        set_body,
                        set_channel_slugs_text,
                        set_locale,
                        set_publish_now,
                        default_locale.as_str(),
                    );
                    set_submit_error.set(Some(format!("Failed to load page: {err}")));
                }
            }
            set_busy_key.set(None);
        });
    });
    let initial_edit_page = edit_page.clone();
    let effect_default_locale = default_locale.clone();
    Effect::new(move |_| match selected_page_query.get() {
        Some(page_id) if !page_id.trim().is_empty() => initial_edit_page.run(page_id),
        _ => reset_page_form(
            set_editing_page_id,
            set_title,
            set_slug,
            set_body,
            set_channel_slugs_text,
            set_locale,
            set_publish_now,
            effect_default_locale.as_str(),
        ),
    });

    let submit_query_writer = query_writer.clone();
    let submit_page = move |ev: SubmitEvent| {
        ev.prevent_default();
        set_submit_error.set(None);
        let submit_query_writer = submit_query_writer.clone();

        let draft = CreatePageDraft {
            locale: locale.get_untracked(),
            title: title.get_untracked().trim().to_string(),
            slug: slug.get_untracked().trim().to_string(),
            body: body.get_untracked().trim().to_string(),
            template: Some("default".to_string()),
            channel_slugs: parse_channel_slugs(&channel_slugs_text.get_untracked()),
            publish: publish_now.get_untracked(),
        };

        if draft.title.is_empty() || draft.slug.is_empty() || draft.body.is_empty() {
            set_submit_error.set(Some(
                "Title, slug and body are required to save a page.".to_string(),
            ));
            return;
        }

        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let editing_page = editing_page_id.get_untracked();
        set_busy_key.set(Some(if let Some(page_id) = editing_page.as_ref() {
            format!("save:{page_id}")
        } else {
            "create".to_string()
        }));

        spawn_local(async move {
            let result = match editing_page {
                Some(page_id) => api::update_page(token_value, tenant_value, page_id, draft).await,
                None => api::create_page(token_value, tenant_value, draft).await,
            };

            match result {
                Ok(page) => {
                    let status = page.status.to_lowercase();
                    let page_id = page.id;
                    set_editing_page_id.set(Some(page_id.clone()));
                    if status == "published" {
                        set_publish_now.set(true);
                    }
                    set_refresh_nonce.update(|value| *value += 1);
                    submit_query_writer.replace_value(AdminQueryKey::PageId.as_str(), page_id);
                }
                Err(err) => {
                    set_submit_error.set(Some(format!("Failed to save page: {err}")));
                }
            }

            set_busy_key.set(None);
        });
    };

    let publish_page = Callback::new(move |(page_id, publish): (String, bool)| {
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        set_submit_error.set(None);
        set_busy_key.set(Some(format!("publish:{page_id}")));

        spawn_local(async move {
            let result = if publish {
                api::publish_page(token_value, tenant_value, page_id.clone()).await
            } else {
                api::unpublish_page(token_value, tenant_value, page_id.clone()).await
            };

            match result {
                Ok(page) => {
                    if editing_page_id.get_untracked().as_deref() == Some(page.id.as_str()) {
                        set_publish_now.set(page.status.eq_ignore_ascii_case("published"));
                    }
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => {
                    set_submit_error.set(Some(format!("Failed to update page status: {err}")));
                }
            }

            set_busy_key.set(None);
        });
    });

    let delete_query_writer = query_writer.clone();
    let delete_page = Callback::new(move |page_id: String| {
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let delete_query_writer = delete_query_writer.clone();
        set_submit_error.set(None);
        set_busy_key.set(Some(format!("delete:{page_id}")));

        spawn_local(async move {
            match api::delete_page(token_value, tenant_value, page_id.clone()).await {
                Ok(true) => {
                    if editing_page_id.get_untracked().as_deref() == Some(page_id.as_str()) {
                        delete_query_writer.clear_key(AdminQueryKey::PageId.as_str());
                        reset_form_action.run(());
                    }
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Ok(false) => {
                    set_submit_error.set(Some("Delete page returned false.".to_string()));
                }
                Err(err) => {
                    set_submit_error.set(Some(format!("Failed to delete page: {err}")));
                }
            }
            set_busy_key.set(None);
        });
    });
    let open_query_writer = query_writer.clone();
    let open_page = Callback::new(move |page_id: String| {
        open_query_writer.push_value(AdminQueryKey::PageId.as_str(), page_id);
    });

    view! {
        <div class="space-y-6">
            <header class="flex flex-col gap-4 rounded-2xl border border-border bg-card p-6 shadow-sm lg:flex-row lg:items-start lg:justify-between">
                <div class="space-y-2">
                    <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground">
                        {badge_text.clone()}
                    </span>
                    <h1 class="text-2xl font-semibold text-card-foreground">
                        {title_text.clone()}
                    </h1>
                    <p class="max-w-2xl text-sm text-muted-foreground">
                        {subtitle_text.clone()}
                    </p>
                </div>
            </header>

            <section class="grid gap-6 xl:grid-cols-[minmax(0,1fr)_24rem]">
                <div class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                    <div class="mb-4 flex items-start justify-between gap-4">
                        <div>
                            <h2 class="text-lg font-semibold text-card-foreground">
                                {list_title_text.clone()}
                            </h2>
                            <p class="text-sm text-muted-foreground">
                                {list_subtitle_text.clone()}
                            </p>
                        </div>
                    </div>

                    <Suspense
                        fallback=move || view! {
                            <div class="space-y-2">
                                {(0..4).map(|_| view! {
                                    <div class="h-14 animate-pulse rounded-xl bg-muted"></div>
                                }).collect_view()}
                            </div>
                        }
                    >
                        {move || {
                            pages_resource.get().map(|result| {
                                match result {
                                    Ok(page_list) => view! {
                                        <PagesTable
                                            items=page_list.items
                                            total=page_list.total
                                            editing_page_id=editing_page_id.get()
                                            busy_key=busy_key.get()
                                            on_edit=open_page
                                            on_toggle_publish=publish_page
                                            on_delete=delete_page
                                        />
                                    }.into_any(),
                                    Err(err) => view! {
                                        <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                                            {format!(
                                                "{}: {err}",
                                                load_error_text.clone()
                                            )}
                                        </div>
                                    }.into_any(),
                                }
                            })
                        }}
                    </Suspense>
                </div>

                <div class="space-y-6">
                <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                    <div class="space-y-1">
                        <h2 class="text-lg font-semibold text-card-foreground">
                            {move || {
                                if editing_page_id.get().is_some() {
                                    t(locale.get().as_str().into(), "pages.form.editTitle", "Edit page")
                                } else {
                                    t(
                                        locale.get().as_str().into(),
                                        "pages.form.createTitle",
                                        "Create page",
                                    )
                                }
                            }}
                        </h2>
                        <p class="text-sm text-muted-foreground">
                            {form_subtitle_text.clone()}
                        </p>
                    </div>

                    <Show when=move || editing_page_id.get().is_some()>
                        <PagesEditBanner
                            banner_text=editing_banner_text
                            create_new_label=create_new_instead_text.clone()
                            on_reset=reset_current_page
                        />
                    </Show>

                    <form class="mt-5 space-y-4" on:submit=submit_page>
                        <label class="block space-y-2">
                            <span class="text-sm font-medium text-card-foreground">
                                {title_label.clone()}
                            </span>
                            <input
                                type="text"
                                class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                prop:value=title
                                on:input=move |ev| {
                                    let value = event_target_value(&ev);
                                    if slug.get_untracked().trim().is_empty() {
                                        set_slug.set(slugify(value.as_str()));
                                    }
                                    set_title.set(value);
                                }
                            />
                        </label>

                        <label class="block space-y-2">
                            <span class="text-sm font-medium text-card-foreground">
                                {slug_label.clone()}
                            </span>
                            <input
                                type="text"
                                class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                prop:value=slug
                                on:input=move |ev| set_slug.set(event_target_value(&ev))
                            />
                        </label>

                        <label class="block space-y-2">
                            <span class="text-sm font-medium text-card-foreground">
                                {locale_label.clone()}
                            </span>
                            <input
                                type="text"
                                class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                prop:value=locale
                                on:input=move |ev| set_locale.set(event_target_value(&ev))
                            />
                        </label>

                        <label class="block space-y-2">
                            <span class="text-sm font-medium text-card-foreground">
                                {channel_slugs_label.clone()}
                            </span>
                            <input
                                type="text"
                                class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                placeholder=channel_slugs_placeholder.clone()
                                prop:value=channel_slugs_text
                                on:input=move |ev| set_channel_slugs_text.set(event_target_value(&ev))
                            />
                            <span class="block text-xs text-muted-foreground">
                                {channel_slugs_help.clone()}
                            </span>
                        </label>

                        <label class="block space-y-2">
                            <span class="text-sm font-medium text-card-foreground">
                                {body_label.clone()}
                            </span>
                            <textarea
                                class="min-h-40 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                prop:value=body
                                on:input=move |ev| set_body.set(event_target_value(&ev))
                            />
                        </label>

                        <label class="flex items-center gap-2 text-sm text-card-foreground">
                            <input
                                type="checkbox"
                                prop:checked=publish_now
                                on:change=move |ev| set_publish_now.set(event_target_checked(&ev))
                            />
                            {publish_now_label.clone()}
                        </label>

                        <Show when=move || submit_error.get().is_some()>
                            <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                                {move || submit_error.get().unwrap_or_default()}
                            </div>
                        </Show>

                        <button
                            type="submit"
                            class="inline-flex w-full items-center justify-center rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50"
                            disabled=move || {
                                busy_key.get().as_deref() == Some("create")
                                    || busy_key
                                        .get()
                                        .as_deref()
                                        .map(|key| key.starts_with("save:"))
                                        .unwrap_or(false)
                            }
                        >
                            {move || {
                                if busy_key.get().as_deref() == Some("create")
                                    || busy_key
                                        .get()
                                        .as_deref()
                                        .map(|key| key.starts_with("save:"))
                                        .unwrap_or(false)
                                {
                                    t(locale.get().as_str().into(), "pages.form.saving", "Saving...")
                                } else if editing_page_id.get().is_some() {
                                    t(locale.get().as_str().into(), "pages.form.update", "Update page")
                                } else {
                                    t(locale.get().as_str().into(), "pages.form.create", "Create page")
                                }
                            }}
                        </button>
                    </form>
                </section>

                <SeoEntityPanel
                    target_kind=SeoTargetKind::Page
                    target_id=Signal::derive(move || editing_page_id.get())
                    locale=Signal::derive(move || locale.get())
                    panel_title=t(locale.get().as_str().into(), "pages.seo.title", "Page SEO")
                    panel_subtitle=t(
                        locale.get().as_str().into(),
                        "pages.seo.subtitle",
                        "Explicit metadata, social tags and diagnostics for the selected page.",
                    )
                    empty_message=t(
                        locale.get().as_str().into(),
                        "pages.seo.empty",
                        "Create or open a page first. The SEO panel stays attached to the page editor instead of a central hub.",
                    )
                />
                </div>
            </section>
        </div>
    }
}

#[component]
fn PagesEditBanner(
    banner_text: Signal<String>,
    create_new_label: String,
    on_reset: Callback<()>,
) -> impl IntoView {
    view! {
        <div class="mt-4 flex items-center justify-between gap-3 rounded-xl border border-border bg-muted/30 px-4 py-3">
            <div class="text-sm text-muted-foreground">
                {move || banner_text.get()}
            </div>
            <button
                type="button"
                class="text-xs font-medium text-primary hover:underline"
                on:click=move |_| on_reset.run(())
            >
                {create_new_label}
            </button>
        </div>
    }
}

#[component]
fn PagesTable(
    items: Vec<PageListItem>,
    total: u64,
    editing_page_id: Option<String>,
    busy_key: Option<String>,
    on_edit: Callback<String>,
    on_toggle_publish: Callback<(String, bool)>,
    on_delete: Callback<String>,
) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    if items.is_empty() {
        return view! {
            <div class="rounded-xl border border-dashed border-border p-12 text-center">
                <p class="text-sm text-muted-foreground">
                    {t(
                        locale.as_deref(),
                        "pages.table.empty",
                        "No pages yet. Create the first one from the module package form.",
                    )}
                </p>
            </div>
        }
        .into_any();
    }

    view! {
        <div class="space-y-4">
            <div class="text-sm text-muted-foreground">
                {t(locale.as_deref(), "pages.table.total", "{count} page(s)")
                    .replace("{count}", &total.to_string())}
            </div>
            <div class="overflow-hidden rounded-xl border border-border">
                <table class="w-full text-sm">
                    <thead class="border-b border-border bg-muted/50">
                        <tr>
                            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale.as_deref(), "pages.table.title", "Title")}</th>
                            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale.as_deref(), "pages.table.slug", "Slug")}</th>
                            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale.as_deref(), "pages.table.status", "Status")}</th>
                            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale.as_deref(), "pages.table.updated", "Updated")}</th>
                            <th class="px-4 py-3"></th>
                        </tr>
                    </thead>
                    <tbody class="divide-y divide-border">
                        {items
                            .into_iter()
                            .map(|page| {
                                let page_id = page.id.clone();
                                let is_editing =
                                    editing_page_id.as_deref() == Some(page_id.as_str());
                                let is_published = page.status.eq_ignore_ascii_case("published");
                                let is_busy = busy_key
                                    .as_deref()
                                    .map(|key| key.ends_with(page_id.as_str()))
                                    .unwrap_or(false);

                                view! {
                                    <tr class=("bg-primary/5", is_editing)>
                                        <td class="px-4 py-3">
                                            <div class="font-medium text-foreground">
                                                {page.title.unwrap_or_else(|| {
                                                    t(locale.as_deref(), "pages.table.untitled", "Untitled page")
                                                })}
                                            </div>
                                            <div class="text-xs text-muted-foreground">{page.template}</div>
                                        </td>
                                        <td class="px-4 py-3 text-muted-foreground">
                                            {page.slug.unwrap_or_else(|| "-".to_string())}
                                        </td>
                                        <td class="px-4 py-3">
                                            <StatusBadge status=page.status.clone() />
                                        </td>
                                        <td class="px-4 py-3 text-xs text-muted-foreground">{page.updated_at}</td>
                                        <td class="px-4 py-3">
                                            <div class="flex justify-end gap-2">
                                                <button
                                                    class="rounded-lg border border-border px-3 py-1 text-xs font-medium text-foreground transition hover:bg-accent hover:text-accent-foreground disabled:opacity-50"
                                                    disabled=is_busy
                                                    on:click={
                                                        let page_id = page.id.clone();
                                                        move |_| on_edit.run(page_id.clone())
                                                    }
                                                >
                                                    {if is_busy && busy_key
                                                        .as_deref()
                                                        .map(|key| key.starts_with("edit:"))
                                                        .unwrap_or(false)
                                                    {
                                                        "...".to_string()
                                                    } else if is_editing {
                                                        t(locale.as_deref(), "pages.table.editing", "Editing")
                                                    } else {
                                                        t(locale.as_deref(), "pages.table.edit", "Edit")
                                                    }}
                                                </button>
                                                <button
                                                    class="rounded-lg border border-border px-3 py-1 text-xs font-medium text-foreground transition hover:bg-accent hover:text-accent-foreground disabled:opacity-50"
                                                    disabled=is_busy
                                                    on:click={
                                                        let page_id = page.id.clone();
                                                        move |_| on_toggle_publish.run((page_id.clone(), !is_published))
                                                    }
                                                >
                                                    {if is_busy && busy_key
                                                        .as_deref()
                                                        .map(|key| key.starts_with("publish:"))
                                                        .unwrap_or(false)
                                                    {
                                                        "...".to_string()
                                                    } else if is_published {
                                                        t(locale.as_deref(), "pages.table.unpublish", "Unpublish")
                                                    } else {
                                                        t(locale.as_deref(), "pages.table.publish", "Publish")
                                                    }}
                                                </button>
                                                <button
                                                    class="rounded-lg border border-destructive/30 px-3 py-1 text-xs font-medium text-destructive transition hover:bg-destructive/10 disabled:opacity-50"
                                                    disabled=is_busy
                                                    on:click={
                                                        let page_id = page.id.clone();
                                                        move |_| on_delete.run(page_id.clone())
                                                    }
                                                >
                                                    {if is_busy && busy_key
                                                        .as_deref()
                                                        .map(|key| key.starts_with("delete:"))
                                                        .unwrap_or(false)
                                                    {
                                                        "...".to_string()
                                                    } else {
                                                        t(locale.as_deref(), "pages.table.delete", "Delete")
                                                    }}
                                                </button>
                                            </div>
                                        </td>
                                    </tr>
                                }
                            })
                            .collect_view()}
                    </tbody>
                </table>
            </div>
        </div>
    }
    .into_any()
}

#[component]
fn StatusBadge(status: String) -> impl IntoView {
    let normalized = status.to_lowercase();
    let class_name = match normalized.as_str() {
        "published" => {
            "bg-emerald-50 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400"
        }
        "archived" => "bg-muted text-muted-foreground",
        _ => "bg-primary/10 text-primary",
    };

    view! {
        <span class=format!("inline-flex rounded-full px-2.5 py-0.5 text-xs font-semibold {class_name}")>
            {status}
        </span>
    }
}

fn slugify(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn parse_channel_slugs(value: &str) -> Vec<String> {
    let mut items = value
        .split(',')
        .map(|item| item.trim().to_ascii_lowercase())
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();
    items.sort();
    items.dedup();
    items
}

fn reset_page_form(
    set_editing_page_id: WriteSignal<Option<String>>,
    set_title: WriteSignal<String>,
    set_slug: WriteSignal<String>,
    set_body: WriteSignal<String>,
    set_channel_slugs_text: WriteSignal<String>,
    set_locale: WriteSignal<String>,
    set_publish_now: WriteSignal<bool>,
    default_locale: &str,
) {
    set_editing_page_id.set(None);
    set_title.set(String::new());
    set_slug.set(String::new());
    set_body.set(String::new());
    set_channel_slugs_text.set(String::new());
    set_locale.set(default_locale.to_string());
    set_publish_now.set(false);
}
