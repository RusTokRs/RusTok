use std::rc::Rc;

use leptos::prelude::*;
use leptos::task::spawn_local;
use rustok_ui_core::UiRouteContext;

use crate::core::{build_marketplace_seller_admin_shell, selected_transport_profile};
use crate::i18n::normalize_admin_locale;
use crate::model::{
    MarketplaceSellerAdminCommand, MarketplaceSellerAdminDetail,
    MarketplaceSellerAdminFilters, MarketplaceSellerCreateDraft,
    MarketplaceSellerMemberCreateDraft, MarketplaceSellerMemberUpdateDraft,
    MarketplaceSellerProfileDraft,
};
use crate::transport::{
    execute_marketplace_seller_command, load_marketplace_seller_detail,
    load_marketplace_seller_directory, MarketplaceSellerAdminTransportContext,
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
    let transport = match profile {
        crate::core::MarketplaceSellerAdminTransportProfile::Native => {
            MarketplaceSellerAdminTransportContext::native()
        }
        crate::core::MarketplaceSellerAdminTransportProfile::Graphql => {
            MarketplaceSellerAdminTransportContext::graphql(
                None,
                option_env!("RUSTOK_TENANT_SLUG").map(str::to_string),
            )
        }
    };

    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (selected_id, set_selected_id) = signal(Option::<String>::None);
    let (search, set_search) = signal(String::new());
    let (status_filter, set_status_filter) = signal(String::new());
    let (onboarding_filter, set_onboarding_filter) = signal(String::new());
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);
    let (notice, set_notice) = signal(Option::<String>::None);
    let (pending_command, set_pending_command) =
        signal(Option::<(String, MarketplaceSellerAdminCommand)>::None);

    let (create_handle, set_create_handle) = signal(String::new());
    let (create_display_name, set_create_display_name) = signal(String::new());
    let (create_legal_name, set_create_legal_name) = signal(String::new());
    let (create_owner_user_id, set_create_owner_user_id) = signal(String::new());
    let (profile_display_name, set_profile_display_name) = signal(String::new());
    let (profile_legal_name, set_profile_legal_name) = signal(String::new());
    let (onboarding_note, set_onboarding_note) = signal(String::new());
    let (suspension_reason, set_suspension_reason) = signal(String::new());
    let (member_user_id, set_member_user_id) = signal(String::new());
    let (member_role, set_member_role) = signal("member".to_string());

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

    let run_command: Rc<dyn Fn(MarketplaceSellerAdminCommand)> = Rc::new({
        let transport = transport.clone();
        move |command: MarketplaceSellerAdminCommand| {
            if busy.get_untracked() {
                return;
            }
            let key = pending_command
                .get_untracked()
                .as_ref()
                .filter(|(_, pending)| pending == &command)
                .map(|(key, _)| key.clone())
                .unwrap_or_else(|| format!("marketplace-seller-admin-{}", uuid::Uuid::new_v4()));
            set_pending_command.set(Some((key.clone(), command.clone())));
            set_busy.set(true);
            set_error.set(None);
            set_notice.set(None);
            let context = transport.clone();
            spawn_local(async move {
                match execute_marketplace_seller_command(context, key, command).await {
                    Ok(result) => {
                        if let Some(seller) = result.seller {
                            set_selected_id.set(Some(seller.id));
                            set_profile_display_name.set(seller.display_name);
                            set_profile_legal_name.set(seller.legal_name.unwrap_or_default());
                        }
                        set_pending_command.set(None);
                        set_notice.set(Some(text(
                            russian,
                            "Marketplace seller command completed.",
                            "Команда продавца выполнена.",
                        )));
                        set_refresh_nonce.update(|value| *value = value.saturating_add(1));
                    }
                    Err(transport_error) => {
                        set_error.set(Some(transport_error.to_string()));
                    }
                }
                set_busy.set(false);
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
                <p class="marketplace-seller-admin__transport">
                    {format!("{}: {}", text_ref(russian, "Transport", "Транспорт"), profile.as_str())}
                </p>
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
                        {text_ref(russian, "Retry same command", "Повторить ту же команду")}
                    </button>
                </div>
            })}
            {move || notice.get().map(|message| view! {
                <div class="marketplace-seller-admin__notice" role="status">{message}</div>
            })}

            <div class="marketplace-seller-admin__layout">
                <aside class="marketplace-seller-admin__directory">
                    <div class="marketplace-seller-admin__filters">
                        <input
                            type="search"
                            placeholder=text_ref(russian, "Search sellers", "Поиск продавцов")
                            prop:value=move || search.get()
                            on:input=move |event| set_search.set(event_target_value(&event))
                        />
                        <select
                            prop:value=move || status_filter.get()
                            on:change=move |event| set_status_filter.set(event_target_value(&event))
                        >
                            <option value="">{text_ref(russian, "All statuses", "Все статусы")}</option>
                            <option value="draft">"draft"</option>
                            <option value="active">"active"</option>
                            <option value="suspended">"suspended"</option>
                            <option value="closed">"closed"</option>
                        </select>
                        <select
                            prop:value=move || onboarding_filter.get()
                            on:change=move |event| set_onboarding_filter.set(event_target_value(&event))
                        >
                            <option value="">{text_ref(russian, "All onboarding states", "Все состояния онбординга")}</option>
                            <option value="draft">"draft"</option>
                            <option value="submitted">"submitted"</option>
                            <option value="approved">"approved"</option>
                            <option value="rejected">"rejected"</option>
                        </select>
                    </div>
                    <Suspense fallback=move || view! { <p>{text_ref(russian, "Loading sellers...", "Загрузка продавцов...")}</p> }>
                        {move || directory.get().map(|result| match result {
                            Ok(directory) if directory.items.is_empty() => view! {
                                <p class="marketplace-seller-admin__empty">{text_ref(russian, "No sellers match the filters.", "Продавцы по фильтрам не найдены.")}</p>
                            }.into_any(),
                            Ok(directory) => view! {
                                <p>{format!("{}: {}", text_ref(russian, "Total", "Всего"), directory.total)}</p>
                                <ul class="marketplace-seller-admin__seller-list">
                                    {directory.items.into_iter().map(|seller| {
                                        let seller_id = seller.id.clone();
                                        view! {
                                            <li>
                                                <button
                                                    type="button"
                                                    class:active=move || selected_id.get().as_deref() == Some(seller_id.as_str())
                                                    on:click=move |_| {
                                                        set_selected_id.set(Some(seller_id.clone()));
                                                        set_profile_display_name.set(seller.display_name.clone());
                                                    }
                                                >
                                                    <strong>{seller.display_name}</strong>
                                                    <span>{format!("@{} · {} · {}", seller.handle, seller.status, seller.onboarding_status)}</span>
                                                </button>
                                            </li>
                                        }
                                    }).collect_view()}
                                </ul>
                            }.into_any(),
                            Err(error) => view! {
                                <p class="marketplace-seller-admin__error">{error.to_string()}</p>
                            }.into_any(),
                        })}
                    </Suspense>

                    <section class="marketplace-seller-admin__create">
                        <h2>{text_ref(russian, "Create seller", "Создать продавца")}</h2>
                        <input
                            placeholder="handle"
                            prop:value=move || create_handle.get()
                            on:input=move |event| set_create_handle.set(event_target_value(&event))
                        />
                        <input
                            placeholder=text_ref(russian, "Display name", "Отображаемое имя")
                            prop:value=move || create_display_name.get()
                            on:input=move |event| set_create_display_name.set(event_target_value(&event))
                        />
                        <input
                            placeholder=text_ref(russian, "Legal name", "Юридическое имя")
                            prop:value=move || create_legal_name.get()
                            on:input=move |event| set_create_legal_name.set(event_target_value(&event))
                        />
                        <input
                            placeholder="owner user UUID"
                            prop:value=move || create_owner_user_id.get()
                            on:input=move |event| set_create_owner_user_id.set(event_target_value(&event))
                        />
                        <button
                            type="button"
                            disabled=move || busy.get()
                            on:click=move |_| {
                                create_command(MarketplaceSellerAdminCommand::Create {
                                    draft: MarketplaceSellerCreateDraft {
                                        handle: create_handle.get_untracked(),
                                        display_name: create_display_name.get_untracked(),
                                        legal_name: optional_text(create_legal_name.get_untracked()),
                                        owner_user_id: create_owner_user_id.get_untracked(),
                                        metadata: serde_json::json!({}),
                                    },
                                });
                            }
                        >
                            {text_ref(russian, "Create", "Создать")}
                        </button>
                    </section>
                </aside>

                <main class="marketplace-seller-admin__detail">
                    <Suspense fallback=move || view! { <p>{text_ref(russian, "Loading seller detail...", "Загрузка продавца...")}</p> }>
                        {move || detail.get().map(|result| match result {
                            Ok(None) => view! {
                                <p>{text_ref(russian, "Select a seller to inspect lifecycle and members.", "Выберите продавца для просмотра жизненного цикла и участников.")}</p>
                            }.into_any(),
                            Ok(Some(detail)) => render_detail(
                                russian,
                                detail,
                                busy,
                                profile_display_name,
                                set_profile_display_name,
                                profile_legal_name,
                                set_profile_legal_name,
                                onboarding_note,
                                set_onboarding_note,
                                suspension_reason,
                                set_suspension_reason,
                                member_user_id,
                                set_member_user_id,
                                member_role,
                                set_member_role,
                                run_command.clone(),
                            ).into_any(),
                            Err(error) => view! {
                                <p class="marketplace-seller-admin__error">{error.to_string()}</p>
                            }.into_any(),
                        })}
                    </Suspense>
                </main>
            </div>
        </section>
    }
}

