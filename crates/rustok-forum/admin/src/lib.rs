mod api;
mod model;

use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use rustok_api::UiRouteContext;

use crate::model::{
    CategoryDetail, CategoryDraft, CategoryListItem, ReplyListItem, TopicDetail, TopicDraft,
    TopicListItem,
};

#[component]
pub fn ForumAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let token = use_token();
    let tenant = use_tenant();
    let default_locale = route_context
        .locale
        .clone()
        .unwrap_or_else(|| "en".to_string());
    let is_categories_page = route_context.subpath_matches("categories");

    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (error, set_error) = signal(Option::<String>::None);
    let (busy_key, set_busy_key) = signal(Option::<String>::None);

    let (editing_category_id, set_editing_category_id) = signal(Option::<String>::None);
    let (category_locale, set_category_locale) = signal(default_locale.clone());
    let (category_name, set_category_name) = signal(String::new());
    let (category_slug, set_category_slug) = signal(String::new());
    let (category_description, set_category_description) = signal(String::new());
    let (category_icon, set_category_icon) = signal(String::new());
    let (category_color, set_category_color) = signal(String::new());
    let (category_position, set_category_position) = signal(0_i32);
    let (category_moderated, set_category_moderated) = signal(false);

    let (editing_topic_id, set_editing_topic_id) = signal(Option::<String>::None);
    let (topic_locale, set_topic_locale) = signal(default_locale);
    let (topic_category_id, set_topic_category_id) = signal(String::new());
    let (topic_title, set_topic_title) = signal(String::new());
    let (topic_slug, set_topic_slug) = signal(String::new());
    let (topic_body, set_topic_body) = signal(String::new());
    let (topic_body_format, set_topic_body_format) = signal("markdown".to_string());
    let (topic_tags, set_topic_tags) = signal(String::new());
    let (topic_filter_category_id, set_topic_filter_category_id) = signal(String::new());

    let categories = Resource::new(
        move || {
            (
                token.get(),
                tenant.get(),
                refresh_nonce.get(),
                category_locale.get(),
            )
        },
        move |(token_value, tenant_value, _, locale)| async move {
            api::fetch_categories(token_value, tenant_value, locale).await
        },
    );

    let topics = Resource::new(
        move || {
            (
                token.get(),
                tenant.get(),
                refresh_nonce.get(),
                topic_locale.get(),
                topic_filter_category_id.get(),
            )
        },
        move |(token_value, tenant_value, _, locale, category_id)| async move {
            api::fetch_topics(
                token_value,
                tenant_value,
                locale,
                (!category_id.trim().is_empty()).then_some(category_id),
            )
            .await
        },
    );

    let replies = Resource::new(
        move || {
            (
                token.get(),
                tenant.get(),
                refresh_nonce.get(),
                topic_locale.get(),
                editing_topic_id.get(),
            )
        },
        move |(token_value, tenant_value, _, locale, topic_id)| async move {
            match topic_id {
                Some(topic_id) => {
                    api::fetch_replies(token_value, tenant_value, topic_id, locale).await
                }
                None => Ok(Vec::new()),
            }
        },
    );

    let reset_category_form = move || {
        set_editing_category_id.set(None);
        set_category_name.set(String::new());
        set_category_slug.set(String::new());
        set_category_description.set(String::new());
        set_category_icon.set(String::new());
        set_category_color.set(String::new());
        set_category_position.set(0);
        set_category_moderated.set(false);
    };

    let reset_topic_form = move || {
        set_editing_topic_id.set(None);
        set_topic_category_id.set(String::new());
        set_topic_title.set(String::new());
        set_topic_slug.set(String::new());
        set_topic_body.set(String::new());
        set_topic_body_format.set("markdown".to_string());
        set_topic_tags.set(String::new());
    };

    let edit_category = Callback::new(move |category_id: String| {
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let locale = category_locale.get_untracked();
        set_error.set(None);
        set_busy_key.set(Some(format!("category:edit:{category_id}")));
        spawn_local(async move {
            match api::fetch_category(token_value, tenant_value, category_id.clone(), locale).await
            {
                Ok(category) => apply_category_to_form(
                    set_editing_category_id,
                    set_category_locale,
                    set_category_name,
                    set_category_slug,
                    set_category_description,
                    set_category_icon,
                    set_category_color,
                    set_category_position,
                    set_category_moderated,
                    &category,
                ),
                Err(err) => set_error.set(Some(format!("Failed to load category: {err}"))),
            }
            set_busy_key.set(None);
        });
    });

    let edit_topic = Callback::new(move |topic_id: String| {
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let locale = topic_locale.get_untracked();
        set_error.set(None);
        set_busy_key.set(Some(format!("topic:edit:{topic_id}")));
        spawn_local(async move {
            match api::fetch_topic(token_value, tenant_value, topic_id.clone(), locale).await {
                Ok(topic) => apply_topic_to_form(
                    set_editing_topic_id,
                    set_topic_locale,
                    set_topic_category_id,
                    set_topic_title,
                    set_topic_slug,
                    set_topic_body,
                    set_topic_body_format,
                    set_topic_tags,
                    &topic,
                ),
                Err(err) => set_error.set(Some(format!("Failed to load topic: {err}"))),
            }
            set_busy_key.set(None);
        });
    });

    let submit_category = move |ev: SubmitEvent| {
        ev.prevent_default();
        set_error.set(None);
        let draft = CategoryDraft {
            locale: category_locale.get_untracked(),
            name: category_name.get_untracked().trim().to_string(),
            slug: category_slug.get_untracked().trim().to_string(),
            description: category_description.get_untracked().trim().to_string(),
            icon: category_icon.get_untracked().trim().to_string(),
            color: category_color.get_untracked().trim().to_string(),
            position: category_position.get_untracked(),
            moderated: category_moderated.get_untracked(),
        };
        if draft.name.is_empty() || draft.slug.is_empty() {
            set_error.set(Some("Category name and slug are required.".to_string()));
            return;
        }
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let editing_id = editing_category_id.get_untracked();
        set_busy_key.set(Some("category:save".to_string()));
        spawn_local(async move {
            let result = match editing_id {
                Some(id) => api::update_category(token_value, tenant_value, id, draft).await,
                None => api::create_category(token_value, tenant_value, draft).await,
            };
            match result {
                Ok(category) => {
                    apply_category_to_form(
                        set_editing_category_id,
                        set_category_locale,
                        set_category_name,
                        set_category_slug,
                        set_category_description,
                        set_category_icon,
                        set_category_color,
                        set_category_position,
                        set_category_moderated,
                        &category,
                    );
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_error.set(Some(format!("Failed to save category: {err}"))),
            }
            set_busy_key.set(None);
        });
    };

    let submit_topic = move |ev: SubmitEvent| {
        ev.prevent_default();
        set_error.set(None);
        let draft = TopicDraft {
            locale: topic_locale.get_untracked(),
            category_id: topic_category_id.get_untracked().trim().to_string(),
            title: topic_title.get_untracked().trim().to_string(),
            slug: topic_slug.get_untracked().trim().to_string(),
            body: topic_body.get_untracked().trim().to_string(),
            body_format: topic_body_format.get_untracked().trim().to_string(),
            tags: parse_tags(topic_tags.get_untracked().as_str()),
        };
        if draft.category_id.is_empty() || draft.title.is_empty() || draft.body.is_empty() {
            set_error.set(Some("Category, title and body are required.".to_string()));
            return;
        }
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let editing_id = editing_topic_id.get_untracked();
        set_busy_key.set(Some("topic:save".to_string()));
        spawn_local(async move {
            let result = match editing_id {
                Some(id) => api::update_topic(token_value, tenant_value, id, draft).await,
                None => api::create_topic(token_value, tenant_value, draft).await,
            };
            match result {
                Ok(topic) => {
                    apply_topic_to_form(
                        set_editing_topic_id,
                        set_topic_locale,
                        set_topic_category_id,
                        set_topic_title,
                        set_topic_slug,
                        set_topic_body,
                        set_topic_body_format,
                        set_topic_tags,
                        &topic,
                    );
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_error.set(Some(format!("Failed to save topic: {err}"))),
            }
            set_busy_key.set(None);
        });
    };

    let delete_category = Callback::new(move |category_id: String| {
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        set_error.set(None);
        set_busy_key.set(Some(format!("category:delete:{category_id}")));
        spawn_local(async move {
            match api::delete_category(token_value, tenant_value, category_id.clone()).await {
                Ok(()) => {
                    if editing_category_id.get_untracked().as_deref() == Some(category_id.as_str())
                    {
                        reset_category_form();
                    }
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_error.set(Some(format!("Failed to delete category: {err}"))),
            }
            set_busy_key.set(None);
        });
    });

    let delete_topic = Callback::new(move |topic_id: String| {
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        set_error.set(None);
        set_busy_key.set(Some(format!("topic:delete:{topic_id}")));
        spawn_local(async move {
            match api::delete_topic(token_value, tenant_value, topic_id.clone()).await {
                Ok(()) => {
                    if editing_topic_id.get_untracked().as_deref() == Some(topic_id.as_str()) {
                        reset_topic_form();
                    }
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_error.set(Some(format!("Failed to delete topic: {err}"))),
            }
            set_busy_key.set(None);
        });
    });

    let topic_count = move || match topics.get() {
        Some(Ok(items)) => items.len(),
        _ => 0,
    };
    let category_count = move || match categories.get() {
        Some(Ok(items)) => items.len(),
        _ => 0,
    };
    let reply_preview_count = move || match replies.get() {
        Some(Ok(items)) => items.len(),
        _ => 0,
    };

    view! {
        <div class="space-y-6">
            <header class="overflow-hidden rounded-[2rem] border border-border bg-gradient-to-br from-card via-card to-muted/40 shadow-sm">
                <div class="grid gap-6 px-6 py-7 lg:grid-cols-[minmax(0,1.5fr)_minmax(0,1fr)] lg:px-8">
                    <div class="space-y-4">
                        <div class="inline-flex items-center gap-2 rounded-full border border-border/70 bg-background/80 px-3 py-1 text-[11px] font-semibold uppercase tracking-[0.26em] text-muted-foreground">
                            <span class="h-2 w-2 rounded-full bg-amber-500"></span>
                            "forum control room"
                        </div>
                        <div class="space-y-2">
                            <h1 class="text-3xl font-semibold tracking-tight text-card-foreground">
                                {move || {
                                    if is_categories_page {
                                        "Category architecture"
                                    } else {
                                        "NodeBB-style moderation workspace"
                                    }
                                }}
                            </h1>
                            <p class="max-w-2xl text-sm leading-6 text-muted-foreground">
                                {move || {
                                    if is_categories_page {
                                        "Shape navigation clusters, assign moderation rules, and keep every forum area ready for new threads."
                                    } else {
                                        "Review topic flow, open a thread for reply preview, and keep publishing controls next to the live feed."
                                    }
                                }}
                            </p>
                        </div>
                    </div>
                    <div class="grid gap-3 sm:grid-cols-3 lg:grid-cols-1 xl:grid-cols-3">
                        <MetricCard
                            label="Categories"
                            value=Signal::derive(move || format_count(category_count()))
                            accent_class="bg-sky-500"
                        />
                        <MetricCard
                            label="Topics"
                            value=Signal::derive(move || format_count(topic_count()))
                            accent_class="bg-amber-500"
                        />
                        <MetricCard
                            label="Reply preview"
                            value=Signal::derive(move || format_count(reply_preview_count()))
                            accent_class="bg-emerald-500"
                        />
                    </div>
                </div>
            </header>

            {move || error.get().map(|value| view! {
                <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{value}</div>
            })}

            {if is_categories_page {
                view! {
                    <CategoriesPage
                        categories=categories
                        busy_key=busy_key
                        editing_id=editing_category_id
                        locale=category_locale
                        set_locale=set_category_locale
                        name=category_name
                        set_name=set_category_name
                        slug=category_slug
                        set_slug=set_category_slug
                        description=category_description
                        set_description=set_category_description
                        icon=category_icon
                        set_icon=set_category_icon
                        color=category_color
                        set_color=set_category_color
                        position=category_position
                        set_position=set_category_position
                        moderated=category_moderated
                        set_moderated=set_category_moderated
                        on_edit=edit_category
                        on_delete=delete_category
                        on_submit=submit_category
                        on_reset=Callback::new(move |_| reset_category_form())
                    />
                }.into_any()
            } else {
                view! {
                    <TopicsPage
                        categories=categories
                        topics=topics
                        replies=replies
                        busy_key=busy_key
                        editing_id=editing_topic_id
                        locale=topic_locale
                        set_locale=set_topic_locale
                        category_id=topic_category_id
                        set_category_id=set_topic_category_id
                        title=topic_title
                        set_title=set_topic_title
                        slug=topic_slug
                        set_slug=set_topic_slug
                        body=topic_body
                        set_body=set_topic_body
                        body_format=topic_body_format
                        set_body_format=set_topic_body_format
                        tags=topic_tags
                        set_tags=set_topic_tags
                        filter_category_id=topic_filter_category_id
                        set_filter_category_id=set_topic_filter_category_id
                        on_edit=edit_topic
                        on_delete=delete_topic
                        on_submit=submit_topic
                        on_reset=Callback::new(move |_| reset_topic_form())
                    />
                }.into_any()
            }}
        </div>
    }
}

#[component]
fn MetricCard(
    label: &'static str,
    value: Signal<String>,
    accent_class: &'static str,
) -> impl IntoView {
    view! {
        <article class="rounded-[1.5rem] border border-border/80 bg-background/80 p-4 backdrop-blur">
            <div class="flex items-center gap-3">
                <span class=format!("h-3 w-3 rounded-full {}", accent_class)></span>
                <span class="text-xs font-semibold uppercase tracking-[0.22em] text-muted-foreground">
                    {label}
                </span>
            </div>
            <div class="mt-4 text-2xl font-semibold text-foreground">{move || value.get()}</div>
        </article>
    }
}

#[component]
fn InsightTile(title: &'static str, body: &'static str) -> impl IntoView {
    view! {
        <article class="rounded-[1.35rem] border border-border bg-background/80 p-4">
            <h3 class="text-sm font-semibold text-foreground">{title}</h3>
            <p class="mt-2 text-sm leading-6 text-muted-foreground">{body}</p>
        </article>
    }
}

#[component]
fn FieldShell(label: &'static str, hint: &'static str, children: Children) -> impl IntoView {
    view! {
        <label class="block space-y-2">
            <span class="block text-sm font-medium text-foreground">{label}</span>
            <span class="block text-xs leading-5 text-muted-foreground">{hint}</span>
            {children()}
        </label>
    }
}

#[component]
fn SidebarStat(label: &'static str, value: Signal<String>) -> impl IntoView {
    view! {
        <div class="rounded-2xl border border-border bg-card px-4 py-3">
            <p class="text-[11px] font-semibold uppercase tracking-[0.22em] text-muted-foreground">
                {label}
            </p>
            <p class="mt-2 text-sm font-medium text-foreground">{move || value.get()}</p>
        </div>
    }
}

#[component]
fn CountChip(label: &'static str, value: i32) -> impl IntoView {
    view! {
        <span class="rounded-full bg-muted px-2.5 py-1 text-xs font-medium text-muted-foreground">
            {format!("{label}: {value}")}
        </span>
    }
}

#[component]
fn CategoriesPage(
    categories: Resource<Result<Vec<CategoryListItem>, String>>,
    busy_key: ReadSignal<Option<String>>,
    editing_id: ReadSignal<Option<String>>,
    locale: ReadSignal<String>,
    set_locale: WriteSignal<String>,
    name: ReadSignal<String>,
    set_name: WriteSignal<String>,
    slug: ReadSignal<String>,
    set_slug: WriteSignal<String>,
    description: ReadSignal<String>,
    set_description: WriteSignal<String>,
    icon: ReadSignal<String>,
    set_icon: WriteSignal<String>,
    color: ReadSignal<String>,
    set_color: WriteSignal<String>,
    position: ReadSignal<i32>,
    set_position: WriteSignal<i32>,
    moderated: ReadSignal<bool>,
    set_moderated: WriteSignal<bool>,
    on_edit: Callback<String>,
    on_delete: Callback<String>,
    on_submit: impl Fn(SubmitEvent) + 'static + Copy,
    on_reset: Callback<()>,
) -> impl IntoView {
    view! {
        <section class="grid gap-6 xl:grid-cols-[minmax(0,1.45fr)_24rem]">
            <div class="space-y-6">
                <section class="rounded-[1.75rem] border border-border bg-card p-6 shadow-sm">
                    <div class="flex flex-wrap items-center justify-between gap-4">
                        <div>
                            <p class="text-xs font-semibold uppercase tracking-[0.24em] text-muted-foreground">
                                "Category matrix"
                            </p>
                            <h2 class="mt-2 text-2xl font-semibold text-card-foreground">
                                "Forum sections"
                            </h2>
                        </div>
                        <button
                            type="button"
                            class="rounded-full border border-border bg-background px-4 py-2 text-sm font-medium text-foreground transition hover:bg-muted"
                            on:click=move |_| on_reset.run(())
                        >
                            "New category"
                        </button>
                    </div>
                    <p class="mt-3 max-w-2xl text-sm leading-6 text-muted-foreground">
                        "This view keeps category hierarchy, counts, and moderation switches close together so moderators can shape the forum like a community map instead of a plain CRUD table."
                    </p>
                    <Suspense fallback=move || view! { <div class="mt-6 h-48 animate-pulse rounded-[1.5rem] bg-muted"></div> }>
                        {move || categories.get().map(|result| render_category_grid(result, editing_id.get(), busy_key.get(), on_edit, on_delete))}
                    </Suspense>
                </section>

                <section class="rounded-[1.75rem] border border-border bg-gradient-to-br from-card via-card to-muted/30 p-6 shadow-sm">
                    <p class="text-xs font-semibold uppercase tracking-[0.24em] text-muted-foreground">
                        "Moderator notes"
                    </p>
                    <div class="mt-4 grid gap-4 md:grid-cols-3">
                        <InsightTile
                            title="Icon + color"
                            body="Use both so each category reads like a quick visual stop in the sidebar."
                        />
                        <InsightTile
                            title="Position"
                            body="Lower numbers bubble important sections to the top of the community map."
                        />
                        <InsightTile
                            title="Moderated"
                            body="Turn this on for queues that need stricter review before topics go live."
                        />
                    </div>
                </section>
            </div>

            <section class="rounded-[1.75rem] border border-border bg-card p-6 shadow-sm xl:sticky xl:top-6 xl:self-start">
                <div class="flex items-center justify-between gap-3">
                    <div>
                        <p class="text-xs font-semibold uppercase tracking-[0.24em] text-muted-foreground">
                            "Composer"
                        </p>
                        <h2 class="mt-2 text-xl font-semibold text-card-foreground">
                            {move || if editing_id.get().is_some() { "Edit category" } else { "Create category" }}
                        </h2>
                    </div>
                    {move || editing_id.get().map(|_| view! {
                        <span class="rounded-full bg-amber-500/15 px-3 py-1 text-xs font-medium text-amber-700 dark:text-amber-300">
                            "Live edit"
                        </span>
                    })}
                </div>
                <form class="mt-6 space-y-4" on:submit=on_submit>
                    <FieldShell label="Locale" hint="Published locale for this category.">
                        <input
                            class="w-full rounded-2xl border border-border bg-background px-4 py-3 text-sm outline-none transition focus:border-primary"
                            prop:value=move || locale.get()
                            on:input=move |ev| set_locale.set(event_target_value(&ev))
                            placeholder="en"
                        />
                    </FieldShell>
                    <FieldShell label="Name" hint="Human-friendly label shown in the admin and forum nav.">
                        <input
                            class="w-full rounded-2xl border border-border bg-background px-4 py-3 text-sm outline-none transition focus:border-primary"
                            prop:value=move || name.get()
                            on:input=move |ev| set_name.set(event_target_value(&ev))
                            placeholder="General discussion"
                        />
                    </FieldShell>
                    <FieldShell label="Slug" hint="Stable identifier for routing and lookups.">
                        <input
                            class="w-full rounded-2xl border border-border bg-background px-4 py-3 font-mono text-sm outline-none transition focus:border-primary"
                            prop:value=move || slug.get()
                            on:input=move |ev| set_slug.set(event_target_value(&ev))
                            placeholder="general-discussion"
                        />
                    </FieldShell>
                    <FieldShell label="Description" hint="Short community-facing summary.">
                        <textarea
                            class="min-h-24 w-full rounded-2xl border border-border bg-background px-4 py-3 text-sm outline-none transition focus:border-primary"
                            prop:value=move || description.get()
                            on:input=move |ev| set_description.set(event_target_value(&ev))
                            placeholder="Space for announcements, introductions, and open questions."
                        ></textarea>
                    </FieldShell>
                    <div class="grid gap-4 sm:grid-cols-2">
                        <FieldShell label="Icon" hint="Optional short token or icon name.">
                            <input
                                class="w-full rounded-2xl border border-border bg-background px-4 py-3 text-sm outline-none transition focus:border-primary"
                                prop:value=move || icon.get()
                                on:input=move |ev| set_icon.set(event_target_value(&ev))
                                placeholder="chat"
                            />
                        </FieldShell>
                        <FieldShell label="Color" hint="Accent color, for example `#f59e0b`.">
                            <input
                                class="w-full rounded-2xl border border-border bg-background px-4 py-3 font-mono text-sm outline-none transition focus:border-primary"
                                prop:value=move || color.get()
                                on:input=move |ev| set_color.set(event_target_value(&ev))
                                placeholder="#f59e0b"
                            />
                        </FieldShell>
                    </div>
                    <FieldShell label="Position" hint="Lower comes first in the list.">
                        <input
                            class="w-full rounded-2xl border border-border bg-background px-4 py-3 text-sm outline-none transition focus:border-primary"
                            prop:value=move || position.get().to_string()
                            on:input=move |ev| set_position.set(event_target_value(&ev).parse::<i32>().unwrap_or(0))
                            placeholder="0"
                        />
                    </FieldShell>
                    <label class="flex items-start gap-3 rounded-2xl border border-border bg-background px-4 py-4 text-sm">
                        <input
                            type="checkbox"
                            class="mt-0.5"
                            prop:checked=move || moderated.get()
                            on:change=move |ev| set_moderated.set(event_target_checked(&ev))
                        />
                        <span class="space-y-1">
                            <span class="block font-medium text-foreground">"Moderated queue"</span>
                            <span class="block text-muted-foreground">
                                "Topics in this category should flow through stricter review."
                            </span>
                        </span>
                    </label>
                    <div class="flex flex-wrap gap-3 pt-2">
                        <button
                            type="submit"
                            class="rounded-full bg-primary px-5 py-2.5 text-sm font-medium text-primary-foreground transition hover:opacity-95"
                            disabled=move || busy_key.get().is_some()
                        >
                            {move || if editing_id.get().is_some() { "Save category" } else { "Create category" }}
                        </button>
                        <button
                            type="button"
                            class="rounded-full border border-border px-5 py-2.5 text-sm font-medium transition hover:bg-muted"
                            on:click=move |_| on_reset.run(())
                        >
                            "Reset"
                        </button>
                    </div>
                </form>
            </section>
        </section>
    }
}

#[component]
fn TopicsPage(
    categories: Resource<Result<Vec<CategoryListItem>, String>>,
    topics: Resource<Result<Vec<TopicListItem>, String>>,
    replies: Resource<Result<Vec<ReplyListItem>, String>>,
    busy_key: ReadSignal<Option<String>>,
    editing_id: ReadSignal<Option<String>>,
    locale: ReadSignal<String>,
    set_locale: WriteSignal<String>,
    category_id: ReadSignal<String>,
    set_category_id: WriteSignal<String>,
    title: ReadSignal<String>,
    set_title: WriteSignal<String>,
    slug: ReadSignal<String>,
    set_slug: WriteSignal<String>,
    body: ReadSignal<String>,
    set_body: WriteSignal<String>,
    body_format: ReadSignal<String>,
    set_body_format: WriteSignal<String>,
    tags: ReadSignal<String>,
    set_tags: WriteSignal<String>,
    filter_category_id: ReadSignal<String>,
    set_filter_category_id: WriteSignal<String>,
    on_edit: Callback<String>,
    on_delete: Callback<String>,
    on_submit: impl Fn(SubmitEvent) + 'static + Copy,
    on_reset: Callback<()>,
) -> impl IntoView {
    let selected_category_name = move || match categories.get() {
        Some(Ok(items)) => {
            let selected_id = filter_category_id.get();
            if selected_id.is_empty() {
                "All categories".to_string()
            } else {
                items
                    .into_iter()
                    .find(|item| item.id == selected_id)
                    .map(|item| item.name)
                    .unwrap_or_else(|| "Filtered category".to_string())
            }
        }
        _ => "All categories".to_string(),
    };
    let topic_form_tag_count = move || parse_tags(tags.get().as_str()).len();

    view! {
        <section class="grid gap-6 xl:grid-cols-[17rem_minmax(0,1fr)_24rem]">
            <aside class="space-y-6 rounded-[1.75rem] border border-border bg-card p-5 shadow-sm xl:sticky xl:top-6 xl:self-start">
                <div>
                    <p class="text-xs font-semibold uppercase tracking-[0.24em] text-muted-foreground">
                        "Navigation"
                    </p>
                    <h2 class="mt-2 text-xl font-semibold text-card-foreground">"Forum feed"</h2>
                    <p class="mt-2 text-sm leading-6 text-muted-foreground">
                        "A left rail similar to NodeBB: jump between categories, keep counts visible, and open a thread into the editor on the right."
                    </p>
                </div>

                <div class="rounded-[1.5rem] border border-border bg-background/80 p-4">
                    <div class="flex items-center justify-between gap-3">
                        <p class="text-sm font-medium text-foreground">"Filter topics"</p>
                        <button
                            type="button"
                            class="text-xs font-medium text-muted-foreground transition hover:text-foreground"
                            on:click=move |_| set_filter_category_id.set(String::new())
                        >
                            "Clear"
                        </button>
                    </div>
                    <Suspense fallback=move || view! { <div class="mt-4 h-24 animate-pulse rounded-2xl bg-muted"></div> }>
                        {move || categories.get().map(|result| render_category_sidebar(result, filter_category_id.get(), set_filter_category_id))}
                    </Suspense>
                </div>

                <div class="space-y-3 rounded-[1.5rem] border border-border bg-gradient-to-br from-background to-muted/40 p-4">
                    <SidebarStat
                        label="Active filter"
                        value=Signal::derive(selected_category_name)
                    />
                    <SidebarStat
                        label="Draft tags"
                        value=Signal::derive(move || format!("{} ready", topic_form_tag_count()))
                    />
                    <SidebarStat
                        label="Editing thread"
                        value=Signal::derive(move || {
                            editing_id
                                .get()
                                .map(|_| "Open in inspector".to_string())
                                .unwrap_or_else(|| "Nothing selected".to_string())
                        })
                    />
                </div>
            </aside>

            <div class="space-y-6">
                <section class="rounded-[1.75rem] border border-border bg-card p-6 shadow-sm">
                    <div class="flex flex-wrap items-start justify-between gap-4">
                        <div>
                            <p class="text-xs font-semibold uppercase tracking-[0.24em] text-muted-foreground">
                                "Topic stream"
                            </p>
                            <h2 class="mt-2 text-2xl font-semibold text-card-foreground">
                                {selected_category_name}
                            </h2>
                            <p class="mt-2 max-w-2xl text-sm leading-6 text-muted-foreground">
                                "Open a topic card to inspect replies and edit the thread without leaving the feed."
                            </p>
                        </div>
                        <button
                            type="button"
                            class="rounded-full border border-border bg-background px-4 py-2 text-sm font-medium transition hover:bg-muted"
                            on:click=move |_| on_reset.run(())
                        >
                            "New topic"
                        </button>
                    </div>
                    <Suspense fallback=move || view! { <div class="mt-6 h-72 animate-pulse rounded-[1.5rem] bg-muted"></div> }>
                        {move || topics.get().map(|result| render_topic_feed(result, editing_id.get(), busy_key.get(), on_edit, on_delete))}
                    </Suspense>
                </section>
            </div>

            <div class="space-y-6 xl:sticky xl:top-6 xl:self-start">
                <section class="rounded-[1.75rem] border border-border bg-card p-6 shadow-sm">
                    <div class="flex items-center justify-between gap-3">
                        <div>
                            <p class="text-xs font-semibold uppercase tracking-[0.24em] text-muted-foreground">
                                "Inspector"
                            </p>
                            <h2 class="mt-2 text-xl font-semibold text-card-foreground">
                                {move || if editing_id.get().is_some() { "Edit topic" } else { "Compose topic" }}
                            </h2>
                        </div>
                        {move || editing_id.get().map(|_| view! {
                            <span class="rounded-full bg-sky-500/15 px-3 py-1 text-xs font-medium text-sky-700 dark:text-sky-300">
                                "Thread open"
                            </span>
                        })}
                    </div>

                    <form class="mt-6 space-y-4" on:submit=on_submit>
                        <FieldShell label="Locale" hint="Thread locale for publishing and reads.">
                            <input
                                class="w-full rounded-2xl border border-border bg-background px-4 py-3 text-sm outline-none transition focus:border-primary"
                                prop:value=move || locale.get()
                                on:input=move |ev| set_locale.set(event_target_value(&ev))
                                placeholder="en"
                            />
                        </FieldShell>
                        <FieldShell label="Category" hint="Choose where the topic should live.">
                            <Suspense fallback=move || view! { <div class="h-14 animate-pulse rounded-2xl bg-muted"></div> }>
                                {move || categories.get().map(|result| match result {
                                    Ok(items) => view! {
                                        <select
                                            class="w-full rounded-2xl border border-border bg-background px-4 py-3 text-sm outline-none transition focus:border-primary"
                                            prop:value=move || category_id.get()
                                            on:change=move |ev| set_category_id.set(event_target_value(&ev))
                                        >
                                            <option value="">"Choose category"</option>
                                            {items.into_iter().map(|item| view! { <option value=item.id>{item.name}</option> }).collect_view()}
                                        </select>
                                    }.into_any(),
                                    Err(err) => view! {
                                        <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                                            {err}
                                        </div>
                                    }.into_any(),
                                })}
                            </Suspense>
                        </FieldShell>
                        <FieldShell label="Title" hint="Headline shown in the feed.">
                            <input
                                class="w-full rounded-2xl border border-border bg-background px-4 py-3 text-sm outline-none transition focus:border-primary"
                                prop:value=move || title.get()
                                on:input=move |ev| set_title.set(event_target_value(&ev))
                                placeholder="How should we structure weekly updates?"
                            />
                        </FieldShell>
                        <FieldShell label="Slug" hint="Stable thread identifier.">
                            <input
                                class="w-full rounded-2xl border border-border bg-background px-4 py-3 font-mono text-sm outline-none transition focus:border-primary"
                                prop:value=move || slug.get()
                                on:input=move |ev| set_slug.set(event_target_value(&ev))
                                placeholder="weekly-updates-structure"
                            />
                        </FieldShell>
                        <div class="grid gap-4 sm:grid-cols-2">
                            <FieldShell label="Body format" hint="Usually `markdown`.">
                                <input
                                    class="w-full rounded-2xl border border-border bg-background px-4 py-3 font-mono text-sm outline-none transition focus:border-primary"
                                    prop:value=move || body_format.get()
                                    on:input=move |ev| set_body_format.set(event_target_value(&ev))
                                    placeholder="markdown"
                                />
                            </FieldShell>
                            <FieldShell label="Tags" hint="Comma-separated labels for discovery.">
                                <input
                                    class="w-full rounded-2xl border border-border bg-background px-4 py-3 text-sm outline-none transition focus:border-primary"
                                    prop:value=move || tags.get()
                                    on:input=move |ev| set_tags.set(event_target_value(&ev))
                                    placeholder="release, roadmap, updates"
                                />
                            </FieldShell>
                        </div>

                        {move || {
                            let parsed_tags = parse_tags(tags.get().as_str());
                            (!parsed_tags.is_empty()).then(|| {
                                view! {
                                    <div class="flex flex-wrap gap-2 rounded-2xl border border-border bg-background px-4 py-3">
                                        {parsed_tags.into_iter().map(|tag| view! {
                                            <span class="rounded-full bg-amber-500/15 px-2.5 py-1 text-xs font-medium text-amber-700 dark:text-amber-300">
                                                {tag}
                                            </span>
                                        }).collect_view()}
                                    </div>
                                }
                            })
                        }}

                        <FieldShell label="Body" hint="Main message shown in the topic detail.">
                            <textarea
                                class="min-h-72 w-full rounded-2xl border border-border bg-background px-4 py-3 font-mono text-sm outline-none transition focus:border-primary"
                                prop:value=move || body.get()
                                on:input=move |ev| set_body.set(event_target_value(&ev))
                                placeholder="Write the first post here..."
                            ></textarea>
                        </FieldShell>

                        <div class="flex flex-wrap gap-3 pt-2">
                            <button
                                type="submit"
                                class="rounded-full bg-primary px-5 py-2.5 text-sm font-medium text-primary-foreground transition hover:opacity-95"
                                disabled=move || busy_key.get().is_some()
                            >
                                {move || if editing_id.get().is_some() { "Save topic" } else { "Publish topic" }}
                            </button>
                            <button
                                type="button"
                                class="rounded-full border border-border px-5 py-2.5 text-sm font-medium transition hover:bg-muted"
                                on:click=move |_| on_reset.run(())
                            >
                                "Reset"
                            </button>
                        </div>
                    </form>
                </section>

                <section class="rounded-[1.75rem] border border-border bg-card p-6 shadow-sm">
                    <div class="flex items-center justify-between gap-3">
                        <div>
                            <p class="text-xs font-semibold uppercase tracking-[0.24em] text-muted-foreground">
                                "Thread preview"
                            </p>
                            <h2 class="mt-2 text-xl font-semibold text-card-foreground">"Replies"</h2>
                        </div>
                        <span class="rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground">
                            {move || format!("{} shown", reply_count_label(replies.get()))}
                        </span>
                    </div>
                    <Suspense fallback=move || view! { <div class="mt-6 h-40 animate-pulse rounded-[1.5rem] bg-muted"></div> }>
                        {move || replies.get().map(render_reply_stack)}
                    </Suspense>
                </section>
            </div>
        </section>
    }
}

fn render_category_grid(
    result: Result<Vec<CategoryListItem>, String>,
    editing_id: Option<String>,
    busy_key: Option<String>,
    on_edit: Callback<String>,
    on_delete: Callback<String>,
) -> AnyView {
    match result {
        Ok(items) if items.is_empty() => view! { <div class="mt-6 rounded-[1.5rem] border border-dashed border-border p-8 text-sm text-muted-foreground">"No categories yet."</div> }.into_any(),
        Ok(items) => view! {
            <div class="mt-6 grid gap-4 md:grid-cols-2">
                {items.into_iter().map(|item| {
                    let item_id = item.id.clone();
                    let is_editing = editing_id.as_deref() == Some(item_id.as_str());
                    let is_busy = busy_key.as_ref().map(|value| value.contains(item_id.as_str())).unwrap_or(false);
                    let accent_style = item.color.as_deref()
                        .filter(|value| !value.trim().is_empty())
                        .map(|value| format!("background:{};", value))
                        .unwrap_or_else(|| "background:linear-gradient(180deg,#0ea5e9 0%,#f59e0b 100%);".to_string());
                    view! {
                        <article class="relative overflow-hidden rounded-[1.5rem] border border-border bg-background p-5 shadow-sm">
                            <span class="absolute inset-y-0 left-0 w-1.5" style=accent_style></span>
                            <div class="pl-3">
                                <div class="flex items-start justify-between gap-4">
                                    <div>
                                        <div class="text-[11px] font-semibold uppercase tracking-[0.22em] text-muted-foreground">
                                            {item.effective_locale.clone()}
                                        </div>
                                        <h3 class="mt-2 text-lg font-semibold text-foreground">{item.name.clone()}</h3>
                                    </div>
                                    <span class="rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground">
                                        {format!("#{}", item.slug)}
                                    </span>
                                </div>
                                <p class="mt-3 text-sm leading-6 text-muted-foreground">
                                    {item.description.clone().unwrap_or_else(|| "No description yet.".to_string())}
                                </p>
                                <div class="mt-4 flex flex-wrap gap-2">
                                    <CountChip label="topics" value=item.topic_count />
                                    <CountChip label="replies" value=item.reply_count />
                                    {item.icon.clone().filter(|value| !value.trim().is_empty()).map(|value| view! {
                                        <span class="rounded-full bg-muted px-2.5 py-1 text-xs font-medium text-muted-foreground">
                                            {format!("icon: {value}")}
                                        </span>
                                    })}
                                </div>
                                <div class="mt-5 flex flex-wrap gap-2">
                                    <button type="button" class="rounded-full border border-border px-4 py-2 text-sm font-medium transition hover:bg-muted" on:click={ let item_id = item_id.clone(); move |_| on_edit.run(item_id.clone()) } disabled=is_busy>{if is_editing { "Editing" } else { "Edit" }}</button>
                                    <button type="button" class="rounded-full border border-destructive/30 bg-destructive/10 px-4 py-2 text-sm font-medium text-destructive transition hover:bg-destructive/15" on:click={ let item_id = item_id.clone(); move |_| on_delete.run(item_id.clone()) } disabled=is_busy>"Delete"</button>
                                </div>
                            </div>
                        </article>
                    }
                }).collect_view()}
            </div>
        }.into_any(),
        Err(err) => view! { <div class="mt-6 rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{err}</div> }.into_any(),
    }
}

fn render_category_sidebar(
    result: Result<Vec<CategoryListItem>, String>,
    active_category_id: String,
    set_filter_category_id: WriteSignal<String>,
) -> AnyView {
    match result {
        Ok(items) if items.is_empty() => view! { <div class="mt-4 rounded-2xl border border-dashed border-border p-4 text-sm text-muted-foreground">"No categories yet."</div> }.into_any(),
        Ok(items) => view! {
            <div class="mt-4 space-y-2">
                <button type="button" class=sidebar_category_class(active_category_id.is_empty()) on:click=move |_| set_filter_category_id.set(String::new())>
                    <span class="truncate">"All categories"</span>
                    <span class="rounded-full bg-background/70 px-2 py-0.5 text-[11px] font-medium text-muted-foreground">{items.len()}</span>
                </button>
                {items.into_iter().map(|item| {
                    let is_active = active_category_id == item.id;
                    let item_id = item.id.clone();
                    view! {
                        <button type="button" class=sidebar_category_class(is_active) on:click=move |_| set_filter_category_id.set(item_id.clone())>
                            <span class="min-w-0">
                                <span class="block truncate text-left text-sm font-medium text-foreground">{item.name.clone()}</span>
                                <span class="block truncate text-left text-xs text-muted-foreground">{item.slug.clone()}</span>
                            </span>
                            <span class="rounded-full bg-background/70 px-2 py-0.5 text-[11px] font-medium text-muted-foreground">{item.topic_count}</span>
                        </button>
                    }
                }).collect_view()}
            </div>
        }.into_any(),
        Err(err) => view! { <div class="mt-4 rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{err}</div> }.into_any(),
    }
}

fn render_topic_feed(
    result: Result<Vec<TopicListItem>, String>,
    editing_id: Option<String>,
    busy_key: Option<String>,
    on_edit: Callback<String>,
    on_delete: Callback<String>,
) -> AnyView {
    match result {
        Ok(items) if items.is_empty() => view! { <div class="mt-6 rounded-[1.5rem] border border-dashed border-border p-8 text-sm text-muted-foreground">"No topics yet."</div> }.into_any(),
        Ok(items) => view! {
            <div class="mt-6 space-y-3">
                {items.into_iter().map(|item| {
                    let item_id = item.id.clone();
                    let is_editing = editing_id.as_deref() == Some(item_id.as_str());
                    let is_busy = busy_key.as_ref().map(|value| value.contains(item_id.as_str())).unwrap_or(false);
                    let status_class = topic_status_class(item.status.as_str());
                    view! {
                        <article class="rounded-[1.5rem] border border-border bg-background p-5 shadow-sm transition hover:border-primary/30 hover:shadow-md">
                            <div class="flex flex-wrap items-start justify-between gap-4">
                                <div class="space-y-3">
                                    <div class="flex flex-wrap items-center gap-2">
                                        <span class=status_badge_class(status_class)>{item.status.clone()}</span>
                                        <span class="rounded-full border border-border px-2.5 py-1 text-[11px] font-medium text-muted-foreground">{item.effective_locale.clone()}</span>
                                        {item.is_pinned.then(|| view! { <span class="rounded-full bg-amber-500/15 px-2.5 py-1 text-[11px] font-medium text-amber-700 dark:text-amber-300">"Pinned"</span> })}
                                        {item.is_locked.then(|| view! { <span class="rounded-full bg-destructive/10 px-2.5 py-1 text-[11px] font-medium text-destructive">"Locked"</span> })}
                                    </div>
                                    <div>
                                        <h3 class="text-lg font-semibold text-foreground">{item.title.clone()}</h3>
                                        <p class="mt-1 text-sm text-muted-foreground">{format!("thread/{}/{}", item.category_id, item.slug)}</p>
                                    </div>
                                </div>
                                <div class="text-right">
                                    <p class="text-[11px] font-semibold uppercase tracking-[0.22em] text-muted-foreground">"Replies"</p>
                                    <p class="mt-1 text-2xl font-semibold text-foreground">{item.reply_count}</p>
                                </div>
                            </div>
                            <div class="mt-5 flex flex-wrap gap-2">
                                <button type="button" class="rounded-full border border-border px-4 py-2 text-sm font-medium transition hover:bg-muted" on:click={ let item_id = item_id.clone(); move |_| on_edit.run(item_id.clone()) } disabled=is_busy>{if is_editing { "Opened" } else { "Open thread" }}</button>
                                <button type="button" class="rounded-full border border-destructive/30 bg-destructive/10 px-4 py-2 text-sm font-medium text-destructive transition hover:bg-destructive/15" on:click={ let item_id = item_id.clone(); move |_| on_delete.run(item_id.clone()) } disabled=is_busy>"Delete"</button>
                            </div>
                        </article>
                    }
                }).collect_view()}
            </div>
        }.into_any(),
        Err(err) => view! { <div class="mt-6 rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{err}</div> }.into_any(),
    }
}

fn render_reply_stack(result: Result<Vec<ReplyListItem>, String>) -> AnyView {
    match result {
        Ok(items) if items.is_empty() => view! { <div class="mt-6 rounded-[1.5rem] border border-dashed border-border p-6 text-sm text-muted-foreground">"Open a topic card to preview replies."</div> }.into_any(),
        Ok(items) => view! {
            <div class="mt-6 space-y-3">
                {items.into_iter().map(|item| {
                    let status_class = topic_status_class(item.status.as_str());
                    view! {
                        <article class="rounded-[1.35rem] border border-border bg-background p-4">
                            <div class="flex items-center justify-between gap-3">
                                <span class=status_badge_class(status_class)>{item.status.clone()}</span>
                                <span class="text-[11px] font-semibold uppercase tracking-[0.22em] text-muted-foreground">{item.effective_locale.clone()}</span>
                            </div>
                            <p class="mt-3 text-sm leading-6 text-muted-foreground">{item.content_preview}</p>
                        </article>
                    }
                }).collect_view()}
            </div>
        }.into_any(),
        Err(err) => view! { <div class="mt-6 rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{err}</div> }.into_any(),
    }
}

fn apply_category_to_form(
    set_editing_category_id: WriteSignal<Option<String>>,
    set_category_locale: WriteSignal<String>,
    set_category_name: WriteSignal<String>,
    set_category_slug: WriteSignal<String>,
    set_category_description: WriteSignal<String>,
    set_category_icon: WriteSignal<String>,
    set_category_color: WriteSignal<String>,
    set_category_position: WriteSignal<i32>,
    set_category_moderated: WriteSignal<bool>,
    category: &CategoryDetail,
) {
    set_editing_category_id.set(Some(category.id.clone()));
    set_category_locale.set(category.locale.clone());
    set_category_name.set(category.name.clone());
    set_category_slug.set(category.slug.clone());
    set_category_description.set(category.description.clone().unwrap_or_default());
    set_category_icon.set(category.icon.clone().unwrap_or_default());
    set_category_color.set(category.color.clone().unwrap_or_default());
    set_category_position.set(category.position);
    set_category_moderated.set(category.moderated);
}

fn apply_topic_to_form(
    set_editing_topic_id: WriteSignal<Option<String>>,
    set_topic_locale: WriteSignal<String>,
    set_topic_category_id: WriteSignal<String>,
    set_topic_title: WriteSignal<String>,
    set_topic_slug: WriteSignal<String>,
    set_topic_body: WriteSignal<String>,
    set_topic_body_format: WriteSignal<String>,
    set_topic_tags: WriteSignal<String>,
    topic: &TopicDetail,
) {
    set_editing_topic_id.set(Some(topic.id.clone()));
    set_topic_locale.set(topic.locale.clone());
    set_topic_category_id.set(topic.category_id.clone());
    set_topic_title.set(topic.title.clone());
    set_topic_slug.set(topic.slug.clone());
    set_topic_body.set(topic.body.clone());
    set_topic_body_format.set(topic.body_format.clone());
    set_topic_tags.set(topic.tags.join(", "));
}

fn parse_tags(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn format_count(value: usize) -> String {
    value.to_string()
}

fn sidebar_category_class(is_active: bool) -> &'static str {
    if is_active {
        "flex w-full items-center justify-between gap-3 rounded-2xl border border-primary/30 bg-primary/10 px-3 py-3 text-left"
    } else {
        "flex w-full items-center justify-between gap-3 rounded-2xl border border-border bg-card px-3 py-3 text-left transition hover:bg-muted"
    }
}

fn topic_status_class(status: &str) -> &'static str {
    match status.to_ascii_lowercase().as_str() {
        "published" | "active" | "open" => "success",
        "draft" | "pending" => "warning",
        "archived" | "closed" => "muted",
        _ => "default",
    }
}

fn status_badge_class(status_class: &'static str) -> &'static str {
    match status_class {
        "success" => {
            "rounded-full bg-emerald-500/15 px-2.5 py-1 text-[11px] font-medium text-emerald-700 dark:text-emerald-300"
        }
        "warning" => {
            "rounded-full bg-amber-500/15 px-2.5 py-1 text-[11px] font-medium text-amber-700 dark:text-amber-300"
        }
        "muted" => {
            "rounded-full bg-muted px-2.5 py-1 text-[11px] font-medium text-muted-foreground"
        }
        _ => {
            "rounded-full border border-border px-2.5 py-1 text-[11px] font-medium text-muted-foreground"
        }
    }
}

fn reply_count_label(replies: Option<Result<Vec<ReplyListItem>, String>>) -> usize {
    match replies {
        Some(Ok(items)) => items.len(),
        _ => 0,
    }
}
