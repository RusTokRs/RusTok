use leptos::prelude::*;
use rustok_api::RichTextDocument;
use rustok_grid::{ColumnFilters, GridPagination, RowSelection};
use rustok_grid_leptos::prelude::*;
use rustok_seo_panel::SeoEntityPanel;
use rustok_seo_targets::SeoTargetSlug;
use rustok_ui_core::UiRouteContext;

use crate::core;
use crate::i18n::t;
use crate::model::{BlogPostDetail, BlogPostListItem};

pub(super) fn local_resource<S, Fut, T>(
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
pub(super) fn BlogSeoConfigError(ui_locale: Option<String>) -> impl IntoView {
    view! {
        <section class="m-6 rounded-2xl border border-destructive/30 bg-destructive/5 p-6">
            <h2 class="text-lg font-semibold text-destructive">
                {t(
                    ui_locale.as_deref(),
                    "blog.error.seoConfiguration",
                    "Blog SEO configuration is unavailable",
                )}
            </h2>
            <p class="mt-2 text-sm text-muted-foreground">
                {t(
                    ui_locale.as_deref(),
                    "blog.error.seoTargetInvalid",
                    "The Blog SEO target is invalid. Contact the platform administrator.",
                )}
            </p>
        </section>
    }
}

pub(super) fn blog_form_view_model(
    locale: Option<&str>,
    editing_post_id: Option<&str>,
    busy_key: Option<&str>,
) -> core::BlogPostAdminFormViewModel {
    core::blog_post_admin_form_view(
        editing_post_id,
        busy_key,
        core::BlogPostAdminFormLabels {
            edit_title: t(locale, "blog.form.editTitle", "Edit post"),
            create_title: t(locale, "blog.form.createTitle", "Create post"),
            saving: t(locale, "blog.form.saving", "Saving..."),
            update: t(locale, "blog.form.update", "Update post"),
            create: t(locale, "blog.form.create", "Create post"),
        },
    )
}

pub(super) fn blog_form_copy_view_model(
    locale: Option<&str>,
) -> core::BlogPostAdminEditorFormCopyViewModel {
    core::blog_post_admin_editor_form_copy_view(core::BlogPostAdminEditorFormCopyLabels {
        subtitle: t(
            locale,
            "blog.form.subtitle",
            "The package owns both the list and the form. apps/admin only hosts the module route.",
        ),
        title_label: t(locale, "blog.form.title", "Title"),
        slug_label: t(locale, "blog.form.slug", "Slug"),
        locale_label: t(locale, "blog.form.locale", "Locale"),
        excerpt_label: t(locale, "blog.form.excerpt", "Excerpt"),
        content_label: t(locale, "blog.form.content", "Content"),
        tags_label: t(locale, "blog.form.tags", "Tags"),
        tags_placeholder: t(locale, "blog.form.tagsPlaceholder", "news, launch, release"),
        publish_now_label: t(locale, "blog.form.publishNow", "Publish immediately"),
    })
}

#[component]
pub(super) fn BlogEditBanner(
    banner_view: Signal<core::BlogPostAdminEditBannerViewModel>,
    on_reset: Callback<()>,
) -> impl IntoView {
    view! {
        <div class=move || banner_view.get().class>
            <div class="text-sm text-muted-foreground">
                {move || banner_view.get().banner_text}
            </div>
            <button
                type="button"
                class="text-xs font-medium text-primary hover:underline"
                on:click=move |_| on_reset.run(())
            >
                {move || banner_view.get().create_new_label}
            </button>
        </div>
    }
}

#[component]
pub(super) fn BlogPostsTable(
    items: Vec<BlogPostListItem>,
    next_cursor: Option<String>,
    on_load_more: Callback<String>,
    /// The sort the loaded page was requested with. Part of the request, not a
    /// client-side reorder: changing it restarts the list at page one.
    sort_field: crate::model::BlogPostSortField,
    sort_order: crate::model::BlogPostSortOrder,
    on_sort_change: Callback<core::BlogPostAdminSort>,
    editing_post_id: Option<String>,
    busy_key: Option<String>,
    on_edit: Callback<(String, String)>,
    on_toggle_publish: Callback<(String, bool, String)>,
    on_archive: Callback<(String, String)>,
    on_restore: Callback<(String, String)>,
    on_delete: Callback<String>,
) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let table_classes = core::blog_post_admin_table_classes_view();
    let table = core::blog_post_admin_posts_table_view_from_items(
        items,
        editing_post_id.as_deref(),
        busy_key.as_deref(),
        core::BlogPostAdminPostsTableLabels {
            empty_message: t(
                locale.as_deref(),
                "blog.table.empty",
                "No posts yet. Create the first one from the module package form.",
            ),
            total_label: t(locale.as_deref(), "blog.table.total", "{count} post(s)"),
            title_header: t(locale.as_deref(), "blog.table.title", "Title"),
            slug_header: t(locale.as_deref(), "blog.table.slug", "Slug"),
            status_header: t(locale.as_deref(), "blog.table.status", "Status"),
            locale_header: t(locale.as_deref(), "blog.table.locale", "Locale"),
            draft_slug: t(locale.as_deref(), "blog.table.draft", "draft"),
            no_excerpt: t(locale.as_deref(), "blog.table.noExcerpt", "No excerpt"),
            editing: t(locale.as_deref(), "blog.table.editing", "Editing"),
            edit: t(locale.as_deref(), "blog.table.edit", "Edit"),
            unpublish: t(locale.as_deref(), "blog.table.unpublish", "Unpublish"),
            publish: t(locale.as_deref(), "blog.table.publish", "Publish"),
            archive: t(locale.as_deref(), "blog.table.archive", "Archive"),
            restore: t(locale.as_deref(), "blog.table.restore", "Restore"),
            delete: t(locale.as_deref(), "blog.table.delete", "Delete"),
        },
    );
    let columns = core::blog_post_grid_columns(locale.as_deref());
    let search = RwSignal::new(String::new());
    let filters = RwSignal::new(ColumnFilters::default());
    let selection = RwSignal::new(RowSelection::default());
    // Rows arrive page by page; the effect below sets the total from the rows on screen.
    let pagination = RwSignal::new(GridPagination::new(1, 10, 0));
    let load_more_label = t(locale.as_deref(), "blog.table.loadMore", "Load more");
    let is_ru = locale
        .as_deref()
        .map(|l| l.starts_with("ru"))
        .unwrap_or(false);
    let sort_label = t(locale.as_deref(), "blog.table.sort", "Sort");
    let sort_field_options = core::BLOG_POST_ADMIN_SORT_FIELDS
        .into_iter()
        .map(|field| {
            (
                field,
                t(
                    locale.as_deref(),
                    core::blog_post_admin_sort_field_label_key(field),
                    core::blog_post_admin_sort_field_default_label(field),
                ),
            )
        })
        .collect::<Vec<_>>();
    let sort_order_options = core::BLOG_POST_ADMIN_SORT_ORDERS
        .into_iter()
        .map(|order| {
            (
                order,
                t(
                    locale.as_deref(),
                    core::blog_post_admin_sort_order_label_key(order),
                    core::blog_post_admin_sort_order_default_label(order),
                ),
            )
        })
        .collect::<Vec<_>>();

    let rows_store = StoredValue::new(table.rows);

    let filtered_rows = Memo::new(move |_| {
        let rows = rows_store.get_value();
        let q = search.get();
        let f = filters.get();
        core::filter_blog_posts(&rows, &f, if q.trim().is_empty() { None } else { Some(&q) })
    });

    Effect::new(move |_| {
        let count = filtered_rows.get().len() as u64;
        pagination.update(|p| p.set_total(count));
    });

    let paged_rows = Memo::new(move |_| {
        let list = filtered_rows.get();
        let p = pagination.get();
        let start = (p.page.saturating_sub(1)) * p.page_size;
        list.into_iter()
            .skip(start)
            .take(p.page_size)
            .collect::<Vec<_>>()
    });

    let on_filters_change = Callback::new(move |new_filters: ColumnFilters| {
        filters.set(new_filters);
    });

    let cell_renderer = {
        let tc = table_classes;
        Callback::new(
            move |(row, col_id): (core::BlogPostAdminTableRowViewModel, String)| {
                let post_id_edit = row.post_id.clone();
                let post_id_publish = row.post_id.clone();
                let post_id_archive = row.post_id.clone();
                let post_id_restore = row.post_id.clone();
                let post_id_delete = row.post_id.clone();
                let post_locale_edit = row.locale.clone();
                let post_locale_publish = row.locale.clone();
                let post_locale_archive = row.locale.clone();
                let post_locale_restore = row.locale.clone();

                match col_id.as_str() {
                "title" => view! {
                    <div class="space-y-0.5">
                        <div class=tc.title_text>{row.title.clone()}</div>
                        <div class=tc.excerpt_text>{row.excerpt.clone()}</div>
                    </div>
                }.into_any(),
                "slug" => view! {
                    <span class=tc.muted_cell>{row.slug.clone()}</span>
                }.into_any(),
                "status" => view! {
                    <StatusBadge status=row.status.clone() />
                }.into_any(),
                "locale" => view! {
                    <span class=tc.muted_cell>{row.locale.clone()}</span>
                }.into_any(),
                "actions" => view! {
                    <div class=tc.actions_group>
                        <button
                            type="button"
                            class=tc.primary_action_button
                            disabled=row.is_busy
                            on:click={
                                move |_| on_edit.run((post_id_edit.clone(), post_locale_edit.clone()))
                            }
                        >
                            {row.edit_label.clone()}
                        </button>
                        {if row.show_publish_action {
                            view! {
                                <button
                                    type="button"
                                    class=tc.primary_action_button
                                    disabled=row.is_busy
                                    on:click={
                                        move |_| on_toggle_publish.run((
                                            post_id_publish.clone(),
                                            row.next_publish_state,
                                            post_locale_publish.clone(),
                                        ))
                                    }
                                >
                                    {row.publish_label.clone()}
                                </button>
                            }
                            .into_any()
                        } else {
                            ().into_any()
                        }}
                        {if row.show_archive_action {
                            view! {
                                <button
                                    type="button"
                                    class=tc.primary_action_button
                                    disabled=row.is_busy
                                    on:click={
                                        move |_| on_archive.run((post_id_archive.clone(), post_locale_archive.clone()))
                                    }
                                >
                                    {row.archive_label.clone()}
                                </button>
                            }
                            .into_any()
                        } else {
                            ().into_any()
                        }}
                        {if row.show_restore_action {
                            view! {
                                <button
                                    type="button"
                                    class=tc.primary_action_button
                                    disabled=row.is_busy
                                    on:click={
                                        move |_| on_restore.run((post_id_restore.clone(), post_locale_restore.clone()))
                                    }
                                >
                                    {row.restore_label.clone()}
                                </button>
                            }
                            .into_any()
                        } else {
                            ().into_any()
                        }}
                        <button
                            type="button"
                            class=tc.destructive_action_button
                            disabled=row.is_busy
                            on:click={
                                move |_| on_delete.run(post_id_delete.clone())
                            }
                        >
                            {row.delete_label.clone()}
                        </button>
                    </div>
                }.into_any(),
                _ => ().into_any(),
            }
            },
        )
    };

    let select_class = "text-xs rounded-lg border border-border bg-background px-2.5 py-1 text-foreground outline-none focus:border-primary font-normal";

    view! {
        <div class="space-y-4">
            <div class="flex flex-col sm:flex-row gap-3 items-center justify-between">
                <div class="flex flex-col sm:flex-row gap-3 items-center">
                    <input
                        type="text"
                        placeholder=if is_ru { "Поиск по статьям блога..." } else { "Search blog posts..." }
                        prop:value=move || search.get()
                        on:input=move |ev| search.set(event_target_value(&ev))
                        class="text-xs rounded-lg border border-border bg-background px-2.5 py-1 text-foreground outline-none focus:border-primary w-64 font-normal"
                    />
                    // The sort is a request, not a view filter: it reloads the list
                    // from the server instead of reordering the loaded rows.
                    <label class="flex items-center gap-1.5 text-xs text-muted-foreground">
                        <span>{sort_label.clone()}</span>
                        <select
                            class=select_class
                            aria-label=sort_label.clone()
                            prop:value=move || sort_field.as_graphql()
                            on:change=move |ev| {
                                let token = event_target_value(&ev);
                                if let Some(field) = crate::model::BlogPostSortField::from_graphql(token.as_str()) {
                                    on_sort_change.run(core::BlogPostAdminSort::default().with_field(field).with_order(sort_order));
                                }
                            }
                        >
                            {sort_field_options
                                .into_iter()
                                .map(|(field, label)| {
                                    view! {
                                        <option value=field.as_graphql()>
                                            {label}
                                        </option>
                                    }
                                })
                                .collect_view()}
                        </select>
                        <select
                            class=select_class
                            aria-label=sort_label.clone()
                            prop:value=move || sort_order.as_graphql()
                            on:change=move |ev| {
                                let token = event_target_value(&ev);
                                if let Some(order) = crate::model::BlogPostSortOrder::from_graphql(token.as_str()) {
                                    on_sort_change.run(core::BlogPostAdminSort::default().with_field(sort_field).with_order(order));
                                }
                            }
                        >
                            {sort_order_options
                                .into_iter()
                                .map(|(order, label)| {
                                    view! {
                                        <option value=order.as_graphql()>
                                            {label}
                                        </option>
                                    }
                                })
                                .collect_view()}
                        </select>
                    </label>
                </div>
                <div class=table_classes.total_label>
                    {table.total_label}
                </div>
            </div>

            <DataGrid
                columns=columns
                data=Signal::derive(move || paged_rows.get())
                key_fn=|row: &core::BlogPostAdminTableRowViewModel| row.post_id.clone()
                cell_renderer=cell_renderer
                empty_message=table.empty_message
                selection=selection
                pagination=pagination
                filters=filters
                on_filter_change=on_filters_change
                on_row_click=Callback::new(|_| ())
            />
            {next_cursor.map(|cursor| {
                let label = load_more_label.clone();
                view! {
                    <div class="flex justify-center">
                        <button
                            type="button"
                            class="rounded-lg border border-border px-3 py-1.5 text-xs text-foreground hover:bg-muted"
                            on:click=move |_| on_load_more.run(cursor.clone())
                        >
                            {label}
                        </button>
                    </div>
                }
            })}
        </div>
    }
    .into_any()
}