#[allow(clippy::too_many_arguments)]
fn render_detail(
    russian: bool,
    detail: MarketplaceSellerAdminDetail,
    busy: ReadSignal<bool>,
    profile_display_name: ReadSignal<String>,
    set_profile_display_name: WriteSignal<String>,
    profile_legal_name: ReadSignal<String>,
    set_profile_legal_name: WriteSignal<String>,
    onboarding_note: ReadSignal<String>,
    set_onboarding_note: WriteSignal<String>,
    suspension_reason: ReadSignal<String>,
    set_suspension_reason: WriteSignal<String>,
    member_user_id: ReadSignal<String>,
    set_member_user_id: WriteSignal<String>,
    member_role: ReadSignal<String>,
    set_member_role: WriteSignal<String>,
    run_command: Rc<dyn Fn(MarketplaceSellerAdminCommand)>,
) -> impl IntoView {
    let seller = detail.seller;
    let seller_id = seller.id.clone();
    let update_profile = run_command.clone();
    let submit = run_command.clone();
    let approve = run_command.clone();
    let reject = run_command.clone();
    let suspend = run_command.clone();
    let reactivate = run_command.clone();
    let add_member = run_command.clone();

    view! {
        <article class="marketplace-seller-admin__seller-detail">
            <header>
                <h2>{seller.display_name.clone()}</h2>
                <p>{format!("@{} · {} · {}", seller.handle, seller.status, seller.onboarding_status)}</p>
                {seller.suspension_reason.clone().map(|reason| view! { <p>{reason}</p> })}
            </header>

            <section>
                <h3>{text_ref(russian, "Profile", "Профиль")}</h3>
                <input
                    placeholder=text_ref(russian, "Display name", "Отображаемое имя")
                    prop:value=move || profile_display_name.get()
                    on:input=move |event| set_profile_display_name.set(event_target_value(&event))
                />
                <input
                    placeholder=text_ref(russian, "Legal name", "Юридическое имя")
                    prop:value=move || profile_legal_name.get()
                    on:input=move |event| set_profile_legal_name.set(event_target_value(&event))
                />
                <button
                    type="button"
                    disabled=move || busy.get()
                    on:click={
                        let seller_id = seller_id.clone();
                        move |_| update_profile(MarketplaceSellerAdminCommand::UpdateProfile {
                            seller_id: seller_id.clone(),
                            draft: MarketplaceSellerProfileDraft {
                                display_name: optional_text(profile_display_name.get_untracked()),
                                legal_name: optional_text(profile_legal_name.get_untracked()),
                                metadata: None,
                            },
                        })
                    }
                >
                    {text_ref(russian, "Save profile", "Сохранить профиль")}
                </button>
            </section>

            <section>
                <h3>{text_ref(russian, "Onboarding and lifecycle", "Онбординг и жизненный цикл")}</h3>
                <textarea
                    placeholder=text_ref(russian, "Review note", "Комментарий проверки")
                    prop:value=move || onboarding_note.get()
                    on:input=move |event| set_onboarding_note.set(event_target_value(&event))
                />
                <div class="marketplace-seller-admin__actions">
                    <button type="button" disabled=move || busy.get() on:click={
                        let seller_id = seller_id.clone();
                        move |_| submit(MarketplaceSellerAdminCommand::SubmitOnboarding {
                            seller_id: seller_id.clone(),
                            note: optional_text(onboarding_note.get_untracked()),
                        })
                    }>{text_ref(russian, "Submit", "Отправить")}</button>
                    <button type="button" disabled=move || busy.get() on:click={
                        let seller_id = seller_id.clone();
                        move |_| approve(MarketplaceSellerAdminCommand::ReviewOnboarding {
                            seller_id: seller_id.clone(),
                            approved: true,
                            note: optional_text(onboarding_note.get_untracked()),
                        })
                    }>{text_ref(russian, "Approve", "Одобрить")}</button>
                    <button type="button" disabled=move || busy.get() on:click={
                        let seller_id = seller_id.clone();
                        move |_| reject(MarketplaceSellerAdminCommand::ReviewOnboarding {
                            seller_id: seller_id.clone(),
                            approved: false,
                            note: optional_text(onboarding_note.get_untracked()),
                        })
                    }>{text_ref(russian, "Reject", "Отклонить")}</button>
                </div>
                <input
                    placeholder=text_ref(russian, "Suspension reason", "Причина блокировки")
                    prop:value=move || suspension_reason.get()
                    on:input=move |event| set_suspension_reason.set(event_target_value(&event))
                />
                <div class="marketplace-seller-admin__actions">
                    <button type="button" disabled=move || busy.get() on:click={
                        let seller_id = seller_id.clone();
                        move |_| suspend(MarketplaceSellerAdminCommand::Suspend {
                            seller_id: seller_id.clone(),
                            reason: suspension_reason.get_untracked(),
                        })
                    }>{text_ref(russian, "Suspend", "Заблокировать")}</button>
                    <button type="button" disabled=move || busy.get() on:click={
                        let seller_id = seller_id.clone();
                        move |_| reactivate(MarketplaceSellerAdminCommand::Reactivate {
                            seller_id: seller_id.clone(),
                        })
                    }>{text_ref(russian, "Reactivate", "Активировать снова")}</button>
                </div>
            </section>

            <section>
                <h3>{text_ref(russian, "Seller members", "Участники продавца")}</h3>
                <ul>
                    {detail.members.into_iter().map(|member| {
                        let disable_member = run_command.clone();
                        let activate_member = run_command.clone();
                        let disable_seller_id = seller_id.clone();
                        let activate_seller_id = seller_id.clone();
                        let disable_member_id = member.id.clone();
                        let activate_member_id = member.id.clone();
                        view! {
                            <li>
                                <span>{format!("{} · {} · {}", member.user_id, member.role, member.status)}</span>
                                <button type="button" disabled=move || busy.get() on:click=move |_| {
                                    disable_member(MarketplaceSellerAdminCommand::UpdateMember {
                                        seller_id: disable_seller_id.clone(),
                                        member_id: disable_member_id.clone(),
                                        draft: MarketplaceSellerMemberUpdateDraft {
                                            role: None,
                                            status: Some("disabled".to_string()),
                                            metadata: None,
                                        },
                                    })
                                }>{text_ref(russian, "Disable", "Отключить")}</button>
                                <button type="button" disabled=move || busy.get() on:click=move |_| {
                                    activate_member(MarketplaceSellerAdminCommand::UpdateMember {
                                        seller_id: activate_seller_id.clone(),
                                        member_id: activate_member_id.clone(),
                                        draft: MarketplaceSellerMemberUpdateDraft {
                                            role: None,
                                            status: Some("active".to_string()),
                                            metadata: None,
                                        },
                                    })
                                }>{text_ref(russian, "Activate", "Активировать")}</button>
                            </li>
                        }
                    }).collect_view()}
                </ul>
                <input
                    placeholder="user UUID"
                    prop:value=move || member_user_id.get()
                    on:input=move |event| set_member_user_id.set(event_target_value(&event))
                />
                <select
                    prop:value=move || member_role.get()
                    on:change=move |event| set_member_role.set(event_target_value(&event))
                >
                    <option value="admin">"admin"</option>
                    <option value="operations">"operations"</option>
                    <option value="finance">"finance"</option>
                    <option value="member">"member"</option>
                </select>
                <button type="button" disabled=move || busy.get() on:click={
                    let seller_id = seller_id.clone();
                    move |_| add_member(MarketplaceSellerAdminCommand::AddMember {
                        seller_id: seller_id.clone(),
                        draft: MarketplaceSellerMemberCreateDraft {
                            user_id: member_user_id.get_untracked(),
                            role: member_role.get_untracked(),
                            metadata: serde_json::json!({}),
                        },
                    })
                }>{text_ref(russian, "Invite member", "Пригласить участника")}</button>
            </section>
        </article>
    }
}

fn optional_text(value: String) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn text(russian: bool, english: &'static str, russian_text: &'static str) -> String {
    text_ref(russian, english, russian_text).to_string()
}

fn text_ref(
    russian: bool,
    english: &'static str,
    russian_text: &'static str,
) -> &'static str {
    if russian {
        russian_text
    } else {
        english
    }
}
