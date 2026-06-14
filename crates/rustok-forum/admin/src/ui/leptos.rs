use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_ui_routing::{use_route_query_value, use_route_query_writer, RouteQueryWriter};
use rustok_api::UiRouteContext;
use rustok_seo_admin_support::SeoEntityPanel;
use rustok_seo_targets::{builtin_slug as seo_builtin_slug, SeoTargetSlug};

use crate::core::{
    category_card_view_model, category_select_options, category_sidebar_total_count,
    category_sidebar_view_model, format_count, forum_admin_action_button_class, forum_admin_busy_key,
    forum_admin_collection_state, forum_admin_delete_outcome, forum_admin_editing_thread_label,
    forum_admin_form_error_message, forum_admin_header_view_model, forum_admin_metric_accent_class,
    forum_admin_moderator_notes_copy_labels, forum_admin_open_query_intent,
    forum_admin_placeholder_policy, forum_admin_position_value, forum_admin_reset_query_intent,
    forum_admin_saved_query_intent, forum_admin_seo_copy_labels, forum_admin_sidebar_category_class,
    forum_admin_sidebar_copy_labels, forum_admin_status_badge_class, forum_admin_tag_chips,
    forum_admin_title_envelope_view_model, forum_admin_topic_tag_count_label,
    forum_admin_transport_error_message, reply_card_view_model, reply_count_label,
    result_item_count, selected_category_filter_label, selected_query_id, topic_card_view_model,
    topic_category_filter, CategoryFormSnapshot, ForumAdminActionButtonKind, ForumAdminBusyAction,
    ForumAdminBusySurface, ForumAdminCategoryRenderLabels, ForumAdminCollectionState,
    ForumAdminFormError, ForumAdminFormErrorLabels, ForumAdminHeaderLabels, ForumAdminMetricSurface,
    ForumAdminModeratorNotesLabels, ForumAdminQuerySurface, ForumAdminRouteQueryIntent,
    ForumAdminRouteQueryOperation, ForumAdminSeoSurface, ForumAdminSidebarLabels,
    ForumAdminTitleEnvelopeLabels, ForumAdminTopicRenderLabels, TopicFormSnapshot,
};
use crate::i18n::t;
use crate::model::{CategoryListItem, ReplyListItem, TopicListItem};
use crate::transport;

