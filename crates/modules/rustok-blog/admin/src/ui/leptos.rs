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
    let submit_post = move |ev: SubmitEvent| {
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
    };

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

    view! {
            <div class=shell_classes.page>
                <BlogPostsPageHeader />

                <section class=shell_classes.layout>
                    <div class=shell_classes.list_card>
                        <BlogPostsListHeader
                            locale=list_locale
                            set_locale=set_list_locale
                            locale_filter_label=form_copy_view.locale_label.clone()
                        />

                        <Suspense
                            fallback=move || view! {
                                <div class=shell_classes.skeleton_stack>
                                    {(0..4).map(|_| view! {
                                        <div class=shell_classes.skeleton_row></div>
                                    }).collect_view()}
                                </div>
                            }
                        >
                            {move || {
                                posts_resource.get().map(|result| {
                                    let contract_unavailable = result
                                        .as_ref()
                                        .err()
                                        .map(transport::is_posts_contract_unavailable)
                                        .unwrap_or(false);
                                    let posts_view = core::blog_post_admin_posts_load_view_from_list(
                                        result.map_err(|err| err.to_string()),
                                        contract_unavailable,
                                        load_posts_error_label.as_str(),
                                    );

                                    match posts_view {
                                        // The error branch owns its own banner; every other
                                        // state renders the list region.
                                        core::BlogPostAdminPostsLoadViewModel::Error { message } => view! {
                                            <div class=shell_classes.load_error>
                                                {message}
                                            </div>
                                        }.into_any(),
                                        loaded => {
                                            // The resource carries the first page; later pages are
                                            // appended to `posts_rows`, so the signals win over the
                                            // view-model payload. A contract-unavailable load shows
                                            // an empty list regardless of what is still buffered.
                                            let rows = posts_rows.get();
                                            let cursor = posts_next_cursor.get();
                                            let load_more_error = posts_load_more_error.get();
                                            let empty_contract = matches!(
                                                loaded,
                                                core::BlogPostAdminPostsLoadViewModel::EmptyContractUnavailable,
                                            );
                                            let (rows, cursor, load_more_error) = if empty_contract {
                                                (Vec::new(), None, None)
                                            } else {
                                                (rows, cursor, load_more_error)
                                            };

                                            view! {
                                                <BlogPostsList
                                                    items=rows
                                                    next_cursor=cursor
                                                    load_more_error=load_more_error
                                                    sort=posts_sort.get()
                                                    editing_post_id=editing_post_id.get()
                                                    busy_key=busy_key.get()
                                                    on_load_more=load_more_posts
                                                    on_sort_change=change_posts_sort
                                                    on_edit=open_post
                                                    on_toggle_publish=toggle_publish
                                                    on_toggle_pin=toggle_pin
                                                    on_archive=archive_post
                                                    on_restore=restore_post
                                                    on_delete=delete_post
                                                />
                                            }.into_any()
                                        }
                                    }
                                })
                            }}
                        </Suspense>
                    </div>

                    <div class=shell_classes.sidebar>
                    <section class=shell_classes.form_card>
                        <div class=shell_classes.form_header>
                            <h2 class=shell_classes.form_title>
    {move || form_view_model.get().title}
                            </h2>
                            <p class=shell_classes.form_subtitle>{form_copy_view.subtitle.clone()}</p>
                        </div>

                        <Show when=move || editing_banner_view.get().visible>
                            <BlogEditBanner
                                banner_view=Signal::derive({
                                    let editing_banner_view = editing_banner_view;
                                    move || editing_banner_view.get()
                                })
                                on_reset=reset_current_post
                            />
                        </Show>

                        <form class="mt-5 space-y-4" on:submit=submit_post>
                            <label class=shell_classes.locale_filter_label>
                                <span class=form_field_classes.label_text>
                                    {form_copy_view.title_label.clone()}
                                </span>
                                <input
                                    type="text"
                                    class=form_field_classes.text_input
                                    prop:value=title
                                    on:input=move |ev| {
                                        let title_input = core::blog_post_admin_title_input_view(
                                            event_target_value(&ev),
                                        );
                                        set_title.set(title_input.title);
                                    }
                                />
                            </label>

                            <label class=shell_classes.locale_filter_label>
                                <span class=form_field_classes.label_text>
                                    {form_copy_view.slug_label.clone()}
                                </span>
                                <input
                                    type="text"
                                    class=form_field_classes.text_input
                                    prop:value=slug
                                    on:input=move |ev| set_slug.set(event_target_value(&ev))
                                />
                            </label>

                            <div class="grid gap-4">
                                <label class=shell_classes.locale_filter_label>
                                    <span class=form_field_classes.label_text>
                                        {form_copy_view.locale_label.clone()}
                                    </span>
                                    <input
                                        type="text"
                                        class=form_field_classes.text_input
                                        prop:value=locale
                                        disabled=move || editing_post_id.get().is_some()
                                        on:input=move |ev| set_locale.set(event_target_value(&ev))
                                    />
                                </label>

                            </div>

                            <label class=shell_classes.locale_filter_label>
                                <span class=form_field_classes.label_text>
                                    {form_copy_view.excerpt_label.clone()}
                                </span>
                                <textarea
                                    class=form_field_classes.textarea_short
                                    prop:value=excerpt
                                    on:input=move |ev| set_excerpt.set(event_target_value(&ev))
                                />
                            </label>

                            <BlogRichTextEditor
                                document=content
                                set_document=set_content
                                content_locale=locale
                                label=form_copy_view.content_label.clone()
                                disabled=Signal::derive(move || busy_key.get().is_some())
                            />

                            <label class=shell_classes.locale_filter_label>
                                <span class=form_field_classes.label_text>
                                    {form_copy_view.tags_label.clone()}
                                </span>
                                <input
                                    type="text"
                                    class=form_field_classes.text_input
                                    placeholder=form_copy_view.tags_placeholder.clone()
                                    prop:value=tags_input
                                    on:input=move |ev| set_tags_input.set(event_target_value(&ev))
                                />
                            </label>

                            <label class=shell_classes.locale_filter_label>
                                <span class=form_field_classes.label_text>
                                    {t(ui_locale.as_deref(), "blog.form.category", "Category")}
                                </span>
                                <select class=form_field_classes.text_input
                                    prop:value=category_id
                                    on:change=move |ev| set_category_id.set(event_target_value(&ev))
                                >
                                    <option value="">{t(ui_locale.as_deref(), "blog.form.uncategorized", "Uncategorized")}</option>
                                    {move || {
                                        match categories_resource.get() {
                                            Some(Ok(items)) => items.into_iter().map(|item| {
                                                view! { <option value=item.id>{item.name}</option> }
                                            }).collect_view().into_any(),
                                            _ => ().into_any(),
                                        }
                                    }}
                                </select>
                                {move || categories_resource.get().and_then(|result| result.err()).map(|error| {
                                    view! { <span class="text-xs text-destructive">{format!("{}: {error}", categories_unavailable_label.clone())}</span> }
                                })}
                            </label>

                            <label class=shell_classes.locale_filter_label>
                                <span class=form_field_classes.label_text>{t(ui_locale.as_deref(), "blog.form.channels", "Channels (comma separated; empty = all channels)")}</span>
                                <input type="text" class=form_field_classes.text_input
                                    placeholder="default, mobile"
                                    prop:value=channel_slugs_input
                                    on:input=move |ev| set_channel_slugs_input.set(event_target_value(&ev))
                                />
                            </label>

                            <label class=shell_classes.locale_filter_label>
                                <span class=form_field_classes.label_text>{t(ui_locale.as_deref(), "blog.form.featuredImage", "Featured image URL")}</span>
                                <input type="text" class=form_field_classes.text_input
                                    placeholder="https://example.com/cover.jpg"
                                    prop:value=featured_image_url
                                    on:input=move |ev| set_featured_image_url.set(event_target_value(&ev))
                                />
                            </label>

                            <label class=shell_classes.locale_filter_label>
                                <span class=form_field_classes.label_text>{t(ui_locale.as_deref(), "blog.form.seoTitle", "SEO title (Blog default)")}</span>
                                <input type="text" class=form_field_classes.text_input
                                    prop:value=seo_title
                                    on:input=move |ev| set_seo_title.set(event_target_value(&ev))
                                />
                            </label>
                            <label class=shell_classes.locale_filter_label>
                                <span class=form_field_classes.label_text>{t(ui_locale.as_deref(), "blog.form.seoDescription", "SEO description (Blog default)")}</span>
                                <textarea class=form_field_classes.textarea_short
                                    prop:value=seo_description
                                    on:input=move |ev| set_seo_description.set(event_target_value(&ev))
                                />
                            </label>

                            <Show when=move || schedule_editable.get()>
                                <label class=shell_classes.locale_filter_label>
                                    <span class=form_field_classes.label_text>
                                        {t(ui_locale.as_deref(), "blog.form.scheduledAt", "Schedule (RFC 3339 with timezone)")}
                                    </span>
                                    <input type="text" class=form_field_classes.text_input
                                        placeholder="2026-11-01T10:00:00+01:00"
                                        prop:value=scheduled_at
                                        on:input=move |ev| {
                                            let value = event_target_value(&ev);
                                            if !value.trim().is_empty() {
                                                set_publish_now.set(false);
                                            }
                                            set_scheduled_at.set(value);
                                        }
                                    />
                                </label>
                            </Show>

                            <Show when=move || editing_post_id.get().is_none()>
                                <label class=form_field_classes.checkbox_label>
                                    <input
                                        type="checkbox"
                                        prop:checked=publish_now
                                        on:change=move |ev| {
                                            let checked = event_target_checked(&ev);
                                            if checked {
                                                set_scheduled_at.set(String::new());
                                            }
                                            set_publish_now.set(checked);
                                        }
                                    />
                                    {form_copy_view.publish_now_label.clone()}
                                </label>
                            </Show>

                            <Show when=move || issue_banner_view.get().visible>
                                <div class=move || issue_banner_view.get().class>
                                    {move || {
                                        let issue_banner = issue_banner_view.get();

                                        view! {
                                            <span>
                                                <strong>{issue_banner.label}</strong>
                                                {": "}
                                                {issue_banner.message}
                                            </span>
                                        }
                                    }}
                                </div>
                            </Show>

                            <button
                                type="submit"
                                class=form_field_classes.submit_button
    disabled=move || form_view_model.get().submit_disabled
                            >
    {move || form_view_model.get().submit_label}
                            </button>
                        </form>
                    </section>

                    <BlogPostSeoPanel
                        target_kind=seo_target_kind
                        target_id=Signal::derive(move || editing_post_id.get())
                        host_locale=Signal::derive({
                            let host_locale_for_seo = host_locale_for_seo.clone();
                            move || host_locale_for_seo.clone()
                        })
                    />
                    </div>
                </section>
            </div>
        }
        .into_any()
}