#[component]
fn StatusBadge(status: String) -> impl IntoView {
    let badge = core::blog_post_admin_status_badge_view(status.as_str());
    view! {
        <span class=badge.class>
            {badge.status}
        </span>
    }
}

#[derive(Clone, Copy)]
pub(super) struct BlogPostFormSetters {
    pub(super) editing_post_id: WriteSignal<Option<String>>,
    pub(super) editing_version: WriteSignal<Option<i32>>,
    pub(super) title: WriteSignal<String>,
    pub(super) slug: WriteSignal<String>,
    pub(super) excerpt: WriteSignal<String>,
    pub(super) content: WriteSignal<RichTextDocument>,
    pub(super) locale: WriteSignal<String>,
    pub(super) tags_input: WriteSignal<String>,
    pub(super) category_id: WriteSignal<String>,
    pub(super) channel_slugs_input: WriteSignal<String>,
    pub(super) featured_image_url: WriteSignal<String>,
    pub(super) seo_title: WriteSignal<String>,
    pub(super) seo_description: WriteSignal<String>,
    pub(super) publish_now: WriteSignal<bool>,
}

pub(super) fn apply_post_to_form(setters: BlogPostFormSetters, post: &BlogPostDetail) {
    apply_form_state(setters, core::BlogPostEditorFormState::from_post(post));
}