fn apply_forum_admin_route_query_intent(
    query_writer: &RouteQueryWriter,
    intent: ForumAdminRouteQueryIntent,
) {
    match intent.operation {
        ForumAdminRouteQueryOperation::Push => {
            if let Some(value) = intent.value {
                query_writer.push_value(intent.key, value);
            }
        }
        ForumAdminRouteQueryOperation::Replace => {
            if let Some(value) = intent.value {
                query_writer.replace_value(intent.key, value);
            }
        }
        ForumAdminRouteQueryOperation::Clear => query_writer.clear_key(intent.key),
    }
}

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
pub fn ForumAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let ui_locale = route_context.locale.clone();
    let selected_category_query =
        use_route_query_value(ForumAdminQuerySurface::Category.query_key());
    let selected_topic_query = use_route_query_value(ForumAdminQuerySurface::Topic.query_key());
    let query_writer = use_route_query_writer();
    let token = use_token();
    let tenant = use_tenant();
    let default_locale = route_context.locale.clone().unwrap_or_default();
    let is_categories_page = route_context.subpath_matches("categories");
    let header_labels = ForumAdminHeaderLabels {
        badge: t(ui_locale.as_deref(), "forum.badge", "forum control room"),
        categories_title: t(
            ui_locale.as_deref(),
            "forum.header.categoriesTitle",
            "Category architecture",
        ),
        topics_title: t(
            ui_locale.as_deref(),
            "forum.header.topicsTitle",
            "NodeBB-style moderation workspace",
        ),
        categories_body: t(
            ui_locale.as_deref(),
            "forum.header.categoriesBody",
            "Shape navigation clusters, assign moderation rules, and keep every forum area ready for new threads.",
        ),
        topics_body: t(
            ui_locale.as_deref(),
            "forum.header.topicsBody",
            "Review topic flow, open a thread for reply preview, and keep publishing controls next to the live feed.",
        ),
    };
    let header_view_model = forum_admin_header_view_model(is_categories_page, &header_labels);
    let metric_categories = t(
        ui_locale.as_deref(),
        "forum.metric.categories",
        "Categories",
    );
    let metric_topics = t(ui_locale.as_deref(), "forum.metric.topics", "Topics");
    let metric_reply_preview = t(
        ui_locale.as_deref(),
        "forum.metric.replyPreview",
        "Reply preview",
    );
    let load_category_error = t(
        ui_locale.as_deref(),
        "forum.error.loadCategory",
        "Failed to load category",
    );
    let load_topic_error = t(
        ui_locale.as_deref(),
        "forum.error.loadTopic",
        "Failed to load topic",
    );
    let category_required_error = t(
        ui_locale.as_deref(),
        "forum.error.categoryRequired",
        "Category name and slug are required.",
    );
    let topic_required_error = t(
        ui_locale.as_deref(),
        "forum.error.topicRequired",
        "Category, title and body are required.",
    );
    let save_category_error = t(
        ui_locale.as_deref(),
        "forum.error.saveCategory",
        "Failed to save category",
    );
    let save_topic_error = t(
        ui_locale.as_deref(),
        "forum.error.saveTopic",
        "Failed to save topic",
    );
    let delete_category_error = t(
        ui_locale.as_deref(),
        "forum.error.deleteCategory",
        "Failed to delete category",
    );
    let delete_topic_error = t(
        ui_locale.as_deref(),
        "forum.error.deleteTopic",
        "Failed to delete topic",
    );

    let form_error_labels = ForumAdminFormErrorLabels {
        category_required: category_required_error.clone(),
        topic_required: topic_required_error.clone(),
    };

    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (error, set_error) = signal(Option::<String>::None);
    let (busy_key, set_busy_key) = signal(Option::<String>::None);

    let (editing_category_id, set_editing_category_id) = signal(Option::<String>::None);
    let (category_locale, set_category_locale) = signal(default_locale.clone());
    let (category_name, set_category_name) = signal(String::new());
    let (category_slug, set_category_slug) = signal(String::new());
    let (category_description, set_category_description) = signal(String::new());
    let (category_icon, set_category_icon) = signal(String::new());
    let (category_color, set_category_color) = signal(String::new());
    let (category_position, set_category_position) = signal(0_i32);
    let (category_moderated, set_category_moderated) = signal(false);

    let (editing_topic_id, set_editing_topic_id) = signal(Option::<String>::None);
    let (topic_locale, set_topic_locale) = signal(default_locale);
    let (topic_category_id, set_topic_category_id) = signal(String::new());
    let (topic_title, set_topic_title) = signal(String::new());
    let (topic_slug, set_topic_slug) = signal(String::new());
    let (topic_body, set_topic_body) = signal(String::new());
    let (topic_body_format, set_topic_body_format) = signal("markdown".to_string());
    let (topic_tags, set_topic_tags) = signal(String::new());
    let (topic_filter_category_id, set_topic_filter_category_id) = signal(String::new());

    let categories = local_resource(
        move || {
            (
                token.get(),
                tenant.get(),
                refresh_nonce.get(),
                category_locale.get(),
            )
        },
        move |(token_value, tenant_value, _, locale)| async move {
            transport::fetch_categories(token_value, tenant_value, locale).await
        },
    );

    let topics = local_resource(
        move || {
            (
                token.get(),
                tenant.get(),
                refresh_nonce.get(),
                topic_locale.get(),
                topic_filter_category_id.get(),
            )
        },
        move |(token_value, tenant_value, _, locale, category_id)| async move {
            transport::fetch_topics(
                token_value,
                tenant_value,
                locale,
                topic_category_filter(category_id),
            )
            .await
        },
    );

    let replies = local_resource(
        move || {
            (
                token.get(),
                tenant.get(),
                refresh_nonce.get(),
                topic_locale.get(),
                editing_topic_id.get(),
            )
        },
        move |(token_value, tenant_value, _, locale, topic_id)| async move {
            match topic_id {
                Some(topic_id) => {
                    transport::fetch_replies(token_value, tenant_value, topic_id, locale).await
                }
                None => Ok(Vec::new()),
            }
        },
    );

    let edit_category = Callback::new(move |category_id: String| {
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let locale = category_locale.get_untracked();
        let load_category_error = load_category_error.clone();
        set_error.set(None);
        set_busy_key.set(Some(forum_admin_busy_key(
            ForumAdminBusySurface::Category,
            ForumAdminBusyAction::Edit,
            Some(category_id.as_str()),
        )));
        spawn_local(async move {
            match transport::fetch_category(token_value, tenant_value, category_id.clone(), locale)
                .await
            {
                Ok(category) => apply_category_to_form(
                    set_editing_category_id,
                    set_category_locale,
                    set_category_name,
                    set_category_slug,
                    set_category_description,
                    set_category_icon,
                    set_category_color,
                    set_category_position,
                    set_category_moderated,
                    CategoryFormSnapshot::from_detail(&category),
                ),
                Err(err) => {
                    clear_category_form(
                        set_editing_category_id,
                        set_category_name,
                        set_category_slug,
                        set_category_description,
                        set_category_icon,
                        set_category_color,
                        set_category_position,
                        set_category_moderated,
                    );
                    set_error.set(Some(forum_admin_transport_error_message(
                        load_category_error.as_str(),
                        err,
                    )));
                }
            }
            set_busy_key.set(None);
        });
    });

    let edit_topic = Callback::new(move |topic_id: String| {
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let locale = topic_locale.get_untracked();
        let load_topic_error = load_topic_error.clone();
        set_error.set(None);
        set_busy_key.set(Some(forum_admin_busy_key(
            ForumAdminBusySurface::Topic,
            ForumAdminBusyAction::Edit,
            Some(topic_id.as_str()),
        )));
        spawn_local(async move {
            match transport::fetch_topic(token_value, tenant_value, topic_id.clone(), locale).await
            {
                Ok(topic) => apply_topic_to_form(
                    set_editing_topic_id,
                    set_topic_locale,
                    set_topic_category_id,
                    set_topic_title,
                    set_topic_slug,
                    set_topic_body,
                    set_topic_body_format,
                    set_topic_tags,
                    TopicFormSnapshot::from_detail(&topic),
                ),
                Err(err) => {
                    clear_topic_form(
                        set_editing_topic_id,
                        set_topic_category_id,
                        set_topic_title,
                        set_topic_slug,
                        set_topic_body,
                        set_topic_body_format,
                        set_topic_tags,
                    );
                    set_error.set(Some(forum_admin_transport_error_message(
                        load_topic_error.as_str(),
                        err,
                    )));
                }
            }
            set_busy_key.set(None);
        });
    });
    let initial_edit_category = edit_category;
    let initial_edit_topic = edit_topic;
    Effect::new(
        move |_| match selected_query_id(selected_category_query.get()) {
            Some(category_id) => initial_edit_category.run(category_id),
            None => clear_category_form(
                set_editing_category_id,
                set_category_name,
                set_category_slug,
                set_category_description,
                set_category_icon,
                set_category_color,
                set_category_position,
                set_category_moderated,
            ),
        },
    );
    Effect::new(
        move |_| match selected_query_id(selected_topic_query.get()) {
            Some(topic_id) => initial_edit_topic.run(topic_id),
            None => clear_topic_form(
                set_editing_topic_id,
                set_topic_category_id,
                set_topic_title,
                set_topic_slug,
                set_topic_body,
                set_topic_body_format,
                set_topic_tags,
            ),
        },
    );

    let category_query_writer = query_writer.clone();
    let topic_query_writer = query_writer.clone();
    let category_form_error_labels = form_error_labels.clone();
    let topic_form_error_labels = form_error_labels.clone();
    let submit_category = move |ev: SubmitEvent| {
        ev.prevent_default();
        set_error.set(None);
        let category_query_writer = category_query_writer.clone();
        let form = CategoryFormSnapshot {
            editing_id: editing_category_id.get_untracked(),
            locale: category_locale.get_untracked(),
            name: category_name.get_untracked(),
            slug: category_slug.get_untracked(),
            description: category_description.get_untracked(),
            icon: category_icon.get_untracked(),
            color: category_color.get_untracked(),
            position: category_position.get_untracked(),
            moderated: category_moderated.get_untracked(),
        };
        let draft = match form.to_draft() {
            Ok(draft) => draft,
            Err(ForumAdminFormError::CategoryRequired) => {
                set_error.set(Some(forum_admin_form_error_message(
                    ForumAdminFormError::CategoryRequired,
                    &category_form_error_labels,
                )));
                return;
            }
            Err(ForumAdminFormError::TopicRequired) => {
                unreachable!("category form cannot produce topic validation errors")
            }
        };
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let editing_id = form.editing_id.clone();
        let save_category_error = save_category_error.clone();
        set_busy_key.set(Some(forum_admin_busy_key(
            ForumAdminBusySurface::Category,
            ForumAdminBusyAction::Save,
            None,
        )));
        spawn_local(async move {
            let result = match editing_id {
                Some(id) => transport::update_category(token_value, tenant_value, id, draft).await,
                None => transport::create_category(token_value, tenant_value, draft).await,
            };
            match result {
                Ok(category) => {
                    let category_id = category.id.clone();
                    apply_category_to_form(
                        set_editing_category_id,
                        set_category_locale,
                        set_category_name,
                        set_category_slug,
                        set_category_description,
                        set_category_icon,
                        set_category_color,
                        set_category_position,
                        set_category_moderated,
                        CategoryFormSnapshot::from_detail(&category),
                    );
                    set_refresh_nonce.update(|value| *value += 1);
                    apply_forum_admin_route_query_intent(
                        &category_query_writer,
                        forum_admin_saved_query_intent(
                            ForumAdminQuerySurface::Category,
                            category_id,
                        ),
                    );
                }
                Err(err) => set_error.set(Some(forum_admin_transport_error_message(
                    save_category_error.as_str(),
                    err,
                ))),
            }
            set_busy_key.set(None);
        });
    };

    let submit_topic = move |ev: SubmitEvent| {
        ev.prevent_default();
        set_error.set(None);
        let topic_query_writer = topic_query_writer.clone();
        let form = TopicFormSnapshot {
            editing_id: editing_topic_id.get_untracked(),
            locale: topic_locale.get_untracked(),
            category_id: topic_category_id.get_untracked(),
            title: topic_title.get_untracked(),
            slug: topic_slug.get_untracked(),
            body: topic_body.get_untracked(),
            body_format: topic_body_format.get_untracked(),
            tags_raw: topic_tags.get_untracked(),
        };
        let draft = match form.to_draft() {
            Ok(draft) => draft,
            Err(ForumAdminFormError::TopicRequired) => {
                set_error.set(Some(forum_admin_form_error_message(
                    ForumAdminFormError::TopicRequired,
                    &topic_form_error_labels,
                )));
                return;
            }
            Err(ForumAdminFormError::CategoryRequired) => {
                unreachable!("topic form cannot produce category validation errors")
            }
        };
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let editing_id = form.editing_id.clone();
        let save_topic_error = save_topic_error.clone();
        set_busy_key.set(Some(forum_admin_busy_key(
            ForumAdminBusySurface::Topic,
            ForumAdminBusyAction::Save,
            None,
        )));
        spawn_local(async move {
            let result = match editing_id {
                Some(id) => transport::update_topic(token_value, tenant_value, id, draft).await,
                None => transport::create_topic(token_value, tenant_value, draft).await,
            };
            match result {
                Ok(topic) => {
                    let topic_id = topic.id.clone();
                    apply_topic_to_form(
                        set_editing_topic_id,
                        set_topic_locale,
                        set_topic_category_id,
                        set_topic_title,
                        set_topic_slug,
                        set_topic_body,
                        set_topic_body_format,
                        set_topic_tags,
                        TopicFormSnapshot::from_detail(&topic),
                    );
                    set_refresh_nonce.update(|value| *value += 1);
                    apply_forum_admin_route_query_intent(
                        &topic_query_writer,
                        forum_admin_saved_query_intent(ForumAdminQuerySurface::Topic, topic_id),
                    );
                }
                Err(err) => set_error.set(Some(forum_admin_transport_error_message(
                    save_topic_error.as_str(),
                    err,
                ))),
            }
            set_busy_key.set(None);
        });
    };

    let delete_category_query_writer = query_writer.clone();
    let delete_category = Callback::new(move |category_id: String| {
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let delete_category_error = delete_category_error.clone();
        let delete_category_query_writer = delete_category_query_writer.clone();
        set_error.set(None);
        set_busy_key.set(Some(forum_admin_busy_key(
            ForumAdminBusySurface::Category,
            ForumAdminBusyAction::Delete,
            Some(category_id.as_str()),
        )));
        spawn_local(async move {
            match transport::delete_category(token_value, tenant_value, category_id.clone()).await {
                Ok(()) => {
                    let outcome = forum_admin_delete_outcome(
                        ForumAdminQuerySurface::Category,
                        editing_category_id.get_untracked().as_deref(),
                        category_id.as_str(),
                    );
                    if let Some(intent) = outcome.route_query_intent {
                        apply_forum_admin_route_query_intent(&delete_category_query_writer, intent);
                    }
                    if outcome.should_clear_form {
                        clear_category_form(
                            set_editing_category_id,
                            set_category_name,
                            set_category_slug,
                            set_category_description,
                            set_category_icon,
                            set_category_color,
                            set_category_position,
                            set_category_moderated,
                        );
                    }
                    if outcome.should_refresh {
                        set_refresh_nonce.update(|value| *value += 1);
                    }
                }
                Err(err) => set_error.set(Some(forum_admin_transport_error_message(
                    delete_category_error.as_str(),
                    err,
                ))),
            }
            set_busy_key.set(None);
        });
    });

    let delete_topic_query_writer = query_writer.clone();
    let delete_topic = Callback::new(move |topic_id: String| {
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let delete_topic_error = delete_topic_error.clone();
        let delete_topic_query_writer = delete_topic_query_writer.clone();
        set_error.set(None);
        set_busy_key.set(Some(forum_admin_busy_key(
            ForumAdminBusySurface::Topic,
            ForumAdminBusyAction::Delete,
            Some(topic_id.as_str()),
        )));
        spawn_local(async move {
            match transport::delete_topic(token_value, tenant_value, topic_id.clone()).await {
                Ok(()) => {
                    let outcome = forum_admin_delete_outcome(
                        ForumAdminQuerySurface::Topic,
                        editing_topic_id.get_untracked().as_deref(),
                        topic_id.as_str(),
                    );
                    if let Some(intent) = outcome.route_query_intent {
                        apply_forum_admin_route_query_intent(&delete_topic_query_writer, intent);
                    }
                    if outcome.should_clear_form {
                        clear_topic_form(
                            set_editing_topic_id,
                            set_topic_category_id,
                            set_topic_title,
                            set_topic_slug,
                            set_topic_body,
                            set_topic_body_format,
                            set_topic_tags,
                        );
                    }
                    if outcome.should_refresh {
                        set_refresh_nonce.update(|value| *value += 1);
                    }
                }
                Err(err) => set_error.set(Some(forum_admin_transport_error_message(
                    delete_topic_error.as_str(),
                    err,
                ))),
            }
            set_busy_key.set(None);
        });
    });

    let topic_count = move || result_item_count(topics.get());
    let category_count = move || result_item_count(categories.get());
    let reply_preview_count = move || result_item_count(replies.get());
    let open_category_query_writer = query_writer.clone();
    let open_topic_query_writer = query_writer.clone();
    let reset_category_query_writer = query_writer.clone();
    let reset_topic_query_writer = query_writer.clone();
    let open_category = Callback::new(move |category_id: String| {
        apply_forum_admin_route_query_intent(
            &open_category_query_writer,
            forum_admin_open_query_intent(ForumAdminQuerySurface::Category, category_id),
        );
    });
    let open_topic = Callback::new(move |topic_id: String| {
        apply_forum_admin_route_query_intent(
            &open_topic_query_writer,
            forum_admin_open_query_intent(ForumAdminQuerySurface::Topic, topic_id),
        );
    });
    let reset_category = Callback::new(move |_| {
        apply_forum_admin_route_query_intent(
            &reset_category_query_writer,
            forum_admin_reset_query_intent(ForumAdminQuerySurface::Category),
        );
        clear_category_form(
            set_editing_category_id,
            set_category_name,
            set_category_slug,
            set_category_description,
            set_category_icon,
            set_category_color,
            set_category_position,
            set_category_moderated,
        );
    });
    let reset_topic = Callback::new(move |_| {
        apply_forum_admin_route_query_intent(
            &reset_topic_query_writer,
            forum_admin_reset_query_intent(ForumAdminQuerySurface::Topic),
        );
        clear_topic_form(
            set_editing_topic_id,
            set_topic_category_id,
            set_topic_title,
            set_topic_slug,
            set_topic_body,
            set_topic_body_format,
            set_topic_tags,
        );
    });

    view! {
        <div class="space-y-6">
            <header class="overflow-hidden rounded-[2rem] border border-border bg-gradient-to-br from-card via-card to-muted/40 shadow-sm">
                <div class="grid gap-6 px-6 py-7 lg:grid-cols-[minmax(0,1.5fr)_minmax(0,1fr)] lg:px-8">
                    <div class="space-y-4">
                        <div class="inline-flex items-center gap-2 rounded-full border border-border/70 bg-background/80 px-3 py-1 text-[11px] font-semibold uppercase tracking-[0.26em] text-muted-foreground">
                            <span class="h-2 w-2 rounded-full bg-amber-500"></span>
                            {header_view_model.badge.clone()}
                        </div>
                        <div class="space-y-2">
                            <h1 class="text-3xl font-semibold tracking-tight text-card-foreground">
                                {header_view_model.title.clone()}
                            </h1>
                            <p class="max-w-2xl text-sm leading-6 text-muted-foreground">
                                {header_view_model.body.clone()}
                            </p>
                        </div>
                    </div>
                    <div class="grid gap-3 sm:grid-cols-3 lg:grid-cols-1 xl:grid-cols-3">
                        <MetricCard
                            label=metric_categories.clone()
                            value=Signal::derive(move || format_count(category_count()))
                            accent_class=forum_admin_metric_accent_class(ForumAdminMetricSurface::Categories)
                        />
                        <MetricCard
                            label=metric_topics.clone()
                            value=Signal::derive(move || format_count(topic_count()))
                            accent_class=forum_admin_metric_accent_class(ForumAdminMetricSurface::Topics)
                        />
                        <MetricCard
                            label=metric_reply_preview.clone()
                            value=Signal::derive(move || format_count(reply_preview_count()))
                            accent_class=forum_admin_metric_accent_class(ForumAdminMetricSurface::ReplyPreview)
                        />
                    </div>
                </div>
            </header>

            {move || error.get().map(|value| view! {
                <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{value}</div>
            })}

            {if is_categories_page {
                view! {
                    <CategoriesPage
                        categories=categories
                        busy_key=busy_key
                        editing_id=editing_category_id
                        locale=category_locale
                        set_locale=set_category_locale
                        name=category_name
                        set_name=set_category_name
                        slug=category_slug
                        set_slug=set_category_slug
                        description=category_description
                        set_description=set_category_description
                        icon=category_icon
                        set_icon=set_category_icon
                        color=category_color
                        set_color=set_category_color
                        position=category_position
                        set_position=set_category_position
                        moderated=category_moderated
                        set_moderated=set_category_moderated
                        on_edit=open_category
                        on_delete=delete_category
                        on_submit=submit_category
                        on_reset=reset_category
                    />
                }.into_any()
            } else {
                view! {
                    <TopicsPage
                        categories=categories
                        topics=topics
                        replies=replies
                        busy_key=busy_key
                        editing_id=editing_topic_id
                        locale=topic_locale
                        set_locale=set_topic_locale
                        category_id=topic_category_id
                        set_category_id=set_topic_category_id
                        title=topic_title
                        set_title=set_topic_title
                        slug=topic_slug
                        set_slug=set_topic_slug
                        body=topic_body
                        set_body=set_topic_body
                        body_format=topic_body_format
                        set_body_format=set_topic_body_format
                        tags=topic_tags
                        set_tags=set_topic_tags
                        filter_category_id=topic_filter_category_id
                        set_filter_category_id=set_topic_filter_category_id
                        on_edit=open_topic
                        on_delete=delete_topic
                        on_submit=submit_topic
                        on_reset=reset_topic
                    />
                }.into_any()
            }}
        </div>
    }
}

