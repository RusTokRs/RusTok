use std::sync::Arc;

use leptos::prelude::*;
use leptos::task::spawn_local;
use rustok_ui_core::UiRouteContext;

use crate::core::{
    build_marketplace_listing_admin_shell, selected_transport_profile,
    MarketplaceListingAdminTransportProfile,
};
use crate::i18n::normalize_admin_locale;
use crate::model::{
    MarketplaceListingAdminCommand, MarketplaceListingAdminDetail,
    MarketplaceListingAdminDirectory, MarketplaceListingAdminFilters,
    MarketplaceListingCreateDraft, MarketplaceListingTermsDraft,
};
use crate::transport::{
    execute_marketplace_listing_command, load_marketplace_listing_detail,
    load_marketplace_listing_directory, MarketplaceListingAdminTransportContext,
};

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
pub fn MarketplaceListingAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = normalize_admin_locale(route_context.locale.as_deref());
    let russian = locale == "ru";
    let profile = selected_transport_profile(option_env!("RUSTOK_UI_TRANSPORT_PROFILE"));
    let shell = build_marketplace_listing_admin_shell(Some(locale), profile);
    let transport = transport_context(profile);

    let refresh_nonce = RwSignal::new(0_u64);
    let selected_id = RwSignal::new(Option::<String>::None);
    let search = RwSignal::new(String::new());
    let status_filter = RwSignal::new(String::new());
    let approval_filter = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let error = RwSignal::new(Option::<String>::None);
    let notice = RwSignal::new(Option::<String>::None);
    let pending_command =
        RwSignal::new(Option::<(String, MarketplaceListingAdminCommand)>::None);

    let create_seller_id = RwSignal::new(String::new());
    let create_variant_id = RwSignal::new(String::new());
    let create_sku = RwSignal::new(String::new());
    let create_market = RwSignal::new(String::new());
    let create_channel = RwSignal::new(String::new());
    let pricing_reference = RwSignal::new(String::new());
    let inventory_reference = RwSignal::new(String::new());
    let fulfillment_profile = RwSignal::new(String::new());
    let moderation_note = RwSignal::new(String::new());
    let suspension_reason = RwSignal::new(String::new());

    let directory_transport = transport.clone();
    let directory = local_resource(
        move || {
            (
                refresh_nonce.get(),
                search.get(),
                status_filter.get(),
                approval_filter.get(),
            )
        },
        move |(_, search, status, approval_status)| {
            let context = directory_transport.clone();
            async move {
                load_marketplace_listing_directory(
                    context,
                    MarketplaceListingAdminFilters {
                        search: optional_text(search),
                        status: optional_text(status),
                        approval_status: optional_text(approval_status),
                        page: 1,
                        per_page: 50,
                        ..Default::default()
                    },
                )
                .await
            }
        },
    );

    let detail_transport = transport.clone();
    let detail = local_resource(
        move || (refresh_nonce.get(), selected_id.get()),
        move |(_, listing_id)| {
            let context = detail_transport.clone();
            async move {
                match listing_id {
                    Some(listing_id) => load_marketplace_listing_detail(context, listing_id)
                        .await
                        .map(Some),
                    None => Ok(None),
                }
            }
        },
    );

    let run_command: Arc<dyn Fn(MarketplaceListingAdminCommand) + Send + Sync> = Arc::new({
        let transport = transport.clone();
        move |command: MarketplaceListingAdminCommand| {
            if busy.get_untracked() {
                return;
            }
            let idempotency_key = pending_command
                .get_untracked()
                .as_ref()
                .filter(|(_, pending)| pending == &command)
                .map(|(key, _)| key.clone())
                .unwrap_or_else(|| format!("marketplace-listing-admin-{}", uuid::Uuid::new_v4()));
            pending_command.set(Some((idempotency_key.clone(), command.clone())));
            busy.set(true);
            error.set(None);
            notice.set(None);
            let context = transport.clone();
            spawn_local(async move {
                match execute_marketplace_listing_command(context, idempotency_key, command).await {
                    Ok(result) => {
                        selected_id.set(Some(result.listing.id));
                        pending_command.set(None);
                        notice.set(Some(label(
                            russian,
                            "Marketplace listing command completed.",
                            "Команда листинга выполнена.",
                        )));
                        refresh_nonce.update(|value| *value = value.saturating_add(1));
                    }
                    Err(transport_error) => error.set(Some(transport_error.to_string())),
                }
                busy.set(false);
            });
        }
    });

    let retry_command = run_command.clone();
    let create_command = run_command.clone();

    view! {
        <section class="marketplace-listing-admin" data-transport-profile=shell.transport_profile>
            <header class="marketplace-listing-admin__header">
                <p class="marketplace-listing-admin__family">"Marketplace Family"</p>
                <h1>{shell.title.clone()}</h1>
                <p>{shell.subtitle}</p>
                <p>{format!("{}: {}", label(russian, "Transport", "Транспорт"), profile.as_str())}</p>
            </header>

            {move || error.get().map(|message| view! {
                <div class="marketplace-listing-admin__error" role="alert">
                    <p>{message}</p>
                    <button
                        type="button"
                        disabled=move || busy.get() || pending_command.get().is_none()
                        on:click={
                            let retry_command = retry_command.clone();
                            move |_| {
                                if let Some((_, command)) = pending_command.get_untracked() {
                                    retry_command(command);
                                }
                            }
                        }
                    >
                        {label(russian, "Retry same command", "Повторить ту же команду")}
                    </button>
                </div>
            })}
            {move || notice.get().map(|message| view! {
                <div class="marketplace-listing-admin__notice" role="status">{message}</div>
            })}

            <div class="marketplace-listing-admin__layout">
                <aside class="marketplace-listing-admin__directory">
                    {render_filters(russian, search, status_filter, approval_filter)}
                    <Suspense fallback=move || view! { <p>{label(russian, "Loading listings...", "Загрузка листингов...")}</p> }>
                        {move || directory.get().map(|result| match result {
                            Ok(directory) => render_directory(russian, directory, selected_id).into_any(),
                            Err(transport_error) => view! {
                                <p class="marketplace-listing-admin__error">{transport_error.to_string()}</p>
                            }.into_any(),
                        })}
                    </Suspense>
                    {render_create_form(
                        russian,
                        busy,
                        create_seller_id,
                        create_variant_id,
                        create_sku,
                        create_market,
                        create_channel,
                        create_command,
                    )}
                </aside>

                <main class="marketplace-listing-admin__detail">
                    <Suspense fallback=move || view! { <p>{label(russian, "Loading listing detail...", "Загрузка листинга...")}</p> }>
                        {move || detail.get().map(|result| match result {
                            Ok(None) => view! {
                                <p>{label(
                                    russian,
                                    "Select a listing to inspect terms and immutable history.",
                                    "Выберите листинг для просмотра условий и неизменяемой истории.",
                                )}</p>
                            }.into_any(),
                            Ok(Some(detail)) => render_detail(
                                russian,
                                detail,
                                shell.legacy_attribution_label.clone(),
                                busy,
                                pricing_reference,
                                inventory_reference,
                                fulfillment_profile,
                                moderation_note,
                                suspension_reason,
                                run_command.clone(),
                            ).into_any(),
                            Err(transport_error) => view! {
                                <p class="marketplace-listing-admin__error">{transport_error.to_string()}</p>
                            }.into_any(),
                        })}
                    </Suspense>
                </main>
            </div>
        </section>
    }
}