pub(super) fn reset_form(setters: BlogPostFormSetters, default_locale: &str) {
    apply_form_state(
        setters,
        core::BlogPostEditorFormState::empty(default_locale),
    );
}

fn apply_form_state(setters: BlogPostFormSetters, state: core::BlogPostEditorFormState) {
    setters.editing_post_id.set(state.editing_post_id);
    setters.editing_version.set(state.version);
    setters.title.set(state.title);
    setters.slug.set(state.slug);
    setters.excerpt.set(state.excerpt);
    setters.content.set(state.content);
    setters.locale.set(state.locale);
    setters.tags_input.set(state.tags_input);
    setters.category_id.set(state.category_id);
    setters.channel_slugs_input.set(state.channel_slugs_input);
    setters.featured_image_url.set(state.featured_image_url);
    setters.seo_title.set(state.seo_title);
    setters.seo_description.set(state.seo_description);
    setters.publish_now.set(state.publish_now);
}

/// The cursor-paged post list region: one table plus the load-more failure banner.
///
/// Owned here rather than in `leptos.rs` so the page component keeps the request wiring
/// (resource, signals, callbacks) and the list keeps its rendering.
#[component]
pub(super) fn BlogPostsList(
    items: Vec<BlogPostListItem>,
    next_cursor: Option<String>,
    load_more_error: Option<String>,
    sort: core::BlogPostAdminSort,
    editing_post_id: Option<String>,
    busy_key: Option<String>,
    on_load_more: Callback<String>,
    on_sort_change: Callback<core::BlogPostAdminSort>,
    on_edit: Callback<(String, String)>,
    on_toggle_publish: Callback<(String, bool, String)>,
    on_archive: Callback<(String, String)>,
    on_restore: Callback<(String, String)>,
    on_delete: Callback<String>,
) -> impl IntoView {
    let shell_classes = core::blog_post_admin_shell_classes_view();
    view! {
        {load_more_error.map(|message| view! {
            <div class=shell_classes.load_error>{message}</div>
        })}
        <BlogPostsTable
            items=items
            next_cursor=next_cursor
            on_load_more=on_load_more
            sort_field=sort.field
            sort_order=sort.order
            on_sort_change=on_sort_change
            editing_post_id=editing_post_id
            busy_key=busy_key
            on_edit=on_edit
            on_toggle_publish=on_toggle_publish
            on_archive=on_archive
            on_restore=on_restore
            on_delete=on_delete
        />
    }
}

