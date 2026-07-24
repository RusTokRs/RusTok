use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_ui_routing::{use_route_query_value, use_route_query_writer};
use rustok_api::{RichTextDocument, WritePathIssue};
use rustok_seo_admin_support::SeoEntityPanel;
use rustok_seo_targets::{SeoTargetSlug, builtin_slug as seo_builtin_slug};
use rustok_ui_core::{AdminQueryKey, UiRouteContext};

use super::richtext::BlogRichTextEditor;
use crate::i18n::t;
use crate::model::{BlogPostDetail, BlogPostListItem};
use crate::{core, transport};

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
pub fn BlogAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let ui_locale = route_context.locale.clone();
    let seo_locale = ui_locale.clone();
    let host_locale_for_seo = ui_locale.clone().unwrap_or_default();
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
    let (title, set_title) = signal(String::new());
    let (slug, set_slug) = signal(String::new());
    let (excerpt, set_excerpt) = signal(String::new());
    let (content, set_content) = signal(RichTextDocument::empty());
    let (locale, set_locale) = signal(default_locale.clone());
    let (tags_input, set_tags_input) = signal(String::new());
    let (publish_now, set_publish_now) = signal(false);
    let (busy_key, set_busy_key) = signal(Option::<String>::None);
    let (submit_error, set_submit_error) = signal(Option::<WritePathIssue>::None);
    let reset_form_action = Callback::new({
        let default_locale = default_locale.clone();
        move |_| {
            reset_form(
                set_editing_post_id,
                set_title,
                set_slug,
                set_excerpt,
                set_content,
                set_locale,
                set_tags_input,
                set_publish_now,
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

    let posts_resource = local_resource(
        move || (token.get(), tenant.get(), refresh_nonce.get(), locale.get()),
        move |(token_value, tenant_value, _, locale_value)| async move {
            transport::fetch_posts(
                token_value,
                tenant_value,
                core::locale_arg(locale_value.as_str()),
            )
            .await
        },
    );

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
                                    set_editing_post_id,
                                    set_title,
                                    set_slug,
                                    set_excerpt,
                                    set_content,
                                    set_locale,
                                    set_tags_input,
                                    set_publish_now,
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
        let draft = core::build_blog_post_draft(core::BlogPostFormInput {
            locale: &locale_value,
            title: &title_value,
            slug: &slug_value,
            excerpt: &excerpt_value,
            content: &content_value,
            publish: publish_now.get_untracked(),
            tags: &tags_value,
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
                            set_editing_post_id,
                            set_title,
                            set_slug,
                            set_excerpt,
                            set_content,
                            set_locale,
                            set_tags_input,
                            set_publish_now,
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
                                set_editing_post_id,
                                set_title,
                                set_slug,
                                set_excerpt,
                                set_content,
                                set_locale,
                                set_tags_input,
                                set_publish_now,
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
                            set_editing_post_id,
                            set_title,
                            set_slug,
                            set_excerpt,
                            set_content,
                            set_locale,
                            set_tags_input,
                            set_publish_now,
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

                <section class=shell_classes.layout>
                    <div class=shell_classes.list_card>
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
                                <span class=shell_classes.locale_filter_text>
                                    {form_copy_view.locale_label.clone()}
                                </span>
                                <input
                                    type="text"
                                    class=shell_classes.locale_filter_input
                                    prop:value=locale
                                    on:input=move |ev| set_locale.set(event_target_value(&ev))
                                />
                            </label>
                        </div>

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
                                        core::BlogPostAdminPostsLoadViewModel::Loaded { items, total } => view! {
                                            <BlogPostsTable
                                                items=items
                                                total=total
                                                editing_post_id=editing_post_id.get()
                                                busy_key=busy_key.get()
                                                on_edit=open_post
                                                on_toggle_publish=toggle_publish
                                                on_archive=archive_post
                                                on_delete=delete_post
                                            />
                                        }.into_any(),
                                        core::BlogPostAdminPostsLoadViewModel::EmptyContractUnavailable => view! {
                                            <BlogPostsTable
                                                items=Vec::new()
                                                total=0
                                                editing_post_id=editing_post_id.get()
                                                busy_key=busy_key.get()
                                                on_edit=open_post
                                                on_toggle_publish=toggle_publish
                                                on_archive=archive_post
                                                on_delete=delete_post
                                            />
                                        }.into_any(),
                                        core::BlogPostAdminPostsLoadViewModel::Error { message } => view! {
                                            <div class=shell_classes.load_error>
                                                {message}
                                            </div>
                                        }.into_any(),
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
                                            slug.get_untracked().as_str(),
                                        );
                                        if let Some(slug_value) = title_input.slug_update {
                                            set_slug.set(slug_value);
                                        }
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
                                label=form_copy_view.content_label.clone()
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

                            <label class=form_field_classes.checkbox_label>
                                <input
                                    type="checkbox"
                                    prop:checked=publish_now
                                    on:change=move |ev| set_publish_now.set(event_target_checked(&ev))
                                />
                                {form_copy_view.publish_now_label.clone()}
                            </label>

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

                    <SeoEntityPanel
                        target_kind=SeoTargetSlug::new(seo_builtin_slug::BLOG_POST).expect("builtin SEO target slug")
                        target_id=Signal::derive(move || editing_post_id.get())
                        locale=Signal::derive({
                            let host_locale_for_seo = host_locale_for_seo.clone();
                            move || host_locale_for_seo.clone()
                        })
                        show_control_plane_widgets=true
                        panel_title=t(seo_locale.as_deref(), "blog.seo.title", "Post SEO")
                        panel_subtitle=t(
                            seo_locale.as_deref(),
                            "blog.seo.subtitle",
                            "Explicit metadata, social tags and diagnostics for the selected blog post.",
                        )
                        empty_message=t(
                            seo_locale.as_deref(),
                            "blog.seo.empty",
                            "Create or open a post first. SEO stays inside the blog editor rather than a global SEO hub.",
                        )
                    />
                    </div>
                </section>
            </div>
        }
}

fn blog_form_view_model(
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

fn blog_form_copy_view_model(locale: Option<&str>) -> core::BlogPostAdminEditorFormCopyViewModel {
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
fn BlogEditBanner(
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
fn BlogPostsTable(
    items: Vec<BlogPostListItem>,
    total: u64,
    editing_post_id: Option<String>,
    busy_key: Option<String>,
    on_edit: Callback<(String, String)>,
    on_toggle_publish: Callback<(String, bool, String)>,
    on_archive: Callback<(String, String)>,
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
                                let post_id_delete = row.post_id.clone();
                                let post_locale_edit = row.locale.clone();
                                let post_locale_publish = row.locale.clone();
                                let post_locale_archive = row.locale.clone();

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
fn apply_post_to_form(
    set_editing_post_id: WriteSignal<Option<String>>,
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
fn reset_form(
    set_editing_post_id: WriteSignal<Option<String>>,
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
    set_title.set(state.title);
    set_slug.set(state.slug);
    set_excerpt.set(state.excerpt);
    set_content.set(state.content);
    set_locale.set(state.locale);
    set_tags_input.set(state.tags_input);
    set_publish_now.set(state.publish_now);
}