#[component]
fn MetricCard(label: String, value: Signal<String>, accent_class: &'static str) -> impl IntoView {
    view! {
        <article class="rounded-[1.5rem] border border-border/80 bg-background/80 p-4 backdrop-blur">
            <div class="flex items-center gap-3">
                <span class=format!("h-3 w-3 rounded-full {}", accent_class)></span>
                <span class="text-xs font-semibold uppercase tracking-[0.22em] text-muted-foreground">
                    {label}
                </span>
            </div>
            <div class="mt-4 text-2xl font-semibold text-foreground">{move || value.get()}</div>
        </article>
    }
}

#[component]
fn InsightTile(title: String, body: String) -> impl IntoView {
    view! {
        <article class="rounded-[1.35rem] border border-border bg-background/80 p-4">
            <h3 class="text-sm font-semibold text-foreground">{title}</h3>
            <p class="mt-2 text-sm leading-6 text-muted-foreground">{body}</p>
        </article>
    }
}

#[component]
fn FieldShell(label: String, hint: String, children: Children) -> impl IntoView {
    view! {
        <label class="block space-y-2">
            <span class="block text-sm font-medium text-foreground">{label}</span>
            <span class="block text-xs leading-5 text-muted-foreground">{hint}</span>
            {children()}
        </label>
    }
}

#[component]
fn SidebarStat(label: String, value: Signal<String>) -> impl IntoView {
    view! {
        <div class="rounded-2xl border border-border bg-card px-4 py-3">
            <p class="text-[11px] font-semibold uppercase tracking-[0.22em] text-muted-foreground">
                {label}
            </p>
            <p class="mt-2 text-sm font-medium text-foreground">{move || value.get()}</p>
        </div>
    }
}

#[component]
fn StaticChip(label: String) -> impl IntoView {
    view! {
        <span class="rounded-full bg-muted px-2.5 py-1 text-xs font-medium text-muted-foreground">
            {label}
        </span>
    }
}

