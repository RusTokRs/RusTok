mod page;

use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_ui_routing::{use_route_query_value, use_route_query_writer};
use rustok_api::{RichTextDocument, WritePathIssue};
use rustok_seo_targets::{SeoTargetSlug, builtin_slug as seo_builtin_slug};
use rustok_ui_core::{AdminQueryKey, UiRouteContext};

use super::components::{
    BlogEditBanner, BlogPostFormSetters, BlogPostSeoPanel, BlogPostsList, BlogPostsListHeader,
    BlogPostsPageHeader, BlogSeoConfigError, apply_post_to_form, blog_form_copy_view_model,
    blog_form_view_model, local_resource, reset_form,
};
use super::richtext::BlogRichTextEditor;
use crate::i18n::t;
use crate::{core, transport};


struct BlogAdminPageState {
    ui_locale: Option<String>,
    host_locale_for_seo: String,
    seo_target_kind: SeoTargetSlug,
    form_copy_view: core::BlogPostAdminEditorFormCopyViewModel,
    form_field_classes: core::BlogPostAdminEditorFieldClassesViewModel,
    shell_classes: core::BlogPostAdminShellClassesViewModel,
    form_view_model: Memo<core::BlogPostAdminFormViewModel>,
    editing_banner_view: Memo<core::BlogPostAdminEditBannerViewModel>,
    issue_banner_view: Memo<core::BlogPostAdminIssueBannerViewModel>,
    reset_current_post: Callback<()>,
    list_locale: ReadSignal<String>,
    set_list_locale: WriteSignal<String>,
    posts_resource: LocalResource<Result<crate::model::BlogPostList, transport::ApiError>>,
    posts_rows: RwSignal<Vec<crate::model::BlogPostListItem>>,
    posts_next_cursor: RwSignal<Option<String>>,
    posts_load_more_error: RwSignal<Option<String>>,
    posts_sort: RwSignal<core::BlogPostAdminSort>,
    load_posts_error_label: String,
    load_more_posts: Callback<String>,
    change_posts_sort: Callback<core::BlogPostAdminSort>,
    open_post: Callback<(String, String)>,
    toggle_publish: Callback<(String, bool, String)>,
    toggle_pin: Callback<(String, bool, String)>,
    archive_post: Callback<(String, String)>,
    restore_post: Callback<(String, String)>,
    delete_post: Callback<String>,
    categories_resource: LocalResource<Result<Vec<crate::model::BlogCategoryOption>, transport::ApiError>>,
    categories_unavailable_label: String,
    form_setters: BlogPostFormSetters,
    editing_post_id: ReadSignal<Option<String>>,
    title: ReadSignal<String>,
    slug: ReadSignal<String>,
    excerpt: ReadSignal<String>,
    content: ReadSignal<RichTextDocument>,
    locale: ReadSignal<String>,
    tags_input: ReadSignal<String>,
    category_id: ReadSignal<String>,
    channel_slugs_input: ReadSignal<String>,
    featured_image_url: ReadSignal<String>,
    seo_title: ReadSignal<String>,
    seo_description: ReadSignal<String>,
    scheduled_at: ReadSignal<String>,
    schedule_editable: ReadSignal<bool>,
    publish_now: ReadSignal<bool>,
    busy_key: ReadSignal<Option<String>>,
    submit_post: Callback<SubmitEvent>,
}

