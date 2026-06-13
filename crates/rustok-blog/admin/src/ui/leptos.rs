use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_ui_routing::{use_route_query_value, use_route_query_writer};
use rustok_api::{AdminQueryKey, UiRouteContext, WritePathIssue};
use rustok_seo_admin_support::SeoEntityPanel;
use rustok_seo_targets::{builtin_slug as seo_builtin_slug, SeoTargetSlug};

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
    let form_subtitle = t(
        ui_locale.as_deref(),
        "blog.form.subtitle",
        "The package owns both the list and the form. apps/admin only hosts the module route.",
    );
    let form_create_new_instead = t(
        ui_locale.as_deref(),
        "blog.form.createNewInstead",
        "Create new instead",
    );
    let form_raw_warning = t(
        ui_locale.as_deref(),
        "blog.form.rawWarning",
        "This exemplar edits non-markdown content as raw serialized payload through the same GraphQL contract.",
    );
    let form_tags_label = t(ui_locale.as_deref(), "blog.form.tags", "Tags");
    let form_tags_placeholder = t(
        ui_locale.as_deref(),
        "blog.form.tagsPlaceholder",
        "news, launch, release",
    );

    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (editing_post_id, set_editing_post_id) = signal(Option::<String>::None);
    let (title, set_title) = signal(String::new());
    let (slug, set_slug) = signal(String::new());
    let (excerpt, set_excerpt) = signal(String::new());
    let (body, set_body) = signal(String::new());
    let (locale, set_locale) = signal(default_locale.clone());
    let (body_format, set_body_format) = signal("markdown".to_string());
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
                set_body,
                set_locale,
                set_body_format,
                set_tags_input,
                set_publish_now,
                default_locale.as_str(),
            )
        }
    });
    let editing_banner_locale = ui_locale.clone();
    let editing_banner_text = Memo::new(move |_| {
        let template = t(
            editing_banner_locale.as_deref(),
            "blog.form.editingBanner",
            "Editing post {id}",
        );
        core::label_with_optional_id(template.as_str(), editing_post_id.get().as_deref())
    });
    let reset_current_post = Callback::new({
        let query_writer = query_writer.clone();
        move |_| {
            query_writer.clear_key(AdminQueryKey::PostId.as_str());
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
                Ok(Some(post)) => {
                    apply_post_to_form(
                        set_editing_post_id,
                        set_title,
                        set_slug,
                        set_excerpt,
                        set_body,
                        set_locale,
                        set_body_format,
                        set_tags_input,
                        set_publish_now,
                        &post,
                    );
                }
                Ok(None) => {
                    reset_form_to_defaults.run(());
                    set_submit_error.set(Some(WritePathIssue::new(t(
                        ui_locale.as_deref(),
                        "blog.error.postNotFound",
                        "Post not found for editing.",
                    ))));
                }
                Err(err) => {
                    reset_form_to_defaults.run(());
                    set_submit_error.set(Some(WritePathIssue::with_context(
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
        let body_value = body.get_untracked();
        let body_format_value = body_format.get_untracked();
        let tags_value = tags_input.get_untracked();
        let draft = core::build_blog_post_draft(core::BlogPostFormInput {
            locale: &locale_value,
            title: &title_value,
            slug: &slug_value,
            excerpt: &excerpt_value,
            body: &body_value,
            body_format: &body_format_value,
            publish: publish_now.get_untracked(),
            tags: &tags_value,
        });

        let required_fields_message = t(
            submit_ui_locale.as_deref(),
            "blog.error.requiredFields",
            "Title and body are required to save a blog post.",
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
                    let post_id = post.id.clone();
                    apply_post_to_form(
                        set_editing_post_id,
                        set_title,
                        set_slug,
                        set_excerpt,
                        set_body,
                        set_locale,
                        set_body_format,
                        set_tags_input,
                        set_publish_now,
                        &post,
                    );
                    set_refresh_nonce.update(|value| *value += 1);
                    submit_query_writer.replace_value(AdminQueryKey::PostId.as_str(), post_id);
                }
                Err(err) => {
                    set_submit_error.set(Some(WritePathIssue::with_context(
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
                        if core::is_editing_post(
                            editing_post_id.get_untracked().as_deref(),
                            post.id.as_str(),
                        ) {
                            apply_post_to_form(
                                set_editing_post_id,
                                set_title,
                                set_slug,
                                set_excerpt,
                                set_body,
                                set_locale,
                                set_body_format,
                                set_tags_input,
                                set_publish_now,
                                &post,
                            );
                        }
                        set_refresh_nonce.update(|value| *value += 1);
                    }
                    Err(err) => {
                        set_submit_error.set(Some(WritePathIssue::with_context(
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
                    if core::is_editing_post(
                        editing_post_id.get_untracked().as_deref(),
                        post.id.as_str(),
                    ) {
                        apply_post_to_form(
                            set_editing_post_id,
                            set_title,
                            set_slug,
                            set_excerpt,
                            set_body,
                            set_locale,
                            set_body_format,
                            set_tags_input,
                            set_publish_now,
                            &post,
                        );
                    }
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => {
                    set_submit_error.set(Some(WritePathIssue::with_context(
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
                Ok(true) => {
                    if core::should_reset_form_after_delete(
                        editing_post_id.get_untracked().as_deref(),
                        command.post_id.as_str(),
                    ) {
                        delete_query_writer.clear_key(AdminQueryKey::PostId.as_str());
                        reset_form_to_defaults.run(());
                    }
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Ok(false) => {
                    set_submit_error.set(Some(WritePathIssue::new(t(
                        ui_locale.as_deref(),
                        "blog.error.deleteReturnedFalse",
                        "Delete post returned false. Unpublish or archive it first.",
                    ))));
                }
                Err(err) => {
                    set_submit_error.set(Some(WritePathIssue::with_context(
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
        open_query_writer.push_value(AdminQueryKey::PostId.as_str(), post_id);
    });

    view! {
        <div class="space-y-6">
            <header class="flex flex-col gap-4 rounded-2xl border border-border bg-card p-6 shadow-sm lg:flex-row lg:items-start lg:justify-between">
                <div class="space-y-2">
                    <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground">
                        {t(ui_locale.as_deref(), "blog.badge", "blog")}
                    </span>
                    <h1 class="text-2xl font-semibold text-card-foreground">
                        {t(ui_locale.as_deref(), "blog.title", "Blog Publishing")}
                    </h1>
                    <p class="max-w-2xl text-sm text-muted-foreground">
                        {t(
                            ui_locale.as_deref(),
                            "blog.subtitle",
                            "Canonical module-owned CRUD flow for blog posts through the blog GraphQL contract.",
                        )}
                    </p>
                </div>
            </header>

            <section class="grid gap-6 xl:grid-cols-[minmax(0,1fr)_28rem]">
                <div class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                    <div class="mb-4 flex items-end justify-between gap-4">
                        <div>
                            <h2 class="text-lg font-semibold text-card-foreground">
                                {t(ui_locale.as_deref(), "blog.posts.title", "Posts")}
                            </h2>
                            <p class="text-sm text-muted-foreground">
                                {t(
                                    ui_locale.as_deref(),
                                    "blog.posts.subtitle",
                                    "Loaded from rustok-blog-admin via GraphQL, not wired manually in apps/admin.",
                                )}
                            </p>
                        </div>
                        <label class="block space-y-2">
                            <span class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                                {t(ui_locale.as_deref(), "blog.form.locale", "Locale")}
                            </span>
                            <input
                                type="text"
                                class="rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                prop:value=locale
                                on:input=move |ev| set_locale.set(event_target_value(&ev))
                            />
                        </label>
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
                            posts_resource.get().map(|result| {
                                match result {
                                    Ok(post_list) => view! {
                                        <BlogPostsTable
                                            items=post_list.items
                                            total=post_list.total
                                            editing_post_id=editing_post_id.get()
                                            busy_key=busy_key.get()
                                            on_edit=open_post
                                            on_toggle_publish=toggle_publish
                                            on_archive=archive_post
                                            on_delete=delete_post
                                        />
                                    }.into_any(),
                                    Err(err) if transport::is_posts_contract_unavailable(&err) => view! {
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
                                    Err(err) => view! {
                                        <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                                            {core::error_with_context(load_posts_error_label.as_str(), &err.to_string())}
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
                                blog_form_view_model(
                                    ui_locale.as_deref(),
                                    editing_post_id.get().as_deref(),
                                    busy_key.get().as_deref(),
                                )
                                .title
                            }}
                        </h2>
                        <p class="text-sm text-muted-foreground">{form_subtitle.clone()}</p>
                    </div>

                    <Show when=move || core::is_editing_mode(editing_post_id.get().as_deref())>
                        <BlogEditBanner
                            banner_text=Signal::derive({
                                let editing_banner_text = editing_banner_text;
                                move || editing_banner_text.get()
                            })
                            create_new_label=form_create_new_instead.clone()
                            on_reset=reset_current_post
                        />
                    </Show>

                    <form class="mt-5 space-y-4" on:submit=submit_post>
                        <label class="block space-y-2">
                            <span class="text-sm font-medium text-card-foreground">
                                {t(ui_locale.as_deref(), "blog.form.title", "Title")}
                            </span>
                            <input
                                type="text"
                                class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                prop:value=title
                                on:input=move |ev| {
                                    let value = event_target_value(&ev);
                                    if core::should_autofill_slug(slug.get_untracked().as_str()) {
                                        set_slug.set(core::slugify(value.as_str()));
                                    }
                                    set_title.set(value);
                                }
                            />
                        </label>

                        <label class="block space-y-2">
                            <span class="text-sm font-medium text-card-foreground">
                                {t(ui_locale.as_deref(), "blog.form.slug", "Slug")}
                            </span>
                            <input
                                type="text"
                                class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                prop:value=slug
                                on:input=move |ev| set_slug.set(event_target_value(&ev))
                            />
                        </label>

                        <div class="grid gap-4 md:grid-cols-2">
                            <label class="block space-y-2">
                                <span class="text-sm font-medium text-card-foreground">
                                    {t(ui_locale.as_deref(), "blog.form.locale", "Locale")}
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
                                    {t(ui_locale.as_deref(), "blog.form.bodyFormat", "Body format")}
                                </span>
                                <select
                                    class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                    prop:value=body_format
                                    on:change=move |ev| set_body_format.set(event_target_value(&ev))
                                >
                                    <option value="markdown">"markdown"</option>
                                    <option value="rt_json_v1">"rt_json_v1"</option>
                                </select>
                            </label>
                        </div>

                        <label class="block space-y-2">
                            <span class="text-sm font-medium text-card-foreground">
                                {t(ui_locale.as_deref(), "blog.form.excerpt", "Excerpt")}
                            </span>
                            <textarea
                                class="min-h-24 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                prop:value=excerpt
                                on:input=move |ev| set_excerpt.set(event_target_value(&ev))
                            />
                        </label>

                        <label class="block space-y-2">
                            <span class="text-sm font-medium text-card-foreground">
                                {t(ui_locale.as_deref(), "blog.form.body", "Body")}
                            </span>
                            <textarea
                                class="min-h-48 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                prop:value=body
                                on:input=move |ev| set_body.set(event_target_value(&ev))
                            />
                        </label>

                        <Show when=move || core::should_show_raw_body_warning(body_format.get().as_str())>
                            <div class="rounded-xl border border-amber-300/60 bg-amber-50 px-4 py-3 text-sm text-amber-900">
                                {form_raw_warning.clone()}
                            </div>
                        </Show>

                        <label class="block space-y-2">
                            <span class="text-sm font-medium text-card-foreground">
                                {form_tags_label.clone()}
                            </span>
                            <input
                                type="text"
                                class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                placeholder=form_tags_placeholder.clone()
                                prop:value=tags_input
                                on:input=move |ev| set_tags_input.set(event_target_value(&ev))
                            />
                        </label>

                        <label class="flex items-center gap-2 text-sm text-card-foreground">
                            <input
                                type="checkbox"
                                prop:checked=publish_now
                                on:change=move |ev| set_publish_now.set(event_target_checked(&ev))
                            />
                            {t(
                                ui_locale.as_deref(),
                                "blog.form.publishNow",
                                "Publish immediately",
                            )}
                        </label>

                        <Show when=move || {
                            core::blog_post_admin_issue_banner_view(submit_error.get().as_ref()).visible
                        }>
                            <div class=move || {
                                core::blog_post_admin_issue_banner_view(submit_error.get().as_ref()).class
                            }>
                                {move || {
                                    let issue_banner =
                                        core::blog_post_admin_issue_banner_view(submit_error.get().as_ref());

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
                            class="inline-flex w-full items-center justify-center rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50"
                            disabled=move || {
                                blog_form_view_model(
                                    ui_locale.as_deref(),
                                    editing_post_id.get().as_deref(),
                                    busy_key.get().as_deref(),
                                )
                                .submit_disabled
                            }
                        >
                            {move || {
                                blog_form_view_model(
                                    ui_locale.as_deref(),
                                    editing_post_id.get().as_deref(),
                                    busy_key.get().as_deref(),
                                )
                                .submit_label
                            }}
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

#[component]
fn BlogEditBanner(
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
    let table = core::blog_post_admin_table_view(
        items.len(),
        total,
        core::BlogPostAdminTableLabels {
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
        },
    );
    if table.is_empty {
        return view! {
            <div class="rounded-xl border border-dashed border-border p-12 text-center">
                <p class="text-sm text-muted-foreground">
                    {table.empty_message}
                </p>
            </div>
        }
        .into_any();
    }

    view! {
        <div class="space-y-4">
            <div class="text-sm text-muted-foreground">
                {table.total_label.clone()}
            </div>
            <div class="overflow-hidden rounded-xl border border-border">
                <table class="w-full text-sm">
                    <thead class="border-b border-border bg-muted/50">
                        <tr>
                            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{table.title_header.clone()}</th>
                            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{table.slug_header.clone()}</th>
                            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{table.status_header.clone()}</th>
                            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{table.locale_header.clone()}</th>
                            <th class="px-4 py-3"></th>
                        </tr>
                    </thead>
                    <tbody class="divide-y divide-border">
                        {items
                            .into_iter()
                            .map(|post| {
                                let locale = locale.clone();
                                let row = core::blog_post_admin_table_row_view(
                                    post,
                                    editing_post_id.as_deref(),
                                    busy_key.as_deref(),
                                    core::BlogPostAdminTableRowLabels {
                                        draft_slug: &t(locale.as_deref(), "blog.table.draft", "draft"),
                                        no_excerpt: &t(locale.as_deref(), "blog.table.noExcerpt", "No excerpt"),
                                        editing: &t(locale.as_deref(), "blog.table.editing", "Editing"),
                                        edit: &t(locale.as_deref(), "blog.table.edit", "Edit"),
                                        unpublish: &t(locale.as_deref(), "blog.table.unpublish", "Unpublish"),
                                        publish: &t(locale.as_deref(), "blog.table.publish", "Publish"),
                                        archive: &t(locale.as_deref(), "blog.table.archive", "Archive"),
                                        delete: &t(locale.as_deref(), "blog.table.delete", "Delete"),
                                    },
                                );
                                let post_id_edit = row.post_id.clone();
                                let post_id_publish = row.post_id.clone();
                                let post_id_archive = row.post_id.clone();
                                let post_id_delete = row.post_id.clone();
                                let post_locale_edit = row.locale.clone();
                                let post_locale_publish = row.locale.clone();
                                let post_locale_archive = row.locale.clone();

                                view! {
                                    <tr class="transition-colors hover:bg-muted/30">
                                        <td class="px-4 py-3 align-top">
                                            <div class="font-medium text-foreground">{row.title.clone()}</div>
                                            <div class="mt-1 text-xs text-muted-foreground">
                                                {row.excerpt.clone()}
                                            </div>
                                        </td>
                                        <td class="px-4 py-3 align-top text-xs text-muted-foreground">{row.slug.clone()}</td>
                                        <td class="px-4 py-3 align-top">
                                            <StatusBadge status=row.status.clone() />
                                        </td>
                                        <td class="px-4 py-3 align-top text-xs text-muted-foreground">{row.locale.clone()}</td>
                                        <td class="px-4 py-3 align-top text-right">
                                            <div class="flex flex-wrap justify-end gap-2">
                                                <button
                                                    type="button"
                                                    class="text-xs font-medium text-primary hover:underline"
                                                    disabled=row.is_busy
                                                    on:click={
                                                        move |_| on_edit.run((post_id_edit.clone(), post_locale_edit.clone()))
                                                    }
                                                >
                                                    {row.edit_label.clone()}
                                                </button>
                                                <button
                                                    type="button"
                                                    class="text-xs font-medium text-primary hover:underline"
                                                    disabled=row.is_busy
                                                    on:click={
                                                        move |_| on_toggle_publish.run((
                                                            post_id_publish.clone(),
                                                            core::next_publish_state(row.is_published),
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
                                                            class="text-xs font-medium text-primary hover:underline"
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
                                                    class="text-xs font-medium text-destructive hover:underline"
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
    let badge_css = core::status_badge_css(status.as_str());
    view! {
        <span class=badge_css>
            {status}
        </span>
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_post_to_form(
    set_editing_post_id: WriteSignal<Option<String>>,
    set_title: WriteSignal<String>,
    set_slug: WriteSignal<String>,
    set_excerpt: WriteSignal<String>,
    set_body: WriteSignal<String>,
    set_locale: WriteSignal<String>,
    set_body_format: WriteSignal<String>,
    set_tags_input: WriteSignal<String>,
    set_publish_now: WriteSignal<bool>,
    post: &BlogPostDetail,
) {
    apply_form_state(
        set_editing_post_id,
        set_title,
        set_slug,
        set_excerpt,
        set_body,
        set_locale,
        set_body_format,
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
    set_body: WriteSignal<String>,
    set_locale: WriteSignal<String>,
    set_body_format: WriteSignal<String>,
    set_tags_input: WriteSignal<String>,
    set_publish_now: WriteSignal<bool>,
    default_locale: &str,
) {
    apply_form_state(
        set_editing_post_id,
        set_title,
        set_slug,
        set_excerpt,
        set_body,
        set_locale,
        set_body_format,
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
    set_body: WriteSignal<String>,
    set_locale: WriteSignal<String>,
    set_body_format: WriteSignal<String>,
    set_tags_input: WriteSignal<String>,
    set_publish_now: WriteSignal<bool>,
    state: core::BlogPostEditorFormState,
) {
    set_editing_post_id.set(state.editing_post_id);
    set_title.set(state.title);
    set_slug.set(state.slug);
    set_excerpt.set(state.excerpt);
    set_body.set(state.body);
    set_locale.set(state.locale);
    set_body_format.set(state.body_format);
    set_tags_input.set(state.tags_input);
    set_publish_now.set(state.publish_now);
}