#[component]
fn CategoriesPage(
    categories: LocalResource<Result<Vec<CategoryListItem>, String>>,
    busy_key: ReadSignal<Option<String>>,
    editing_id: ReadSignal<Option<String>>,
    locale: ReadSignal<String>,
    set_locale: WriteSignal<String>,
    name: ReadSignal<String>,
    set_name: WriteSignal<String>,
    slug: ReadSignal<String>,
    set_slug: WriteSignal<String>,
    description: ReadSignal<String>,
    set_description: WriteSignal<String>,
    icon: ReadSignal<String>,
    set_icon: WriteSignal<String>,
    color: ReadSignal<String>,
    set_color: WriteSignal<String>,
    position: ReadSignal<i32>,
    set_position: WriteSignal<i32>,
    moderated: ReadSignal<bool>,
    set_moderated: WriteSignal<bool>,
    on_edit: Callback<String>,
    on_delete: Callback<String>,
    on_submit: impl Fn(SubmitEvent) + 'static,
    on_reset: Callback<()>,
) -> impl IntoView {
    let ui_locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let host_locale_for_seo = ui_locale.clone().unwrap_or_default();
    let placeholders = forum_admin_placeholder_policy(locale.get_untracked().as_str());
    let matrix_labels = forum_admin_category_matrix_labels(
        t(
            ui_locale.as_deref(),
            "forum.categories.matrixLabel",
            "Category matrix",
        ),
        t(
            ui_locale.as_deref(),
            "forum.categories.matrixTitle",
            "Forum sections",
        ),
        t(
            ui_locale.as_deref(),
            "forum.categories.matrixBody",
            "This view keeps category hierarchy, counts, and moderation switches close together so moderators can shape the forum like a community map instead of a plain CRUD table.",
        ),
        t(ui_locale.as_deref(), "forum.categories.new", "New category"),
    );
    let notes_copy = forum_admin_moderator_notes_copy_labels(
        t(
            ui_locale.as_deref(),
            "forum.categories.notesLabel",
            "Moderator notes",
        ),
        t(
            ui_locale.as_deref(),
            "forum.categories.noteIconTitle",
            "Icon + color",
        ),
        t(
            ui_locale.as_deref(),
            "forum.categories.noteIconBody",
            "Use both so each category reads like a quick visual stop in the sidebar.",
        ),
        t(
            ui_locale.as_deref(),
            "forum.categories.notePositionTitle",
            "Position",
        ),
        t(
            ui_locale.as_deref(),
            "forum.categories.notePositionBody",
            "Lower numbers bubble important sections to the top of the community map.",
        ),
        t(
            ui_locale.as_deref(),
            "forum.categories.noteModeratedTitle",
            "Moderated",
        ),
        t(
            ui_locale.as_deref(),
            "forum.categories.noteModeratedBody",
            "Turn this on for queues that need stricter review before topics go live.",
        ),
    );
    let category_form_labels = forum_admin_category_form_labels(
        t(
            ui_locale.as_deref(),
            "forum.categories.composerLabel",
            "Composer",
        ),
        t(ui_locale.as_deref(), "forum.form.locale", "Locale"),
        t(
            ui_locale.as_deref(),
            "forum.form.localeHintCategory",
            "Published locale for this category.",
        ),
        t(ui_locale.as_deref(), "forum.form.name", "Name"),
        t(
            ui_locale.as_deref(),
            "forum.form.nameHint",
            "Human-friendly label shown in the admin and forum nav.",
        ),
        t(ui_locale.as_deref(), "forum.form.slug", "Slug"),
        t(
            ui_locale.as_deref(),
            "forum.form.slugHintCategory",
            "Stable identifier for routing and lookups.",
        ),
        t(
            ui_locale.as_deref(),
            "forum.form.description",
            "Description",
        ),
        t(
            ui_locale.as_deref(),
            "forum.form.descriptionHint",
            "Short community-facing summary.",
        ),
        t(ui_locale.as_deref(), "forum.form.icon", "Icon"),
        t(
            ui_locale.as_deref(),
            "forum.form.iconHint",
            "Optional short token or icon name.",
        ),
        t(ui_locale.as_deref(), "forum.form.color", "Color"),
        t(
            ui_locale.as_deref(),
            "forum.form.colorHint",
            "Accent color, for example `#f59e0b`.",
        ),
        t(ui_locale.as_deref(), "forum.form.position", "Position"),
        t(
            ui_locale.as_deref(),
            "forum.form.positionHint",
            "Lower comes first in the list.",
        ),
        t(
            ui_locale.as_deref(),
            "forum.form.moderatedTitle",
            "Moderated queue",
        ),
        t(
            ui_locale.as_deref(),
            "forum.form.moderatedHint",
            "Topics in this category should flow through stricter review.",
        ),
        t(
            ui_locale.as_deref(),
            "forum.form.saveCategory",
            "Save category",
        ),
        t(
            ui_locale.as_deref(),
            "forum.form.createCategory",
            "Create category",
        ),
        t(ui_locale.as_deref(), "forum.form.reset", "Reset"),
    );
    let edit_title = t(
        ui_locale.as_deref(),
        "forum.categories.editTitle",
        "Edit category",
    );
    let create_title = t(
        ui_locale.as_deref(),
        "forum.categories.createTitle",
        "Create category",
    );
    let live_edit_label = t(
        ui_locale.as_deref(),
        "forum.categories.liveEdit",
        "Live edit",
    );
    let category_title_labels = ForumAdminTitleEnvelopeLabels {
        edit_title: edit_title.clone(),
        create_title: create_title.clone(),
        active_badge: live_edit_label.clone(),
    };
    let category_heading_labels = category_title_labels.clone();
    let category_badge_labels = category_title_labels.clone();
    let category_seo_copy = forum_admin_seo_copy_labels(
        ForumAdminSeoSurface::Category,
        t(
            locale.get_untracked().as_str().into(),
            "forum.categories.seo.title",
            "Category SEO",
        ),
        t(
            locale.get_untracked().as_str().into(),
            "forum.categories.seo.subtitle",
            "Explicit metadata, social tags and diagnostics for the selected forum category.",
        ),
        t(
            locale.get_untracked().as_str().into(),
            "forum.categories.seo.empty",
            "Create or open a category first. SEO stays attached to the forum category editor.",
        ),
    );
    view! {
        <section class="grid gap-6 xl:grid-cols-[minmax(0,1.45fr)_24rem]">
            <div class="space-y-6">
                <section class="rounded-[1.75rem] border border-border bg-card p-6 shadow-sm">
                    <div class="flex flex-wrap items-center justify-between gap-4">
                        <div>
                            <p class="text-xs font-semibold uppercase tracking-[0.24em] text-muted-foreground">
                                {matrix_labels.matrix_label.clone()}
                            </p>
                            <h2 class="mt-2 text-2xl font-semibold text-card-foreground">
                                {matrix_labels.matrix_title.clone()}
                            </h2>
                        </div>
                        <button
                            type="button"
                            class="rounded-full border border-border bg-background px-4 py-2 text-sm font-medium text-foreground transition hover:bg-muted"
                            on:click=move |_| on_reset.run(())
                        >
                            {matrix_labels.new_category_label.clone()}
                        </button>
                    </div>
                    <p class="mt-3 max-w-2xl text-sm leading-6 text-muted-foreground">
                        {matrix_labels.matrix_body.clone()}
                    </p>
                    <Suspense fallback=move || view! { <div class="mt-6 h-48 animate-pulse rounded-[1.5rem] bg-muted"></div> }>
                        {move || categories.get().map(|result| render_category_grid(result, editing_id.get(), busy_key.get(), on_edit, on_delete, ui_locale.clone()))}
                    </Suspense>
                </section>

                <section class="rounded-[1.75rem] border border-border bg-gradient-to-br from-card via-card to-muted/30 p-6 shadow-sm">
                    <p class="text-xs font-semibold uppercase tracking-[0.24em] text-muted-foreground">
                        {notes_copy.notes_label.clone()}
                    </p>
                    <div class="mt-4 grid gap-4 md:grid-cols-3">
                        <InsightTile
                            title=notes_copy.note_icon_title.clone()
                            body=notes_copy.note_icon_body.clone()
                        />
                        <InsightTile
                            title=notes_copy.note_position_title.clone()
                            body=notes_copy.note_position_body.clone()
                        />
                        <InsightTile
                            title=notes_copy.note_moderated_title.clone()
                            body=notes_copy.note_moderated_body.clone()
                        />
                    </div>
                </section>
            </div>

            <section class="rounded-[1.75rem] border border-border bg-card p-6 shadow-sm xl:sticky xl:top-6 xl:self-start">
                <div class="flex items-center justify-between gap-3">
                    <div>
                        <p class="text-xs font-semibold uppercase tracking-[0.24em] text-muted-foreground">
                            {category_form_labels.composer_label.clone()}
                        </p>
                        <h2 class="mt-2 text-xl font-semibold text-card-foreground">
                            {move || forum_admin_title_envelope_view_model(editing_id.get().is_some(), &category_heading_labels).title}
                        </h2>
                    </div>
                    {move || forum_admin_title_envelope_view_model(editing_id.get().is_some(), &category_badge_labels)
                        .active_badge
                        .map(|label| view! {
                            <span class="rounded-full bg-amber-500/15 px-3 py-1 text-xs font-medium text-amber-700 dark:text-amber-300">
                                {label}
                            </span>
                        })}
                </div>
                <form class="mt-6 space-y-4" on:submit=on_submit>
                    <FieldShell label=category_form_labels.locale_label.clone() hint=category_form_labels.locale_hint.clone()>
                        <input
                            class="w-full rounded-2xl border border-border bg-background px-4 py-3 text-sm outline-none transition focus:border-primary"
                            prop:value=move || locale.get()
                            on:input=move |ev| set_locale.set(event_target_value(&ev))
                            placeholder=placeholders.locale.clone()
                        />
                    </FieldShell>
                    <FieldShell label=category_form_labels.name_label.clone() hint=category_form_labels.name_hint.clone()>
                        <input
                            class="w-full rounded-2xl border border-border bg-background px-4 py-3 text-sm outline-none transition focus:border-primary"
                            prop:value=move || name.get()
                            on:input=move |ev| set_name.set(event_target_value(&ev))
                            placeholder=placeholders.category_name.clone()
                        />
                    </FieldShell>
                    <FieldShell label=category_form_labels.slug_label.clone() hint=category_form_labels.slug_hint.clone()>
                        <input
                            class="w-full rounded-2xl border border-border bg-background px-4 py-3 font-mono text-sm outline-none transition focus:border-primary"
                            prop:value=move || slug.get()
                            on:input=move |ev| set_slug.set(event_target_value(&ev))
                            placeholder=placeholders.category_slug.clone()
                        />
                    </FieldShell>
                    <FieldShell label=category_form_labels.description_label.clone() hint=category_form_labels.description_hint.clone()>
                        <textarea
                            class="min-h-24 w-full rounded-2xl border border-border bg-background px-4 py-3 text-sm outline-none transition focus:border-primary"
                            prop:value=move || description.get()
                            on:input=move |ev| set_description.set(event_target_value(&ev))
                            placeholder=placeholders.category_description.clone()
                        ></textarea>
                    </FieldShell>
                    <div class="grid gap-4 sm:grid-cols-2">
                        <FieldShell label=category_form_labels.icon_label.clone() hint=category_form_labels.icon_hint.clone()>
                            <input
                                class="w-full rounded-2xl border border-border bg-background px-4 py-3 text-sm outline-none transition focus:border-primary"
                                prop:value=move || icon.get()
                                on:input=move |ev| set_icon.set(event_target_value(&ev))
                                placeholder=placeholders.category_icon.clone()
                            />
                        </FieldShell>
                        <FieldShell label=category_form_labels.color_label.clone() hint=category_form_labels.color_hint.clone()>
                            <input
                                class="w-full rounded-2xl border border-border bg-background px-4 py-3 font-mono text-sm outline-none transition focus:border-primary"
                                prop:value=move || color.get()
                                on:input=move |ev| set_color.set(event_target_value(&ev))
                                placeholder=placeholders.category_color.clone()
                            />
                        </FieldShell>
                    </div>
                    <FieldShell label=category_form_labels.position_label.clone() hint=category_form_labels.position_hint.clone()>
                        <input
                            class="w-full rounded-2xl border border-border bg-background px-4 py-3 text-sm outline-none transition focus:border-primary"
                            prop:value=move || position.get().to_string()
                            on:input=move |ev| set_position.set(forum_admin_position_value(event_target_value(&ev).as_str()))
                            placeholder=placeholders.category_position.clone()
                        />
                    </FieldShell>
                    <label class="flex items-start gap-3 rounded-2xl border border-border bg-background px-4 py-4 text-sm">
                        <input
                            type="checkbox"
                            class="mt-0.5"
                            prop:checked=move || moderated.get()
                            on:change=move |ev| set_moderated.set(event_target_checked(&ev))
                        />
                        <span class="space-y-1">
                            <span class="block font-medium text-foreground">{category_form_labels.moderated_title.clone()}</span>
                            <span class="block text-muted-foreground">
                                {category_form_labels.moderated_hint.clone()}
                            </span>
                        </span>
                    </label>
                    <div class="flex flex-wrap gap-3 pt-2">
                        <button
                            type="submit"
                            class="rounded-full bg-primary px-5 py-2.5 text-sm font-medium text-primary-foreground transition hover:opacity-95"
                            disabled=move || busy_key.get().is_some()
                        >
                            {move || if editing_id.get().is_some() { category_form_labels.save_category_label.clone() } else { category_form_labels.create_category_label.clone() }}
                        </button>
                        <button
                            type="button"
                            class="rounded-full border border-border px-5 py-2.5 text-sm font-medium transition hover:bg-muted"
                            on:click=move |_| on_reset.run(())
                        >
                            {category_form_labels.reset_label.clone()}
                        </button>
                    </div>
                </form>

                <div class="mt-6">
                    <SeoEntityPanel
                        target_kind=SeoTargetSlug::new(seo_builtin_slug::FORUM_CATEGORY).expect("builtin SEO target slug")
                        target_id=Signal::derive(move || editing_id.get())
                        locale=Signal::derive({
                            let host_locale_for_seo = host_locale_for_seo.clone();
                            move || host_locale_for_seo.clone()
                        })
                        show_control_plane_widgets=true
                        panel_title={
                            let category_seo_copy = category_seo_copy.clone();
                            move || category_seo_copy.title.clone()
                        }
                        panel_subtitle={
                            let category_seo_copy = category_seo_copy.clone();
                            move || category_seo_copy.subtitle.clone()
                        }
                        empty_message=move || category_seo_copy.empty_message.clone()
                    />
                </div>
            </section>
        </section>
    }
}

