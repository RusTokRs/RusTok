use std::sync::Arc;

use leptos::prelude::*;
use leptos::task::spawn_local;
use rustok_ui_core::UiRouteContext;

use crate::core::{
    MarketplaceSellerAdminTransportProfile, build_marketplace_seller_admin_shell,
    selected_transport_profile,
};
use crate::i18n::normalize_admin_locale;
use crate::model::{
    MarketplaceSellerAdminCommand, MarketplaceSellerAdminDetail, MarketplaceSellerAdminDirectory,
    MarketplaceSellerAdminFilters, MarketplaceSellerCreateDraft,
    MarketplaceSellerMemberCreateDraft, MarketplaceSellerMemberUpdateDraft,
    MarketplaceSellerProfileDraft,
};
use crate::transport::{
    MarketplaceSellerAdminTransportContext, execute_marketplace_seller_command,
    load_marketplace_seller_detail, load_marketplace_seller_directory,
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
pub fn MarketplaceSellerAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = normalize_admin_locale(route_context.locale.as_deref());
    let russian = locale == "ru";
    let profile = selected_transport_profile(option_env!("RUSTOK_UI_TRANSPORT_PROFILE"));
    let shell = build_marketplace_seller_admin_shell(Some(locale), profile);
    let transport = transport_context(profile);

    let refresh_nonce = RwSignal::new(0_u64);
    let selected_id = RwSignal::new(Option::<String>::None);
    let search = RwSignal::new(String::new());
    let status_filter = RwSignal::new(String::new());
    let onboarding_filter = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let error = RwSignal::new(Option::<String>::None);
    let notice = RwSignal::new(Option::<String>::None);
    let pending_command = RwSignal::new(Option::<(String, MarketplaceSellerAdminCommand)>::None);

    let create_handle = RwSignal::new(String::new());
    let create_display_name = RwSignal::new(String::new());
    let create_legal_name = RwSignal::new(String::new());
    let create_owner_user_id = RwSignal::new(String::new());
    let profile_display_name = RwSignal::new(String::new());
    let profile_legal_name = RwSignal::new(String::new());
    let onboarding_note = RwSignal::new(String::new());
    let suspension_reason = RwSignal::new(String::new());
    let member_user_id = RwSignal::new(String::new());
    let member_role = RwSignal::new("member".to_string());

    let directory_transport = transport.clone();
    let directory = local_resource(
        move || {
            (
                refresh_nonce.get(),
                search.get(),
                status_filter.get(),
                onboarding_filter.get(),
            )
        },
        move |(_, search, status, onboarding_status)| {
            let context = directory_transport.clone();
            async move {
                load_marketplace_seller_directory(
                    context,
                    MarketplaceSellerAdminFilters {
                        search: optional_text(search),
                        status: optional_text(status),
                        onboarding_status: optional_text(onboarding_status),
                        page: 1,
                        per_page: 50,
                    },
                )
                .await
            }
        },
    );

    let detail_transport = transport.clone();
    let detail = local_resource(
        move || (refresh_nonce.get(), selected_id.get()),
        move |(_, seller_id)| {
            let context = detail_transport.clone();
            async move {
                match seller_id {
                    Some(seller_id) => load_marketplace_seller_detail(context, seller_id)
                        .await
                        .map(Some),
                    None => Ok(None),
                }
            }
        },
    );

    let run_command: Arc<dyn Fn(MarketplaceSellerAdminCommand) + Send + Sync> = Arc::new({
        let transport = transport.clone();
        move |command: MarketplaceSellerAdminCommand| {
            if busy.get_untracked() {
                return;
            }
            let idempotency_key = pending_command
                .get_untracked()
                .as_ref()
                .filter(|(_, pending)| pending == &command)
                .map(|(key, _)| key.clone())
                .unwrap_or_else(|| format!("marketplace-seller-admin-{}", uuid::Uuid::new_v4()));
            pending_command.set(Some((idempotency_key.clone(), command.clone())));
            busy.set(true);
            error.set(None);
            notice.set(None);
            let context = transport.clone();
            spawn_local(async move {
                match execute_marketplace_seller_command(context, idempotency_key, command).await {
                    Ok(result) => {
                        if let Some(seller) = result.seller {
                            profile_display_name.set(seller.display_name.clone());
                            profile_legal_name.set(seller.legal_name.clone().unwrap_or_default());
                            selected_id.set(Some(seller.id));
                        }
                        pending_command.set(None);
                        notice.set(Some(localized(
                            russian,
                            "Marketplace seller command completed.",
                            "Команда продавца выполнена.",
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
        <section class="marketplace-seller-admin" data-transport-profile=shell.transport_profile>
            <header class="marketplace-seller-admin__header">
                <p class="marketplace-seller-admin__family">"Marketplace Family"</p>
                <h1>{shell.title}</h1>
                <p>{shell.subtitle}</p>
                <p>{format!("{}: {}", label(russian, "Transport", "Транспорт"), profile.as_str())}</p>
            </header>

            {move || error.get().map(|message| view! {
                <div class="marketplace-seller-admin__error" role="alert">
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
                <div class="marketplace-seller-admin__notice" role="status">{message}</div>
            })}

            <div class="marketplace-seller-admin__layout">
                <aside class="marketplace-seller-admin__directory">
                    {render_filters(russian, search, status_filter, onboarding_filter)}
                    <Suspense fallback=move || view! { <p>{label(russian, "Loading sellers...", "Загрузка продавцов...")}</p> }>
                        {move || directory.get().map(|result| match result {
                            Ok(directory) => render_directory(
                                russian,
                                directory,
                                selected_id,
                                profile_display_name,
                                profile_legal_name,
                            ).into_any(),
                            Err(transport_error) => view! {
                                <p class="marketplace-seller-admin__error">{transport_error.to_string()}</p>
                            }.into_any(),
                        })}
                    </Suspense>
                    {render_create_form(
                        russian,
                        busy,
                        create_handle,
                        create_display_name,
                        create_legal_name,
                        create_owner_user_id,
                        create_command,
                    )}
                </aside>

                <main class="marketplace-seller-admin__detail">
                    <Suspense fallback=move || view! { <p>{label(russian, "Loading seller detail...", "Загрузка продавца...")}</p> }>
                        {move || detail.get().map(|result| match result {
                            Ok(None) => view! {
                                <p>{label(
                                    russian,
                                    "Select a seller to inspect lifecycle and members.",
                                    "Выберите продавца для просмотра жизненного цикла и участников.",
                                )}</p>
                            }.into_any(),
                            Ok(Some(detail)) => render_detail(
                                russian,
                                detail,
                                busy,
                                profile_display_name,
                                profile_legal_name,
                                onboarding_note,
                                suspension_reason,
                                member_user_id,
                                member_role,
                                run_command.clone(),
                            ).into_any(),
                            Err(transport_error) => view! {
                                <p class="marketplace-seller-admin__error">{transport_error.to_string()}</p>
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
    onboarding: RwSignal<String>,
) -> impl IntoView {
    view! {
        <div class="marketplace-seller-admin__filters">
            <input
                type="search"
                placeholder=label(russian, "Search sellers", "Поиск продавцов")
                prop:value=move || search.get()
                on:input=move |event| search.set(event_target_value(&event))
            />
            <select
                prop:value=move || status.get()
                on:change=move |event| status.set(event_target_value(&event))
            >
                <option value="">{label(russian, "All statuses", "Все статусы")}</option>
                <option value="draft">"draft"</option>
                <option value="active">"active"</option>
                <option value="suspended">"suspended"</option>
                <option value="closed">"closed"</option>
            </select>
            <select
                prop:value=move || onboarding.get()
                on:change=move |event| onboarding.set(event_target_value(&event))
            >
                <option value="">{label(russian, "All onboarding states", "Все состояния онбординга")}</option>
                <option value="draft">"draft"</option>
                <option value="submitted">"submitted"</option>
                <option value="approved">"approved"</option>
                <option value="rejected">"rejected"</option>
            </select>
        </div>
    }
}

fn render_directory(
    russian: bool,
    directory: MarketplaceSellerAdminDirectory,
    selected_id: RwSignal<Option<String>>,
    profile_display_name: RwSignal<String>,
    profile_legal_name: RwSignal<String>,
) -> impl IntoView {
    if directory.items.is_empty() {
        return view! {
            <p class="marketplace-seller-admin__empty">
                {label(russian, "No sellers match the filters.", "Продавцы по фильтрам не найдены.")}
            </p>
        }
        .into_any();
    }

    view! {
        <p>{format!("{}: {}", label(russian, "Total", "Всего"), directory.total)}</p>
        <ul class="marketplace-seller-admin__seller-list">
            {directory.items.into_iter().map(|seller| {
                let active_id = seller.id.clone();
                let click_id = seller.id.clone();
                let click_display_name = seller.display_name.clone();
                let display_name = seller.display_name;
                let handle = seller.handle;
                let status = seller.status;
                let onboarding_status = seller.onboarding_status;
                view! {
                    <li>
                        <button
                            type="button"
                            class:active=move || selected_id.get().as_deref() == Some(active_id.as_str())
                            on:click=move |_| {
                                selected_id.set(Some(click_id.clone()));
                                profile_display_name.set(click_display_name.clone());
                                profile_legal_name.set(String::new());
                            }
                        >
                            <strong>{display_name}</strong>
                            <span>{format!("@{handle} · {status} · {onboarding_status}")}</span>
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
    handle: RwSignal<String>,
    display_name: RwSignal<String>,
    legal_name: RwSignal<String>,
    owner_user_id: RwSignal<String>,
    run_command: Arc<dyn Fn(MarketplaceSellerAdminCommand) + Send + Sync>,
) -> impl IntoView {
    view! {
        <section class="marketplace-seller-admin__create">
            <h2>{label(russian, "Create seller", "Создать продавца")}</h2>
            <input
                placeholder="handle"
                prop:value=move || handle.get()
                on:input=move |event| handle.set(event_target_value(&event))
            />
            <input
                placeholder=label(russian, "Display name", "Отображаемое имя")
                prop:value=move || display_name.get()
                on:input=move |event| display_name.set(event_target_value(&event))
            />
            <input
                placeholder=label(russian, "Legal name", "Юридическое имя")
                prop:value=move || legal_name.get()
                on:input=move |event| legal_name.set(event_target_value(&event))
            />
            <input
                placeholder="owner user UUID"
                prop:value=move || owner_user_id.get()
                on:input=move |event| owner_user_id.set(event_target_value(&event))
            />
            <button
                type="button"
                disabled=move || busy.get()
                on:click=move |_| run_command(MarketplaceSellerAdminCommand::Create {
                    draft: MarketplaceSellerCreateDraft {
                        handle: handle.get_untracked(),
                        display_name: display_name.get_untracked(),
                        legal_name: optional_text(legal_name.get_untracked()),
                        owner_user_id: owner_user_id.get_untracked(),
                        metadata: serde_json::json!({}),
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
    detail: MarketplaceSellerAdminDetail,
    busy: RwSignal<bool>,
    profile_display_name: RwSignal<String>,
    profile_legal_name: RwSignal<String>,
    onboarding_note: RwSignal<String>,
    suspension_reason: RwSignal<String>,
    member_user_id: RwSignal<String>,
    member_role: RwSignal<String>,
    run_command: Arc<dyn Fn(MarketplaceSellerAdminCommand) + Send + Sync>,
) -> impl IntoView {
    let seller = detail.seller;
    let seller_id = seller.id.clone();
    let profile_command = run_command.clone();
    let submit_command = run_command.clone();
    let approve_command = run_command.clone();
    let reject_command = run_command.clone();
    let suspend_command = run_command.clone();
    let reactivate_command = run_command.clone();
    let add_member_command = run_command.clone();

    if profile_display_name.get_untracked().is_empty() {
        profile_display_name.set(seller.display_name.clone());
    }
    if profile_legal_name.get_untracked().is_empty() {
        profile_legal_name.set(seller.legal_name.clone().unwrap_or_default());
    }

    let profile_seller_id = seller_id.clone();
    let submit_seller_id = seller_id.clone();
    let approve_seller_id = seller_id.clone();
    let reject_seller_id = seller_id.clone();
    let suspend_seller_id = seller_id.clone();
    let reactivate_seller_id = seller_id.clone();
    let add_member_seller_id = seller_id.clone();

    view! {
        <article class="marketplace-seller-admin__seller-detail">
            <header>
                <h2>{seller.display_name}</h2>
                <p>{format!("@{} · {} · {}", seller.handle, seller.status, seller.onboarding_status)}</p>
                {seller.suspension_reason.map(|reason| view! { <p>{reason}</p> })}
            </header>

            <section>
                <h3>{label(russian, "Profile", "Профиль")}</h3>
                <input
                    placeholder=label(russian, "Display name", "Отображаемое имя")
                    prop:value=move || profile_display_name.get()
                    on:input=move |event| profile_display_name.set(event_target_value(&event))
                />
                <input
                    placeholder=label(russian, "Legal name", "Юридическое имя")
                    prop:value=move || profile_legal_name.get()
                    on:input=move |event| profile_legal_name.set(event_target_value(&event))
                />
                <button
                    type="button"
                    disabled=move || busy.get()
                    on:click=move |_| profile_command(MarketplaceSellerAdminCommand::UpdateProfile {
                        seller_id: profile_seller_id.clone(),
                        draft: MarketplaceSellerProfileDraft {
                            display_name: optional_text(profile_display_name.get_untracked()),
                            legal_name: optional_text(profile_legal_name.get_untracked()),
                            metadata: None,
                        },
                    })
                >
                    {label(russian, "Save profile", "Сохранить профиль")}
                </button>
            </section>

            <section>
                <h3>{label(russian, "Onboarding and lifecycle", "Онбординг и жизненный цикл")}</h3>
                <textarea
                    placeholder=label(russian, "Review note", "Комментарий проверки")
                    prop:value=move || onboarding_note.get()
                    on:input=move |event| onboarding_note.set(event_target_value(&event))
                />
                <div class="marketplace-seller-admin__actions">
                    <button type="button" disabled=move || busy.get() on:click=move |_| {
                        submit_command(MarketplaceSellerAdminCommand::SubmitOnboarding {
                            seller_id: submit_seller_id.clone(),
                            note: optional_text(onboarding_note.get_untracked()),
                        })
                    }>{label(russian, "Submit", "Отправить")}</button>
                    <button type="button" disabled=move || busy.get() on:click=move |_| {
                        approve_command(MarketplaceSellerAdminCommand::ReviewOnboarding {
                            seller_id: approve_seller_id.clone(),
                            approved: true,
                            note: optional_text(onboarding_note.get_untracked()),
                        })
                    }>{label(russian, "Approve", "Одобрить")}</button>
                    <button type="button" disabled=move || busy.get() on:click=move |_| {
                        reject_command(MarketplaceSellerAdminCommand::ReviewOnboarding {
                            seller_id: reject_seller_id.clone(),
                            approved: false,
                            note: optional_text(onboarding_note.get_untracked()),
                        })
                    }>{label(russian, "Reject", "Отклонить")}</button>
                </div>
                <input
                    placeholder=label(russian, "Suspension reason", "Причина блокировки")
                    prop:value=move || suspension_reason.get()
                    on:input=move |event| suspension_reason.set(event_target_value(&event))
                />
                <div class="marketplace-seller-admin__actions">
                    <button type="button" disabled=move || busy.get() on:click=move |_| {
                        suspend_command(MarketplaceSellerAdminCommand::Suspend {
                            seller_id: suspend_seller_id.clone(),
                            reason: suspension_reason.get_untracked(),
                        })
                    }>{label(russian, "Suspend", "Заблокировать")}</button>
                    <button type="button" disabled=move || busy.get() on:click=move |_| {
                        reactivate_command(MarketplaceSellerAdminCommand::Reactivate {
                            seller_id: reactivate_seller_id.clone(),
                        })
                    }>{label(russian, "Reactivate", "Активировать снова")}</button>
                </div>
            </section>

            <section>
                <h3>{label(russian, "Seller members", "Участники продавца")}</h3>
                <ul>
                    {detail.members.into_iter().map(|member| {
                        let disable_command = run_command.clone();
                        let activate_command = run_command.clone();
                        let disable_seller_id = seller_id.clone();
                        let activate_seller_id = seller_id.clone();
                        let disable_member_id = member.id.clone();
                        let activate_member_id = member.id.clone();
                        view! {
                            <li>
                                <span>{format!("{} · {} · {}", member.user_id, member.role, member.status)}</span>
                                <button type="button" disabled=move || busy.get() on:click=move |_| {
                                    disable_command(MarketplaceSellerAdminCommand::UpdateMember {
                                        seller_id: disable_seller_id.clone(),
                                        member_id: disable_member_id.clone(),
                                        draft: MarketplaceSellerMemberUpdateDraft {
                                            role: None,
                                            status: Some("disabled".to_string()),
                                            metadata: None,
                                        },
                                    })
                                }>{label(russian, "Disable", "Отключить")}</button>
                                <button type="button" disabled=move || busy.get() on:click=move |_| {
                                    activate_command(MarketplaceSellerAdminCommand::UpdateMember {
                                        seller_id: activate_seller_id.clone(),
                                        member_id: activate_member_id.clone(),
                                        draft: MarketplaceSellerMemberUpdateDraft {
                                            role: None,
                                            status: Some("active".to_string()),
                                            metadata: None,
                                        },
                                    })
                                }>{label(russian, "Activate", "Активировать")}</button>
                            </li>
                        }
                    }).collect_view()}
                </ul>
                <input
                    placeholder="user UUID"
                    prop:value=move || member_user_id.get()
                    on:input=move |event| member_user_id.set(event_target_value(&event))
                />
                <select
                    prop:value=move || member_role.get()
                    on:change=move |event| member_role.set(event_target_value(&event))
                >
                    <option value="admin">"admin"</option>
                    <option value="operations">"operations"</option>
                    <option value="finance">"finance"</option>
                    <option value="member">"member"</option>
                </select>
                <button type="button" disabled=move || busy.get() on:click=move |_| {
                    add_member_command(MarketplaceSellerAdminCommand::AddMember {
                        seller_id: add_member_seller_id.clone(),
                        draft: MarketplaceSellerMemberCreateDraft {
                            user_id: member_user_id.get_untracked(),
                            role: member_role.get_untracked(),
                            metadata: serde_json::json!({}),
                        },
                    })
                }>{label(russian, "Invite member", "Пригласить участника")}</button>
            </section>
        </article>
    }
}

fn transport_context(
    profile: MarketplaceSellerAdminTransportProfile,
) -> MarketplaceSellerAdminTransportContext {
    match profile {
        MarketplaceSellerAdminTransportProfile::Native => {
            MarketplaceSellerAdminTransportContext::native()
        }
        MarketplaceSellerAdminTransportProfile::Graphql => {
            MarketplaceSellerAdminTransportContext::graphql(
                None,
                option_env!("RUSTOK_TENANT_SLUG").map(str::to_string),
            )
        }
    }
}

fn optional_text(value: String) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn localized(russian: bool, english: &'static str, russian_text: &'static str) -> String {
    label(russian, english, russian_text).to_string()
}

fn label(russian: bool, english: &'static str, russian_text: &'static str) -> &'static str {
    if russian { russian_text } else { english }
}
