use leptos::prelude::*;
use super::*;

pub(super) fn render(state: BlogAdminPageState) -> impl IntoView {
    let BlogAdminPageState {
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
    } = state;
    let BlogPostFormSetters {
        editing_post_id: _,
        editing_version: _,
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
    } = form_setters;

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