#[component]
fn TopicsPage(
    categories: LocalResource<Result<Vec<CategoryListItem>, String>>,
    topics: LocalResource<Result<Vec<TopicListItem>, String>>,
    replies: LocalResource<Result<Vec<ReplyListItem>, String>>,
    busy_key: ReadSignal<Option<String>>,
    editing_id: ReadSignal<Option<String>>,
    locale: ReadSignal<String>,
    set_locale: WriteSignal<String>,
    category_id: ReadSignal<String>,
    set_category_id: WriteSignal<String>,
    title: ReadSignal<String>,
    set_title: WriteSignal<String>,
    slug: ReadSignal<String>,
    set_slug: WriteSignal<String>,
    body: ReadSignal<String>,
    set_body: WriteSignal<String>,
    body_format: ReadSignal<String>,
    set_body_format: WriteSignal<String>,
    tags: ReadSignal<String>,
    set_tags: WriteSignal<String>,
    filter_category_id: ReadSignal<String>,
    set_filter_category_id: WriteSignal<String>,
    on_edit: Callback<String>,
    on_delete: Callback<String>,
    on_submit: impl Fn(SubmitEvent) + 'static,
    on_reset: Callback<()>,
) -> impl IntoView {
    let ui_locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let host_locale_for_seo = ui_locale.clone().unwrap_or_default();
    let placeholders = forum_admin_placeholder_policy(locale.get_untracked().as_str());
    let all_categories_label = t(
        ui_locale.as_deref(),
        "forum.topics.allCategories",
        "All categories",
    );
    let filtered_category_label = t(
        ui_locale.as_deref(),
        "forum.topics.filteredCategory",
        "Filtered category",
    );
    let ready_template = t(ui_locale.as_deref(), "forum.topics.ready", "{count} ready");
    let sidebar_copy = forum_admin_sidebar_copy_labels(
        t(
            ui_locale.as_deref(),
            "forum.topics.navigationLabel",
            "Navigation",
        ),
        t(
            ui_locale.as_deref(),
            "forum.topics.navigationTitle",
            "Forum feed",
        ),
        t(
            ui_locale.as_deref(),
            "forum.topics.navigationBody",
            "A left rail similar to NodeBB: jump between categories, keep counts visible, and open a thread into the editor on the right.",
        ),
        t(
            ui_locale.as_deref(),
            "forum.topics.filterTitle",
            "Filter topics",
        ),
        t(ui_locale.as_deref(), "forum.topics.clear", "Clear"),
        t(
            ui_locale.as_deref(),
            "forum.topics.activeFilter",
            "Active filter",
        ),
        t(ui_locale.as_deref(), "forum.topics.draftTags", "Draft tags"),
        t(
            ui_locale.as_deref(),
            "forum.topics.editingThread",
            "Editing thread",
        ),
    );
    let open_inspector_label = t(
        ui_locale.as_deref(),
        "forum.topics.openInspector",
        "Open in inspector",
    );
    let nothing_selected_label = t(
        ui_locale.as_deref(),
        "forum.topics.nothingSelected",
        "Nothing selected",
    );
    let topic_stream_labels = forum_admin_topic_stream_labels(
        t(
            ui_locale.as_deref(),
            "forum.topics.streamLabel",
            "Topic stream",
        ),
        t(
            ui_locale.as_deref(),
            "forum.topics.streamBody",
            "Open a topic card to inspect replies and edit the thread without leaving the feed.",
        ),
        t(ui_locale.as_deref(), "forum.topics.new", "New topic"),
    );
    let topic_form_labels = forum_admin_topic_form_labels(
        t(
            ui_locale.as_deref(),
            "forum.topics.inspectorLabel",
            "Inspector",
        ),
        t(ui_locale.as_deref(), "forum.form.locale", "Locale"),
        t(
            ui_locale.as_deref(),
            "forum.form.localeHintTopic",
            "Thread locale for publishing and reads.",
        ),
        t(ui_locale.as_deref(), "forum.form.category", "Category"),
        t(
            ui_locale.as_deref(),
            "forum.form.categoryHint",
            "Choose where the topic should live.",
        ),
        t(
            ui_locale.as_deref(),
            "forum.form.chooseCategory",
            "Choose category",
        ),
        t(ui_locale.as_deref(), "forum.form.title", "Title"),
        t(
            ui_locale.as_deref(),
            "forum.form.titleHint",
            "Headline shown in the feed.",
        ),
        t(ui_locale.as_deref(), "forum.form.slug", "Slug"),
        t(
            ui_locale.as_deref(),
            "forum.form.slugHintTopic",
            "Stable thread identifier.",
        ),
        t(ui_locale.as_deref(), "forum.form.bodyFormat", "Body format"),
        t(
            ui_locale.as_deref(),
            "forum.form.bodyFormatHint",
            "Usually `markdown`.",
        ),
        t(ui_locale.as_deref(), "forum.form.tags", "Tags"),
        t(
            ui_locale.as_deref(),
            "forum.form.tagsHint",
            "Comma-separated labels for discovery.",
        ),
        t(ui_locale.as_deref(), "forum.form.body", "Body"),
        t(
            ui_locale.as_deref(),
            "forum.form.bodyHint",
            "Main message shown in the topic detail.",
        ),
        t(ui_locale.as_deref(), "forum.form.saveTopic", "Save topic"),
        t(
            ui_locale.as_deref(),
            "forum.form.publishTopic",
            "Publish topic",
        ),
        t(ui_locale.as_deref(), "forum.form.reset", "Reset"),
    );
    let reply_preview_labels = forum_admin_reply_preview_labels(
        t(
            ui_locale.as_deref(),
            "forum.topics.previewLabel",
            "Thread preview",
        ),
        t(ui_locale.as_deref(), "forum.topics.previewTitle", "Replies"),
        t(ui_locale.as_deref(), "forum.topics.shown", "{count} shown"),
    );
    let edit_topic_title = t(ui_locale.as_deref(), "forum.topics.editTitle", "Edit topic");
    let compose_topic_title = t(
        ui_locale.as_deref(),
        "forum.topics.composeTitle",
        "Compose topic",
    );
    let thread_open_label = t(
        ui_locale.as_deref(),
        "forum.topics.threadOpen",
        "Thread open",
    );
    let topic_title_labels = ForumAdminTitleEnvelopeLabels {
        edit_title: edit_topic_title.clone(),
        create_title: compose_topic_title.clone(),
        active_badge: thread_open_label.clone(),
    };
    let topic_heading_labels = topic_title_labels.clone();
    let topic_badge_labels = topic_title_labels.clone();
    let topic_seo_copy = forum_admin_seo_copy_labels(
        ForumAdminSeoSurface::Topic,
        t(
            locale.get_untracked().as_str().into(),
            "forum.topics.seo.title",
            "Topic SEO",
        ),
        t(
            locale.get_untracked().as_str().into(),
            "forum.topics.seo.subtitle",
            "Explicit metadata, social tags and diagnostics for the selected forum topic.",
        ),
        t(
            locale.get_untracked().as_str().into(),
            "forum.topics.seo.empty",
            "Create or open a topic first. SEO stays attached to the forum thread editor.",
        ),
    );
    let selected_category_name = Memo::new(move |_| {
        selected_category_filter_label(
            categories.get(),
            filter_category_id.get().as_str(),
            all_categories_label.as_str(),
            filtered_category_label.as_str(),
        )
    });
    let topic_form_tag_count =
        move || forum_admin_topic_tag_count_label(tags.get().as_str(), ready_template.as_str());
    let sidebar_locale = ui_locale.clone();
    let topic_feed_locale = ui_locale.clone();
    let replies_locale = ui_locale.clone();

    view! {
        <section class="grid gap-6 xl:grid-cols-[17rem_minmax(0,1fr)_24rem]">
            <aside class="space-y-6 rounded-[1.75rem] border border-border bg-card p-5 shadow-sm xl:sticky xl:top-6 xl:self-start">
                <div>
                    <p class="text-xs font-semibold uppercase tracking-[0.24em] text-muted-foreground">
                        {sidebar_copy.navigation_label.clone()}
                    </p>
                    <h2 class="mt-2 text-xl font-semibold text-card-foreground">{sidebar_copy.navigation_title.clone()}</h2>
                    <p class="mt-2 text-sm leading-6 text-muted-foreground">
                        {sidebar_copy.navigation_body.clone()}
                    </p>
                </div>

                <div class="rounded-[1.5rem] border border-border bg-background/80 p-4">
                    <div class="flex items-center justify-between gap-3">
                        <p class="text-sm font-medium text-foreground">{sidebar_copy.filter_title.clone()}</p>
                        <button
                            type="button"
                            class="text-xs font-medium text-muted-foreground transition hover:text-foreground"
                            on:click=move |_| set_filter_category_id.set(String::new())
                        >
                            {sidebar_copy.clear_label.clone()}
                        </button>
                    </div>
                    <Suspense fallback=move || view! { <div class="mt-4 h-24 animate-pulse rounded-2xl bg-muted"></div> }>
                        {move || categories.get().map(|result| render_category_sidebar(result, filter_category_id.get(), set_filter_category_id, sidebar_locale.clone()))}
                    </Suspense>
                </div>

                <div class="space-y-3 rounded-[1.5rem] border border-border bg-gradient-to-br from-background to-muted/40 p-4">
                    <SidebarStat
                        label=sidebar_copy.active_filter_label.clone()
                        value=Signal::derive(move || selected_category_name.get())
                    />
                    <SidebarStat
                        label=sidebar_copy.draft_tags_label.clone()
                        value=Signal::derive(topic_form_tag_count)
                    />
                    <SidebarStat
                        label=sidebar_copy.editing_thread_label.clone()
                        value=Signal::derive(move || {
                            forum_admin_editing_thread_label(
                                editing_id.get().as_deref(),
                                open_inspector_label.as_str(),
                                nothing_selected_label.as_str(),
                            )
                        })
                    />
                </div>
            </aside>

            <div class="space-y-6">
                <section class="rounded-[1.75rem] border border-border bg-card p-6 shadow-sm">
                    <div class="flex flex-wrap items-start justify-between gap-4">
                        <div>
                            <p class="text-xs font-semibold uppercase tracking-[0.24em] text-muted-foreground">
                                {topic_stream_labels.stream_label.clone()}
                            </p>
                            <h2 class="mt-2 text-2xl font-semibold text-card-foreground">
                                {move || selected_category_name.get()}
                            </h2>
                            <p class="mt-2 max-w-2xl text-sm leading-6 text-muted-foreground">
                                {topic_stream_labels.stream_body.clone()}
                            </p>
                        </div>
                        <button
                            type="button"
                            class="rounded-full border border-border bg-background px-4 py-2 text-sm font-medium transition hover:bg-muted"
                            on:click=move |_| on_reset.run(())
                        >
                            {topic_stream_labels.new_topic_label.clone()}
                        </button>
                    </div>
                    <Suspense fallback=move || view! { <div class="mt-6 h-72 animate-pulse rounded-[1.5rem] bg-muted"></div> }>
                        {move || topics.get().map(|result| render_topic_feed(result, editing_id.get(), busy_key.get(), on_edit, on_delete, topic_feed_locale.clone()))}
                    </Suspense>
                </section>
            </div>

            <div class="space-y-6 xl:sticky xl:top-6 xl:self-start">
                <section class="rounded-[1.75rem] border border-border bg-card p-6 shadow-sm">
                    <div class="flex items-center justify-between gap-3">
                        <div>
                            <p class="text-xs font-semibold uppercase tracking-[0.24em] text-muted-foreground">
                                {topic_form_labels.inspector_label.clone()}
                            </p>
                            <h2 class="mt-2 text-xl font-semibold text-card-foreground">
                                {move || forum_admin_title_envelope_view_model(editing_id.get().is_some(), &topic_heading_labels).title}
                            </h2>
                        </div>
                        {move || forum_admin_title_envelope_view_model(editing_id.get().is_some(), &topic_badge_labels)
                            .active_badge
                            .map(|label| view! {
                                <span class="rounded-full bg-sky-500/15 px-3 py-1 text-xs font-medium text-sky-700 dark:text-sky-300">
                                    {label}
                                </span>
                            })}
                    </div>

                    <form class="mt-6 space-y-4" on:submit=on_submit>
                        <FieldShell label=topic_form_labels.locale_label.clone() hint=topic_form_labels.locale_hint.clone()>
                            <input
                                class="w-full rounded-2xl border border-border bg-background px-4 py-3 text-sm outline-none transition focus:border-primary"
                                prop:value=move || locale.get()
                                on:input=move |ev| set_locale.set(event_target_value(&ev))
                                placeholder=placeholders.locale.clone()
                            />
                        </FieldShell>
                        <FieldShell label=topic_form_labels.category_label.clone() hint=topic_form_labels.category_hint.clone()>
                            <Suspense fallback=move || view! { <div class="h-14 animate-pulse rounded-2xl bg-muted"></div> }>
                                {move || categories.get().map(|result| match result {
                                    Ok(items) => view! {
                                        <select
                                            class="w-full rounded-2xl border border-border bg-background px-4 py-3 text-sm outline-none transition focus:border-primary"
                                            prop:value=move || category_id.get()
                                            on:change=move |ev| set_category_id.set(event_target_value(&ev))
                                        >
                                            <option value="">{topic_form_labels.choose_category_label.clone()}</option>
                                            {category_select_options(&items, category_id.get().as_str())
                                                .into_iter()
                                                .map(|option| view! { <option value=option.value selected=option.is_selected>{option.label}</option> })
                                                .collect_view()}
                                        </select>
                                    }.into_any(),
                                    Err(err) => view! {
                                        <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                                            {err}
                                        </div>
                                    }.into_any(),
                                })}
                            </Suspense>
                        </FieldShell>
                        <FieldShell label=topic_form_labels.title_label.clone() hint=topic_form_labels.title_hint.clone()>
                            <input
                                class="w-full rounded-2xl border border-border bg-background px-4 py-3 text-sm outline-none transition focus:border-primary"
                                prop:value=move || title.get()
                                on:input=move |ev| set_title.set(event_target_value(&ev))
                                placeholder=placeholders.topic_title.clone()
                            />
                        </FieldShell>
                        <FieldShell label=topic_form_labels.slug_label.clone() hint=topic_form_labels.slug_hint.clone()>
                            <input
                                class="w-full rounded-2xl border border-border bg-background px-4 py-3 font-mono text-sm outline-none transition focus:border-primary"
                                prop:value=move || slug.get()
                                on:input=move |ev| set_slug.set(event_target_value(&ev))
                                placeholder=placeholders.topic_slug.clone()
                            />
                        </FieldShell>
                        <div class="grid gap-4 sm:grid-cols-2">
                            <FieldShell label=topic_form_labels.body_format_label.clone() hint=topic_form_labels.body_format_hint.clone()>
                                <input
                                    class="w-full rounded-2xl border border-border bg-background px-4 py-3 font-mono text-sm outline-none transition focus:border-primary"
                                    prop:value=move || body_format.get()
                                    on:input=move |ev| set_body_format.set(event_target_value(&ev))
                                    placeholder=placeholders.topic_body_format.clone()
                                />
                            </FieldShell>
                            <FieldShell label=topic_form_labels.tags_label.clone() hint=topic_form_labels.tags_hint.clone()>
                                <input
                                    class="w-full rounded-2xl border border-border bg-background px-4 py-3 text-sm outline-none transition focus:border-primary"
                                    prop:value=move || tags.get()
                                    on:input=move |ev| set_tags.set(event_target_value(&ev))
                                    placeholder=placeholders.topic_tags.clone()
                                />
                            </FieldShell>
                        </div>

                        {move || {
                            let parsed_tags = forum_admin_tag_chips(tags.get().as_str());
                            (!parsed_tags.is_empty()).then(|| {
                                view! {
                                    <div class="flex flex-wrap gap-2 rounded-2xl border border-border bg-background px-4 py-3">
                                        {parsed_tags.into_iter().map(|tag| view! {
                                            <span class="rounded-full bg-amber-500/15 px-2.5 py-1 text-xs font-medium text-amber-700 dark:text-amber-300">
                                                {tag}
                                            </span>
                                        }).collect_view()}
                                    </div>
                                }
                            })
                        }}

                        <FieldShell label=topic_form_labels.body_label.clone() hint=topic_form_labels.body_hint.clone()>
                            <textarea
                                class="min-h-72 w-full rounded-2xl border border-border bg-background px-4 py-3 font-mono text-sm outline-none transition focus:border-primary"
                                prop:value=move || body.get()
                                on:input=move |ev| set_body.set(event_target_value(&ev))
                                placeholder=placeholders.topic_body.clone()
                            ></textarea>
                        </FieldShell>

                        <div class="flex flex-wrap gap-3 pt-2">
                            <button
                                type="submit"
                                class="rounded-full bg-primary px-5 py-2.5 text-sm font-medium text-primary-foreground transition hover:opacity-95"
                                disabled=move || busy_key.get().is_some()
                            >
                                {move || if editing_id.get().is_some() { topic_form_labels.save_topic_label.clone() } else { topic_form_labels.publish_topic_label.clone() }}
                            </button>
                            <button
                                type="button"
                                class="rounded-full border border-border px-5 py-2.5 text-sm font-medium transition hover:bg-muted"
                                on:click=move |_| on_reset.run(())
                            >
                                {topic_form_labels.reset_label.clone()}
                            </button>
                        </div>
                    </form>
                </section>

                <section class="rounded-[1.75rem] border border-border bg-card p-6 shadow-sm">
                    <div class="flex items-center justify-between gap-3">
                        <div>
                            <p class="text-xs font-semibold uppercase tracking-[0.24em] text-muted-foreground">
                                {reply_preview_labels.preview_label.clone()}
                            </p>
                            <h2 class="mt-2 text-xl font-semibold text-card-foreground">{reply_preview_labels.preview_title.clone()}</h2>
                        </div>
                        <span class="rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground">
                            {move || reply_preview_labels.shown_template.replace("{count}", reply_count_label(replies.get()).to_string().as_str())}
                        </span>
                    </div>
                    <Suspense fallback=move || view! { <div class="mt-6 h-40 animate-pulse rounded-[1.5rem] bg-muted"></div> }>
                        {move || replies.get().map(|result| render_reply_stack(result, replies_locale.clone()))}
                    </Suspense>
                </section>

                <SeoEntityPanel
                    target_kind=SeoTargetSlug::new(seo_builtin_slug::FORUM_TOPIC).expect("builtin SEO target slug")
                    target_id=Signal::derive(move || editing_id.get())
                    locale=Signal::derive({
                        let host_locale_for_seo = host_locale_for_seo.clone();
                        move || host_locale_for_seo.clone()
                    })
                    show_control_plane_widgets=true
                    panel_title={
                        let topic_seo_copy = topic_seo_copy.clone();
                        move || topic_seo_copy.title.clone()
                    }
                    panel_subtitle={
                        let topic_seo_copy = topic_seo_copy.clone();
                        move || topic_seo_copy.subtitle.clone()
                    }
                    empty_message=move || topic_seo_copy.empty_message.clone()
                />
            </div>
        </section>
    }
}

