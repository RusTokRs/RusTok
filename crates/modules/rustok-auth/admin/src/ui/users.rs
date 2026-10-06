use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_router::components::A;
use leptos_router::hooks::{use_navigate, use_query_map};
use leptos_ui::{Badge, BadgeVariant};
use leptos_use::use_debounce_fn;
use rustok_grid_leptos::prelude::*;
use rustok_ui_core::UiRouteContext;

use crate::core::{
    CreateUserInputError, GraphqlUserViewModel, filter_users, graphql_user_view,
    prepare_create_user_input, user_grid_columns, user_list_page, user_list_pagination,
    user_list_query_params,
};
use crate::i18n::{auth_transport_error_message, t};
use crate::transport::{create_user, fetch_users};
use crate::ui::components::{Button, Input, PageHeader};

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

fn users_table_skeleton() -> impl IntoView {
    view! {
        <div>
            <div class="mb-4 grid gap-3 md:grid-cols-3">
                {(0..3)
                    .map(|_| view! { <div class="h-12 animate-pulse rounded-xl bg-muted"></div> })
                    .collect_view()}
            </div>
            <div class="space-y-3">
                {(0..6)
                    .map(|_| view! { <div class="h-10 animate-pulse rounded-lg bg-muted"></div> })
                    .collect_view()}
            </div>
            <div class="mt-4 flex items-center gap-3">
                <div class="h-9 w-24 animate-pulse rounded-lg bg-muted"></div>
                <div class="h-4 w-20 animate-pulse rounded bg-muted"></div>
                <div class="h-9 w-24 animate-pulse rounded-lg bg-muted"></div>
            </div>
        </div>
    }
}

