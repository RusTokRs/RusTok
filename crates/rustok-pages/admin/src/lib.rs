mod api;
mod core;
mod i18n;
mod model;

use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_ui_routing::{use_route_query_value, use_route_query_writer};
use rustok_api::{AdminQueryKey, UiRouteContext, WritePathIssue};
use rustok_seo_admin_support::SeoEntityPanel;
use rustok_seo_targets::{builtin_slug as seo_builtin_slug, SeoTargetSlug};

use crate::i18n::t;
use crate::model::{PageBlock, PageListItem};

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
pub fn PagesAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let selected_page_query = use_route_query_value(AdminQueryKey::PageId.as_str());
    let query_writer = use_route_query_writer();
    let token = use_token();
    let tenant = use_tenant();
    let default_locale = route_context.locale.clone().unwrap_or_default();
    let badge_text = t(route_context.locale.as_deref(), "pages.badge", "pages");
    let title_text = t(
        route_context.locale.as_deref(),
        "pages.title",
        "Pages Builder",
    );
    let subtitle_text = t(
        route_context.locale.as_deref(),
        "pages.subtitle",
        "Canonical module-owned admin slice: list, create, edit, publish and delete pages through the pages module GraphQL contract.",
    );
    let list_title_text = t(route_context.locale.as_deref(), "pages.list.title", "Pages");
    let list_subtitle_text = t(
        route_context.locale.as_deref(),
        "pages.list.subtitle",
        "This list is loaded from the module package itself, not from apps/admin.",
    );
    let load_error_text = t(
        route_context.locale.as_deref(),
        "pages.error.load",
        "Failed to load pages",
    );
    let load_page_error_text = t(
        route_context.locale.as_deref(),
        "pages.error.loadPage",
        "Failed to load page",
    );
    let save_page_error_text = t(
        route_context.locale.as_deref(),
        "pages.error.save",
        "Failed to save page",
    );
    let update_status_error_text = t(
        route_context.locale.as_deref(),
        "pages.error.updateStatus",
        "Failed to update page status",
    );
    let delete_page_error_text = t(
        route_context.locale.as_deref(),
        "pages.error.delete",
        "Failed to delete page",
    );
    let page_not_found_text = t(
        route_context.locale.as_deref(),
        "pages.error.notFound",
        "Page not found for editing.",
    );
    let required_fields_text = t(
        route_context.locale.as_deref(),
        "pages.error.requiredFields",
        "Title and slug are required to save a page.",
    );
    let delete_returned_false_text = t(
        route_context.locale.as_deref(),
        "pages.error.deleteReturnedFalse",
        "Delete page returned false.",
    );
    let validation_issue_label = t(
        route_context.locale.as_deref(),
        "pages.error.validationBadge",
        "Validation",
    );
    let sanitize_issue_label = t(
        route_context.locale.as_deref(),
        "pages.error.sanitizeBadge",
        "Sanitize",
    );
    let runtime_issue_label = t(
        route_context.locale.as_deref(),
        "pages.error.runtimeBadge",
        "Runtime",
    );
    let validation_issue_guidance = t(
        route_context.locale.as_deref(),
        "pages.error.validationGuidance",
        "Check required page fields and ensure project data is valid grapesjs_v1 JSON.",
    );
    let sanitize_issue_guidance = t(
        route_context.locale.as_deref(),
        "pages.error.sanitizeGuidance",
        "Review the GrapesJS payload for unsafe HTML, scripts, URLs, or attributes before retrying.",
    );
    let runtime_issue_guidance = t(
        route_context.locale.as_deref(),
        "pages.error.runtimeGuidance",
        "Retry after checking server health, tenant context, and page-builder runtime logs.",
    );
    let feature_disabled_issue_guidance = t(
        route_context.locale.as_deref(),
        "pages.error.featureDisabledGuidance",
        "Builder publish is disabled for this tenant. Keep the page readable and ask Platform to check builder.publish.enabled before retrying.",
    );
    let validation_issue_label = StoredValue::new(validation_issue_label);
    let sanitize_issue_label = StoredValue::new(sanitize_issue_label);
    let runtime_issue_label = StoredValue::new(runtime_issue_label);
    let validation_issue_guidance = StoredValue::new(validation_issue_guidance);
    let sanitize_issue_guidance = StoredValue::new(sanitize_issue_guidance);
    let runtime_issue_guidance = StoredValue::new(runtime_issue_guidance);
    let feature_disabled_issue_guidance = StoredValue::new(feature_disabled_issue_guidance);
    let form_subtitle_text = t(
        route_context.locale.as_deref(),
        "pages.form.subtitle",
        "A standard module-owned CRUD form that lives entirely inside the package.",
    );
    let create_new_instead_text = t(
        route_context.locale.as_deref(),
        "pages.form.createNewInstead",
        "Create new instead",
    );
    let title_label = t(route_context.locale.as_deref(), "pages.form.title", "Title");
    let slug_label = t(route_context.locale.as_deref(), "pages.form.slug", "Slug");
    let locale_label = t(
        route_context.locale.as_deref(),
        "pages.form.locale",
        "Locale",
    );
    let channel_slugs_label = t(
        route_context.locale.as_deref(),
        "pages.form.channelSlugs",
        "Channel slugs",
    );
    let channel_slugs_placeholder = t(
        route_context.locale.as_deref(),
        "pages.form.channelSlugsPlaceholder",
        "web, mobile-app",
    );
    let channel_slugs_help = t(
        route_context.locale.as_deref(),
        "pages.form.channelSlugsHelp",
        "Comma-separated allowlist. Leave empty to publish on all channels.",
    );
    let project_data_label = t(
        route_context.locale.as_deref(),
        "pages.form.projectData",
        "Project data (grapesjs_v1)",
    );
    let project_data_help = t(
        route_context.locale.as_deref(),
        "pages.form.projectDataHelp",
        "Vendor-neutral project JSON passed into body.contentJson with format grapesjs_v1.",
    );
    let publish_now_label = t(
        route_context.locale.as_deref(),
        "pages.form.publishNow",
        "Publish immediately",
    );
    let preview_surface_title = t(
        route_context.locale.as_deref(),
        "pages.surface.preview.title",
        "Preview",
    );
    let preview_surface_body = t(
        route_context.locale.as_deref(),
        "pages.surface.preview.body",
        "Contract-safe host preview generated from grapesjs_v1 project data.",
    );
    let tree_surface_title = t(
        route_context.locale.as_deref(),
        "pages.surface.tree.title",
        "Tree",
    );
    let tree_surface_body = t(
        route_context.locale.as_deref(),
        "pages.surface.tree.body",
        "Page/component tree from projectData plus existing-block compatibility snapshot.",
    );
    let properties_surface_title = t(
        route_context.locale.as_deref(),
        "pages.surface.properties.title",
        "Properties",
    );
    let properties_surface_body = t(
        route_context.locale.as_deref(),
        "pages.surface.properties.body",
        "Host-owned metadata surface: locale, channels, template, and body format.",
    );
    let publish_surface_title = t(
        route_context.locale.as_deref(),
        "pages.surface.publish.title",
        "Publish",
    );
    let publish_surface_body = t(
        route_context.locale.as_deref(),
        "pages.surface.publish.body",
        "Publish controls reuse the same backend contract as table actions.",
    );
    let project_invalid_preview_text = t(
        route_context.locale.as_deref(),
        "pages.surface.preview.invalidProject",
        "Preview is unavailable until project JSON becomes valid.",
    );
    let tree_empty_text = t(
        route_context.locale.as_deref(),
        "pages.surface.tree.empty",
        "Project tree is empty.",
    );
    let existing_blocks_title = t(
        route_context.locale.as_deref(),
        "pages.surface.tree.existingBlocks",
        "Existing blocks",
    );
    let no_existing_blocks_text = t(
        route_context.locale.as_deref(),
        "pages.surface.tree.noExistingBlocks",
        "No existing blocks attached.",
    );
    let body_format_label = t(
        route_context.locale.as_deref(),
        "pages.surface.properties.bodyFormat",
        "Body format",
    );
    let template_label = t(
        route_context.locale.as_deref(),
        "pages.surface.properties.template",
        "Template",
    );
    let channels_count_label = t(
        route_context.locale.as_deref(),
        "pages.surface.properties.channels",
        "Channels",
    );
    let locale_property_label = t(
        route_context.locale.as_deref(),
        "pages.surface.properties.locale",
        "Locale",
    );
    let last_saved_label = t(
        route_context.locale.as_deref(),
        "pages.surface.publish.lastSaved",
        "Last write",
    );
    let publish_state_label = t(
        route_context.locale.as_deref(),
        "pages.surface.publish.state",
        "Publish state",
    );
    let publish_live_button = t(
        route_context.locale.as_deref(),
        "pages.surface.publish.publishLive",
        "Publish now",
    );
    let unpublish_live_button = t(
        route_context.locale.as_deref(),
        "pages.surface.publish.unpublishLive",
        "Unpublish now",
    );
    let publish_requires_saved_page = t(
        route_context.locale.as_deref(),
        "pages.surface.publish.requiresSavedPage",
        "Save the page first to run a direct publish action.",
    );
    let compatibility_title = t(
        route_context.locale.as_deref(),
        "pages.compat.title",
        "Compatibility notes",
    );
    let compatibility_non_grapes = t(
        route_context.locale.as_deref(),
        "pages.compat.nonGrapes",
        "Current body format is not grapesjs_v1. Save once to migrate this page body while keeping existing blocks untouched.",
    );
    let compatibility_existing_blocks = t(
        route_context.locale.as_deref(),
        "pages.compat.existingBlocks",
        "Existing blocks remain attached and are not deleted automatically by grapesjs_v1 writes.",
    );
    let compatibility_title = StoredValue::new(compatibility_title);
    let compatibility_non_grapes = StoredValue::new(compatibility_non_grapes);
    let compatibility_existing_blocks = StoredValue::new(compatibility_existing_blocks);

    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (editing_page_id, set_editing_page_id) = signal(Option::<String>::None);
    let (title, set_title) = signal(String::new());
    let (slug, set_slug) = signal(String::new());
    let (project_data_text, set_project_data_text) = signal(core::default_project_data_text(""));
    let (channel_slugs_text, set_channel_slugs_text) = signal(String::new());
    let (locale, set_locale) = signal(default_locale.clone());
    let (publish_now, set_publish_now) = signal(false);
    let (body_format, set_body_format) = signal(core::GRAPESJS_FORMAT.to_string());
    let (body_updated_at, set_body_updated_at) = signal(Option::<String>::None);
    let (existing_blocks, set_existing_blocks) = signal(Vec::<PageBlock>::new());
    let (busy_key, set_busy_key) = signal(Option::<String>::None);
    let (submit_issue, set_submit_issue) = signal(Option::<WritePathIssue>::None);

    let reset_form_action = Callback::new({
        let default_locale = default_locale.clone();
        move |_| {
            reset_page_form(
                set_editing_page_id,
                set_title,
                set_slug,
                set_project_data_text,
                set_channel_slugs_text,
                set_locale,
                set_publish_now,
                set_body_format,
                set_body_updated_at,
                set_existing_blocks,
                default_locale.as_str(),
            )
        }
    });

    let editing_banner_text = Signal::derive({
        move || {
            editing_page_id
                .get()
                .map(|page_id| {
                    core::label_with_id(
                        &t(
                            locale.get().as_str().into(),
                            "pages.form.editingBanner",
                            "Editing page {id}",
                        ),
                        page_id.as_str(),
                    )
                })
                .unwrap_or_default()
        }
    });

    let reset_current_page = Callback::new({
        let query_writer = query_writer.clone();
        move |_| {
            query_writer.clear_key(AdminQueryKey::PageId.as_str());
            reset_form_action.run(());
        }
    });

    let pages_resource = local_resource(
        move || (token.get(), tenant.get(), refresh_nonce.get()),
        move |(token_value, tenant_value, _)| async move {
            api::fetch_pages(token_value, tenant_value).await
        },
    );

    let edit_default_locale = default_locale.clone();
    let edit_page = Callback::new(move |page_id: String| {
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let default_locale = edit_default_locale.clone();
        let page_not_found_text = page_not_found_text.clone();
        let load_page_error_text = load_page_error_text.clone();
        set_submit_issue.set(None);
        set_busy_key.set(Some(core::busy_key_with_id("edit", &page_id)));

        spawn_local(async move {
            match api::fetch_page(token_value, tenant_value, page_id.clone()).await {
                Ok(Some(page)) => {
                    let seed = core::edit_form_seed_from_page(&page, default_locale.as_str());

                    set_editing_page_id.set(Some(page_id.clone()));
                    set_locale.set(seed.locale);
                    set_title.set(seed.title);
                    set_slug.set(seed.slug);
                    set_project_data_text.set(seed.project_data_text);
                    set_channel_slugs_text.set(seed.channel_slugs_text);
                    set_publish_now.set(seed.publish_now);
                    set_body_format.set(seed.body_format);
                    set_body_updated_at.set(seed.body_updated_at);
                    set_existing_blocks.set(seed.existing_blocks);
                }
                Ok(None) => {
                    reset_page_form(
                        set_editing_page_id,
                        set_title,
                        set_slug,
                        set_project_data_text,
                        set_channel_slugs_text,
                        set_locale,
                        set_publish_now,
                        set_body_format,
                        set_body_updated_at,
                        set_existing_blocks,
                        default_locale.as_str(),
                    );
                    set_submit_issue.set(Some(WritePathIssue::new(page_not_found_text.clone())));
                }
                Err(err) => {
                    reset_page_form(
                        set_editing_page_id,
                        set_title,
                        set_slug,
                        set_project_data_text,
                        set_channel_slugs_text,
                        set_locale,
                        set_publish_now,
                        set_body_format,
                        set_body_updated_at,
                        set_existing_blocks,
                        default_locale.as_str(),
                    );
                    set_submit_issue.set(Some(core::write_path_issue_with_context(
                        load_page_error_text.as_str(),
                        &err.to_string(),
                    )));
                }
            }
            set_busy_key.set(None);
        });
    });

    let initial_edit_page = edit_page;
    let effect_default_locale = default_locale.clone();
    Effect::new(move |_| match selected_page_query.get() {
        Some(page_id) if core::optional_ui_text(page_id.as_str()).is_some() => {
            initial_edit_page.run(page_id)
        }
        _ => reset_page_form(
            set_editing_page_id,
            set_title,
            set_slug,
            set_project_data_text,
            set_channel_slugs_text,
            set_locale,
            set_publish_now,
            set_body_format,
            set_body_updated_at,
            set_existing_blocks,
            effect_default_locale.as_str(),
        ),
    });

    let submit_query_writer = query_writer.clone();
    let submit_page = move |ev: SubmitEvent| {
        ev.prevent_default();
        set_submit_issue.set(None);
        let submit_query_writer = submit_query_writer.clone();

        let project_data = match core::parse_project_data(&project_data_text.get_untracked()) {
            Ok(project_data) => project_data,
            Err(error) => {
                set_submit_issue.set(Some(WritePathIssue::new(error)));
                return;
            }
        };

        let locale_value = locale.get_untracked();
        let title_value = title.get_untracked();
        let slug_value = slug.get_untracked();
        let channel_slugs_value = channel_slugs_text.get_untracked();
        let draft = core::build_create_page_draft(
            core::PageDraftFormInput {
                locale: &locale_value,
                title: &title_value,
                slug: &slug_value,
                channel_slugs: &channel_slugs_value,
                publish: publish_now.get_untracked(),
            },
            project_data,
        );

        if core::missing_required_page_field(&draft).is_some() {
            set_submit_issue.set(Some(WritePathIssue::new(required_fields_text.clone())));
            return;
        }

        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let editing_page = editing_page_id.get_untracked();
        let save_page_error_text = save_page_error_text.clone();
        set_busy_key.set(Some(core::busy_key_for_save(editing_page.as_deref())));

        spawn_local(async move {
            let result = match editing_page {
                Some(page_id) => api::update_page(token_value, tenant_value, page_id, draft).await,
                None => api::create_page(token_value, tenant_value, draft).await,
            };

            match result {
                Ok(page) => {
                    let status = page.status.to_lowercase();
                    let page_id = page.id;
                    set_editing_page_id.set(Some(page_id.clone()));
                    set_body_format.set(core::GRAPESJS_FORMAT.to_string());
                    set_body_updated_at.set(Some(page.updated_at));
                    if status == "published" {
                        set_publish_now.set(true);
                    }
                    set_refresh_nonce.update(|value| *value += 1);
                    submit_query_writer.replace_value(AdminQueryKey::PageId.as_str(), page_id);
                }
                Err(err) => {
                    set_submit_issue.set(Some(core::write_path_issue_with_context(
                        save_page_error_text.as_str(),
                        &err.to_string(),
                    )));
                }
            }

            set_busy_key.set(None);
        });
    };

    let publish_page = Callback::new(move |(page_id, publish): (String, bool)| {
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let update_status_error_text = update_status_error_text.clone();
        set_submit_issue.set(None);
        set_busy_key.set(Some(core::busy_key_with_id("publish", &page_id)));

        spawn_local(async move {
            let result = if publish {
                api::publish_page(token_value, tenant_value, page_id.clone()).await
            } else {
                api::unpublish_page(token_value, tenant_value, page_id.clone()).await
            };

            match result {
                Ok(page) => {
                    if editing_page_id.get_untracked().as_deref() == Some(page.id.as_str()) {
                        set_publish_now.set(page.status.eq_ignore_ascii_case("published"));
                    }
                    set_body_updated_at.set(Some(page.updated_at));
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => {
                    set_submit_issue.set(Some(core::write_path_issue_with_context(
                        update_status_error_text.as_str(),
                        &err.to_string(),
                    )));
                }
            }

            set_busy_key.set(None);
        });
    });

    let delete_query_writer = query_writer.clone();
    let delete_page = Callback::new(move |page_id: String| {
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let delete_query_writer = delete_query_writer.clone();
        let delete_page_error_text = delete_page_error_text.clone();
        let delete_returned_false_text = delete_returned_false_text.clone();
        set_submit_issue.set(None);
        set_busy_key.set(Some(core::busy_key_with_id("delete", &page_id)));

        spawn_local(async move {
            match api::delete_page(token_value, tenant_value, page_id.clone()).await {
                Ok(true) => {
                    if editing_page_id.get_untracked().as_deref() == Some(page_id.as_str()) {
                        delete_query_writer.clear_key(AdminQueryKey::PageId.as_str());
                        reset_form_action.run(());
                    }
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Ok(false) => {
                    set_submit_issue.set(Some(WritePathIssue::new(
                        delete_returned_false_text.clone(),
                    )));
                }
                Err(err) => {
                    set_submit_issue.set(Some(core::write_path_issue_with_context(
                        delete_page_error_text.as_str(),
                        &err.to_string(),
                    )));
                }
            }
            set_busy_key.set(None);
        });
    });

    let open_query_writer = query_writer.clone();
    let open_page = Callback::new(move |page_id: String| {
        open_query_writer.push_value(AdminQueryKey::PageId.as_str(), page_id);
    });

    let parsed_project = Memo::new(move |_| core::parse_project_data(&project_data_text.get()));
    let preview_document = Memo::new(move |_| match parsed_project.get() {
        Ok(project) => core::preview_html(&title.get(), &slug.get(), &locale.get(), &project),
        Err(_) => core::preview_html(
            &title.get(),
            &slug.get(),
            &locale.get(),
            &core::default_project_data(title.get().as_str()),
        ),
    });
    let project_tree = Signal::derive(move || match parsed_project.get() {
        Ok(project) => core::project_tree(&project),
        Err(_) => Vec::new(),
    });
    let project_parse_error = Signal::derive(move || parsed_project.get().err());

    let compatibility_warning = Signal::derive(move || {
        !body_format
            .get()
            .eq_ignore_ascii_case(core::GRAPESJS_FORMAT)
            || !existing_blocks.get().is_empty()
    });

    view! {
        <div class="space-y-6">
            <header class="flex flex-col gap-4 rounded-2xl border border-border bg-card p-6 shadow-sm lg:flex-row lg:items-start lg:justify-between">
                <div class="space-y-2">
                    <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground">
                        {badge_text.clone()}
                    </span>
                    <h1 class="text-2xl font-semibold text-card-foreground">
                        {title_text.clone()}
                    </h1>
                    <p class="max-w-2xl text-sm text-muted-foreground">
                        {subtitle_text.clone()}
                    </p>
                </div>
            </header>

            <section class="grid gap-6 xl:grid-cols-[minmax(0,1fr)_28rem]">
                <div class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                    <div class="mb-4 flex items-start justify-between gap-4">
                        <div>
                            <h2 class="text-lg font-semibold text-card-foreground">
                                {list_title_text.clone()}
                            </h2>
                            <p class="text-sm text-muted-foreground">
                                {list_subtitle_text.clone()}
                            </p>
                        </div>
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
                            pages_resource.get().map(|result| {
                                match result {
                                    Ok(page_list) => view! {
                                        <PagesTable
                                            items=page_list.items
                                            total=page_list.total
                                            editing_page_id=editing_page_id.get()
                                            busy_key=busy_key.get()
                                            on_edit=open_page
                                            on_toggle_publish=publish_page
                                            on_delete=delete_page
                                        />
                                    }.into_any(),
                                    Err(err) => view! {
                                        <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                                            {core::error_with_context(load_error_text.as_str(), &err.to_string())}
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
                                    if editing_page_id.get().is_some() {
                                        t(locale.get().as_str().into(), "pages.form.editTitle", "Edit page")
                                    } else {
                                        t(
                                            locale.get().as_str().into(),
                                            "pages.form.createTitle",
                                            "Create page",
                                        )
                                    }
                                }}
                            </h2>
                            <p class="text-sm text-muted-foreground">
                                {form_subtitle_text.clone()}
                            </p>
                        </div>

                        <Show when=move || editing_page_id.get().is_some()>
                            <PagesEditBanner
                                banner_text=editing_banner_text
                                create_new_label=create_new_instead_text.clone()
                                on_reset=reset_current_page
                            />
                        </Show>

                        <div class="mt-5 grid gap-4">
                            <CapabilityCard
                                title=preview_surface_title.clone()
                                subtitle=preview_surface_body.clone()
                            >
                                <Show
                                    when=move || project_parse_error.get().is_none()
                                    fallback=move || view! {
                                        <div class="rounded-lg border border-amber-300/50 bg-amber-50 px-3 py-2 text-xs text-amber-900">
                                            {project_parse_error
                                                .get()
                                                .unwrap_or_else(|| project_invalid_preview_text.clone())}
                                        </div>
                                    }
                                >
                                    <iframe
                                        class="h-52 w-full rounded-lg border border-border bg-background"
                                        srcdoc=move || preview_document.get()
                                    ></iframe>
                                </Show>
                            </CapabilityCard>

                            <CapabilityCard
                                title=tree_surface_title.clone()
                                subtitle=tree_surface_body.clone()
                            >
                                <Show
                                    when=move || !project_tree.get().is_empty()
                                    fallback=move || view! {
                                        <p class="text-xs text-muted-foreground">{tree_empty_text.clone()}</p>
                                    }
                                >
                                    <ul class="max-h-40 space-y-1 overflow-auto rounded-lg border border-border bg-background px-3 py-2 text-xs text-muted-foreground">
                                        {move || {
                                            project_tree
                                                .get()
                                                .into_iter()
                                                .map(|line| view! { <li>{line}</li> })
                                                .collect_view()
                                        }}
                                    </ul>
                                </Show>

                                <div class="mt-3 rounded-lg border border-border bg-muted/20 px-3 py-2">
                                    <div class="text-[11px] font-semibold uppercase tracking-[0.18em] text-muted-foreground">
                                        {existing_blocks_title.clone()}
                                    </div>
                                    <Show
                                        when=move || !existing_blocks.get().is_empty()
                                        fallback=move || view! {
                                            <p class="mt-2 text-xs text-muted-foreground">{no_existing_blocks_text.clone()}</p>
                                        }
                                    >
                                        <ul class="mt-2 space-y-1 text-xs text-muted-foreground">
                                            {move || {
                                                existing_blocks
                                                    .get()
                                                    .into_iter()
                                                    .map(|block| {
                                                        view! {
                                                            <li>
                                                                {format!(
                                                                    "#{} · {} · position {}",
                                                                    block.id,
                                                                    block.block_type,
                                                                    block.position,
                                                                )}
                                                            </li>
                                                        }
                                                    })
                                                    .collect_view()
                                            }}
                                        </ul>
                                    </Show>
                                </div>
                            </CapabilityCard>

                            <CapabilityCard
                                title=properties_surface_title.clone()
                                subtitle=properties_surface_body.clone()
                            >
                                <dl class="grid grid-cols-[auto,1fr] gap-x-3 gap-y-2 text-xs text-muted-foreground">
                                    <dt class="font-medium text-card-foreground">{body_format_label.clone()}</dt>
                                    <dd>{move || body_format.get()}</dd>
                                    <dt class="font-medium text-card-foreground">{template_label.clone()}</dt>
                                    <dd>{"default"}</dd>
                                    <dt class="font-medium text-card-foreground">{channels_count_label.clone()}</dt>
                                    <dd>{move || core::parse_channel_slugs(&channel_slugs_text.get()).len().to_string()}</dd>
                                    <dt class="font-medium text-card-foreground">{locale_property_label.clone()}</dt>
                                    <dd>{move || locale.get()}</dd>
                                </dl>
                            </CapabilityCard>

                            <CapabilityCard
                                title=publish_surface_title.clone()
                                subtitle=publish_surface_body.clone()
                            >
                                <div class="space-y-3 text-xs text-muted-foreground">
                                    <div class="flex items-center justify-between gap-3">
                                        <span class="font-medium text-card-foreground">{publish_state_label.clone()}</span>
                                        <span class=move || core::status_badge_css(if publish_now.get() { "published" } else { "draft" })>
                                            {move || if publish_now.get() { "published" } else { "draft" }}
                                        </span>
                                    </div>
                                    <div class="flex items-center justify-between gap-3">
                                        <span class="font-medium text-card-foreground">{last_saved_label.clone()}</span>
                                        <span>{move || body_updated_at.get().unwrap_or_else(|| "-".to_string())}</span>
                                    </div>
                                </div>
                                <label class="mt-3 flex items-center gap-2 text-sm text-card-foreground">
                                    <input
                                        type="checkbox"
                                        prop:checked=publish_now
                                        on:change=move |ev| set_publish_now.set(event_target_checked(&ev))
                                    />
                                    {publish_now_label.clone()}
                                </label>
                                <button
                                    type="button"
                                    class="mt-3 inline-flex w-full items-center justify-center rounded-lg border border-border bg-background px-3 py-2 text-sm font-medium text-card-foreground transition hover:bg-muted disabled:opacity-50"
                                    disabled=move || {
                                        editing_page_id.get().is_none()
                                            || busy_key
                                                .get()
                                                .as_deref()
                                                .map(|key| core::busy_key_matches_action(Some(key), "publish"))
                                                .unwrap_or(false)
                                    }
                                    on:click=move |_| {
                                        if let Some(page_id) = editing_page_id.get() {
                                            publish_page.run((page_id, !publish_now.get()));
                                        }
                                    }
                                >
                                    {move || {
                                        if publish_now.get() {
                                            unpublish_live_button.clone()
                                        } else {
                                            publish_live_button.clone()
                                        }
                                    }}
                                </button>
                                <Show when=move || editing_page_id.get().is_none()>
                                    <p class="mt-2 text-xs text-muted-foreground">
                                        {publish_requires_saved_page.clone()}
                                    </p>
                                </Show>
                            </CapabilityCard>
                        </div>

                        <Show when=move || compatibility_warning.get()>
                            <div class="mt-4 rounded-xl border border-amber-300/60 bg-amber-50 px-4 py-3 text-xs text-amber-900">
                                <div class="font-semibold">{compatibility_title.get_value()}</div>
                                <ul class="mt-2 list-disc space-y-1 pl-4">
                                    <Show when=move || !body_format.get().eq_ignore_ascii_case(core::GRAPESJS_FORMAT)>
                                        <li>{move || compatibility_non_grapes.get_value()}</li>
                                    </Show>
                                    <Show when=move || !existing_blocks.get().is_empty()>
                                        <li>{move || compatibility_existing_blocks.get_value()}</li>
                                    </Show>
                                </ul>
                            </div>
                        </Show>

                        <form class="mt-5 space-y-4" on:submit=submit_page>
                            <label class="block space-y-2">
                                <span class="text-sm font-medium text-card-foreground">
                                    {title_label.clone()}
                                </span>
                                <input
                                    type="text"
                                    class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                    prop:value=title
                                    on:input=move |ev| {
                                        let value = event_target_value(&ev);
                                        if core::optional_ui_text(&slug.get_untracked()).is_none() {
                                            set_slug.set(core::slugify(value.as_str()));
                                        }
                                        set_title.set(value);
                                    }
                                />
                            </label>

                            <label class="block space-y-2">
                                <span class="text-sm font-medium text-card-foreground">
                                    {slug_label.clone()}
                                </span>
                                <input
                                    type="text"
                                    class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                    prop:value=slug
                                    on:input=move |ev| set_slug.set(event_target_value(&ev))
                                />
                            </label>

                            <label class="block space-y-2">
                                <span class="text-sm font-medium text-card-foreground">
                                    {locale_label.clone()}
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
                                    {channel_slugs_label.clone()}
                                </span>
                                <input
                                    type="text"
                                    class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                    placeholder=channel_slugs_placeholder.clone()
                                    prop:value=channel_slugs_text
                                    on:input=move |ev| set_channel_slugs_text.set(event_target_value(&ev))
                                />
                                <span class="block text-xs text-muted-foreground">
                                    {channel_slugs_help.clone()}
                                </span>
                            </label>

                            <label class="block space-y-2">
                                <span class="text-sm font-medium text-card-foreground">
                                    {project_data_label.clone()}
                                </span>
                                <textarea
                                    class="min-h-56 w-full rounded-lg border border-input bg-background px-3 py-2 font-mono text-xs"
                                    prop:value=project_data_text
                                    on:input=move |ev| set_project_data_text.set(event_target_value(&ev))
                                />
                                <span class="block text-xs text-muted-foreground">
                                    {project_data_help.clone()}
                                </span>
                            </label>

                            <Show when=move || submit_issue.get().is_some()>
                                <div class=move || {
                                    submit_issue
                                        .get()
                                        .map(|issue| core::issue_banner_class(issue.kind))
                                        .unwrap_or("hidden")
                                }>
                                    {move || {
                                        submit_issue.get().map(move |issue| {
                                            let validation_issue_label = validation_issue_label.get_value();
                                            let sanitize_issue_label = sanitize_issue_label.get_value();
                                            let runtime_issue_label = runtime_issue_label.get_value();
                                            let label = core::issue_label(
                                                issue.kind,
                                                validation_issue_label.as_str(),
                                                sanitize_issue_label.as_str(),
                                                runtime_issue_label.as_str(),
                                            );
                                            let validation_issue_guidance = validation_issue_guidance.get_value();
                                            let sanitize_issue_guidance = sanitize_issue_guidance.get_value();
                                            let runtime_issue_guidance = runtime_issue_guidance.get_value();
                                            let feature_disabled_issue_guidance = feature_disabled_issue_guidance.get_value();
                                            let guidance = core::issue_guidance(
                                                &issue,
                                                validation_issue_guidance.as_str(),
                                                sanitize_issue_guidance.as_str(),
                                                runtime_issue_guidance.as_str(),
                                                feature_disabled_issue_guidance.as_str(),
                                            );

                                            view! {
                                                <div class="space-y-1">
                                                    <div>
                                                        <strong>{label}</strong>
                                                        {": "}
                                                        {issue.message}
                                                    </div>
                                                    <div class="text-xs opacity-80">{guidance}</div>
                                                </div>
                                            }
                                            .into_any()
                                        })
                                    }}
                                </div>
                            </Show>

                            <button
                                type="submit"
                                class="inline-flex w-full items-center justify-center rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50"
                                disabled=move || {
                                    busy_key.get().as_deref() == Some("create")
                                        || busy_key
                                            .get()
                                            .as_deref()
                                            .map(|key| core::busy_key_matches_action(Some(key), "save"))
                                            .unwrap_or(false)
                                }
                            >
                                {move || {
                                    if busy_key.get().as_deref() == Some("create")
                                        || busy_key
                                            .get()
                                            .as_deref()
                                            .map(|key| core::busy_key_matches_action(Some(key), "save"))
                                            .unwrap_or(false)
                                    {
                                        t(locale.get().as_str().into(), "pages.form.saving", "Saving...")
                                    } else if editing_page_id.get().is_some() {
                                        t(locale.get().as_str().into(), "pages.form.update", "Update page")
                                    } else {
                                        t(locale.get().as_str().into(), "pages.form.create", "Create page")
                                    }
                                }}
                            </button>
                        </form>
                    </section>

                    <SeoEntityPanel
                        target_kind=SeoTargetSlug::new(seo_builtin_slug::PAGE).expect("builtin SEO target slug")
                        target_id=Signal::derive(move || editing_page_id.get())
                        locale=Signal::derive(move || locale.get())
                        panel_title=move || t(locale.get().as_str().into(), "pages.seo.title", "Page SEO")
                        panel_subtitle=move || t(
                            locale.get().as_str().into(),
                            "pages.seo.subtitle",
                            "Explicit metadata, social tags and diagnostics for the selected page.",
                        )
                        empty_message=move || t(
                            locale.get().as_str().into(),
                            "pages.seo.empty",
                            "Create or open a page first. The SEO panel stays attached to the page editor instead of a central hub.",
                        )
                    />
                </div>
            </section>
        </div>
    }
}

#[component]
fn CapabilityCard(title: String, subtitle: String, children: Children) -> impl IntoView {
    view! {
        <article class="rounded-xl border border-border bg-muted/20 p-4">
            <h3 class="text-sm font-semibold text-card-foreground">{title}</h3>
            <p class="mt-1 text-xs text-muted-foreground">{subtitle}</p>
            <div class="mt-3">{children()}</div>
        </article>
    }
}

#[component]
fn PagesEditBanner(
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
fn PagesTable(
    items: Vec<PageListItem>,
    total: u64,
    editing_page_id: Option<String>,
    busy_key: Option<String>,
    on_edit: Callback<String>,
    on_toggle_publish: Callback<(String, bool)>,
    on_delete: Callback<String>,
) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    if items.is_empty() {
        return view! {
            <div class="rounded-xl border border-dashed border-border p-12 text-center">
                <p class="text-sm text-muted-foreground">
                    {t(
                        locale.as_deref(),
                        "pages.table.empty",
                        "No pages yet. Create the first one from the module package form.",
                    )}
                </p>
            </div>
        }
        .into_any();
    }

    view! {
        <div class="space-y-4">
            <div class="text-sm text-muted-foreground">
                {core::count_label(
                    &t(locale.as_deref(), "pages.table.total", "{count} page(s)"),
                    total,
                )}
            </div>
            <div class="overflow-hidden rounded-xl border border-border">
                <table class="w-full text-sm">
                    <thead class="border-b border-border bg-muted/50">
                        <tr>
                            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale.as_deref(), "pages.table.title", "Title")}</th>
                            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale.as_deref(), "pages.table.slug", "Slug")}</th>
                            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale.as_deref(), "pages.table.status", "Status")}</th>
                            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale.as_deref(), "pages.table.updated", "Updated")}</th>
                            <th class="px-4 py-3"></th>
                        </tr>
                    </thead>
                    <tbody class="divide-y divide-border">
                        {items
                            .into_iter()
                            .map(|page| {
                                let page_id = page.id.clone();
                                let is_editing =
                                    editing_page_id.as_deref() == Some(page_id.as_str());
                                let is_published = page.status.eq_ignore_ascii_case("published");
                                let is_busy = busy_key
                                    .as_deref()
                                    .map(|key| key.ends_with(page_id.as_str()))
                                    .unwrap_or(false);

                                view! {
                                    <tr class=("bg-primary/5", is_editing)>
                                        <td class="px-4 py-3">
                                            <div class="font-medium text-foreground">
                                                {page.title.unwrap_or_else(|| {
                                                    t(locale.as_deref(), "pages.table.untitled", "Untitled page")
                                                })}
                                            </div>
                                            <div class="text-xs text-muted-foreground">{page.template}</div>
                                        </td>
                                        <td class="px-4 py-3 text-muted-foreground">
                                            {page.slug.unwrap_or_else(|| "-".to_string())}
                                        </td>
                                        <td class="px-4 py-3">
                                            <StatusBadge status=page.status.clone() />
                                        </td>
                                        <td class="px-4 py-3 text-xs text-muted-foreground">{page.updated_at}</td>
                                        <td class="px-4 py-3">
                                            <div class="flex justify-end gap-2">
                                                <button
                                                    class="rounded-lg border border-border px-3 py-1 text-xs font-medium text-foreground transition hover:bg-accent hover:text-accent-foreground disabled:opacity-50"
                                                    disabled=is_busy
                                                    on:click={
                                                        let page_id = page.id.clone();
                                                        move |_| on_edit.run(page_id.clone())
                                                    }
                                                >
                                                    {if is_busy && busy_key
                                                        .as_deref()
                                                        .map(|key| core::busy_key_matches_action(Some(key), "edit"))
                                                        .unwrap_or(false)
                                                    {
                                                        "...".to_string()
                                                    } else if is_editing {
                                                        t(locale.as_deref(), "pages.table.editing", "Editing")
                                                    } else {
                                                        t(locale.as_deref(), "pages.table.edit", "Edit")
                                                    }}
                                                </button>
                                                <button
                                                    class="rounded-lg border border-border px-3 py-1 text-xs font-medium text-foreground transition hover:bg-accent hover:text-accent-foreground disabled:opacity-50"
                                                    disabled=is_busy
                                                    on:click={
                                                        let page_id = page.id.clone();
                                                        move |_| on_toggle_publish.run((page_id.clone(), !is_published))
                                                    }
                                                >
                                                    {if is_busy && busy_key
                                                        .as_deref()
                                                        .map(|key| core::busy_key_matches_action(Some(key), "publish"))
                                                        .unwrap_or(false)
                                                    {
                                                        "...".to_string()
                                                    } else if is_published {
                                                        t(locale.as_deref(), "pages.table.unpublish", "Unpublish")
                                                    } else {
                                                        t(locale.as_deref(), "pages.table.publish", "Publish")
                                                    }}
                                                </button>
                                                <button
                                                    class="rounded-lg border border-destructive/30 px-3 py-1 text-xs font-medium text-destructive transition hover:bg-destructive/10 disabled:opacity-50"
                                                    disabled=is_busy
                                                    on:click={
                                                        let page_id = page.id.clone();
                                                        move |_| on_delete.run(page_id.clone())
                                                    }
                                                >
                                                    {if is_busy && busy_key
                                                        .as_deref()
                                                        .map(|key| core::busy_key_matches_action(Some(key), "delete"))
                                                        .unwrap_or(false)
                                                    {
                                                        "...".to_string()
                                                    } else {
                                                        t(locale.as_deref(), "pages.table.delete", "Delete")
                                                    }}
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
    let class_name = core::status_badge_css(&status);

    view! {
        <span class=class_name>
            {status}
        </span>
    }
}

#[allow(clippy::too_many_arguments)]
fn reset_page_form(
    set_editing_page_id: WriteSignal<Option<String>>,
    set_title: WriteSignal<String>,
    set_slug: WriteSignal<String>,
    set_project_data_text: WriteSignal<String>,
    set_channel_slugs_text: WriteSignal<String>,
    set_locale: WriteSignal<String>,
    set_publish_now: WriteSignal<bool>,
    set_body_format: WriteSignal<String>,
    set_body_updated_at: WriteSignal<Option<String>>,
    set_existing_blocks: WriteSignal<Vec<PageBlock>>,
    default_locale: &str,
) {
    let seed = core::empty_edit_form_seed(default_locale);
    set_editing_page_id.set(None);
    set_title.set(seed.title);
    set_slug.set(seed.slug);
    set_project_data_text.set(seed.project_data_text);
    set_channel_slugs_text.set(seed.channel_slugs_text);
    set_locale.set(seed.locale);
    set_publish_now.set(seed.publish_now);
    set_body_format.set(seed.body_format);
    set_body_updated_at.set(seed.body_updated_at);
    set_existing_blocks.set(seed.existing_blocks);
}
