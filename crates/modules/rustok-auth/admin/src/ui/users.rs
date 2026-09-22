use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_router::components::A;
use leptos_router::hooks::{use_navigate, use_query_map};
use leptos_ui::{Badge, BadgeVariant};
use leptos_use::use_debounce_fn;
use rustok_ui_core::UiRouteContext;

use crate::core::{
    CreateUserInputError, graphql_user_view, prepare_create_user_input, user_list_page,
    user_list_pagination, user_list_previous_page, user_list_query_params,
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

        navigate(&format!("/users{}", search_string), Default::default());
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
    let next_page = Callback::new(move |_| set_page.update(|value| *value += 1));
    let previous_page =
        Callback::new(move |_| set_page.update(|value| *value = user_list_previous_page(*value)));

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
                            let edges = response.users.edges;
                            view! {
                            <div>
                                <p class="text-xs text-muted-foreground mb-4">
                                    {t_local("users.graphql.total", "Total users:")} " " {total_count}
                                </p>
                                <div class="mb-4 grid gap-3 md:grid-cols-3">
                                    <Input
                                        value=search_query
                                        set_value=set_search_query
                                        placeholder=t_local("users.filters.searchPlaceholder", "Email or name")
                                        label=t_local("users.filters.search", "Search")
                                    />
                                    <Input
                                        value=role_filter
                                        set_value=set_role_filter
                                        placeholder=t_local("users.filters.rolePlaceholder", "admin, editor")
                                        label=t_local("users.filters.role", "Role filter")
                                    />
                                    <Input
                                        value=status_filter
                                        set_value=set_status_filter
                                        placeholder=t_local("users.filters.statusPlaceholder", "active, disabled")
                                        label=t_local("users.filters.status", "Status filter")
                                    />
                                </div>
                                <div class="overflow-x-auto">
                                    <table class="w-full border-collapse text-sm">
                                        <thead>
                                            <tr>
                                                <th class="pb-2 text-left text-xs font-semibold text-muted-foreground">
                                                    {t_local("users.graphql.email", "Email")}
                                                </th>
                                                <th class="pb-2 text-left text-xs font-semibold text-muted-foreground">
                                                    {t_local("users.graphql.name", "Name")}
                                                </th>
                                                <th class="pb-2 text-left text-xs font-semibold text-muted-foreground">
                                                    {t_local("users.graphql.role", "Role")}
                                                </th>
                                                <th class="pb-2 text-left text-xs font-semibold text-muted-foreground">
                                                    {t_local("users.graphql.status", "Status")}
                                                </th>
                                                <th class="pb-2 text-left text-xs font-semibold text-muted-foreground">
                                                    {t_local("users.graphql.createdAt", "Created")}
                                                </th>
                                            </tr>
                                        </thead>
                                        <tbody>
                                            {{
                                                edges
                                                    .iter()
                                                    .map(|edge| {
                                                        let user = graphql_user_view(
                                                            edge.node.clone(),
                                                            t_local("users.placeholderDash", "—"),
                                                        );
                                                        view! {
                                                            <tr>
                                                                <td class="border-b border-border py-2">
                                                                    <A href=user.detail_href>
                                                                        <span class="text-primary hover:underline">
                                                                            {user.email}
                                                                        </span>
                                                                    </A>
                                                                </td>
                                                                <td class="border-b border-border py-2 text-foreground">
                                                                    {user.name}
                                                                </td>
                                                                <td class="border-b border-border py-2 text-foreground">{user.role}</td>
                                                                <td class="border-b border-border py-2">
                                                                    <Badge variant=if user.is_active { BadgeVariant::Success } else { BadgeVariant::Default }>{user.status}</Badge>
                                                                </td>
                                                                <td class="border-b border-border py-2 text-foreground">{user.created_at}</td>
                                                            </tr>
                                                        }
                                                    })
                                                    .collect_view()
                                            }}
                                        </tbody>
                                    </table>
                                </div>
                                <div class="mt-4 flex flex-wrap items-center gap-3">
                                    <Button
                                        on_click=previous_page
                                        class="border border-input bg-transparent text-foreground hover:bg-accent hover:text-accent-foreground"
                                        disabled=Signal::derive(move || {
                                            !user_list_pagination(page.get(), limit.get(), total_count).can_previous
                                        })
                                    >
                                        {t_local("users.pagination.prev", "Previous")}
                                    </Button>
                                    <span class="text-xs text-muted-foreground">
                                        {t_local("users.pagination.page", "Page")} " " {page.get()}
                                    </span>
                                    <Button
                                        on_click=next_page
                                        class="border border-input bg-transparent text-foreground hover:bg-accent hover:text-accent-foreground"
                                        disabled=Signal::derive(move || {
                                            !user_list_pagination(page.get(), limit.get(), total_count).can_next
                                        })
                                    >
                                        {t_local("users.pagination.next", "Next")}
                                    </Button>
                                </div>
                            </div>
                            }
                            .into_any()
                        }
                        Some(Err(_err)) => view! {
                            <div class="rounded-xl bg-destructive/10 border border-destructive/20 px-4 py-2 text-sm text-destructive">
                                {t_local("users.loadError", "Failed to load users. Check API availability and access permissions.")}
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