#[component]
pub fn BlogAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let ui_locale = route_context.locale.clone();
    let host_locale_for_seo = ui_locale.clone().unwrap_or_default();
    let seo_target_kind = match SeoTargetSlug::new(seo_builtin_slug::BLOG_POST) {
        Ok(target_kind) => target_kind,
        Err(_) => {
            return view! { <BlogSeoConfigError ui_locale=ui_locale.clone() /> }.into_any();
        }
    };
    let selected_post_query = use_route_query_value(AdminQueryKey::PostId.as_str());
    let query_writer = use_route_query_writer();
    let token = use_token();
    let tenant = use_tenant();
    let default_locale = ui_locale.clone().unwrap_or_default();
    let load_posts_error_label = t(
        ui_locale.as_deref(),
        "blog.error.loadPosts",
        "Failed to load posts",
    );
    let form_create_new_instead = t(
        ui_locale.as_deref(),
        "blog.form.createNewInstead",
        "Create new instead",
    );

    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (editing_post_id, set_editing_post_id) = signal(Option::<String>::None);
    let (editing_version, set_editing_version) = signal(Option::<i32>::None);
    let (title, set_title) = signal(String::new());
    let (slug, set_slug) = signal(String::new());
    let (excerpt, set_excerpt) = signal(String::new());
    let (content, set_content) = signal(RichTextDocument::empty());
    let (locale, set_locale) = signal(default_locale.clone());
    // Keep the owner translation locale independent of admin list filters.
    let (list_locale, set_list_locale) = signal(default_locale.clone());
    let (tags_input, set_tags_input) = signal(String::new());
    let (category_id, set_category_id) = signal(String::new());
    let (channel_slugs_input, set_channel_slugs_input) = signal(String::new());
    let (featured_image_url, set_featured_image_url) = signal(String::new());
    let (seo_title, set_seo_title) = signal(String::new());
    let (seo_description, set_seo_description) = signal(String::new());
    let (scheduled_at, set_scheduled_at) = signal(String::new());
    let (schedule_editable, set_schedule_editable) = signal(true);
    let (publish_now, set_publish_now) = signal(false);
    let (busy_key, set_busy_key) = signal(Option::<String>::None);
    let (submit_error, set_submit_error) = signal(Option::<WritePathIssue>::None);
    let reset_form_action = Callback::new({
        let default_locale = default_locale.clone();
        move |_| {
            reset_form(
                BlogPostFormSetters {
                    editing_post_id: set_editing_post_id,
                    editing_version: set_editing_version,
                    title: set_title,
                    slug: set_slug,
                    excerpt: set_excerpt,
                    content: set_content,
                    locale: set_locale,
                    tags_input: set_tags_input,
                    category_id: set_category_id,
                    channel_slugs_input: set_channel_slugs_input,
                    featured_image_url: set_featured_image_url,
                    seo_title: set_seo_title,
                    seo_description: set_seo_description,
                    scheduled_at: set_scheduled_at,
                    schedule_editable: set_schedule_editable,
                    publish_now: set_publish_now,
                },
                default_locale.as_str(),
            )
        }
    });
    let editing_banner_locale = ui_locale.clone();
    let editing_banner_create_new_label = form_create_new_instead.clone();
    let editing_banner_view = Memo::new(move |_| {
        core::blog_post_admin_edit_banner_view(
            editing_post_id.get().as_deref(),
            t(
                editing_banner_locale.as_deref(),
                "blog.form.editingBanner",
                "Editing post {id}",
            )
            .as_str(),
            editing_banner_create_new_label.clone(),
        )
    });
    let issue_banner_view =
        Memo::new(move |_| core::blog_post_admin_issue_banner_view(submit_error.get().as_ref()));
    let form_copy_view = blog_form_copy_view_model(ui_locale.as_deref());
    let form_field_classes = core::blog_post_admin_editor_field_classes_view();
    let shell_classes = core::blog_post_admin_shell_classes_view();

    let categories_unavailable_label = t(
        ui_locale.as_deref(),
        "blog.error.categories",
        "Categories unavailable",
    );
    let form_view_locale = ui_locale.clone();
    let form_view_model = Memo::new(move |_| {
        blog_form_view_model(
            form_view_locale.as_deref(),
            editing_post_id.get().as_deref(),
            busy_key.get().as_deref(),
        )
    });
    let reset_current_post = Callback::new({
        let query_writer = query_writer.clone();
        move |_| {
            query_writer.apply_query_intent(core::blog_post_admin_clear_post_query_intent());
            reset_form_action.run(());
        }
    });

    // Categories are loaded from the same selected owner transport as post edits.
    // A failed lookup remains visible; the editor never silently replaces a category.
    let categories_resource = local_resource(
        move || (token.get(), tenant.get(), locale.get(), refresh_nonce.get()),
        move |(token_value, tenant_value, locale_value, _)| async move {
            transport::fetch_categories(
                token_value,
                tenant_value,
                core::locale_arg(locale_value.as_str()),
            )
            .await
        },
    );

    // Admin posts are cursor-paged. The resource loads the first page; later pages are appended to `posts_rows`.
    let posts_rows = RwSignal::new(Vec::<crate::model::BlogPostListItem>::new());
    let posts_next_cursor = RwSignal::new(None::<String>);
    let posts_load_more_error = RwSignal::new(None::<String>);
    // The sort is part of the request source, so changing it reloads page one instead of
    // reordering the rows already on screen.
    let posts_sort = RwSignal::new(core::BlogPostAdminSort::default());
    let posts_resource = local_resource(
        move || {
            (
                token.get(),
                tenant.get(),
                refresh_nonce.get(),
                list_locale.get(),
                posts_sort.get(),
            )
        },
        move |(token_value, tenant_value, _, locale_value, sort_value)| async move {
            transport::fetch_posts(
                token_value,
                tenant_value,
                core::locale_arg(locale_value.as_str()),
                None,
                sort_value,
            )
            .await
        },
    );
    Effect::new(move |_| {
        if let Some(Ok(list)) = posts_resource.get() {
            posts_rows.set(list.items);
            posts_next_cursor.set(list.next_cursor);
            posts_load_more_error.set(None);
        }
    });
    let load_more_error_label = load_posts_error_label.clone();
    let load_more_posts = Callback::new(move |cursor: String| {
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let locale_value = list_locale.get_untracked();
        let sort_value = posts_sort.get_untracked();
        let error_label = load_more_error_label.clone();
        posts_load_more_error.set(None);
        spawn_local(async move {
            match transport::fetch_posts(
                token_value,
                tenant_value,
                core::locale_arg(locale_value.as_str()),
                Some(cursor),
                sort_value,
            )
            .await
            {
                Ok(list) => {
                    posts_rows.update(|rows| rows.extend(list.items));
                    posts_next_cursor.set(list.next_cursor);
                }
                Err(err) => {
                    posts_load_more_error.set(Some(format!("{error_label}: {err}")));
                }
            }
        });
    });

    // A sort change drops the loaded pages: the cursor of the previous sort cannot be
    // replayed against the new one.
    let change_posts_sort = Callback::new(move |sort: core::BlogPostAdminSort| {
        posts_sort.set(sort);
        posts_load_more_error.set(None);
    });

    let edit_post_locale = ui_locale.clone();
    let edit_post_reset_form_action = reset_form_action;
    let edit_post = Callback::new(move |(post_id, requested_locale): (String, String)| {
        let reset_form_to_defaults = edit_post_reset_form_action;
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let ui_locale = edit_post_locale.clone();
        set_submit_error.set(None);
        set_busy_key.set(Some(core::busy_key_for_edit(post_id.as_str())));

        spawn_local(async move {
            match transport::fetch_post(
                token_value,
                tenant_value,
                post_id.clone(),
                core::locale_arg(requested_locale.as_str()),
            )
            .await
            {
                Ok(post) => {
                    let result_view = core::blog_post_load_result_view(
                        post.is_some(),
                        t(
                            ui_locale.as_deref(),
                            "blog.error.postNotFound",
                            "Post not found for editing.",
                        ),
                    );

                    match (result_view, post) {
                        (Ok(view_model), Some(post)) => {
                            if view_model.apply_returned_post_to_form {
                                apply_post_to_form(
                                    BlogPostFormSetters {
                                        editing_post_id: set_editing_post_id,
                                        editing_version: set_editing_version,
                                        title: set_title,
                                        slug: set_slug,
                                        excerpt: set_excerpt,
                                        content: set_content,
                                        locale: set_locale,
                                        tags_input: set_tags_input,
                    category_id: set_category_id,
                    channel_slugs_input: set_channel_slugs_input,
                    featured_image_url: set_featured_image_url,
                    seo_title: set_seo_title,
                    seo_description: set_seo_description,
                    scheduled_at: set_scheduled_at,
                    schedule_editable: set_schedule_editable,
                                        publish_now: set_publish_now,
                                    },
                                    &post,
                                );
                            }
                        }
                        (Ok(view_model), None) => {
                            if view_model.reset_form {
                                reset_form_to_defaults.run(());
                            }
                        }
                        (Err(issue), _) => {
                            reset_form_to_defaults.run(());
                            set_submit_error.set(Some(issue));
                        }
                    }
                }
                Err(err) => {
                    reset_form_to_defaults.run(());
                    set_submit_error.set(Some(core::blog_post_transport_failure_issue(
                        &t(
                            ui_locale.as_deref(),
                            "blog.error.loadPost",
                            "Failed to load post",
                        ),
                        &err.to_string(),
                    )));
                }
            }

            set_busy_key.set(None);
        });
    });
    let initial_edit_post = edit_post;
    let effect_default_locale = default_locale.clone();
    Effect::new(move |_| {
        if let Some(request) = core::selected_post_request(
            selected_post_query.get().as_deref(),
            effect_default_locale.as_str(),
        ) {
            initial_edit_post.run(request);
        } else {
            reset_form_action.run(())
        }
    });

    let submit_ui_locale = ui_locale.clone();
    let submit_query_writer = query_writer.clone();
    let submit_post = Callback::new(move |ev: SubmitEvent| {
        ev.prevent_default();
        set_submit_error.set(None);
        let submit_ui_locale = submit_ui_locale.clone();
        let submit_query_writer = submit_query_writer.clone();

        let locale_value = locale.get_untracked();
        let title_value = title.get_untracked();
        let slug_value = slug.get_untracked();
        let excerpt_value = excerpt.get_untracked();
        let content_value = content.get_untracked();
        let tags_value = tags_input.get_untracked();
        let category_id_value = category_id.get_untracked();
        let channel_slugs_value = channel_slugs_input.get_untracked();
        let featured_image_value = featured_image_url.get_untracked();
        let seo_title_value = seo_title.get_untracked();
        let seo_description_value = seo_description.get_untracked();
        let scheduled_at_value = scheduled_at.get_untracked();
        let draft = core::build_blog_post_draft(core::BlogPostFormInput {
            locale: &locale_value,
            version: editing_version.get_untracked(),
            title: &title_value,
            slug: &slug_value,
            excerpt: &excerpt_value,
            content: &content_value,
            publish: publish_now.get_untracked(),
            tags: &tags_value,
            category_id: &category_id_value,
            channel_slugs: &channel_slugs_value,
            featured_image_url: &featured_image_value,
            seo_title: &seo_title_value,
            seo_description: &seo_description_value,
            scheduled_at: &scheduled_at_value,
            schedule_editable: schedule_editable.get_untracked(),
        });

        let required_fields_message = t(
            submit_ui_locale.as_deref(),
            "blog.error.requiredFields",
            "Title and content are required to save a blog post.",
        );
        let command = match core::prepare_blog_post_save_command(
            editing_post_id.get_untracked(),
            draft,
            required_fields_message,
        ) {
            Ok(command) => command,
            Err(issue) => {
                set_submit_error.set(Some(issue));
                return;
            }
        };

        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        set_busy_key.set(Some(command.busy_key.clone()));

        spawn_local(async move {
            let result = match command.operation {
                core::BlogPostSaveOperation::Update { post_id } => {
                    transport::update_post(token_value, tenant_value, post_id, command.draft).await
                }
                core::BlogPostSaveOperation::Create => {
                    transport::create_post(token_value, tenant_value, command.draft).await
                }
            };

            match result {
                Ok(post) => {
                    let result_view = core::blog_post_save_result_view(post.id.as_str());
                    if result_view.apply_returned_post_to_form {
                        apply_post_to_form(
                            BlogPostFormSetters {
                                editing_post_id: set_editing_post_id,
                                editing_version: set_editing_version,
                                title: set_title,
                                slug: set_slug,
                                excerpt: set_excerpt,
                                content: set_content,
                                locale: set_locale,
                                tags_input: set_tags_input,
                    category_id: set_category_id,
                    channel_slugs_input: set_channel_slugs_input,
                    featured_image_url: set_featured_image_url,
                    seo_title: set_seo_title,
                    seo_description: set_seo_description,
                    scheduled_at: set_scheduled_at,
                    schedule_editable: set_schedule_editable,
                                publish_now: set_publish_now,
                            },
                            &post,
                        );
                    }
                    if result_view.refresh_posts {
                        set_refresh_nonce.update(|value| *value += 1);
                    }
                    if let Some(intent) = result_view.selected_post_query_intent {
                        submit_query_writer.apply_query_intent(intent);
                    }
                }
                Err(err) => {
                    set_submit_error.set(Some(core::blog_post_transport_failure_issue(
                        &t(
                            submit_ui_locale.as_deref(),
                            "blog.error.savePost",
                            "Failed to save post",
                        ),
                        &err.to_string(),
                    )));
                }
            }

            set_busy_key.set(None);
        });
    });

    let toggle_publish_locale = ui_locale.clone();
    let toggle_publish = Callback::new(
        move |(post_id, publish, post_locale): (String, bool, String)| {
            let token_value = token.get_untracked();
            let tenant_value = tenant.get_untracked();
            let ui_locale = toggle_publish_locale.clone();
            let command =
                core::prepare_blog_post_status_command(post_id, publish, post_locale.as_str());
            set_submit_error.set(None);
            set_busy_key.set(Some(command.busy_key.clone()));

            spawn_local(async move {
                let result = match command.operation {
                    core::BlogPostStatusOperation::Publish => {
                        transport::publish_post(
                            token_value,
                            tenant_value,
                            command.post_id.clone(),
                            command.locale.clone(),
                        )
                        .await
                    }
                    core::BlogPostStatusOperation::Unpublish => {
                        transport::unpublish_post(
                            token_value,
                            tenant_value,
                            command.post_id.clone(),
                            command.locale.clone(),
                        )
                        .await
                    }
                };

                match result {
                    Ok(post) => {
                        let result_view = core::blog_post_mutation_result_view(
                            editing_post_id.get_untracked().as_deref(),
                            post.id.as_str(),
                        );
                        if result_view.apply_returned_post_to_form {
                            apply_post_to_form(
                                BlogPostFormSetters {
                                    editing_post_id: set_editing_post_id,
                                    editing_version: set_editing_version,
                                    title: set_title,
                                    slug: set_slug,
                                    excerpt: set_excerpt,
                                    content: set_content,
                                    locale: set_locale,
                                    tags_input: set_tags_input,
                    category_id: set_category_id,
                    channel_slugs_input: set_channel_slugs_input,
                    featured_image_url: set_featured_image_url,
                    seo_title: set_seo_title,
                    seo_description: set_seo_description,
                    scheduled_at: set_scheduled_at,
                    schedule_editable: set_schedule_editable,
                                    publish_now: set_publish_now,
                                },
                                &post,
                            );
                        }
                        if result_view.refresh_posts {
                            set_refresh_nonce.update(|value| *value += 1);
                        }
                    }
                    Err(err) => {
                        set_submit_error.set(Some(core::blog_post_transport_failure_issue(
                            &t(
                                ui_locale.as_deref(),
                                "blog.error.updateStatus",
                                "Failed to update post status",
                            ),
                            &err.to_string(),
                        )));
                    }
                }

                set_busy_key.set(None);
            });
        },
    );

    let toggle_pin_locale = ui_locale.clone();
    let toggle_pin = Callback::new(
        move |(post_id, pin, post_locale): (String, bool, String)| {
            let token_value = token.get_untracked();
            let tenant_value = tenant.get_untracked();
            let ui_locale = toggle_pin_locale.clone();
            let command =
                core::prepare_blog_post_pin_command(post_id, pin, post_locale.as_str());
            set_submit_error.set(None);
            set_busy_key.set(Some(command.busy_key.clone()));

            spawn_local(async move {
                let result = if command.pin {
                    transport::pin_post(
                        token_value,
                        tenant_value,
                        command.post_id.clone(),
                        command.locale.clone(),
                    )
                    .await
                } else {
                    transport::unpin_post(
                        token_value,
                        tenant_value,
                        command.post_id.clone(),
                        command.locale.clone(),
                    )
                    .await
                };

                match result {
                    Ok(post) => {
                        let result_view = core::blog_post_mutation_result_view(
                            editing_post_id.get_untracked().as_deref(),
                            post.id.as_str(),
                        );
                        if result_view.apply_returned_post_to_form {
                            apply_post_to_form(
                                BlogPostFormSetters {
                                    editing_post_id: set_editing_post_id,
                                    editing_version: set_editing_version,
                                    title: set_title,
                                    slug: set_slug,
                                    excerpt: set_excerpt,
                                    content: set_content,
                                    locale: set_locale,
                                    tags_input: set_tags_input,
                    category_id: set_category_id,
                    channel_slugs_input: set_channel_slugs_input,
                    featured_image_url: set_featured_image_url,
                    seo_title: set_seo_title,
                    seo_description: set_seo_description,
                    scheduled_at: set_scheduled_at,
                    schedule_editable: set_schedule_editable,
                                    publish_now: set_publish_now,
                                },
                                &post,
                            );
                        }
                        if result_view.refresh_posts {
                            set_refresh_nonce.update(|value| *value += 1);
                        }
                    }
                    Err(err) => {
                        set_submit_error.set(Some(core::blog_post_transport_failure_issue(
                            &t(
                                ui_locale.as_deref(),
                                "blog.error.updatePin",
                                "Failed to change post pin state",
                            ),
                            &err.to_string(),
                        )));
                    }
                }

                set_busy_key.set(None);
            });
        },
    );

    let archive_post_locale = ui_locale.clone();
    let archive_post = Callback::new(move |(post_id, post_locale): (String, String)| {
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let ui_locale = archive_post_locale.clone();
        let command = core::prepare_blog_post_archive_command(post_id, post_locale.as_str());
        set_submit_error.set(None);
        set_busy_key.set(Some(command.busy_key.clone()));

        spawn_local(async move {
            match transport::archive_post(
                token_value,
                tenant_value,
                command.post_id.clone(),
                command.locale.clone(),
            )
            .await
            {
                Ok(post) => {
                    let result_view = core::blog_post_mutation_result_view(
                        editing_post_id.get_untracked().as_deref(),
                        post.id.as_str(),
                    );
                    if result_view.apply_returned_post_to_form {
                        apply_post_to_form(
                            BlogPostFormSetters {
                                editing_post_id: set_editing_post_id,
                                editing_version: set_editing_version,
                                title: set_title,
                                slug: set_slug,
                                excerpt: set_excerpt,
                                content: set_content,
                                locale: set_locale,
                                tags_input: set_tags_input,
                    category_id: set_category_id,
                    channel_slugs_input: set_channel_slugs_input,
                    featured_image_url: set_featured_image_url,
                    seo_title: set_seo_title,
                    seo_description: set_seo_description,
                    scheduled_at: set_scheduled_at,
                    schedule_editable: set_schedule_editable,
                                publish_now: set_publish_now,
                            },
                            &post,
                        );
                    }
                    if result_view.refresh_posts {
                        set_refresh_nonce.update(|value| *value += 1);
                    }
                }
                Err(err) => {
                    set_submit_error.set(Some(core::blog_post_transport_failure_issue(
                        &t(
                            ui_locale.as_deref(),
                            "blog.error.archivePost",
                            "Failed to archive post",
                        ),
                        &err.to_string(),
                    )));
                }
            }

            set_busy_key.set(None);
        });
    });

    let restore_post_locale = ui_locale.clone();
    let restore_post = Callback::new(move |(post_id, post_locale): (String, String)| {
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let ui_locale = restore_post_locale.clone();
        let command = core::prepare_blog_post_restore_command(post_id, post_locale.as_str());
        set_submit_error.set(None);
        set_busy_key.set(Some(command.busy_key.clone()));

        spawn_local(async move {
            match transport::restore_post(
                token_value,
                tenant_value,
                command.post_id.clone(),
                command.locale.clone(),
            )
            .await
            {
                Ok(post) => {
                    let result_view = core::blog_post_mutation_result_view(
                        editing_post_id.get_untracked().as_deref(),
                        post.id.as_str(),
                    );
                    if result_view.apply_returned_post_to_form {
                        apply_post_to_form(
                            BlogPostFormSetters {
                                editing_post_id: set_editing_post_id,
                                editing_version: set_editing_version,
                                title: set_title,
                                slug: set_slug,
                                excerpt: set_excerpt,
                                content: set_content,
                                locale: set_locale,
                                tags_input: set_tags_input,
                    category_id: set_category_id,
                    channel_slugs_input: set_channel_slugs_input,
                    featured_image_url: set_featured_image_url,
                    seo_title: set_seo_title,
                    seo_description: set_seo_description,
                    scheduled_at: set_scheduled_at,
                    schedule_editable: set_schedule_editable,
                                publish_now: set_publish_now,
                            },
                            &post,
                        );
                    }
                    if result_view.refresh_posts {
                        set_refresh_nonce.update(|value| *value += 1);
                    }
                }
                Err(err) => {
                    set_submit_error.set(Some(core::blog_post_transport_failure_issue(
                        &t(
                            ui_locale.as_deref(),
                            "blog.error.restorePost",
                            "Failed to restore post",
                        ),
                        &err.to_string(),
                    )));
                }
            }

            set_busy_key.set(None);
        });
    });

    let delete_post_locale = ui_locale.clone();
    let delete_post_reset_form_action = reset_form_action;
    let delete_query_writer = query_writer.clone();
    let delete_post = Callback::new(move |post_id: String| {
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let ui_locale = delete_post_locale.clone();
        let reset_form_to_defaults = delete_post_reset_form_action;
        let delete_query_writer = delete_query_writer.clone();
        let command = core::prepare_blog_post_delete_command(post_id);
        set_submit_error.set(None);
        set_busy_key.set(Some(command.busy_key.clone()));

        spawn_local(async move {
            match transport::delete_post(token_value, tenant_value, command.post_id.clone()).await {
                Ok(deleted) => {
                    let delete_result = core::blog_post_delete_result_view(
                        deleted,
                        editing_post_id.get_untracked().as_deref(),
                        command.post_id.as_str(),
                        t(
                            ui_locale.as_deref(),
                            "blog.error.deleteReturnedFalse",
                            "Delete post returned false. Unpublish or archive it first.",
                        ),
                    );

                    match delete_result {
                        Ok(view_model) => {
                            if let Some(intent) = view_model.selected_post_query_intent {
                                delete_query_writer.apply_query_intent(intent);
                            }
                            if view_model.reset_form {
                                reset_form_to_defaults.run(());
                            }
                            if view_model.refresh_posts {
                                set_refresh_nonce.update(|value| *value += 1);
                            }
                        }
                        Err(issue) => {
                            set_submit_error.set(Some(issue));
                        }
                    }
                }
                Err(err) => {
                    set_submit_error.set(Some(core::blog_post_transport_failure_issue(
                        &t(
                            ui_locale.as_deref(),
                            "blog.error.deletePost",
                            "Failed to delete post",
                        ),
                        &err.to_string(),
                    )));
                }
            }

            set_busy_key.set(None);
        });
    });
    let open_query_writer = query_writer.clone();
    let open_post = Callback::new(move |(post_id, _requested_locale): (String, String)| {
        open_query_writer.apply_query_intent(core::blog_post_admin_open_post_query_intent(post_id));
    });

    page::render(BlogAdminPageState {
        ui_locale,
        host_locale_for_seo,
        seo_target_kind,
        form_copy_view,
        form_field_classes,
        shell_classes,
        form_view_model,
        editing_banner_view,
        issue_banner_view,
        reset_current_post,
        list_locale,
        set_list_locale,
        posts_resource,
        posts_rows,
        posts_next_cursor,
        posts_load_more_error,
        posts_sort,
        load_posts_error_label,
        load_more_posts,
        change_posts_sort,
        open_post,
        toggle_publish,
        toggle_pin,
        archive_post,
        restore_post,
        delete_post,
        categories_resource,
        categories_unavailable_label,
        form_setters: BlogPostFormSetters {
            editing_post_id: set_editing_post_id,
            editing_version: set_editing_version,
            title: set_title,
            slug: set_slug,
            excerpt: set_excerpt,
            content: set_content,
            locale: set_locale,
            tags_input: set_tags_input,
            category_id: set_category_id,
            channel_slugs_input: set_channel_slugs_input,
            featured_image_url: set_featured_image_url,
            seo_title: set_seo_title,
            seo_description: set_seo_description,
            scheduled_at: set_scheduled_at,
            schedule_editable: set_schedule_editable,
            publish_now: set_publish_now,
        },
        editing_post_id,
        title,
        slug,
        excerpt,
        content,
        locale,
        tags_input,
        category_id,
        channel_slugs_input,
        featured_image_url,
        seo_title,
        seo_description,
        scheduled_at,
        schedule_editable,
        publish_now,
        busy_key,
        submit_post,
    }).into_any()
}
