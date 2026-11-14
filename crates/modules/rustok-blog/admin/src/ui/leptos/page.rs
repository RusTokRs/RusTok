use super::*;
use leptos::prelude::*;
use rustok_ui_leptos::{Alert, Button};
use rustok_forms_leptos::{
    Form, FormCheckbox, FormError, FormField, FormInput, FormLabel, FormMessage, FormSelect,
    FormTextarea, SubmitButton,
};
use rustok_ui_forms::{DirtyTracker, FormState};

pub(super) struct BlogAdminPageState {
    pub(super) ui_locale: Option<String>,
    pub(super) host_locale_for_seo: String,
    pub(super) seo_target_kind: SeoTargetSlug,
    pub(super) form_copy_view: core::BlogPostAdminEditorFormCopyViewModel,
    pub(super) shell_classes: core::BlogPostAdminShellClassesViewModel,
    pub(super) form_view_model: Memo<core::BlogPostAdminFormViewModel>,
    pub(super) editing_banner_view: Memo<core::BlogPostAdminEditBannerViewModel>,
    pub(super) issue_banner_view: Memo<core::BlogPostAdminIssueBannerViewModel>,
    pub(super) reset_current_post: Callback<()>,
    pub(super) list_locale: ReadSignal<String>,
    pub(super) set_list_locale: WriteSignal<String>,
    pub(super) posts_resource:
        Resource<Result<crate::model::BlogPostList, transport::ApiError>>,
    pub(super) posts_rows: RwSignal<Vec<crate::model::BlogPostListItem>>,
    pub(super) posts_next_cursor: RwSignal<Option<String>>,
    pub(super) posts_load_more_error: RwSignal<Option<String>>,
    pub(super) posts_sort: RwSignal<core::BlogPostAdminSort>,
    pub(super) load_posts_error_label: String,
    pub(super) load_more_posts: Callback<String>,
    pub(super) change_posts_sort: Callback<core::BlogPostAdminSort>,
    pub(super) open_post: Callback<(String, String)>,
    pub(super) toggle_publish: Callback<(String, bool, String)>,
    pub(super) toggle_pin: Callback<(String, bool, String)>,
    pub(super) archive_post: Callback<(String, String)>,
    pub(super) restore_post: Callback<(String, String)>,
    pub(super) delete_post: Callback<String>,
    pub(super) create_preview_link: Callback<String>,
    pub(super) preview_token: ReadSignal<Option<crate::model::BlogPreviewToken>>,
    pub(super) close_preview_banner: Callback<()>,
    pub(super) categories_resource:
        Resource<Result<Vec<crate::model::BlogCategoryOption>, transport::ApiError>>,
    pub(super) categories_unavailable_label: String,
    pub(super) retry_categories: Callback<()>,
    pub(super) form_setters: BlogPostFormSetters,
    pub(super) editing_post_id: Signal<Option<String>>,
    pub(super) title: ReadSignal<String>,
    pub(super) slug: ReadSignal<String>,
    pub(super) excerpt: ReadSignal<String>,
    pub(super) content: ReadSignal<RichTextDocument>,
    pub(super) locale: ReadSignal<String>,
    pub(super) tags_input: ReadSignal<String>,
    pub(super) category_id: ReadSignal<String>,
    pub(super) channel_slugs_input: ReadSignal<String>,
    pub(super) featured_image_url: ReadSignal<String>,
    pub(super) seo_title: ReadSignal<String>,
    pub(super) seo_description: ReadSignal<String>,
    pub(super) scheduled_at: ReadSignal<String>,
    pub(super) schedule_editable: ReadSignal<bool>,
    pub(super) publish_now: ReadSignal<bool>,
    pub(super) busy_key: ReadSignal<Option<String>>,
    pub(super) submit_post: Callback<()>,
    pub(super) loaded_post: Signal<Option<crate::model::BlogPostDetail>>,
    pub(super) editor_session: RwSignal<crate::core::PostEditorSession>,
    pub(super) save_notice: ReadSignal<Option<String>>,
    pub(super) creation_blocked: Signal<bool>,
}

