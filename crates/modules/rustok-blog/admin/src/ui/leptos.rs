mod lifecycle;
mod page;
use page::BlogAdminPageState;

use leptos::either::Either;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_current_user, use_tenant, use_token};
use leptos_ui_routing::{use_route_query_value, use_route_query_writer};
use rustok_api::{RichTextDocument, WritePathIssue};
use rustok_seo_targets::{SeoTargetSlug, builtin_slug as seo_builtin_slug};
use rustok_ui_core::{AdminQueryKey, UiRouteContext};

use super::components::{
    BlogEditBanner, BlogPostFormSetters, BlogPostSeoPanel, BlogPostsList, BlogPostsListHeader,
    BlogPostsPageHeader, BlogPreviewTokenBanner, BlogSeoConfigError, apply_pending_creation,
    apply_post_to_form, blog_form_copy_view_model, blog_form_view_model, local_resource,
    reset_form,
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
    let current_user = use_current_user();
    let editor_principal = Memo::new(move |_| core::EditorPrincipal {
        actor_id: current_user.get().map(|user| user.id),
        tenant: tenant.get(),
    });
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
    let creation_scope =
        RwSignal::new(None::<(core::EditorPrincipal, core::creation::CreationScope)>);
    let creation_load = RwSignal::new(uuid::Uuid::new_v4());
    let editor_session = RwSignal::new(crate::core::PostEditorSession::Creating);
    let editor_request = RwSignal::new(None::<core::EditorRequest>);
    let editing_post_id = Signal::derive(move || {
        editor_session.with(|session| session.post_id().map(str::to_string))
    });
    let editing_version = Signal::derive(move || editor_session.with(|session| session.version()));
    let loaded_post =
        Signal::derive(move || editor_session.with(|session| session.snapshot().cloned()));
    let (save_notice, set_save_notice) = signal(Option::<String>::None);
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
    let storage_issue = StoredValue::new(t(
        ui_locale.as_deref(),
        "blog.editor.creationStorage",
        "Post creation recovery is unavailable. Reload this tab to try again.",
    ));
    let creation_notice = StoredValue::new(t(
        ui_locale.as_deref(),
        "blog.editor.creationUnknown",
        "Creation is unconfirmed. Save again to retry the original request.",
    ));
    let form_setters = BlogPostFormSetters {
        editor_session,
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
    };
    let reset_form_action = Callback::new({
        let default_locale = default_locale.clone();
        move |_| {
            creation_load.set(uuid::Uuid::new_v4());
            creation_scope.set(None);
            editor_request.set(None);
            set_busy_key.set(None);
            set_save_notice.set(None);
            reset_form(form_setters, default_locale.as_str())
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
            if busy_key.get_untracked().is_some()
                || editor_session.with_untracked(|s| s.pending_creation().is_some())
            {
                return;
            }
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
    let posts_page_chain = RwSignal::new(uuid::Uuid::new_v4());
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
        move |(token_value, tenant_value, _, locale_value, sort_value)| {
            posts_page_chain.set(uuid::Uuid::new_v4());
            async move {
                transport::fetch_posts(
                    token_value,
                    tenant_value,
                    core::locale_arg(locale_value.as_str()),
                    None,
                    sort_value,
                )
                .await
            }
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
        let page_chain = posts_page_chain.get_untracked();
        let refresh_value = refresh_nonce.get_untracked();
        let error_label = load_more_error_label.clone();
        posts_load_more_error.set(None);
        spawn_local(async move {
            let result = transport::fetch_posts(
                token_value.clone(),
                tenant_value.clone(),
                core::locale_arg(locale_value.as_str()),
                Some(cursor.clone()),
                sort_value,
            )
            .await;
            // A cursor belongs to one authenticated page chain. A late page or
            // its error cannot append to a newly sorted/refreshed/locale list.
            if posts_page_chain.get_untracked() != page_chain
                || refresh_nonce.get_untracked() != refresh_value
                || token.get_untracked() != token_value
                || tenant.get_untracked() != tenant_value
                || list_locale.get_untracked() != locale_value
                || posts_sort.get_untracked() != sort_value
                || posts_next_cursor.get_untracked().as_deref() != Some(cursor.as_str())
            {
                return;
            }
            match result {
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
        reset_form_to_defaults.run(());
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let ui_locale = edit_post_locale.clone();
        let request = core::EditorRequest::new(Some(post_id.clone()), requested_locale.clone());
        editor_request.set(Some(request.clone()));
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
                _ if !request.is_current(editor_request.get_untracked().as_ref()) => return,
                Ok(Some(post))
                    if !request.accepts_post(editor_request.get_untracked().as_ref(), &post) =>
                {
                    reset_form_to_defaults.run(());
                    set_submit_error.set(Some(rustok_api::WritePathIssue::new(t(
                        ui_locale.as_deref(),
                        "blog.error.loadPost",
                        "Failed to load post",
                    ))));
                }
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
                            if view_model.apply_returned_post_to_form
                                && request
                                    .accepts_post(editor_request.get_untracked().as_ref(), &post)
                            {
                                apply_post_to_form(form_setters, &post);
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
        // A new principal/tenant owns a new editor session even when the URL is
        // unchanged. In-flight responses from the predecessor are discarded.
        let principal = editor_principal.get();
        if let Some(request) = core::selected_post_request(
            selected_post_query.get().as_deref(),
            effect_default_locale.as_str(),
        ) {
            initial_edit_post.run(request);
        } else {
            reset_form_action.run(());
            set_submit_error.set(None);
            let load = creation_load.get_untracked();
            let token_value = token.get_untracked();
            spawn_local(async move {
                let result =
                    transport::fetch_creation_scope(token_value, principal.tenant.clone()).await;
                if creation_load.try_get_untracked() != Some(load)
                    || editor_principal.try_get_untracked().as_ref() != Some(&principal)
                {
                    return;
                }
                match result {
                    Ok(scope) if scope.matches_actor(principal.actor_id.as_deref()) => {
                        match transport::creation_journal::load(scope) {
                            Ok(pending) => {
                                if let Some(draft) = pending {
                                    apply_pending_creation(form_setters, draft);
                                    set_save_notice.set(Some(creation_notice.get_value()));
                                }
                                creation_scope.set(Some((principal, scope)));
                            }
                            Err(_) => set_submit_error
                                .set(Some(WritePathIssue::new(storage_issue.get_value()))),
                        }
                    }
                    _ => set_submit_error.set(Some(WritePathIssue::new(storage_issue.get_value()))),
                }
            });
        }
    });

    let submit_ui_locale = ui_locale.clone();
    let submit_query_writer = query_writer.clone();
    let submit_post = Callback::new(move |_: ()| {
        if busy_key.get_untracked().is_some() || !editor_session.with_untracked(|s| s.can_save()) {
            return;
        }
        set_save_notice.set(None);
        set_submit_error.set(None);
        let submit_ui_locale = submit_ui_locale.clone();
        let submit_query_writer = submit_query_writer.clone();

        let locale_value = locale.get_untracked();
        let pending = editor_session.with_untracked(|s| s.pending_creation().cloned());
        let is_retry = pending.is_some();
        let command = match pending {
            Some(draft) => Ok(core::BlogPostSaveCommand {
                operation: core::BlogPostSaveOperation::Create(draft),
                busy_key: core::busy_key_for_save(None),
            }),
            None => core::prepare_blog_post_save_command(
                editing_post_id.get_untracked(),
                loaded_post.get_untracked().as_ref(),
                core::build_blog_post_draft(core::BlogPostFormInput {
                    locale: &locale_value,
                    version: editing_version.get_untracked(),
                    title: &title.get_untracked(),
                    slug: &slug.get_untracked(),
                    excerpt: &excerpt.get_untracked(),
                    content: &content.get_untracked(),
                    publish: publish_now.get_untracked(),
                    tags: &tags_input.get_untracked(),
                    category_id: &category_id.get_untracked(),
                    channel_slugs: &channel_slugs_input.get_untracked(),
                    featured_image_url: &featured_image_url.get_untracked(),
                    seo_title: &seo_title.get_untracked(),
                    seo_description: &seo_description.get_untracked(),
                    scheduled_at: &scheduled_at.get_untracked(),
                    schedule_editable: schedule_editable.get_untracked(),
                }),
                t(
                    submit_ui_locale.as_deref(),
                    "blog.error.requiredFields",
                    "Title and content are required to save a blog post.",
                ),
                t(
                    submit_ui_locale.as_deref(),
                    "blog.editor.slugRequired",
                    "Canonical slug is required",
                ),
            ),
        };
        let command = match command {
            Ok(command) => command,
            Err(issue) => {
                set_submit_error.set(Some(issue));
                return;
            }
        };

        let principal = editor_principal.get_untracked();
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let write_token = token_value.clone();
        let write_tenant = tenant_value.clone();
        let (write, retained) = match command.operation {
            core::BlogPostSaveOperation::Create(draft) => {
                let Some((_, scope)) = creation_scope
                    .get_untracked()
                    .filter(|(loaded, _)| loaded == &principal)
                else {
                    return;
                };
                if transport::creation_journal::retain(scope, &draft, is_retry).is_err() {
                    creation_scope.set(None);
                    set_submit_error.set(Some(WritePathIssue::new(storage_issue.get_value())));
                    return;
                }
                editor_session.set(core::PostEditorSession::CreatingPending(Box::new(
                    draft.clone(),
                )));
                (
                    Either::Left(transport::create_post(
                        write_token,
                        write_tenant,
                        scope,
                        draft.clone(),
                    )),
                    Some((scope, draft)),
                )
            }
            core::BlogPostSaveOperation::Update { post_id, update } => (
                Either::Right(async move {
                    transport::update_post(write_token, write_tenant, post_id.clone(), update)
                        .await
                        .map(|()| crate::model::BlogPostCreationOutcome::Created(post_id))
                }),
                None,
            ),
        };

        let reload_locale = retained
            .as_ref()
            .map_or(locale_value, |(_, draft)| draft.locale.clone());
        let request =
            core::EditorRequest::new(editing_post_id.get_untracked(), reload_locale.clone());
        editor_request.set(Some(request.clone()));
        set_busy_key.set(Some(command.busy_key.clone()));

        spawn_local(async move {
            let result = match write {
                Either::Left(create) => create.await,
                Either::Right(update) => update.await,
            };
            if !request.is_current(editor_request.try_get_untracked().flatten().as_ref())
                || editor_principal.try_get_untracked().as_ref() != Some(&principal)
            {
                return;
            }
            let cleanup_failed = retained.as_ref().is_some_and(|(scope, draft)| {
                result.is_ok() && transport::creation_journal::clear(*scope, draft).is_err()
            });
            if cleanup_failed {
                creation_scope.set(None);
                set_submit_error.set(Some(WritePathIssue::new(storage_issue.get_value())));
            }
            match result {
                Ok(crate::model::BlogPostCreationOutcome::Created(post_id)) => {
                    let result_view = core::blog_post_save_result_view(post_id.as_str());
                    // The write is acknowledged. Block another save until a fresh
                    // revision arrives, even if the independent reload fails.
                    editor_session.set(crate::core::PostEditorSession::Acknowledged {
                        post_id: post_id.clone(),
                    });
                    if result_view.refresh_posts {
                        set_refresh_nonce.update(|value| *value += 1);
                    }
                    set_save_notice.set(Some(t(
                        submit_ui_locale.as_deref(),
                        "blog.editor.saved",
                        "Post saved.",
                    )));
                    match transport::fetch_post(
                        token_value,
                        tenant_value,
                        post_id.clone(),
                        Some(reload_locale),
                    )
                    .await
                    {
                        _ if !request
                            .is_current(editor_request.try_get_untracked().flatten().as_ref())
                            || editor_principal.try_get_untracked().as_ref()
                                != Some(&principal) =>
                        {
                            return;
                        }
                        Ok(Some(post))
                            if request
                                .accepts_post(editor_request.get_untracked().as_ref(), &post) =>
                        {
                            apply_post_to_form(form_setters, &post);
                            if let Some(intent) = result_view.selected_post_query_intent {
                                submit_query_writer.apply_query_intent(intent);
                            }
                        }
                        Ok(_) | Err(_) => {
                            editor_session
                                .set(crate::core::PostEditorSession::ReloadFailed { post_id });
                        }
                    }
                }
                Ok(crate::model::BlogPostCreationOutcome::Rejected(message)) => {
                    if !cleanup_failed {
                        editor_session.update(|session| session.reject_creation());
                        set_submit_error.set(Some(WritePathIssue::new(message)));
                    }
                }
                Err(err) => {
                    if editor_session.with_untracked(|s| s.pending_creation().is_some()) {
                        set_save_notice.set(Some(creation_notice.get_value()));
                    }
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

    let lifecycle::LifecycleCallbacks {
        toggle_publish,
        toggle_pin,
        archive_post,
        restore_post,
    } = lifecycle::bind(lifecycle::LifecycleBindings {
        ui_locale: ui_locale.clone(),
        token,
        tenant,
        busy_key,
        set_busy_key,
        editor_request,
        editing_post_id,
        locale,
        editor_session,
        form_setters,
        set_save_notice,
        set_refresh_nonce,
        set_submit_error,
    });

    let delete_post_locale = ui_locale.clone();
    let delete_post_reset_form_action = reset_form_action;
    let delete_query_writer = query_writer.clone();
    let delete_post = Callback::new(move |post_id: String| {
        if busy_key.get_untracked().is_some() {
            return;
        }
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let ui_locale = delete_post_locale.clone();
        let reset_form_to_defaults = delete_post_reset_form_action;
        let delete_query_writer = delete_query_writer.clone();
        let command = core::prepare_blog_post_delete_command(post_id);
        let request =
            core::EditorRequest::new(editing_post_id.get_untracked(), locale.get_untracked());
        editor_request.set(Some(request.clone()));
        set_submit_error.set(None);
        set_busy_key.set(Some(command.busy_key.clone()));

        spawn_local(async move {
            match transport::delete_post(token_value, tenant_value, command.post_id.clone()).await {
                _ if !request.is_current(editor_request.get_untracked().as_ref()) => return,
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

    // Draft preview tokens: minting is a one-shot secret handoff. The plaintext
    // exists only in the mint result (the backend stores a SHA-256 digest), so
    // the banner keeps it in page state until the editor closes it. The token is
    // shared through the URL fragment, never a query string.
    let (preview_token, set_preview_token) = signal(Option::<crate::model::BlogPreviewToken>::None);
    let close_preview_banner = Callback::new(move |_| set_preview_token.set(None));
    let create_preview_link_locale = ui_locale.clone();
    let create_preview_link = Callback::new(move |post_id: String| {
        if busy_key.get_untracked().is_some() {
            return;
        }
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let ui_locale = create_preview_link_locale.clone();
        let request =
            core::EditorRequest::new(editing_post_id.get_untracked(), locale.get_untracked());
        editor_request.set(Some(request.clone()));
        set_submit_error.set(None);
        set_busy_key.set(Some(core::busy_key_for_preview(post_id.as_str())));

        spawn_local(async move {
            // The default TTL (168h) matches the backend default; the owner
            // service validates the 1..720h range and revokes tokens on any
            // lifecycle transition away from draft.
            match transport::create_preview_token(token_value, tenant_value, post_id, None).await {
                _ if !request.is_current(editor_request.get_untracked().as_ref()) => return,
                Ok(minted) => {
                    set_preview_token.set(Some(minted));
                }
                Err(err) => {
                    set_submit_error.set(Some(core::blog_post_transport_failure_issue(
                        &t(
                            ui_locale.as_deref(),
                            "blog.error.createPreviewToken",
                            "Failed to create preview token",
                        ),
                        &err.to_string(),
                    )));
                }
            }

            set_busy_key.set(None);
        });
    });

    page::render(BlogAdminPageState {
        ui_locale,
        host_locale_for_seo,
        seo_target_kind,
        form_copy_view,
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
        create_preview_link,
        preview_token,
        close_preview_banner,
        categories_resource,
        categories_unavailable_label,
        retry_categories: Callback::new(move |()| set_refresh_nonce.update(|value| *value += 1)),
        form_setters,
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
        loaded_post,
        editor_session,
        save_notice,
        creation_blocked: Signal::derive(move || {
            editing_post_id.get().is_none() && creation_scope.get().is_none()
        }),
    })
    .into_any()
}