#[component]
pub fn Users() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = StoredValue::new(route_context.locale);
    let t_local =
        move |key: &str, fallback: &str| locale.with_value(|l| t(l.as_deref(), key, fallback));

    let token = use_token();
    let tenant = use_tenant();
    let navigate = use_navigate();
    let query = use_query_map();

    let initial_search = query.get_untracked().get("search").unwrap_or_default();
    let initial_role = query.get_untracked().get("role").unwrap_or_default();
    let initial_status = query.get_untracked().get("status").unwrap_or_default();
    let initial_page = user_list_page(query.get_untracked().get("page").as_deref());

    let (refresh_counter, set_refresh_counter) = signal(0u32);
    let (page, set_page) = signal(initial_page);
    let (limit, _set_limit) = signal(12i64);

    let (search_query, set_search_query) = signal(initial_search.clone());
    let (role_filter, set_role_filter) = signal(initial_role);
    let (status_filter, set_status_filter) = signal(initial_status);

    let (debounced_search, set_debounced_search) = signal(initial_search);
    let debounce_search = use_debounce_fn(
        move || set_debounced_search.set(search_query.get_untracked()),
        300.0,
    );
    Effect::new(move |_| {
        let _ = search_query.get();
        debounce_search();
    });

    Effect::new(move |_| {
        let _ = debounced_search.get();
        let _ = role_filter.get();
        let _ = status_filter.get();
        set_page.set(1);
    });

    let navigate_effect = navigate.clone();
    Effect::new(move |_| {
        let s = debounced_search.get();
        let r = role_filter.get();
        let st = status_filter.get();
        let p = page.get();

        let params = user_list_query_params(s, r, st, p);

        let search_string = serde_urlencoded::to_string(params)
            .ok()
            .filter(|encoded| !encoded.is_empty())
            .map(|encoded| format!("?{}", encoded))
            .unwrap_or_default();

        navigate_effect(&format!("/users{}", search_string), Default::default());
    });

    let users_resource = local_resource(
        move || {
            (
                refresh_counter.get(),
                page.get(),
                limit.get(),
                debounced_search.get(),
                role_filter.get(),
                status_filter.get(),
            )
        },
        move |(_, page_val, limit_val, search_val, role_val, status_val)| {
            let token_value = token.get();
            let tenant_value = tenant.get();
            async move {
                fetch_users(
                    page_val,
                    limit_val,
                    search_val,
                    role_val,
                    status_val,
                    token_value,
                    tenant_value,
                )
                .await
            }
        },
    );

    let refresh = Callback::new(move |_| set_refresh_counter.update(|value| *value += 1));

    let is_ru = locale.with_value(|l| l.as_deref().map(|s| s.starts_with("ru")).unwrap_or(false));
    let columns = locale.with_value(|l| user_grid_columns(l.as_deref()));
    let filters = RwSignal::new(ColumnFilters::new());
    let selection = RwSignal::new(RowSelection::new());
    let pagination = RwSignal::new(GridPagination::new(initial_page as usize, 12, 0));

    let on_filters_change = Callback::new(move |new_filters: ColumnFilters| {
        if let Some(FilterValue::Select(r)) = new_filters.get("role") {
            set_role_filter.set(r.clone());
        } else {
            set_role_filter.set(String::new());
        }
        if let Some(FilterValue::Select(s)) = new_filters.get("status") {
            set_status_filter.set(s.clone());
        } else {
            set_status_filter.set(String::new());
        }
        filters.set(new_filters);
    });

    Effect::new(move |_| {
        let current_grid_page = pagination.get().page as i64;
        if current_grid_page != page.get() {
            set_page.set(current_grid_page);
        }
    });

    let navigate_click = navigate.clone();
    let on_row_click = Callback::new(move |item: GraphqlUserViewModel| {
        navigate_click(&item.detail_href, Default::default());
    });

    let cell_renderer = Callback::new(move |(item, col_id): (GraphqlUserViewModel, String)| {
        match col_id.as_str() {
            "email" => {
                let detail_href = item.detail_href.clone();
                let email = item.email.clone();
                view! {
                    <A href=detail_href>
                        <span class="text-primary hover:underline font-medium">
                            {email}
                        </span>
                    </A>
                }
                .into_any()
            }
            "name" => {
                let name = item.name.clone();
                view! {
                    <span class="text-foreground">{name}</span>
                }
                .into_any()
            }
            "role" => {
                let role = item.role.clone();
                view! {
                    <span class="text-xs font-mono uppercase bg-muted text-muted-foreground px-2 py-0.5 rounded">
                        {role}
                    </span>
                }
                .into_any()
            }
            "status" => {
                let is_active = item.is_active;
                let status = item.status.clone();
                view! {
                    <Badge variant=if is_active { BadgeVariant::Success } else { BadgeVariant::Default }>
                        {status}
                    </Badge>
                }
                .into_any()
            }
            "created_at" => {
                let created = item.created_at.clone();
                view! {
                    <span class="text-xs text-muted-foreground font-mono">
                        {created}
                    </span>
                }
                .into_any()
            }
            "actions" => {
                let detail_href = item.detail_href.clone();
                let view_label = if is_ru { "Просмотр" } else { "View" };
                view! {
                    <div class="flex items-center justify-end">
                        <A href=detail_href>
                            <span class="text-xs text-primary hover:underline font-medium px-2 py-1 rounded hover:bg-muted/50">
                                {view_label}
                            </span>
                        </A>
                    </div>
                }
                .into_any()
            }
            _ => ().into_any(),
        }
    });

    let (show_create_modal, set_show_create_modal) = signal(false);
    let (new_email, set_new_email) = signal(String::new());
    let (new_password, set_new_password) = signal(String::new());
    let (new_name, set_new_name) = signal(String::new());
    let (new_role, set_new_role) = signal(String::new());
    let (new_status, set_new_status) = signal(String::new());
    let (create_error, set_create_error) = signal(Option::<String>::None);
    let (is_creating, set_is_creating) = signal(false);

    let open_create_modal = Callback::new(move |_| {
        set_new_email.set(String::new());
        set_new_password.set(String::new());
        set_new_name.set(String::new());
        set_new_role.set(String::new());
        set_new_status.set(String::new());
        set_create_error.set(None);
        set_show_create_modal.set(true);
    });

    let close_create_modal = Callback::new(move |_| {
        set_show_create_modal.set(false);
    });

    let create_user_msg = StoredValue::new(t_local(
        "users.create.errorRequired",
        "Email and password are required.",
    ));

    let create_user_action = Callback::new(move |_| {
        let email_val = new_email.get();
        let password_val = new_password.get();
        let name_val = new_name.get();
        let role_val = new_role.get();
        let status_val = new_status.get();
        let token_val = token.get();
        let tenant_val = tenant.get();

        let input = match prepare_create_user_input(
            email_val,
            password_val,
            name_val,
            role_val,
            status_val,
        ) {
            Ok(input) => input,
            Err(CreateUserInputError::MissingCredentials) => {
                set_create_error.set(Some(create_user_msg.get_value()));
                return;
            }
        };

        set_is_creating.set(true);
        set_create_error.set(None);

        spawn_local(async move {
            match create_user(token_val, tenant_val, input).await {
                Ok(_) => {
                    set_is_creating.set(false);
                    set_show_create_modal.set(false);
                    set_refresh_counter.update(|value| *value += 1);
                }
                Err(e) => {
                    set_is_creating.set(false);
                    set_create_error.set(Some(locale.with_value(|locale| {
                        auth_transport_error_message(locale.as_deref(), &e.to_string())
                    })));
                }
            }
        });
    });

    view! {
        <section class="flex flex-1 flex-col p-4 md:px-6">
            <PageHeader
                title=t_local("users.title", "Users")
                subtitle=t_local("users.subtitle", "GraphQL API user management. View, create, and manage users.")
                eyebrow=t_local("app.nav.users", "Users")
                actions=view! {
                    <Button
                        on_click=refresh
                        class="border border-input bg-transparent text-foreground hover:bg-accent hover:text-accent-foreground"
                    >
                        {t_local("users.refresh", "Refresh")}
                    </Button>
                    <Button on_click=open_create_modal>
                        {t_local("users.create.button", "Create user")}
                    </Button>
                }
                .into_any()
            />

            <div class="rounded-xl border border-border bg-card p-6 shadow-sm">
                <h4 class="mb-4 text-lg font-semibold text-card-foreground">
                    {t_local("users.graphql.title", "GraphQL: users")}
                </h4>
                <Suspense
                    fallback=move || view! { <div>{users_table_skeleton()}</div> }
                >
                    {move || match users_resource.get() {
                        None => view! { <div>{users_table_skeleton()}</div> }.into_any(),
                        Some(Ok(response)) => {
                            let total_count = response.users.page_info.total_count;
                            let _pagination_policy = user_list_pagination(page.get(), limit.get(), total_count);
                            pagination.update(|p| p.total = total_count as u64);
                            let user_items = response
                                .users
                                .edges
                                .into_iter()
                                .map(|edge| {
                                    graphql_user_view(
                                        edge.node,
                                        t_local("users.placeholderDash", "—"),
                                    )
                                })
                                .collect::<Vec<_>>();
                            let search_term = search_query.get();
                            let search_trimmed = search_term.trim();
                            let filtered_users = filter_users(
                                &user_items,
                                &filters.get(),
                                if search_trimmed.is_empty() {
                                    None
                                } else {
                                    Some(search_trimmed)
                                },
                            );

                            view! {
                                <div class="space-y-4">
                                    <Show when=move || !selection.get().is_empty()>
                                        <div class="flex items-center justify-between gap-3 px-4 py-2.5 rounded-xl border border-primary/20 bg-primary/5 text-sm">
                                            <span class="font-medium text-foreground">
                                                {move || format!("{} {} {}", selection.get().count(), if is_ru { "выбрано" } else { "selected" }, if is_ru { "пользователей" } else { "users" })}
                                            </span>
                                            <button
                                                type="button"
                                                class="h-6 px-2.5 rounded-lg text-xs text-muted-foreground hover:text-foreground transition border border-border bg-background"
                                                on:click=move |_| selection.update(|s| s.clear())
                                            >
                                                {if is_ru { "Снять выбор" } else { "Clear" }}
                                            </button>
                                        </div>
                                    </Show>
                                    <div class="flex flex-col md:flex-row gap-3 items-center justify-between">
                                        <div class="w-full md:w-72">
                                            <Input
                                                value=search_query
                                                set_value=set_search_query
                                                placeholder=t_local("users.filters.searchPlaceholder", "Email or name")
                                                label=t_local("users.filters.search", "Search")
                                            />
                                        </div>
                                        <div class="text-xs text-muted-foreground">
                                            {t_local("users.graphql.total", "Total users:")} " " {total_count}
                                        </div>
                                    </div>
                                    <DataGrid
                                        columns=columns.clone()
                                        data=Signal::derive(move || filtered_users.clone())
                                        key_fn=|item: &GraphqlUserViewModel| item.id.clone()
                                        cell_renderer=cell_renderer
                                        is_loading=Signal::derive(move || users_resource.get().is_none())
                                        empty_message=t_local("users.empty", "No users found").to_string()
                                        selection=selection
                                        pagination=pagination
                                        filters=filters
                                        on_filter_change=on_filters_change
                                        on_row_click=on_row_click
                                    />
                                </div>
                            }
                            .into_any()
                        }
                        Some(Err(err)) => view! {
                            <div class="rounded-xl bg-destructive/10 border border-destructive/20 px-4 py-2 text-sm text-destructive">
                                {format!("{}: {}", t_local("users.loadError", "Failed to load users. Check API availability and access permissions."), err)}
                            </div>
                        }
                        .into_any(),
                    }}
                </Suspense>
            </div>

            <Show when=move || show_create_modal.get()>
                <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
                    <div class="w-full max-w-md rounded-xl border border-border bg-card p-6 shadow-xl">
                        <h3 class="mb-4 text-lg font-semibold text-card-foreground">
                            {t_local("users.create.title", "Create new user")}
                        </h3>

                        <Show when=move || create_error.get().is_some()>
                            <div class="mb-4 rounded-xl bg-destructive/10 border border-destructive/20 px-4 py-2 text-sm text-destructive">
                                {move || create_error.get().unwrap_or_default()}
                            </div>
                        </Show>

                        <div class="space-y-4">
                            <Input
                                value=new_email
                                set_value=set_new_email
                                placeholder="admin@rustok.io"
                                label=t_local("users.create.emailLabel", "Email")
                            />
                            <Input
                                value=new_name
                                set_value=set_new_name
                                placeholder="John Doe"
                                label=t_local("users.create.nameLabel", "Full name")
                            />
                            <Input
                                value=new_password
                                set_value=set_new_password
                                placeholder="••••••••"
                                type_="password"
                                label=t_local("users.create.passwordLabel", "Password")
                            />
                            <Input
                                value=new_role
                                set_value=set_new_role
                                placeholder="ADMIN, MANAGER, CUSTOMER"
                                label=t_local("users.create.roleLabel", "Role (optional)")
                            />
                            <Input
                                value=new_status
                                set_value=set_new_status
                                placeholder="ACTIVE, INACTIVE"
                                label=t_local("users.create.statusLabel", "Status (optional)")
                            />
                        </div>

                        <div class="mt-6 flex gap-3">
                            <Button
                                on_click=create_user_action
                                disabled=is_creating.into()
                                class="flex-1"
                            >
                                {move || if is_creating.get() {
                                    t_local("users.create.creating", "Creating...")
                                } else {
                                    t_local("users.create.submit", "Create user")
                                }}
                            </Button>
                            <Button
                                on_click=close_create_modal
                                class="border border-input bg-transparent text-foreground hover:bg-accent hover:text-accent-foreground"
                            >
                                {t_local("users.create.cancel", "Cancel")}
                            </Button>
                        </div>
                    </div>
                </div>
            </Show>
        </section>
    }
}