fn render_category_grid(
    result: Result<Vec<CategoryListItem>, String>,
    editing_id: Option<String>,
    busy_key: Option<String>,
    on_edit: Callback<String>,
    on_delete: Callback<String>,
    locale: Option<String>,
) -> AnyView {
    let no_categories_label = t(
        locale.as_deref(),
        "forum.render.noCategories",
        "No categories yet.",
    );
    let category_labels = ForumAdminCategoryRenderLabels {
        no_description: t(
            locale.as_deref(),
            "forum.render.noDescription",
            "No description yet.",
        ),
        topics_count_template: t(
            locale.as_deref(),
            "forum.render.topicsCount",
            "topics: {count}",
        ),
        replies_count_template: t(
            locale.as_deref(),
            "forum.render.repliesCount",
            "replies: {count}",
        ),
        icon_template: t(locale.as_deref(), "forum.render.icon", "icon: {value}"),
        editing: t(locale.as_deref(), "forum.render.editing", "Editing"),
        edit: t(locale.as_deref(), "forum.render.edit", "Edit"),
    };
    let delete_label = t(locale.as_deref(), "forum.render.delete", "Delete");
    match forum_admin_collection_state(result) {
        ForumAdminCollectionState::Empty => view! { <div class="mt-6 rounded-[1.5rem] border border-dashed border-border p-8 text-sm text-muted-foreground">{no_categories_label}</div> }.into_any(),
        ForumAdminCollectionState::Ready(items) => view! {
            <div class="mt-6 grid gap-4 md:grid-cols-2">
                {items.into_iter().map(|item| {
                    let vm = category_card_view_model(
                        &item,
                        editing_id.as_deref(),
                        busy_key.as_deref(),
                        &category_labels,
                    );
                    let item_id = vm.id.clone();
                    view! {
                        <article class="relative overflow-hidden rounded-[1.5rem] border border-border bg-background p-5 shadow-sm">
                            <span class="absolute inset-y-0 left-0 w-1.5" style=vm.accent_style.clone()></span>
                            <div class="pl-3">
                                <div class="flex items-start justify-between gap-4">
                                    <div>
                                        <div class="text-[11px] font-semibold uppercase tracking-[0.22em] text-muted-foreground">
                                            {vm.effective_locale.clone()}
                                        </div>
                                        <h3 class="mt-2 text-lg font-semibold text-foreground">{vm.name.clone()}</h3>
                                    </div>
                                    <span class="rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground">
                                        {vm.slug_badge.clone()}
                                    </span>
                                </div>
                                <p class="mt-3 text-sm leading-6 text-muted-foreground">
                                    {vm.description.clone()}
                                </p>
                                <div class="mt-4 flex flex-wrap gap-2">
                                    <StaticChip label=vm.topics_count_label.clone() />
                                    <StaticChip label=vm.replies_count_label.clone() />
                                    {vm.icon_label.clone().map(|label| view! {
                                        <StaticChip label=label />
                                    })}
                                </div>
                                <div class="mt-5 flex flex-wrap gap-2">
                                    <button type="button" class=forum_admin_action_button_class(ForumAdminActionButtonKind::Action) on:click={ let item_id = item_id.clone(); move |_| on_edit.run(item_id.clone()) } disabled=vm.is_busy>{vm.action_label.clone()}</button>
                                    <button type="button" class=forum_admin_action_button_class(ForumAdminActionButtonKind::Delete) on:click={ let item_id = item_id.clone(); move |_| on_delete.run(item_id.clone()) } disabled=vm.is_busy>{delete_label.clone()}</button>
                                </div>
                            </div>
                        </article>
                    }
                }).collect_view()}
            </div>
        }.into_any(),
        ForumAdminCollectionState::Error(err) => view! { <div class="mt-6 rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{err}</div> }.into_any(),
    }
}

fn render_category_sidebar(
    result: Result<Vec<CategoryListItem>, String>,
    active_category_id: String,
    set_filter_category_id: WriteSignal<String>,
    locale: Option<String>,
) -> AnyView {
    let no_categories_label = t(
        locale.as_deref(),
        "forum.render.noCategories",
        "No categories yet.",
    );
    let all_categories_label = t(
        locale.as_deref(),
        "forum.topics.allCategories",
        "All categories",
    );
    match forum_admin_collection_state(result) {
        ForumAdminCollectionState::Empty => view! { <div class="mt-4 rounded-2xl border border-dashed border-border p-4 text-sm text-muted-foreground">{no_categories_label}</div> }.into_any(),
        ForumAdminCollectionState::Ready(items) => {
            let total_count = category_sidebar_total_count(&items);
            view! {
            <div class="mt-4 space-y-2">
                <button type="button" class=forum_admin_sidebar_category_class(active_category_id.is_empty()) on:click=move |_| set_filter_category_id.set(String::new())>
                    <span class="truncate">{all_categories_label}</span>
                    <span class="rounded-full bg-background/70 px-2 py-0.5 text-[11px] font-medium text-muted-foreground">{total_count}</span>
                </button>
                {items.into_iter().map(|item| {
                    let vm = category_sidebar_view_model(&item, active_category_id.as_str());
                    let item_id = vm.id.clone();
                    view! {
                        <button type="button" class=forum_admin_sidebar_category_class(vm.is_active) on:click=move |_| set_filter_category_id.set(item_id.clone())>
                            <span class="min-w-0">
                                <span class="block truncate text-left text-sm font-medium text-foreground">{vm.name.clone()}</span>
                                <span class="block truncate text-left text-xs text-muted-foreground">{vm.slug.clone()}</span>
                            </span>
                            <span class="rounded-full bg-background/70 px-2 py-0.5 text-[11px] font-medium text-muted-foreground">{vm.topic_count}</span>
                        </button>
                    }
                }).collect_view()}
            </div>
        }.into_any()
        },
        ForumAdminCollectionState::Error(err) => view! { <div class="mt-4 rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{err}</div> }.into_any(),
    }
}

fn render_topic_feed(
    result: Result<Vec<TopicListItem>, String>,
    editing_id: Option<String>,
    busy_key: Option<String>,
    on_edit: Callback<String>,
    on_delete: Callback<String>,
    locale: Option<String>,
) -> AnyView {
    let no_topics_label = t(locale.as_deref(), "forum.render.noTopics", "No topics yet.");
    let pinned_label = t(locale.as_deref(), "forum.render.pinned", "Pinned");
    let locked_label = t(locale.as_deref(), "forum.render.locked", "Locked");
    let topic_labels = ForumAdminTopicRenderLabels {
        thread_path_template: t(
            locale.as_deref(),
            "forum.render.threadPath",
            "thread/{category}/{slug}",
        ),
        opened: t(locale.as_deref(), "forum.render.opened", "Opened"),
        open_thread: t(locale.as_deref(), "forum.render.openThread", "Open thread"),
    };
    let replies_label = t(locale.as_deref(), "forum.render.replies", "Replies");
    let delete_label = t(locale.as_deref(), "forum.render.delete", "Delete");
    match forum_admin_collection_state(result) {
        ForumAdminCollectionState::Empty => view! { <div class="mt-6 rounded-[1.5rem] border border-dashed border-border p-8 text-sm text-muted-foreground">{no_topics_label}</div> }.into_any(),
        ForumAdminCollectionState::Ready(items) => view! {
            <div class="mt-6 space-y-3">
                {items.into_iter().map(|item| {
                    let vm = topic_card_view_model(
                        &item,
                        editing_id.as_deref(),
                        busy_key.as_deref(),
                        &topic_labels,
                    );
                    let item_id = vm.id.clone();
                    view! {
                        <article class="rounded-[1.5rem] border border-border bg-background p-5 shadow-sm transition hover:border-primary/30 hover:shadow-md">
                            <div class="flex flex-wrap items-start justify-between gap-4">
                                <div class="space-y-3">
                                    <div class="flex flex-wrap items-center gap-2">
                                        <span class=forum_admin_status_badge_class(vm.status_class)>{vm.status.clone()}</span>
                                        <span class="rounded-full border border-border px-2.5 py-1 text-[11px] font-medium text-muted-foreground">{vm.effective_locale.clone()}</span>
                                        {vm.pinned.then(|| view! { <span class="rounded-full bg-amber-500/15 px-2.5 py-1 text-[11px] font-medium text-amber-700 dark:text-amber-300">{pinned_label.clone()}</span> })}
                                        {vm.locked.then(|| view! { <span class="rounded-full bg-destructive/10 px-2.5 py-1 text-[11px] font-medium text-destructive">{locked_label.clone()}</span> })}
                                    </div>
                                    <div>
                                        <h3 class="text-lg font-semibold text-foreground">{vm.title.clone()}</h3>
                                        <p class="mt-1 text-sm text-muted-foreground">{vm.thread_path.clone()}</p>
                                    </div>
                                </div>
                                <div class="text-right">
                                    <p class="text-[11px] font-semibold uppercase tracking-[0.22em] text-muted-foreground">{replies_label.clone()}</p>
                                    <p class="mt-1 text-2xl font-semibold text-foreground">{vm.reply_count}</p>
                                </div>
                            </div>
                            <div class="mt-5 flex flex-wrap gap-2">
                                <button type="button" class=forum_admin_action_button_class(ForumAdminActionButtonKind::Action) on:click={ let item_id = item_id.clone(); move |_| on_edit.run(item_id.clone()) } disabled=vm.is_busy>{vm.action_label.clone()}</button>
                                <button type="button" class=forum_admin_action_button_class(ForumAdminActionButtonKind::Delete) on:click={ let item_id = item_id.clone(); move |_| on_delete.run(item_id.clone()) } disabled=vm.is_busy>{delete_label.clone()}</button>
                            </div>
                        </article>
                    }
                }).collect_view()}
            </div>
        }.into_any(),
        ForumAdminCollectionState::Error(err) => view! { <div class="mt-6 rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{err}</div> }.into_any(),
    }
}

fn render_reply_stack(
    result: Result<Vec<ReplyListItem>, String>,
    locale: Option<String>,
) -> AnyView {
    let empty_label = t(
        locale.as_deref(),
        "forum.render.openTopicForReplies",
        "Open a topic card to preview replies.",
    );
    match forum_admin_collection_state(result) {
        ForumAdminCollectionState::Empty => view! { <div class="mt-6 rounded-[1.5rem] border border-dashed border-border p-6 text-sm text-muted-foreground">{empty_label}</div> }.into_any(),
        ForumAdminCollectionState::Ready(items) => view! {
            <div class="mt-6 space-y-3">
                {items.into_iter().map(|item| {
                    let vm = reply_card_view_model(&item);
                    view! {
                        <article class="rounded-[1.35rem] border border-border bg-background p-4">
                            <div class="flex items-center justify-between gap-3">
                                <span class=forum_admin_status_badge_class(vm.status_class)>{vm.status.clone()}</span>
                                <span class="text-[11px] font-semibold uppercase tracking-[0.22em] text-muted-foreground">{vm.effective_locale.clone()}</span>
                            </div>
                            <p class="mt-3 text-sm leading-6 text-muted-foreground">{vm.content_preview.clone()}</p>
                        </article>
                    }
                }).collect_view()}
            </div>
        }.into_any(),
        ForumAdminCollectionState::Error(err) => view! { <div class="mt-6 rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{err}</div> }.into_any(),
    }
}

fn apply_category_to_form(
    set_editing_category_id: WriteSignal<Option<String>>,
    set_category_locale: WriteSignal<String>,
    set_category_name: WriteSignal<String>,
    set_category_slug: WriteSignal<String>,
    set_category_description: WriteSignal<String>,
    set_category_icon: WriteSignal<String>,
    set_category_color: WriteSignal<String>,
    set_category_position: WriteSignal<i32>,
    set_category_moderated: WriteSignal<bool>,
    form: CategoryFormSnapshot,
) {
    set_editing_category_id.set(form.editing_id);
    set_category_locale.set(form.locale);
    set_category_name.set(form.name);
    set_category_slug.set(form.slug);
    set_category_description.set(form.description);
    set_category_icon.set(form.icon);
    set_category_color.set(form.color);
    set_category_position.set(form.position);
    set_category_moderated.set(form.moderated);
}

fn apply_topic_to_form(
    set_editing_topic_id: WriteSignal<Option<String>>,
    set_topic_locale: WriteSignal<String>,
    set_topic_category_id: WriteSignal<String>,
    set_topic_title: WriteSignal<String>,
    set_topic_slug: WriteSignal<String>,
    set_topic_body: WriteSignal<String>,
    set_topic_body_format: WriteSignal<String>,
    set_topic_tags: WriteSignal<String>,
    form: TopicFormSnapshot,
) {
    set_editing_topic_id.set(form.editing_id);
    set_topic_locale.set(form.locale);
    set_topic_category_id.set(form.category_id);
    set_topic_title.set(form.title);
    set_topic_slug.set(form.slug);
    set_topic_body.set(form.body);
    set_topic_body_format.set(form.body_format);
    set_topic_tags.set(form.tags_raw);
}

fn clear_category_form(
    set_editing_category_id: WriteSignal<Option<String>>,
    set_category_name: WriteSignal<String>,
    set_category_slug: WriteSignal<String>,
    set_category_description: WriteSignal<String>,
    set_category_icon: WriteSignal<String>,
    set_category_color: WriteSignal<String>,
    set_category_position: WriteSignal<i32>,
    set_category_moderated: WriteSignal<bool>,
) {
    set_editing_category_id.set(None);
    set_category_name.set(String::new());
    set_category_slug.set(String::new());
    set_category_description.set(String::new());
    set_category_icon.set(String::new());
    set_category_color.set(String::new());
    set_category_position.set(0);
    set_category_moderated.set(false);
}

fn clear_topic_form(
    set_editing_topic_id: WriteSignal<Option<String>>,
    set_topic_category_id: WriteSignal<String>,
    set_topic_title: WriteSignal<String>,
    set_topic_slug: WriteSignal<String>,
    set_topic_body: WriteSignal<String>,
    set_topic_body_format: WriteSignal<String>,
    set_topic_tags: WriteSignal<String>,
) {
    set_editing_topic_id.set(None);
    set_topic_category_id.set(String::new());
    set_topic_title.set(String::new());
    set_topic_slug.set(String::new());
    set_topic_body.set(String::new());
    set_topic_body_format.set("markdown".to_string());
    set_topic_tags.set(String::new());
}