/// The SEO sidebar panel for the post currently open in the editor.
///
/// The panel is data-driven by `target_kind`/`target_id`, so the editor only supplies the
/// selected post and the tenant locale; the SEO crate owns the fetches and the widgets.
#[component]
pub(super) fn BlogPostSeoPanel(
    target_kind: SeoTargetSlug,
    target_id: Signal<Option<String>>,
    host_locale: Signal<String>,
) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    view! {
        <SeoEntityPanel
            target_kind=target_kind
            target_id=target_id
            locale=host_locale
            show_control_plane_widgets=true
            panel_title=t(locale.as_deref(), "blog.seo.title", "Post SEO")
            panel_subtitle=t(
                locale.as_deref(),
                "blog.seo.subtitle",
                "Explicit metadata, social tags and diagnostics for the selected blog post.",
            )
            empty_message=t(
                locale.as_deref(),
                "blog.seo.empty",
                "Create or open a post first. SEO stays inside the blog editor rather than a global SEO hub.",
            )
        />
    }
}

/// The page header block: badge, title and subtitle copy for the blog admin surface.
#[component]
pub(super) fn BlogPostsPageHeader() -> impl IntoView {
    let shell_classes = core::blog_post_admin_shell_classes_view();
    let ui_locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    view! {
        <header class=shell_classes.header>
            <div class=shell_classes.header_content>
                <span class=shell_classes.badge>
                    {t(ui_locale.as_deref(), "blog.badge", "blog")}
                </span>
                <h1 class=shell_classes.title>
                    {t(ui_locale.as_deref(), "blog.title", "Blog Publishing")}
                </h1>
                <p class=shell_classes.subtitle>
                    {t(
                        ui_locale.as_deref(),
                        "blog.subtitle",
                        "Canonical module-owned CRUD flow for blog posts through the blog GraphQL contract.",
                    )}
                </p>
            </div>
        </header>
    }
}

/// The posts list card header: list copy plus the locale filter that drives the query.
#[component]
pub(super) fn BlogPostsListHeader(
    locale: ReadSignal<String>,
    set_locale: WriteSignal<String>,
    locale_filter_label: String,
) -> impl IntoView {
    let shell_classes = core::blog_post_admin_shell_classes_view();
    let ui_locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    view! {
        <div class=shell_classes.list_header>
            <div>
                <h2 class=shell_classes.list_title>
                    {t(ui_locale.as_deref(), "blog.posts.title", "Posts")}
                </h2>
                <p class=shell_classes.list_subtitle>
                    {t(
                        ui_locale.as_deref(),
                        "blog.posts.subtitle",
                        "Loaded from rustok-blog-admin via GraphQL, not wired manually in apps/admin.",
                    )}
                </p>
            </div>
            <label class=shell_classes.locale_filter_label>
                <span class=shell_classes.locale_filter_text>{locale_filter_label}</span>
                <input
                    type="text"
                    class=shell_classes.locale_filter_input
                    prop:value=locale
                    on:input=move |ev| set_locale.set(event_target_value(&ev))
                />
            </label>
        </div>
    }
}
