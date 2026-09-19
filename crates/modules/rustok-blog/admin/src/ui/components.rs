use leptos::prelude::*;
use rustok_api::RichTextDocument;
use rustok_ui_core::UiRouteContext;

use crate::core;
use crate::i18n::t;
use crate::model::{BlogPostDetail, BlogPostListItem};

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

pub(super) fn blog_form_copy_view_model(locale: Option<&str>) -> core::BlogPostAdminEditorFormCopyViewModel {
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
    total: u64,
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
        total,
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
    if table.is_empty {
        return view! {
            <div class=table_classes.empty_state>
                <p class=table_classes.total_label>
                    {table.empty_message}
                </p>
            </div>
        }
        .into_any();
    }

    view! {
        <div class="space-y-4">
            <div class=table_classes.total_label>
                {table.total_label.clone()}
            </div>
            <div class=table_classes.table_container>
                <table class=table_classes.table>
                    <thead class=table_classes.table_head>
                        <tr>
                            <th class=table_classes.header_cell>{table.title_header.clone()}</th>
                            <th class=table_classes.header_cell>{table.slug_header.clone()}</th>
                            <th class=table_classes.header_cell>{table.status_header.clone()}</th>
                            <th class=table_classes.header_cell>{table.locale_header.clone()}</th>
                            <th class=table_classes.actions_header_cell></th>
                        </tr>
                    </thead>
                    <tbody class=table_classes.table_body>
                        {table.rows
                            .into_iter()
                            .map(|row| {
                                let post_id_edit = row.post_id.clone();
                                let post_id_publish = row.post_id.clone();
                                let post_id_archive = row.post_id.clone();
                                let post_id_restore = row.post_id.clone();
                                let post_id_delete = row.post_id.clone();
                                let post_locale_edit = row.locale.clone();
                                let post_locale_publish = row.locale.clone();
                                let post_locale_archive = row.locale.clone();
                                let post_locale_restore = row.locale.clone();

                                view! {
                                    <tr class=table_classes.row>
                                        <td class=table_classes.title_cell>
                                            <div class=table_classes.title_text>{row.title.clone()}</div>
                                            <div class=table_classes.excerpt_text>
                                                {row.excerpt.clone()}
                                            </div>
                                        </td>
                                        <td class=table_classes.muted_cell>{row.slug.clone()}</td>
                                        <td class=table_classes.title_cell>
                                            <StatusBadge status=row.status.clone() />
                                        </td>
                                        <td class=table_classes.muted_cell>{row.locale.clone()}</td>
                                        <td class=table_classes.actions_cell>
                                            <div class=table_classes.actions_group>
                                                <button
                                                    type="button"
                                                    class=table_classes.primary_action_button
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
                                                            class=table_classes.primary_action_button
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
                                                            class=table_classes.primary_action_button
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
                                                            class=table_classes.primary_action_button
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
                                                    class=table_classes.destructive_action_button
                                                    disabled=row.is_busy
                                                    on:click={
                                                        move |_| on_delete.run(post_id_delete.clone())
                                                    }
                                                >
                                                    {row.delete_label.clone()}
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
    let badge = core::blog_post_admin_status_badge_view(status.as_str());
    view! {
        <span class=badge.class>
            {badge.status}
        </span>
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn apply_post_to_form(
    set_editing_post_id: WriteSignal<Option<String>>,
    set_editing_version: WriteSignal<Option<i32>>,
    set_title: WriteSignal<String>,
    set_slug: WriteSignal<String>,
    set_excerpt: WriteSignal<String>,
    set_content: WriteSignal<RichTextDocument>,
    set_locale: WriteSignal<String>,
    set_tags_input: WriteSignal<String>,
    set_publish_now: WriteSignal<bool>,
    post: &BlogPostDetail,
) {
    apply_form_state(
        set_editing_post_id,
        set_editing_version,
        set_title,
        set_slug,
        set_excerpt,
        set_content,
        set_locale,
        set_tags_input,
        set_publish_now,
        core::BlogPostEditorFormState::from_post(post),
    );
}

#[allow(clippy::too_many_arguments)]
pub(super) fn reset_form(
    set_editing_post_id: WriteSignal<Option<String>>,
    set_editing_version: WriteSignal<Option<i32>>,
    set_title: WriteSignal<String>,
    set_slug: WriteSignal<String>,
    set_excerpt: WriteSignal<String>,
    set_content: WriteSignal<RichTextDocument>,
    set_locale: WriteSignal<String>,
    set_tags_input: WriteSignal<String>,
    set_publish_now: WriteSignal<bool>,
    default_locale: &str,
) {
    apply_form_state(
        set_editing_post_id,
        set_editing_version,
        set_title,
        set_slug,
        set_excerpt,
        set_content,
        set_locale,
        set_tags_input,
        set_publish_now,
        core::BlogPostEditorFormState::empty(default_locale),
    );
}

#[allow(clippy::too_many_arguments)]
fn apply_form_state(
    set_editing_post_id: WriteSignal<Option<String>>,
    set_editing_version: WriteSignal<Option<i32>>,
    set_title: WriteSignal<String>,
    set_slug: WriteSignal<String>,
    set_excerpt: WriteSignal<String>,
    set_content: WriteSignal<RichTextDocument>,
    set_locale: WriteSignal<String>,
    set_tags_input: WriteSignal<String>,
    set_publish_now: WriteSignal<bool>,
    state: core::BlogPostEditorFormState,
) {
    set_editing_post_id.set(state.editing_post_id);
    set_editing_version.set(state.version);
    set_title.set(state.title);
    set_slug.set(state.slug);
    set_excerpt.set(state.excerpt);
    set_content.set(state.content);
    set_locale.set(state.locale);
    set_tags_input.set(state.tags_input);
    set_publish_now.set(state.publish_now);
}