fn render_filters(
    russian: bool,
    search: RwSignal<String>,
    status: RwSignal<String>,
    approval: RwSignal<String>,
) -> impl IntoView {
    view! {
        <div class="marketplace-listing-admin__filters">
            <input
                type="search"
                placeholder=label(russian, "Search listing scope or SKU", "Поиск по листингу или SKU")
                prop:value=move || search.get()
                on:input=move |event| search.set(event_target_value(&event))
            />
            <select prop:value=move || status.get() on:change=move |event| status.set(event_target_value(&event))>
                <option value="">{label(russian, "All statuses", "Все статусы")}</option>
                <option value="draft">"draft"</option>
                <option value="pending_review">"pending_review"</option>
                <option value="active">"active"</option>
                <option value="suspended">"suspended"</option>
                <option value="archived">"archived"</option>
            </select>
            <select prop:value=move || approval.get() on:change=move |event| approval.set(event_target_value(&event))>
                <option value="">{label(russian, "All approval states", "Все состояния проверки")}</option>
                <option value="draft">"draft"</option>
                <option value="pending">"pending"</option>
                <option value="approved">"approved"</option>
                <option value="rejected">"rejected"</option>
            </select>
        </div>
    }
}

fn render_directory(
    russian: bool,
    directory: MarketplaceListingAdminDirectory,
    selected_id: RwSignal<Option<String>>,
) -> impl IntoView {
    if directory.items.is_empty() {
        return view! {
            <p class="marketplace-listing-admin__empty">
                {label(russian, "No listings match the filters.", "Листинги по фильтрам не найдены.")}
            </p>
        }
        .into_any();
    }

    view! {
        <p>{format!("{}: {}", label(russian, "Total", "Всего"), directory.total)}</p>
        <ul class="marketplace-listing-admin__listing-list">
            {directory.items.into_iter().map(|listing| {
                let active_id = listing.id.clone();
                let click_id = listing.id.clone();
                view! {
                    <li>
                        <button
                            type="button"
                            class:active=move || selected_id.get().as_deref() == Some(active_id.as_str())
                            on:click=move |_| selected_id.set(Some(click_id.clone()))
                        >
                            <strong>{listing.seller_sku}</strong>
                            <span>{format!(
                                "{} / {} · {} · {}",
                                listing.market_slug,
                                listing.channel_slug,
                                listing.status,
                                listing.approval_status,
                            )}</span>
                        </button>
                    </li>
                }
            }).collect_view()}
        </ul>
    }
    .into_any()
}