pub(super) fn render(state: BlogAdminPageState) -> impl IntoView {
    let BlogAdminPageState {
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
        retry_categories,
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
        creation_blocked,
    } = state;
    let BlogPostFormSetters {
        editor_session: _,
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
        schedule_editable: _set_schedule_editable,
        publish_now: set_publish_now,
    } = form_setters;

    let ui_locale = StoredValue::new(ui_locale);
    let form_copy_view = StoredValue::new(form_copy_view);
    let categories_unavailable_label = StoredValue::new(categories_unavailable_label);
    let form_state = RwSignal::new(FormState::idle());
    let dirty_tracker = RwSignal::new(DirtyTracker::new());
    Effect::new(move |_| {
        let mut state = FormState::idle();
        state.is_submitting = busy_key.get().is_some();
        state.form_error = issue_banner_view
            .get()
            .visible
            .then(|| issue_banner_view.get().message);
        form_state.set(state);
    });
    Effect::new(move |_| {
        let _snapshot = loaded_post.get().map(|post| (post.id, post.version));
        dirty_tracker.update(|tracker| tracker.reset());
    });
    let fields_disabled = Signal::derive(move || {
        creation_blocked.get()
            || busy_key.get().is_some()
            || editor_session
                .with(|session| session.pending_creation().is_some() || !session.can_save())
    });
    let category_options = Signal::derive(move || {
        let items = categories_resource
            .get()
            .and_then(Result::ok)
            .unwrap_or_default();
        core::post_category_options(
            &items,
            &category_id.get(),
            t(
                ui_locale.get_value().as_deref(),
                "blog.form.uncategorized",
                "Uncategorized",
            ),
            t(
                ui_locale.get_value().as_deref(),
                "blog.editor.currentCategory",
                "Current category",
            ),
        )
    });

    view! {
            <div class=shell_classes.page>
                <BlogPostsPageHeader />

                // The one-time secret stays visible until the editor closes the
                // banner; closing drops it from page state for good.
                {move || {
                    preview_token.get().map(|minted| {
                        view! {
                            <BlogPreviewTokenBanner
                                token=minted
                                on_close=close_preview_banner
                            />
                        }
                    })
                }}

                <section class=shell_classes.layout>
                    <div class=shell_classes.list_card>
                        <BlogPostsListHeader
                            locale=list_locale
                            set_locale=set_list_locale
                            locale_filter_label=form_copy_view.with_value(|value| value.locale_label.clone())
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
                                                    on_preview=create_preview_link
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
                            <p class=shell_classes.form_subtitle>{form_copy_view.with_value(|value| value.subtitle.clone())}</p>
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

                        {move || { let notice = editor_session.get().reload_notice_key().map(|key| t(ui_locale.get_value().as_deref(), key, "Post saved. Reload before editing again; its current snapshot could not be loaded.")).or_else(|| save_notice.get()); notice.map(|notice| view! { <Alert>{notice}</Alert> }) }}
                        {move || loaded_post.get().filter(|post| post.requested_locale != post.effective_locale).map(|post| {
                            view! {
                                <Alert>
                                    <p>{t(ui_locale.get_value().as_deref(), "blog.editor.fallbackNotice", "The requested translation does not exist. The source below is read-only; enter target-language title and content to create it.")}</p>
                                    <p lang=post.effective_locale.clone()>{post.title}</p>
                                    <p lang=post.effective_locale>{post.content_plain_text.unwrap_or_default()}</p>
                                </Alert>
                            }
                        })}
                        <Form state=form_state dirty_tracker=dirty_tracker auto_submitting=false on_submit=submit_post class="mt-5 space-y-4">
                            <FormError />
                            <FormField name="title">
                                <FormLabel>{form_copy_view.with_value(|value| value.title_label.clone())}</FormLabel>
                                <FormInput value=title disabled=fields_disabled on_input=move |value| set_title.set(core::blog_post_admin_title_input_view(value).title) />
                                <FormMessage />
                            </FormField>
                            <FormField name="slug">
                                <FormLabel>{form_copy_view.with_value(|value| value.slug_label.clone())}</FormLabel>
                                <FormInput value=slug disabled=fields_disabled on_input=move |value| set_slug.set(value) />
                                <FormMessage />
                            </FormField>
                            <FormField name="locale">
                                <FormLabel>{form_copy_view.with_value(|value| value.locale_label.clone())}</FormLabel>
                                <FormInput value=locale on_input=move |value| set_locale.set(value) disabled=Signal::derive(move || editing_post_id.get().is_some() || fields_disabled.get()) />
                                <FormMessage />
                            </FormField>
                            <FormField name="excerpt">
                                <FormLabel>{form_copy_view.with_value(|value| value.excerpt_label.clone())}</FormLabel>
                                <FormTextarea value=excerpt disabled=fields_disabled on_input=move |value| set_excerpt.set(value) />
                                <FormMessage />
                            </FormField>
                            <BlogRichTextEditor document=content set_document=set_content content_locale=locale
                                label=form_copy_view.with_value(|value| value.content_label.clone()) disabled=fields_disabled />
                            <FormField name="tags">
                                <FormLabel>{form_copy_view.with_value(|value| value.tags_label.clone())}</FormLabel>
                                <FormInput value=tags_input disabled=fields_disabled on_input=move |value| set_tags_input.set(value) />
                                <FormMessage />
                            </FormField>
                            <FormField name="category_id">
                                <FormLabel>{t(ui_locale.get_value().as_deref(), "blog.form.category", "Category")}</FormLabel>
                                {move || view! { <FormSelect value=category_id options=category_options.get()
                                    on_change=move |value| set_category_id.set(value)
                                    disabled=Signal::derive(move || fields_disabled.get() || !matches!(categories_resource.get(), Some(Ok(_)))) /> }}
                                {move || categories_resource.get().and_then(Result::err).map(|_| view! { <Alert>{categories_unavailable_label.get_value()}<Button on_click=Box::new(move || retry_categories.run(()))>{t(ui_locale.get_value().as_deref(), "blog.editor.retry", "Retry")}</Button></Alert> })}
                                <FormMessage />
                            </FormField>
                            <FormField name="channel_slugs">
                                <FormLabel>{t(ui_locale.get_value().as_deref(), "blog.form.channels", "Channels (comma separated; empty = all channels)")}</FormLabel>
                                <FormInput value=channel_slugs_input disabled=fields_disabled on_input=move |value| set_channel_slugs_input.set(value) />
                                <FormMessage />
                            </FormField>
                            <FormField name="featured_image_url">
                                <FormLabel>{t(ui_locale.get_value().as_deref(), "blog.form.featuredImage", "Featured image URL")}</FormLabel>
                                <FormInput value=featured_image_url disabled=fields_disabled on_input=move |value| set_featured_image_url.set(value) />
                                <FormMessage />
                            </FormField>
                            <FormField name="seo_title">
                                <FormLabel>{t(ui_locale.get_value().as_deref(), "blog.form.seoTitle", "SEO title (Blog default)")}</FormLabel>
                                <FormInput value=seo_title disabled=fields_disabled on_input=move |value| set_seo_title.set(value) />
                                <FormMessage />
                            </FormField>
                            <FormField name="seo_description">
                                <FormLabel>{t(ui_locale.get_value().as_deref(), "blog.form.seoDescription", "SEO description (Blog default)")}</FormLabel>
                                <FormTextarea value=seo_description disabled=fields_disabled on_input=move |value| set_seo_description.set(value) />
                                <FormMessage />
                            </FormField>
                            <Show when=move || schedule_editable.get()>
                                <FormField name="scheduled_at">
                                    <FormLabel>{t(ui_locale.get_value().as_deref(), "blog.form.scheduledAt", "Schedule (RFC 3339 with timezone)")}</FormLabel>
                                    <FormInput value=scheduled_at disabled=fields_disabled on_input=move |value: String| {
                                        if !value.trim().is_empty() { set_publish_now.set(false); }
                                        set_scheduled_at.set(value);
                                    } />
                                    <FormMessage />
                                </FormField>
                            </Show>
                            <Show when=move || editing_post_id.get().is_none()>
                                <FormField name="publish">
                                    <FormCheckbox checked=publish_now disabled=fields_disabled label=form_copy_view.with_value(|value| value.publish_now_label.clone())
                                        on_change=move |checked| {
                                            if checked { set_scheduled_at.set(String::new()); }
                                            set_publish_now.set(checked);
                                        } />
                                </FormField>
                            </Show>
                            <SubmitButton disabled=Signal::derive(move || form_view_model.get().submit_disabled
                                || !editor_session.get().can_save() || creation_blocked.get())>
                                {move || form_view_model.get().submit_label}
                            </SubmitButton>
                        </Form>
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