#[allow(clippy::too_many_arguments)]
fn render_create_form(
    russian: bool,
    busy: RwSignal<bool>,
    seller_id: RwSignal<String>,
    variant_id: RwSignal<String>,
    sku: RwSignal<String>,
    market: RwSignal<String>,
    channel: RwSignal<String>,
    run_command: Arc<dyn Fn(MarketplaceListingAdminCommand) + Send + Sync>,
) -> impl IntoView {
    view! {
        <section class="marketplace-listing-admin__create">
            <h2>{label(russian, "Create listing", "Создать листинг")}</h2>
            <input placeholder="seller UUID" prop:value=move || seller_id.get() on:input=move |event| seller_id.set(event_target_value(&event)) />
            <input placeholder="master variant UUID" prop:value=move || variant_id.get() on:input=move |event| variant_id.set(event_target_value(&event)) />
            <input placeholder="seller SKU" prop:value=move || sku.get() on:input=move |event| sku.set(event_target_value(&event)) />
            <input placeholder="market" prop:value=move || market.get() on:input=move |event| market.set(event_target_value(&event)) />
            <input placeholder="channel" prop:value=move || channel.get() on:input=move |event| channel.set(event_target_value(&event)) />
            <button
                type="button"
                disabled=move || busy.get()
                on:click=move |_| run_command(MarketplaceListingAdminCommand::Create {
                    draft: MarketplaceListingCreateDraft {
                        seller_id: seller_id.get_untracked(),
                        master_variant_id: variant_id.get_untracked(),
                        seller_sku: sku.get_untracked(),
                        market_slug: market.get_untracked(),
                        channel_slug: channel.get_untracked(),
                        metadata: serde_json::json!({}),
                        ..Default::default()
                    },
                })
            >
                {label(russian, "Create", "Создать")}
            </button>
        </section>
    }
}

#[allow(clippy::too_many_arguments)]
fn render_detail(
    russian: bool,
    detail: MarketplaceListingAdminDetail,
    legacy_label: String,
    busy: RwSignal<bool>,
    pricing_reference: RwSignal<String>,
    inventory_reference: RwSignal<String>,
    fulfillment_profile: RwSignal<String>,
    moderation_note: RwSignal<String>,
    suspension_reason: RwSignal<String>,
    run_command: Arc<dyn Fn(MarketplaceListingAdminCommand) + Send + Sync>,
) -> impl IntoView {
    let listing = detail.listing;
    let listing_id = listing.id.clone();
    let update_id = listing_id.clone();
    let submit_id = listing_id.clone();
    let approve_id = listing_id.clone();
    let reject_id = listing_id.clone();
    let publish_id = listing_id.clone();
    let suspend_id = listing_id.clone();
    let reactivate_id = listing_id.clone();
    let archive_id = listing_id.clone();
    let update_command = run_command.clone();
    let submit_command = run_command.clone();
    let approve_command = run_command.clone();
    let reject_command = run_command.clone();
    let publish_command = run_command.clone();
    let suspend_command = run_command.clone();
    let reactivate_command = run_command.clone();
    let archive_command = run_command.clone();

    view! {
        <article class="marketplace-listing-admin__record">
            <h2>{listing.seller_sku}</h2>
            <dl>
                <dt>"ID"</dt><dd>{listing_id.clone()}</dd>
                <dt>{label(russian, "Seller", "Продавец")}</dt><dd>{listing.seller_id}</dd>
                <dt>{label(russian, "Variant", "Вариант")}</dt><dd>{listing.master_variant_id}</dd>
                <dt>{label(russian, "Scope", "Область")}</dt><dd>{format!("{} / {}", listing.market_slug, listing.channel_slug)}</dd>
                <dt>{label(russian, "State", "Состояние")}</dt><dd>{format!("{} / {}", listing.status, listing.approval_status)}</dd>
                <dt>{label(russian, "Terms version", "Версия условий")}</dt><dd>{listing.current_terms_version}</dd>
            </dl>

            <section class="marketplace-listing-admin__terms">
                <h3>{label(russian, "Commercial references", "Коммерческие ссылки")}</h3>
                <input placeholder="pricing reference" prop:value=move || pricing_reference.get() on:input=move |event| pricing_reference.set(event_target_value(&event)) />
                <input placeholder="inventory reference" prop:value=move || inventory_reference.get() on:input=move |event| inventory_reference.set(event_target_value(&event)) />
                <input placeholder="fulfillment profile" prop:value=move || fulfillment_profile.get() on:input=move |event| fulfillment_profile.set(event_target_value(&event)) />
                <button type="button" disabled=move || busy.get() on:click=move |_| {
                    update_command(MarketplaceListingAdminCommand::UpdateTerms {
                        listing_id: update_id.clone(),
                        draft: MarketplaceListingTermsDraft {
                            pricing_reference: optional_text(pricing_reference.get_untracked()),
                            inventory_reference: optional_text(inventory_reference.get_untracked()),
                            fulfillment_profile_slug: optional_text(fulfillment_profile.get_untracked()),
                            metadata: serde_json::json!({}),
                        },
                    })
                }>{label(russian, "Save terms", "Сохранить условия")}</button>
            </section>

            <section class="marketplace-listing-admin__commands">
                <h3>{label(russian, "Lifecycle", "Жизненный цикл")}</h3>
                <textarea placeholder=label(russian, "Moderation note", "Комментарий модерации") prop:value=move || moderation_note.get() on:input=move |event| moderation_note.set(event_target_value(&event)) />
                <input placeholder=label(russian, "Suspension reason", "Причина приостановки") prop:value=move || suspension_reason.get() on:input=move |event| suspension_reason.set(event_target_value(&event)) />
                <button type="button" disabled=move || busy.get() on:click=move |_| submit_command(MarketplaceListingAdminCommand::SubmitForReview { listing_id: submit_id.clone() })>{label(russian, "Submit", "Отправить на проверку")}</button>
                <button type="button" disabled=move || busy.get() on:click=move |_| approve_command(MarketplaceListingAdminCommand::Review { listing_id: approve_id.clone(), approved: true, note: optional_text(moderation_note.get_untracked()) })>{label(russian, "Approve", "Одобрить")}</button>
                <button type="button" disabled=move || busy.get() on:click=move |_| reject_command(MarketplaceListingAdminCommand::Review { listing_id: reject_id.clone(), approved: false, note: optional_text(moderation_note.get_untracked()) })>{label(russian, "Reject", "Отклонить")}</button>
                <button type="button" disabled=move || busy.get() on:click=move |_| publish_command(MarketplaceListingAdminCommand::Publish { listing_id: publish_id.clone() })>{label(russian, "Publish", "Опубликовать")}</button>
                <button type="button" disabled=move || busy.get() on:click=move |_| suspend_command(MarketplaceListingAdminCommand::Suspend { listing_id: suspend_id.clone(), reason: suspension_reason.get_untracked() })>{label(russian, "Suspend", "Приостановить")}</button>
                <button type="button" disabled=move || busy.get() on:click=move |_| reactivate_command(MarketplaceListingAdminCommand::Reactivate { listing_id: reactivate_id.clone() })>{label(russian, "Reactivate", "Возобновить")}</button>
                <button type="button" disabled=move || busy.get() on:click=move |_| archive_command(MarketplaceListingAdminCommand::Archive { listing_id: archive_id.clone() })>{label(russian, "Archive", "Архивировать")}</button>
            </section>

            <section class="marketplace-listing-admin__events">
                <h3>{label(russian, "Immutable history", "Неизменяемая история")}</h3>
                <ol>
                    {detail.events.into_iter().map(|event| {
                        let attribution = if event.has_unknown_attribution() {
                            legacy_label.clone()
                        } else {
                            format!(
                                "{} · {}",
                                event.actor_id.unwrap_or_else(|| "unknown".to_string()),
                                event.locale.unwrap_or_else(|| "unknown".to_string()),
                            )
                        };
                        view! {
                            <li data-provenance=event.provenance>
                                <strong>{event.event_kind}</strong>
                                <span>{event.created_at}</span>
                                <span>{attribution}</span>
                                {event.note.map(|note| view! { <p>{note}</p> })}
                            </li>
                        }
                    }).collect_view()}
                </ol>
            </section>
        </article>
    }
}

fn transport_context(
    profile: MarketplaceListingAdminTransportProfile,
) -> MarketplaceListingAdminTransportContext {
    match profile {
        MarketplaceListingAdminTransportProfile::Native => {
            MarketplaceListingAdminTransportContext::native()
        }
        MarketplaceListingAdminTransportProfile::Graphql => {
            MarketplaceListingAdminTransportContext::graphql(None, None)
        }
    }
}

fn optional_text(value: String) -> Option<String> {
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn label(russian: bool, english: &str, russian_text: &str) -> String {
    if russian {
        russian_text.to_string()
    } else {
        english.to_string()
    }
}
